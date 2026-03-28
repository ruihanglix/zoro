// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

//! Tauri commands for the ACP Proxy lab feature.
//! Manages the ACP proxy server lifecycle, worker pool, and configuration.

use std::sync::Arc;

use serde::Serialize;
use tauri::State;
use tokio::sync::Mutex;
use zoro_lab_acpproxy::{AcpProxyConfig, AcpProxyServer, WorkerInfo, WorkerPool};

use crate::AcpState;

/// Shared state for the ACP Proxy feature.
pub struct AcpProxyState {
    pub config: Mutex<AcpProxyConfig>,
    pub server: Mutex<Option<AcpProxyServer>>,
    pub pool: Mutex<Option<Arc<WorkerPool>>>,
    pub data_dir: std::path::PathBuf,
}

impl AcpProxyState {
    pub fn new(data_dir: &std::path::Path) -> Self {
        let config = zoro_lab_acpproxy::config::load_config(data_dir);
        Self {
            config: Mutex::new(config),
            server: Mutex::new(None),
            pool: Mutex::new(None),
            data_dir: data_dir.to_path_buf(),
        }
    }
}

// ── Response types ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpProxyStatusResponse {
    pub running: bool,
    pub port: u16,
    pub listen_addr: String,
    pub worker_count: usize,
    pub queue_size: usize,
    pub workers: Vec<WorkerInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpProxyConfigResponse {
    pub enabled: bool,
    pub agent_name: String,
    pub mode_config_id: String,
    pub mode_value: String,
    pub model_config_id: String,
    pub model_value: String,
    pub worker_count: usize,
    pub port: u16,
    pub lan_access: bool,
    pub access_token: String,
}

// ── Commands ─────────────────────────────────────────────────────────────────

/// Get the current ACP Proxy configuration.
#[tauri::command]
pub async fn acp_proxy_get_config(
    state: State<'_, AcpProxyState>,
) -> Result<AcpProxyConfigResponse, String> {
    let config = state.config.lock().await;
    Ok(AcpProxyConfigResponse {
        enabled: config.enabled,
        agent_name: config.agent_name.clone(),
        mode_config_id: config.mode_config_id.clone(),
        mode_value: config.mode_value.clone(),
        model_config_id: config.model_config_id.clone(),
        model_value: config.model_value.clone(),
        worker_count: config.worker_count,
        port: config.port,
        lan_access: config.lan_access,
        access_token: config.access_token.clone(),
    })
}

/// Update ACP Proxy configuration. Saves to disk.
#[tauri::command]
pub async fn acp_proxy_update_config(
    state: State<'_, AcpProxyState>,
    config: AcpProxyConfig,
) -> Result<(), String> {
    let mut current = state.config.lock().await;
    *current = config;
    zoro_lab_acpproxy::config::save_config(&state.data_dir, &current);
    Ok(())
}

/// Get the current ACP Proxy server status.
#[tauri::command]
pub async fn acp_proxy_get_status(
    state: State<'_, AcpProxyState>,
) -> Result<AcpProxyStatusResponse, String> {
    let server = state.server.lock().await;
    let config = state.config.lock().await;

    match &*server {
        Some(srv) => Ok(AcpProxyStatusResponse {
            running: true,
            port: srv.port(),
            listen_addr: if config.lan_access {
                "0.0.0.0".into()
            } else {
                "127.0.0.1".into()
            },
            worker_count: srv.worker_infos().len(),
            queue_size: srv.queue_size(),
            workers: srv.worker_infos(),
        }),
        None => Ok(AcpProxyStatusResponse {
            running: false,
            port: 0,
            listen_addr: "127.0.0.1".into(),
            worker_count: 0,
            queue_size: 0,
            workers: Vec::new(),
        }),
    }
}

