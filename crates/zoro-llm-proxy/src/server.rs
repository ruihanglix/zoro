// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

//! Axum-based local OpenAI-compatible proxy server.
//! Forwards requests to upstream providers with routing, retry, and health tracking.

use crate::config::{ApiFormat, ProxyConfig, RoutingStrategy, UpstreamProvider};
use crate::error::ProxyError;
use crate::health::{HealthTracker, ProviderHealthStatus};
use crate::router::Router;

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use axum::Json;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

// ── Shared state ─────────────────────────────────────────────────────────────

struct ProxyState {
    providers: RwLock<Vec<UpstreamProvider>>,
    router: Router,
    health: HealthTracker,
    max_retries: usize,
    access_token: RwLock<String>,
    http_client: reqwest::Client,
}

// ── ProxyServer public API ───────────────────────────────────────────────────

/// A running local LLM proxy server instance.
pub struct ProxyServer {
    state: Arc<ProxyState>,
    /// The actual port the server is listening on.
    port: u16,
    /// Shutdown signal sender.
    shutdown_tx: tokio::sync::watch::Sender<bool>,
}

impl ProxyServer {
    /// Start the proxy server with the given configuration.
    /// Returns once the server is listening; call `shutdown()` to stop.
    pub async fn start(config: ProxyConfig) -> Result<Self, ProxyError> {
        let health = HealthTracker::new();

        // Initialize health records for all providers
        for p in &config.providers {
            health.ensure_provider(&p.id);
        }

        let state = Arc::new(ProxyState {
            providers: RwLock::new(config.providers),
            router: Router::new(config.routing_strategy),
            health,
            max_retries: config.max_retries,
            access_token: RwLock::new(config.access_token),
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .unwrap_or_default(),
        });

        let app = axum::Router::new()
            .route("/v1/chat/completions", post(handle_chat_completions))
            .route("/v1/models", get(handle_list_models))
            .route("/health", get(handle_health))
            .with_state(state.clone());

        let addr = format!("{}:{}", config.listen_addr, config.port);
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| ProxyError::Bind(format!("{}: {}", addr, e)))?;

        let actual_port = listener
            .local_addr()
            .map(|a| a.port())
            .unwrap_or(config.port);

        tracing::info!(
            addr = %addr,
            port = actual_port,
            "LLM proxy server started"
        );

        let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);

        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.changed().await;
                })
                .await
                .ok();
        });

        Ok(Self {
            state,
            port: actual_port,
            shutdown_tx,
        })
    }

    /// Get the port the server is actually listening on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Hot-update the upstream provider list without restarting the server.
    pub async fn update_providers(&self, providers: Vec<UpstreamProvider>) {
        // Ensure health records exist for new providers
        for p in &providers {
            self.state.health.ensure_provider(&p.id);
        }
        *self.state.providers.write().await = providers;
        tracing::info!("LLM proxy: providers updated");
    }

    /// Hot-update the routing strategy.
    pub fn update_strategy(&self, strategy: RoutingStrategy) {
        self.state.router.set_strategy(strategy);
        tracing::info!(strategy = ?strategy, "LLM proxy: routing strategy updated");
    }

    /// Hot-update the access token.
    pub async fn update_access_token(&self, token: String) {
        *self.state.access_token.write().await = token;
    }

    /// Get health status of all providers.
    pub fn health_status(&self) -> Vec<ProviderHealthStatus> {
        self.state.health.all_statuses()
    }

    /// Gracefully shut down the server.
    pub fn shutdown(self) {
        let _ = self.shutdown_tx.send(true);
        tracing::info!("LLM proxy server shutting down");
    }
}

// ── Request / response types ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<serde_json::Value>,
    #[serde(default)]
    stream: bool,
    #[serde(flatten)]
    rest: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct ModelListResponse {
    object: &'static str,
    data: Vec<ModelInfo>,
}

#[derive(Debug, Serialize)]
struct ModelInfo {
    id: String,
    object: &'static str,
    owned_by: String,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: ErrorDetail,
}

