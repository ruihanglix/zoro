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

/// Batch-fetch missing arXiv HTML for all papers that have an arXiv ID but no
/// downloaded `paper.html`. Returns the number of papers enqueued.
#[tauri::command]
pub async fn fetch_all_missing_arxiv_html(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<i32, String> {
    use zoro_db::queries::papers;

    let (rows, data_dir, semaphore, delay_secs, proxy_config) = {
        let db = state
            .db
            .lock()
            .map_err(|e| format!("DB lock error: {}", e))?;
        let filter = papers::PaperFilter {
            collection_id: None,
            tag_name: None,
            read_status: None,
            search_query: None,
            uncategorized: None,
            sort_by: None,
            sort_order: None,
            limit: Some(100_000),
            offset: None,
        };
        let rows = papers::list_papers(&db.conn, &filter)
            .map_err(|e| format!("Failed to list papers: {}", e))?;
        let delay = state
            .config
            .lock()
            .map(|c| c.general.html_fetch_delay_secs)
            .unwrap_or(3);
        let proxy_config = state
            .config
            .lock()
            .map(|c| c.proxy.clone())
            .unwrap_or_default();
        (
            rows,
            state.data_dir.clone(),
            state.html_fetch_semaphore.clone(),
            delay,
            proxy_config,
        )
    };

    let db_path = data_dir.join("library.db");
    let mut count = 0i32;

    for row in &rows {
        let arxiv_id = match row.arxiv_id.as_deref() {
            Some(id) if !id.is_empty() => id,
            _ => continue,
        };
        let paper_dir = data_dir.join("library").join(&row.dir_path);
        let html_path = paper_dir.join("paper.html");
        if html_path.exists() {
            continue;
        }

        crate::connector::handlers::enqueue_html_fetch(
            &app,
            semaphore.clone(),
            &db_path,
            delay_secs,
            &row.id,
            &row.title,
            arxiv_id,
            &paper_dir,
            proxy_config.clone(),
        );
        count += 1;
    }

    Ok(count)
}

/// Response for the HTML fetch configuration.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HtmlFetchConfigResponse {
    pub auto_fetch_arxiv_html: bool,
    pub html_fetch_concurrency: u32,
    pub html_fetch_delay_secs: u32,
}

/// Get the current HTML fetch configuration.
#[tauri::command]
pub async fn get_html_fetch_config(
    state: State<'_, AppState>,
) -> Result<HtmlFetchConfigResponse, String> {
    let config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?;
    Ok(HtmlFetchConfigResponse {
        auto_fetch_arxiv_html: config.general.auto_fetch_arxiv_html,
        html_fetch_concurrency: config.general.html_fetch_concurrency,
        html_fetch_delay_secs: config.general.html_fetch_delay_secs,
    })
}

/// Update the HTML fetch configuration. Concurrency changes require app restart.
#[tauri::command]
pub async fn update_html_fetch_config(
    state: State<'_, AppState>,
    auto_fetch_arxiv_html: Option<bool>,
    html_fetch_concurrency: Option<u32>,
    html_fetch_delay_secs: Option<u32>,
) -> Result<(), String> {
    let mut config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?;

    if let Some(v) = auto_fetch_arxiv_html {
        config.general.auto_fetch_arxiv_html = v;
    }
    if let Some(v) = html_fetch_concurrency {
        config.general.html_fetch_concurrency = v;
    }
    if let Some(v) = html_fetch_delay_secs {
        config.general.html_fetch_delay_secs = v;
    }

    crate::storage::config::save_config(&state.data_dir, &config)
        .map_err(|e| format!("Failed to save config: {}", e))?;

    Ok(())
}

/// Response for the proxy configuration.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyConfigResponse {
    pub enabled: bool,
    pub url: String,
    pub no_proxy: String,
}

/// Get the current network proxy configuration.
#[tauri::command]
pub async fn get_proxy_config(state: State<'_, AppState>) -> Result<ProxyConfigResponse, String> {
    let config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?;
    Ok(ProxyConfigResponse {
        enabled: config.proxy.enabled,
        url: config.proxy.url.clone(),
        no_proxy: config.proxy.no_proxy.clone(),
    })
}

/// Update the network proxy configuration. Changes take effect after restart.
#[tauri::command]
pub async fn update_proxy_config(
    state: State<'_, AppState>,
    enabled: Option<bool>,
    url: Option<String>,
    no_proxy: Option<String>,
) -> Result<(), String> {
    let mut config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?;

    if let Some(v) = enabled {
        config.proxy.enabled = v;
    }
    if let Some(v) = url {
        config.proxy.url = v;
    }
    if let Some(v) = no_proxy {
        config.proxy.no_proxy = v;
    }

    crate::storage::config::save_config(&state.data_dir, &config)
        .map_err(|e| format!("Failed to save config: {}", e))?;

    Ok(())
}