/// Start the ACP Proxy server with worker pool.
#[tauri::command]
pub async fn acp_proxy_start(
    state: State<'_, AcpProxyState>,
    acp_state: State<'_, AcpState>,
) -> Result<AcpProxyStatusResponse, String> {
    let mut server_guard = state.server.lock().await;

    // If already running, return current status
    if server_guard.is_some() {
        drop(server_guard);
        return acp_proxy_get_status(state).await;
    }

    let config = state.config.lock().await.clone();

    // Find the agent config — auto-select the first available agent if none configured
    let acp_config = zoro_acp::config::load_config(&acp_state.data_dir);
    let agent_config = if config.agent_name.is_empty() {
        // Pick the first available agent automatically
        let first = acp_config.agents.first().cloned();
        match first {
            Some(agent) => {
                // Persist the auto-selected agent name
                let mut cfg = state.config.lock().await;
                cfg.agent_name = agent.name.clone();
                zoro_lab_acpproxy::config::save_config(&state.data_dir, &cfg);
                agent
            }
            None => {
                return Err("No ACP agents available. Please install an ACP agent first.".into());
            }
        }
    } else {
        acp_config
            .agents
            .iter()
            .find(|a| a.name == config.agent_name)
            .ok_or_else(|| format!("Agent '{}' not found in ACP config", config.agent_name))?
            .clone()
    };

    // Build config overrides from mode + model settings
    let mut config_overrides = Vec::new();
    if !config.mode_config_id.is_empty() && !config.mode_value.is_empty() {
        config_overrides.push((config.mode_config_id.clone(), config.mode_value.clone()));
    }
    if !config.model_config_id.is_empty() && !config.model_value.is_empty() {
        config_overrides.push((config.model_config_id.clone(), config.model_value.clone()));
    }

    let listen_addr = if config.lan_access {
        "0.0.0.0"
    } else {
        "127.0.0.1"
    };

    // Start worker pool
    let pool = WorkerPool::start(
        acp_state.manager.clone(),
        agent_config,
        config_overrides,
        config.worker_count,
    )
    .await
    .map_err(|e| format!("Failed to start worker pool: {}", e))?;

    let pool = Arc::new(pool);
    *state.pool.lock().await = Some(pool.clone());

    // Start HTTP server
    let server = AcpProxyServer::start(
        pool.clone(),
        listen_addr,
        config.port,
        config.access_token.clone(),
    )
    .await
    .map_err(|e| format!("Failed to start ACP Proxy server: {}", e))?;

    let port = server.port();
    let workers = server.worker_infos();
    let worker_count = workers.len();

    *server_guard = Some(server);

    Ok(AcpProxyStatusResponse {
        running: true,
        port,
        listen_addr: listen_addr.to_string(),
        worker_count,
        queue_size: 0,
        workers,
    })
}

/// Stop the ACP Proxy server and all workers.
#[tauri::command]
pub async fn acp_proxy_stop(
    state: State<'_, AcpProxyState>,
    acp_state: State<'_, AcpState>,
) -> Result<(), String> {
    // Shutdown the HTTP server
    let mut server_guard = state.server.lock().await;
    if let Some(server) = server_guard.take() {
        server.shutdown();
    }

    // Stop all worker sessions
    let pool_guard = state.pool.lock().await;
    if let Some(ref pool) = *pool_guard {
        let mgr = acp_state.manager.lock().await;
        for name in pool.worker_names() {
            let _ = mgr.stop_session(name).await;
        }
    }
    drop(pool_guard);
    *state.pool.lock().await = None;

    Ok(())
}

/// Enable or disable ACP Proxy.
/// When enabled, automatically tries to start the proxy server. If no agent
/// is available yet, the proxy stays enabled but not running (no error).
/// When disabled, stops the proxy if running.
#[tauri::command]
pub async fn acp_proxy_set_enabled(
    state: State<'_, AcpProxyState>,
    acp_state: State<'_, AcpState>,
    enabled: bool,
) -> Result<AcpProxyStatusResponse, String> {
    {
        let mut config = state.config.lock().await;
        config.enabled = enabled;
        zoro_lab_acpproxy::config::save_config(&state.data_dir, &config);
    }

    if enabled {
        // Try to start; if it fails (e.g. no agents available), that's OK —
        // the proxy is enabled but not running. It will start once the user
        // selects an agent or an agent becomes available.
        match acp_proxy_start(state.clone(), acp_state).await {
            Ok(status) => return Ok(status),
            Err(e) => {
                tracing::info!(error = %e, "ACP Proxy enabled but could not auto-start");
            }
        }
    } else {
        // Stop if currently running
        acp_proxy_stop(state.clone(), acp_state).await?;
    }

    acp_proxy_get_status(state).await
}

