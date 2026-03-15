// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::AppState;
use tauri::State;

#[derive(Debug, serde::Serialize)]
pub struct ConnectorStatus {
    pub enabled: bool,
    pub port: u16,
    pub running: bool,
    pub zotero_compat_enabled: bool,
    pub zotero_compat_port: u16,
    pub zotero_compat_running: bool,
    pub zotero_compat_error: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct ConnectorConfigResponse {
    pub port: u16,
    pub enabled: bool,
    pub zotero_compat_enabled: bool,
    pub zotero_compat_port: u16,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateConnectorConfigInput {
    pub zotero_compat_enabled: Option<bool>,
    pub zotero_compat_port: Option<u16>,
}

#[tauri::command]
pub async fn get_connector_status(state: State<'_, AppState>) -> Result<ConnectorStatus, String> {
    let config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?;
    let zotero_running = state
        .zotero_compat_cancel
        .lock()
        .map(|g| g.is_some())
        .unwrap_or(false);
    let zotero_error = state
        .zotero_compat_error
        .lock()
        .ok()
        .and_then(|g| g.clone());

    Ok(ConnectorStatus {
        enabled: config.connector.enabled,
        port: config.connector.port,
        running: true, // Native connector always runs
        zotero_compat_enabled: config.connector.zotero_compat_enabled,
        zotero_compat_port: config.connector.zotero_compat_port,
        zotero_compat_running: zotero_running,
        zotero_compat_error: zotero_error,
    })
}

#[tauri::command]
pub async fn get_connector_config(
    state: State<'_, AppState>,
) -> Result<ConnectorConfigResponse, String> {
    let config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?;
    Ok(ConnectorConfigResponse {
        port: config.connector.port,
        enabled: config.connector.enabled,
        zotero_compat_enabled: config.connector.zotero_compat_enabled,
        zotero_compat_port: config.connector.zotero_compat_port,
    })
}

#[tauri::command]
pub async fn update_connector_config(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    input: UpdateConnectorConfigInput,
) -> Result<ConnectorConfigResponse, String> {
    let (old_enabled, new_enabled, port) = {
        let mut config = state
            .config
            .lock()
            .map_err(|e| format!("Config lock error: {}", e))?;

        let old_enabled = config.connector.zotero_compat_enabled;

        if let Some(enabled) = input.zotero_compat_enabled {
            config.connector.zotero_compat_enabled = enabled;
        }
        if let Some(port) = input.zotero_compat_port {
            config.connector.zotero_compat_port = port;
        }

        // Persist config
        crate::storage::config::save_config(&state.data_dir, &config)
            .map_err(|e| format!("Failed to save config: {}", e))?;

        let new_enabled = config.connector.zotero_compat_enabled;
        let port = config.connector.zotero_compat_port;

        (old_enabled, new_enabled, port)
    };

    // Start or stop the Zotero compat server based on config change
    if old_enabled != new_enabled {
        if new_enabled {
            crate::connector::zotero_compat::spawn_zotero_compat_server(app, port);
        } else {
            crate::connector::zotero_compat::stop_zotero_compat_server(&app);
        }
    }

    // Return updated config
    let config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?;
    Ok(ConnectorConfigResponse {
        port: config.connector.port,
        enabled: config.connector.enabled,
        zotero_compat_enabled: config.connector.zotero_compat_enabled,
        zotero_compat_port: config.connector.zotero_compat_port,
    })
}
