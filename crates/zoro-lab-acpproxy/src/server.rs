// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

//! Axum-based OpenAI-compatible HTTP server that forwards requests to the
//! ACP Worker Pool. Supports both streaming (SSE) and non-streaming responses.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use axum::Json;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::AcpProxyError;
use crate::worker::{WorkerInfo, WorkerPool};

// ── Shared state ─────────────────────────────────────────────────────────────

struct ServerState {
    pool: Arc<WorkerPool>,
    access_token: RwLock<String>,
}

// ── Public API ───────────────────────────────────────────────────────────────

/// A running ACP Proxy server instance.
pub struct AcpProxyServer {
    state: Arc<ServerState>,
    port: u16,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
}

impl AcpProxyServer {
    /// Start the ACP Proxy HTTP server.
    pub async fn start(
        pool: Arc<WorkerPool>,
        listen_addr: &str,
        port: u16,
        access_token: String,
    ) -> Result<Self, AcpProxyError> {
        let state = Arc::new(ServerState {
            pool,
            access_token: RwLock::new(access_token),
        });

        let app = axum::Router::new()
            .route("/v1/chat/completions", post(handle_chat_completions))
            .route("/v1/models", get(handle_list_models))
            .route("/health", get(handle_health))
            .with_state(state.clone());

        let addr = format!("{}:{}", listen_addr, port);
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| AcpProxyError::Bind(format!("{}: {}", addr, e)))?;

        let actual_port = listener.local_addr().map(|a| a.port()).unwrap_or(port);

        tracing::info!(
            addr = %addr,
            port = actual_port,
            "ACP Proxy server started"
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

    /// Get the actual port the server is listening on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Get worker status info.
    pub fn worker_infos(&self) -> Vec<WorkerInfo> {
        self.state.pool.worker_infos()
    }

    /// Get queue size.
    pub fn queue_size(&self) -> usize {
        self.state.pool.queue_size()
    }

    /// Update the access token at runtime.
    pub async fn update_access_token(&self, token: String) {
        *self.state.access_token.write().await = token;
    }

    /// Gracefully shut down the server.
    pub fn shutdown(self) {
        let _ = self.shutdown_tx.send(true);
        tracing::info!("ACP Proxy server shutting down");
    }
}

// ── Request / response types ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ChatCompletionRequest {
    #[serde(default)]
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(default)]
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    role: String,
    #[serde(default)]
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatCompletionResponse {
    id: String,
    object: &'static str,
    model: String,
    choices: Vec<Choice>,
}

#[derive(Debug, Serialize)]
struct Choice {
    index: usize,
    message: AssistantMessage,
    finish_reason: &'static str,
}

#[derive(Debug, Serialize)]
struct AssistantMessage {
    role: &'static str,
    content: String,
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
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    Json(req): Json<ChatCompletionRequest>,
) -> Response {
    // Check access token
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

    // Convert OpenAI messages into a single prompt string
    let prompt = messages_to_prompt(&req.messages);

    tracing::info!(
        model = %req.model,
        stream = req.stream,
        msg_count = req.messages.len(),
        prompt_len = prompt.len(),
        "ACP Proxy server received chat completion request"
    );

    if prompt.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "No message content provided");
    }

    // Send to worker pool and wait for response
    let reply_rx = state.pool.send_request(prompt);

    let result = match reply_rx.await {
        Ok(r) => r,
        Err(_) => Err("Worker pool channel closed".to_string()),
    };

    match result {
        Ok(text) => {
            tracing::info!(
                response_len = text.len(),
                "ACP Proxy server sending response"
            );
            let model = if req.model.is_empty() {
                "Zoro-ACP-Proxy".to_string()
            } else {
                req.model
            };

            if req.stream {
                // Return SSE stream with the complete text as a single chunk
                // (ACP collects text synchronously, so we return it all at once)
                build_sse_response(&model, &text)
            } else {
                let response = ChatCompletionResponse {
                    id: format!("chatcmpl-acp-{}", uuid_simple()),
                    object: "chat.completion",
                    model,
                    choices: vec![Choice {
                        index: 0,
                        message: AssistantMessage {
                            role: "assistant",
                            content: text,
                        },
                        finish_reason: "stop",
                    }],
                };

                Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        serde_json::to_string(&response).unwrap_or_default(),
                    ))
                    .unwrap()
            }
        }
        Err(e) => error_response(StatusCode::BAD_GATEWAY, &e),
    }
}

async fn handle_list_models(State(_state): State<Arc<ServerState>>) -> Json<ModelListResponse> {
    Json(ModelListResponse {
        object: "list",
        data: vec![ModelInfo {
            id: "Zoro-ACP-Proxy".to_string(),
            object: "model",
            owned_by: "acp-proxy".to_string(),
        }],
    })
}

async fn handle_health(State(state): State<Arc<ServerState>>) -> Json<serde_json::Value> {
    let workers = state.pool.worker_infos();
    let queue = state.pool.queue_size();
    Json(serde_json::json!({
        "status": "ok",
        "workers": workers,
        "queue_size": queue,
    }))
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Convert OpenAI chat messages into a single prompt string for ACP.
/// System messages are prepended as instructions, user messages become
/// the main content, and assistant messages are included for context.
fn messages_to_prompt(messages: &[ChatMessage]) -> String {
    let mut parts = Vec::new();

    for msg in messages {
        if msg.content.is_empty() {
            continue;
        }
        match msg.role.as_str() {
            "system" => {
                parts.push(format!("[System Instructions]\n{}", msg.content));
            }
            "user" => {
                parts.push(msg.content.clone());
            }
            "assistant" => {
                parts.push(format!("[Previous Assistant Response]\n{}", msg.content));
            }
            _ => {
                parts.push(msg.content.clone());
            }
        }
    }

    parts.join("\n\n")
}

/// Build an SSE response that sends the text as streaming chunks, then [DONE].
fn build_sse_response(model: &str, text: &str) -> Response {
    let chunk = serde_json::json!({
        "id": format!("chatcmpl-acp-{}", uuid_simple()),
        "object": "chat.completion.chunk",
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {
                "role": "assistant",
                "content": text,
            },
            "finish_reason": null,
        }],
    });

    let done_chunk = serde_json::json!({
        "id": format!("chatcmpl-acp-{}", uuid_simple()),
        "object": "chat.completion.chunk",
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {},
            "finish_reason": "stop",
        }],
    });

    let mut body = String::new();
    body.push_str(&format!(
        "data: {}\n\n",
        serde_json::to_string(&chunk).unwrap_or_default()
    ));
    body.push_str(&format!(
        "data: {}\n\n",
        serde_json::to_string(&done_chunk).unwrap_or_default()
    ));
    body.push_str("data: [DONE]\n\n");

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .body(Body::from(body))
        .unwrap()
}

fn error_response(status: StatusCode, message: &str) -> Response {
    let body = ErrorResponse {
        error: ErrorDetail {
            message: message.to_string(),
            r#type: "acp_proxy_error",
            code: status.as_u16(),
        },
    };
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap_or_default()))
        .unwrap()
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:x}", ts)
}