/// Fetch available config options (mode, model, etc.) for a given agent.
/// Starts a temporary session, collects config_options from the session init
/// or from an async ConfigOptionUpdate notification, then stops the session.
#[tauri::command]
pub async fn acp_proxy_fetch_config_options(
    agent_name: String,
    acp_state: State<'_, AcpState>,
) -> Result<Vec<zoro_acp::ConfigOptionInfo>, String> {
    let cfg = zoro_acp::config::load_config(&acp_state.data_dir);
    let agent_config = cfg
        .agents
        .iter()
        .find(|a| a.name == agent_name)
        .ok_or_else(|| format!("Agent '{}' not found", agent_name))?
        .clone();

    // Use a unique probe session name to avoid conflicts
    let probe_name = format!("__acp_proxy_probe_{}", agent_name);
    let probe_agent = zoro_acp::AgentConfig {
        name: probe_name.clone(),
        ..agent_config
    };

    // Collect config_options from the session callback (may arrive via
    // new_session response OR via an async ConfigOptionUpdate notification).
    let config_options = Arc::new(Mutex::new(Vec::<zoro_acp::ConfigOptionInfo>::new()));
    let opts_clone = config_options.clone();
    let notify = Arc::new(tokio::sync::Notify::new());
    let notify_clone = notify.clone();

    let manager = acp_state.manager.lock().await;

    // Start the probe session — config_options are pushed via the callback
    let session_result = manager
        .start_session(&probe_agent, None, move |update| {
            if let zoro_acp::AgentUpdate::ConfigOptions {
                config_options: opts,
                ..
            } = update
            {
                // We run inside a sync callback, so use try_lock
                if let Ok(mut guard) = opts_clone.try_lock() {
                    *guard = opts;
                }
                notify_clone.notify_one();
            }
        })
        .await;

    match session_result {
        Ok(_session_id) => {
            // Config options may arrive synchronously (in new_session response)
            // or asynchronously (via ConfigOptionUpdate notification).
            // Check if we already have them; if not, wait up to 3 seconds.
            {
                let current = config_options.lock().await;
                if current.is_empty() {
                    drop(current);
                    let _ = tokio::time::timeout(
                        std::time::Duration::from_secs(3),
                        notify.notified(),
                    )
                    .await;
                }
            }
            // Stop the probe session
            let _ = manager.stop_session(&probe_name).await;
            let result = config_options.lock().await.clone();
            Ok(result)
        }
        Err(e) => {
            let _ = manager.stop_session(&probe_name).await;
            Err(format!("Failed to probe config options: {}", e))
        }
    }
}

/// Get the persisted config options cache (agent_name → config options).
/// Used by the frontend to instantly display cached mode/model lists on startup.
#[tauri::command]
pub async fn acp_proxy_get_options_cache(
    state: State<'_, AcpProxyState>,
) -> Result<std::collections::HashMap<String, serde_json::Value>, String> {
    Ok(zoro_lab_acpproxy::config::load_options_cache(&state.data_dir))
}

/// Save the config options cache to disk.
/// Called by the frontend after fetching fresh config options from an agent.
#[tauri::command]
pub async fn acp_proxy_save_options_cache(
    state: State<'_, AcpProxyState>,
    cache: std::collections::HashMap<String, serde_json::Value>,
) -> Result<(), String> {
    zoro_lab_acpproxy::config::save_options_cache(&state.data_dir, &cache);
    Ok(())
}
