// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

//! Tauri commands for the Lab free LLM service.
//! Manages the lab proxy server lifecycle, provider keys, model lists, etc.

use serde::Serialize;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;
use zoro_lab_freellm::{LabConfig, LabService};
use zoro_llm_proxy::{ProxyServer, RoutingStrategy};

/// Shared state for the Lab feature (async Mutex because LabService
/// contains async operations and proxy start/stop is async).
pub struct LabState {
    pub service: Arc<Mutex<LabService>>,
    pub proxy: Arc<Mutex<Option<ProxyServer>>>,
}

impl LabState {
    pub fn new(data_dir: &std::path::Path) -> Self {
        Self {
            service: Arc::new(Mutex::new(LabService::new(data_dir))),
            proxy: Arc::new(Mutex::new(None)),
        }
    }
}

// ── Response types ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct LabProxyStatus {
    pub running: bool,
    pub port: u16,
    pub listen_addr: String,
    pub provider_count: usize,
    pub model_count: usize,
    pub strategy: String,
    pub health: Vec<zoro_llm_proxy::ProviderHealthStatus>,
}

#[derive(Debug, Serialize)]
pub struct LabProviderInfo {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub sign_up_url: String,
    pub key_prefix: String,
    pub tier: String,
    pub has_key: bool,
    pub model_count: usize,
}

#[derive(Debug, Serialize)]
pub struct LabModelInfo {
    pub id: String,
    pub name: String,
    pub provider_id: String,
    pub disabled: bool,
}

// ── Commands ─────────────────────────────────────────────────────────────────

/// Get the current lab configuration.
#[tauri::command]
pub async fn lab_get_config(state: State<'_, LabState>) -> Result<LabConfig, String> {
    let service = state.service.lock().await;
    Ok(service.config().clone())
}

/// Update the lab configuration.
#[tauri::command]
pub async fn lab_update_config(
    state: State<'_, LabState>,
    config: LabConfig,
) -> Result<(), String> {
    let mut service = state.service.lock().await;
    *service.config_mut() = config;
    Ok(())
}

/// Get the list of all free providers with their status.
#[tauri::command]
pub async fn lab_list_providers(
    state: State<'_, LabState>,
) -> Result<Vec<LabProviderInfo>, String> {
    let service = state.service.lock().await;
    let providers = zoro_lab_freellm::FREE_PROVIDERS
        .iter()
        .map(|p| {
            let has_key = service
                .get_provider_key(&p.id)
                .map(|k| !k.is_empty())
                .unwrap_or(false);
            let models = service.available_models();
            let model_count = models.iter().filter(|m| m.provider_id == p.id).count();
            LabProviderInfo {
                id: p.id.clone(),
                name: p.name.clone(),
                display_name: p.display_name.clone(),
                sign_up_url: p.sign_up_url.clone(),
                key_prefix: p.key_prefix.clone(),
                tier: match p.tier {
                    zoro_lab_freellm::ProviderTier::Primary => "primary".into(),
                    zoro_lab_freellm::ProviderTier::Secondary => "secondary".into(),
                },
                has_key,
                model_count,
            }
        })
        .collect();
    Ok(providers)
}

/// Set an API key for a provider.
#[tauri::command]
pub async fn lab_set_provider_key(
    state: State<'_, LabState>,
    provider_id: String,
    api_key: String,
) -> Result<(), String> {
    let mut service = state.service.lock().await;
    service.set_provider_key(&provider_id, api_key);
    Ok(())
}

/// Get all available models.
#[tauri::command]
pub async fn lab_list_models(state: State<'_, LabState>) -> Result<Vec<LabModelInfo>, String> {
    let service = state.service.lock().await;
    let models = service
        .available_models()
        .into_iter()
        .map(|m| LabModelInfo {
            id: m.id,
            name: m.name,
            provider_id: m.provider_id,
            disabled: m.disabled,
        })
        .collect();
    Ok(models)
}

/// Toggle a model's disabled state.
#[tauri::command]
pub async fn lab_toggle_model(
    state: State<'_, LabState>,
    provider_id: String,
    model_id: String,
    disabled: bool,
) -> Result<(), String> {
    let mut service = state.service.lock().await;
    service.set_model_disabled(&provider_id, &model_id, disabled);
    Ok(())
}

/// Enable or disable all models for a given provider at once.
#[tauri::command]
pub async fn lab_toggle_provider(
    state: State<'_, LabState>,
    provider_id: String,
    disabled: bool,
) -> Result<(), String> {
    let mut service = state.service.lock().await;
    service.set_provider_all_disabled(&provider_id, disabled);
    Ok(())
}

/// Refresh model lists from all configured providers.
#[tauri::command]
pub async fn lab_refresh_models(
    state: State<'_, LabState>,
) -> Result<Vec<(String, Result<usize, String>)>, String> {
    let mut service = state.service.lock().await;
    let results = service.refresh_all_models().await;
    Ok(results)
}

/// Set the routing strategy.
#[tauri::command]
pub async fn lab_set_strategy(
    state: State<'_, LabState>,
    strategy: RoutingStrategy,
) -> Result<(), String> {
    let mut service = state.service.lock().await;
    service.set_routing_strategy(strategy);

    // If proxy is running, update it live
    let proxy = state.proxy.lock().await;
    if let Some(ref p) = *proxy {
        p.update_strategy(strategy);
    }

    Ok(())
}

