// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::storage;
use crate::AppState;
use tauri_plugin_updater::UpdaterExt;

/// Response returned when checking for updates.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResponse {
    /// Whether an update is available.
    pub available: bool,
    /// The new version string (e.g. "0.2.0"), empty if no update.
    pub version: String,
    /// Release notes / changelog body, if any.
    pub body: String,
    /// Current app version.
    pub current_version: String,
}

/// Response for the updater config.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterConfigResponse {
    pub auto_check: bool,
    pub skipped_version: String,
}

/// Check for available updates. Returns update info without downloading.
#[tauri::command]
pub async fn check_for_update(app: tauri::AppHandle) -> Result<UpdateCheckResponse, String> {
    let current_version = app.package_info().version.to_string();

    let updater = app
        .updater()
        .map_err(|e| format!("Failed to build updater: {}", e))?;

    let update = updater
        .check()
        .await
        .map_err(|e| format!("Failed to check for updates: {}", e))?;

    match update {
        Some(update) => Ok(UpdateCheckResponse {
            available: true,
            version: update.version.clone(),
            body: update.body.clone().unwrap_or_default(),
            current_version,
        }),
        None => Ok(UpdateCheckResponse {
            available: false,
            version: String::new(),
            body: String::new(),
            current_version,
        }),
    }
}

/// Download and install the pending update, then restart the app.
#[tauri::command]
pub async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    let updater = app
        .updater()
        .map_err(|e| format!("Failed to build updater: {}", e))?;

    let update = updater
        .check()
        .await
        .map_err(|e| format!("Failed to check for updates: {}", e))?;

    let update = update.ok_or_else(|| "No update available".to_string())?;

    // Download and install in one step
    update
        .download_and_install(
            |chunk_length, content_length| {
                tracing::debug!("Downloaded chunk {} / {:?}", chunk_length, content_length);
            },
            || {
                tracing::info!("Update download complete");
            },
        )
        .await
        .map_err(|e| format!("Failed to download and install update: {}", e))?;

    tracing::info!("Update installed, restarting app");

    // Request app restart
    app.restart();
}

/// Get the current updater configuration.
#[tauri::command]
pub async fn get_updater_config(
    state: tauri::State<'_, AppState>,
) -> Result<UpdaterConfigResponse, String> {
    let config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?;
    Ok(UpdaterConfigResponse {
        auto_check: config.updater.auto_check,
        skipped_version: config.updater.skipped_version.clone(),
    })
}

/// Update the updater configuration (auto_check toggle, skipped version).
#[tauri::command]
pub async fn update_updater_config(
    state: tauri::State<'_, AppState>,
    auto_check: Option<bool>,
    skipped_version: Option<String>,
) -> Result<(), String> {
    let mut config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?;

    if let Some(ac) = auto_check {
        config.updater.auto_check = ac;
    }
    if let Some(sv) = skipped_version {
        config.updater.skipped_version = sv;
    }

    storage::config::save_config(&state.data_dir, &config)
        .map_err(|e| format!("Failed to save config: {}", e))?;

    Ok(())
}
