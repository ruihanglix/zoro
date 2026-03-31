// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::log_buffer::LogEntry;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_logs(
    state: State<'_, AppState>,
    since_id: Option<u64>,
) -> Result<Vec<LogEntry>, String> {
    let buf = state
        .log_buffer
        .lock()
        .map_err(|e| format!("Log buffer lock error: {}", e))?;

    let entries: Vec<LogEntry> = match since_id {
        Some(id) => buf.iter().filter(|e| e.id > id).cloned().collect(),
        None => buf.iter().cloned().collect(),
    };

    Ok(entries)
}

#[tauri::command]
pub async fn set_debug_mode(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let new_filter = crate::make_filter(enabled);
    state
        .filter_handle
        .reload(new_filter)
        .map_err(|e| format!("Failed to reload log filter: {}", e))?;

    state
        .debug_mode
        .store(enabled, std::sync::atomic::Ordering::Relaxed);

    let level = if enabled { "DEBUG" } else { "INFO" };
    tracing::info!(
        "Debug mode {}: log level set to {}",
        if enabled { "enabled" } else { "disabled" },
        level
    );

    Ok(())
}

#[tauri::command]
pub async fn clear_logs(state: State<'_, AppState>) -> Result<(), String> {
    let mut buf = state
        .log_buffer
        .lock()
        .map_err(|e| format!("Log buffer lock error: {}", e))?;
    buf.clear();
    Ok(())
}

/// Receive a log entry from the frontend and push it into the shared log buffer.
/// This allows frontend logs to appear in the in-app LogPanel alongside backend logs.
#[tauri::command]
pub async fn push_frontend_log(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    level: String,
    source: String,
    message: String,
) -> Result<(), String> {
    use crate::log_buffer::LogEntry;

    let entry = LogEntry {
        id: crate::log_buffer::next_id(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        level,
        target: format!("frontend::{}", source),
        message,
    };

    // Push into ring buffer
    {
        let mut buf = state
            .log_buffer
            .lock()
            .map_err(|e| format!("Log buffer lock error: {}", e))?;
        if buf.len() >= 2000 {
            buf.pop_front();
        }
        buf.push_back(entry.clone());
    }

    // Emit to frontend LogPanel
    use tauri::Emitter;
    let _ = app.emit("log-entry", &entry);

    Ok(())
}

/// Response for the log configuration.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogConfigResponse {
    pub log_to_file: bool,
    pub log_retention_days: u32,
}

/// Get the current log configuration.
#[tauri::command]
pub async fn get_log_config(state: State<'_, AppState>) -> Result<LogConfigResponse, String> {
    let config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?;
    Ok(LogConfigResponse {
        log_to_file: config.general.log_to_file,
        log_retention_days: config.general.log_retention_days,
    })
}

/// Update the log configuration. Changes take effect after restart.
#[tauri::command]
pub async fn update_log_config(
    state: State<'_, AppState>,
    log_to_file: Option<bool>,
    log_retention_days: Option<u32>,
) -> Result<(), String> {
    let mut config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?;

    if let Some(v) = log_to_file {
        config.general.log_to_file = v;
    }
    if let Some(v) = log_retention_days {
        config.general.log_retention_days = v;
    }

    crate::storage::config::save_config(&state.data_dir, &config)
        .map_err(|e| format!("Failed to save config: {}", e))?;

    Ok(())
}