#[derive(Debug, Serialize)]
struct ErrorDetail {
    message: String,
    r#type: &'static str,
    code: u16,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

async fn handle_chat_completions(
    State(state): State<Arc<ProxyState>>,
    headers: HeaderMap,
    Json(mut req): Json<ChatCompletionRequest>,
) -> Response {
    // Check access token if configured
    {
        let token = state.access_token.read().await;
        if !token.is_empty() {
            let auth = headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            let expected = format!("Bearer {}", *token);
            if auth != expected {
                return error_response(StatusCode::UNAUTHORIZED, "Invalid access token");
            }
        }
    }

    let providers = state.providers.read().await;
    let requested_model = req.model.clone();

    // First routing attempt
    let target = match state
        .router
        .select(&requested_model, &providers, &state.health)
    {
        Some(t) => t,
        None => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                &format!(
                    "No healthy provider available for model '{}'",
                    requested_model
                ),
            );
        }
    };

    // Attempt to forward the request, with retries on failure
    let mut tried_provider_ids: Vec<String> = Vec::new();
    let mut tried_models: Vec<String> = Vec::new();
    let mut current_provider = target.provider;
    let mut current_model = target.model;

    for attempt in 0..state.max_retries {
        tracing::debug!(
            attempt = attempt,
            provider = %current_provider.id,
            model = %current_model,
            upstream_url = %current_provider.base_url,
            stream = req.stream,
            "Proxy forwarding request"
        );

        // Update the model in the request body
        req.model = current_model.clone();

        match forward_request(&state.http_client, current_provider, &req).await {
            Ok(resp) => {
                state.health.report_success(&current_provider.id);
                return resp;
            }
            Err(e) => {
                let (is_rate_limited, is_server_error) = match &e {
                    ProxyError::Upstream { status, .. } => (*status == 429, *status >= 500),
                    _ => (false, false),
                };
                let is_retryable = is_rate_limited || is_server_error;

                tracing::warn!(
                    attempt = attempt,
                    provider = %current_provider.id,
                    model = %current_model,
                    error = %e,
                    retryable = is_retryable,
                    rate_limited = is_rate_limited,
                    "Upstream request failed"
                );

                // 429 is a transient traffic signal — don't trigger cooldown
                if is_rate_limited {
                    state.health.report_rate_limited(&current_provider.id);
                } else {
                    state.health.report_failure(&current_provider.id);
                }

                if !is_retryable || attempt + 1 >= state.max_retries {
                    return proxy_error_to_response(e);
                }

                // Backoff delay for 429 to avoid hammering the upstream
                if is_rate_limited {
                    let delay = std::time::Duration::from_secs(1 << attempt.min(3));
                    tracing::info!(
                        delay_secs = delay.as_secs(),
                        "Rate limited (429), retrying in {}s...",
                        delay.as_secs()
                    );
                    tokio::time::sleep(delay).await;
                }

                // Track what we've tried so far
                tried_models.push(current_model.clone());
                if !tried_provider_ids.contains(&current_provider.id.to_string()) {
                    tried_provider_ids.push(current_provider.id.clone());
                }

                // Try to find another provider/model for retry.
                // For __lab_auto__ / rate-limited: prefer switching models
                // on the same provider to spread across rate-limit buckets.
                // For 5xx: prefer switching providers entirely.
                let exclude_providers: Vec<&str> = if is_rate_limited {
                    // Don't exclude the provider; just exclude tried models
                    Vec::new()
                } else {
                    tried_provider_ids.iter().map(|s| s.as_str()).collect()
                };
                let exclude_models: Vec<&str> = tried_models.iter().map(|s| s.as_str()).collect();

                match state.router.select_retry(
                    &requested_model,
                    &providers,
                    &state.health,
                    &exclude_providers,
                    &exclude_models,
                ) {
                    Some(retry_target) => {
                        tracing::info!(
                            from_provider = %current_provider.id,
                            from_model = %current_model,
                            to_provider = %retry_target.provider.id,
                            to_model = %retry_target.model,
                            "Falling back to another provider/model"
                        );
                        current_provider = retry_target.provider;
                        current_model = retry_target.model;
                    }
                    None => {
                        return proxy_error_to_response(e);
                    }
                }
            }
        }
    }

    error_response(StatusCode::BAD_GATEWAY, "All retry attempts exhausted")
}