/// Start the lab proxy server.
#[tauri::command]
pub async fn lab_start_proxy(state: State<'_, LabState>) -> Result<LabProxyStatus, String> {
    let mut proxy_guard = state.proxy.lock().await;

    // If already running, return current status
    if proxy_guard.is_some() {
        drop(proxy_guard);
        return lab_get_proxy_status(state).await;
    }

    let service = state.service.lock().await;

    if !service.has_configured_providers() {
        return Err("No providers configured. Please add at least one API key.".into());
    }

    let config = service.to_proxy_config();
    let provider_count = config.providers.len();
    let model_count: usize = config.providers.iter().map(|p| p.models.len()).sum();
    let listen_addr = config.listen_addr.clone();
    let strategy = format!("{:?}", config.routing_strategy);

    drop(service); // Release service lock before starting server

    let server = ProxyServer::start(config)
        .await
        .map_err(|e| e.to_string())?;
    let port = server.port();
    let health = server.health_status();

    *proxy_guard = Some(server);

    Ok(LabProxyStatus {
        running: true,
        port,
        listen_addr,
        provider_count,
        model_count,
        strategy,
        health,
    })
}

/// Stop the lab proxy server.
#[tauri::command]
pub async fn lab_stop_proxy(state: State<'_, LabState>) -> Result<(), String> {
    let mut proxy_guard = state.proxy.lock().await;
    if let Some(server) = proxy_guard.take() {
        server.shutdown();
    }
    Ok(())
}

/// Get the current proxy status.
#[tauri::command]
pub async fn lab_get_proxy_status(state: State<'_, LabState>) -> Result<LabProxyStatus, String> {
    let proxy_guard = state.proxy.lock().await;
    match &*proxy_guard {
        Some(server) => {
            let service = state.service.lock().await;
            let providers = service.to_proxy_providers();
            let model_count: usize = providers.iter().map(|p| p.models.len()).sum();
            let config = service.config();

            Ok(LabProxyStatus {
                running: true,
                port: server.port(),
                listen_addr: if config.lan_access {
                    "0.0.0.0".into()
                } else {
                    "127.0.0.1".into()
                },
                provider_count: providers.len(),
                model_count,
                strategy: format!("{:?}", config.routing_strategy),
                health: server.health_status(),
            })
        }
        None => Ok(LabProxyStatus {
            running: false,
            port: 0,
            listen_addr: "127.0.0.1".into(),
            provider_count: 0,
            model_count: 0,
            strategy: "Auto".into(),
            health: Vec::new(),
        }),
    }
}

/// Reload the proxy with updated provider configuration.
/// Call this after changing provider keys or model settings.
#[tauri::command]
pub async fn lab_reload_proxy(state: State<'_, LabState>) -> Result<LabProxyStatus, String> {
    let proxy_guard = state.proxy.lock().await;

    if let Some(ref server) = *proxy_guard {
        let service = state.service.lock().await;
        let providers = service.to_proxy_providers();
        server.update_providers(providers).await;
        server.update_strategy(service.routing_strategy());

        let config = service.config();
        let upstream = service.to_proxy_providers();
        let model_count: usize = upstream.iter().map(|p| p.models.len()).sum();

        Ok(LabProxyStatus {
            running: true,
            port: server.port(),
            listen_addr: if config.lan_access {
                "0.0.0.0".into()
            } else {
                "127.0.0.1".into()
            },
            provider_count: upstream.len(),
            model_count,
            strategy: format!("{:?}", config.routing_strategy),
            health: server.health_status(),
        })
    } else {
        Err("Proxy is not running".into())
    }
}

/// Set lab enabled/disabled. When enabled and providers are configured,
/// auto-starts the proxy. When disabled, stops the proxy.
#[tauri::command]
pub async fn lab_set_enabled(
    state: State<'_, LabState>,
    enabled: bool,
) -> Result<LabProxyStatus, String> {
    {
        let mut service = state.service.lock().await;
        service.set_enabled(enabled);
    }

    if enabled {
        // Refresh stale model caches before starting
        {
            let mut service = state.service.lock().await;
            service.refresh_stale_models().await;
        }
        // Try to start proxy; if no providers are configured yet, that's OK —
        // just return a stopped status so the user can configure keys first.
        match lab_start_proxy(state.clone()).await {
            Ok(status) => Ok(status),
            Err(_) => lab_get_proxy_status(state).await,
        }
    } else {
        lab_stop_proxy(state.clone()).await?;
        lab_get_proxy_status(state).await
    }
}

/// Set proxy port.
#[tauri::command]
pub async fn lab_set_proxy_port(state: State<'_, LabState>, port: u16) -> Result<(), String> {
    let mut service = state.service.lock().await;
    service.set_proxy_port(port);
    Ok(())
}

/// Set LAN access.
#[tauri::command]
pub async fn lab_set_lan_access(state: State<'_, LabState>, enabled: bool) -> Result<(), String> {
    let mut service = state.service.lock().await;
    service.set_lan_access(enabled);
    Ok(())
}
