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