/// Forward a request to an upstream provider. Returns the raw HTTP response.
async fn forward_request(
    client: &reqwest::Client,
    provider: &UpstreamProvider,
    req: &ChatCompletionRequest,
) -> Result<Response, ProxyError> {
    match provider.format {
        ApiFormat::OpenAI => forward_openai(client, provider, req).await,
        ApiFormat::Gemini => forward_gemini(client, provider, req).await,
    }
}

/// Forward to an OpenAI-compatible endpoint (the common case).
async fn forward_openai(
    client: &reqwest::Client,
    provider: &UpstreamProvider,
    req: &ChatCompletionRequest,
) -> Result<Response, ProxyError> {
    let url = format!(
        "{}/chat/completions",
        provider.base_url.trim_end_matches('/')
    );

    // Reconstruct full JSON body (model + messages + stream + rest)
    let mut body = serde_json::json!({
        "model": &req.model,
        "messages": &req.messages,
    });
    if req.stream {
        body["stream"] = serde_json::json!(true);
    }
    // Merge in extra fields (temperature, max_tokens, etc.)
    if let serde_json::Value::Object(rest) = &req.rest {
        if let serde_json::Value::Object(ref mut obj) = body {
            for (k, v) in rest {
                if k != "model" && k != "messages" && k != "stream" {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }
    }

    let resp = client
        .post(&url)
        .header(
            "Authorization",
            format!("Bearer {}", provider.next_api_key()),
        )
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    let status = resp.status().as_u16();

    if !resp.status().is_success() {
        let body_text = resp.text().await.unwrap_or_default();
        return Err(ProxyError::Upstream {
            status,
            body: body_text,
        });
    }

    // Pass through the response (streaming or non-streaming)
    if req.stream {
        // Stream through the SSE response
        let stream = resp
            .bytes_stream()
            .map(|chunk| chunk.map_err(|e| std::io::Error::other(e.to_string())));

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive")
            .body(Body::from_stream(stream))
            .unwrap())
    } else {
        let body_text = resp.text().await.unwrap_or_default();
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(Body::from(body_text))
            .unwrap())
    }
}

/// Forward to Google Gemini API by converting OpenAI format to Gemini format
/// and converting the response back.
async fn forward_gemini(
    client: &reqwest::Client,
    provider: &UpstreamProvider,
    req: &ChatCompletionRequest,
) -> Result<Response, ProxyError> {
    let base = provider.base_url.trim_end_matches('/');
    let method = if req.stream {
        "streamGenerateContent?alt=sse"
    } else {
        "generateContent"
    };
    let url = format!(
        "{}/models/{}:{}&key={}",
        base,
        req.model,
        method,
        provider.next_api_key()
    );

    // Convert OpenAI messages to Gemini contents format
    let contents = convert_messages_to_gemini(&req.messages);

    let body = serde_json::json!({
        "contents": contents,
    });

    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    let status = resp.status().as_u16();

    if !resp.status().is_success() {
        let body_text = resp.text().await.unwrap_or_default();
        return Err(ProxyError::Upstream {
            status,
            body: body_text,
        });
    }

    if req.stream {
        // Convert Gemini SSE stream to OpenAI SSE stream
        let model_name = req.model.clone();
        let stream = resp.bytes_stream().map(move |chunk| match chunk {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                let converted = convert_gemini_sse_to_openai(&text, &model_name);
                Ok::<_, std::io::Error>(axum::body::Bytes::from(converted))
            }
            Err(e) => Err(std::io::Error::other(e.to_string())),
        });

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive")
            .body(Body::from_stream(stream))
            .unwrap())
    } else {
        // Convert Gemini response to OpenAI format
        let body_text = resp.text().await.unwrap_or_default();
        let converted = convert_gemini_response_to_openai(&body_text, &req.model);
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(Body::from(converted))
            .unwrap())
    }
}

// ── Gemini format converters ─────────────────────────────────────────────────

fn convert_messages_to_gemini(messages: &[serde_json::Value]) -> Vec<serde_json::Value> {
    let mut contents = Vec::new();
    for msg in messages {
        let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
        let content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");

        // Gemini uses "user" and "model" as roles; map "assistant" → "model",
        // and embed "system" as a user message prefix.
        let gemini_role = match role {
            "assistant" => "model",
            "system" => "user",
            _ => "user",
        };

        contents.push(serde_json::json!({
            "role": gemini_role,
            "parts": [{"text": content}]
        }));
    }
    contents
}

fn convert_gemini_response_to_openai(body: &str, model: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(body).unwrap_or_default();

    let text = parsed
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.get(0))
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");

    let openai_resp = serde_json::json!({
        "id": format!("chatcmpl-gemini-{}", uuid_simple()),
        "object": "chat.completion",
        "model": model,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": text,
            },
            "finish_reason": "stop",
        }],
    });

    serde_json::to_string(&openai_resp).unwrap_or_default()
}

fn convert_gemini_sse_to_openai(sse_text: &str, model: &str) -> String {
    let mut output = String::new();

    for line in sse_text.lines() {
        let data = match line.strip_prefix("data: ") {
            Some(d) => d.trim(),
            None => continue,
        };

        if data == "[DONE]" {
            output.push_str("data: [DONE]\n\n");
            continue;
        }

        let parsed: serde_json::Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let text = parsed
            .get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.get(0))
            .and_then(|p| p.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("");

        let chunk = serde_json::json!({
            "id": format!("chatcmpl-gemini-{}", uuid_simple()),
            "object": "chat.completion.chunk",
            "model": model,
            "choices": [{
                "index": 0,
                "delta": {
                    "content": text,
                },
                "finish_reason": null,
            }],
        });

        output.push_str(&format!(
            "data: {}\n\n",
            serde_json::to_string(&chunk).unwrap_or_default()
        ));
    }

    output
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:x}", ts)
}

// ── /v1/models handler ───────────────────────────────────────────────────────

async fn handle_list_models(State(state): State<Arc<ProxyState>>) -> Json<ModelListResponse> {
    let providers = state.providers.read().await;
    let mut models = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Inject the virtual "__lab_auto__" model for automatic routing.
    // When a request targets this model, the router picks the best available provider.
    models.push(ModelInfo {
        id: "__lab_auto__".to_string(),
        object: "model",
        owned_by: "lab-proxy".to_string(),
    });
    seen.insert("__lab_auto__".to_string());

    for provider in providers.iter() {
        for model_id in &provider.models {
            if seen.insert(model_id.clone()) {
                models.push(ModelInfo {
                    id: model_id.clone(),
                    object: "model",
                    owned_by: provider.name.clone(),
                });
            }
        }
    }

    Json(ModelListResponse {
        object: "list",
        data: models,
    })
}

// ── /health handler ──────────────────────────────────────────────────────────

async fn handle_health(State(state): State<Arc<ProxyState>>) -> Json<serde_json::Value> {
    let statuses = state.health.all_statuses();
    let strategy = state.router.strategy();
    Json(serde_json::json!({
        "status": "ok",
        "strategy": format!("{:?}", strategy),
        "providers": statuses,
    }))
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn error_response(status: StatusCode, message: &str) -> Response {
    let body = ErrorResponse {
        error: ErrorDetail {
            message: message.to_string(),
            r#type: "proxy_error",
            code: status.as_u16(),
        },
    };
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap_or_default()))
        .unwrap()
}

fn proxy_error_to_response(e: ProxyError) -> Response {
    match e {
        ProxyError::Upstream { status, body } => {
            let sc = StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
            Response::builder()
                .status(sc)
                .header("Content-Type", "application/json")
                .body(Body::from(body))
                .unwrap()
        }
        _ => error_response(StatusCode::BAD_GATEWAY, &e.to_string()),
    }
}
