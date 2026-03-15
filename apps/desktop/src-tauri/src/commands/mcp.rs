// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::AppState;
use std::path::PathBuf;
use tauri::{Manager, State};

#[derive(Debug, serde::Serialize)]
pub struct McpStatus {
    pub enabled: bool,
    pub running: bool,
    pub transport: String,
    pub port: u16,
    pub pid: Option<u32>,
    pub binary_found: bool,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateMcpConfigInput {
    pub enabled: Option<bool>,
    pub transport: Option<String>,
    pub port: Option<u16>,
}

fn find_mcp_binary() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "zoro-mcp.exe"
    } else {
        "zoro-mcp"
    };

    // 1. Next to the current executable (production bundles and dev when both are
    //    built into the same target/<profile>/ directory).
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    // 2. Cargo workspace target directories (dev mode). Walk up from the
    //    executable path looking for a `target` directory that contains the
    //    binary in debug or release sub-directories.
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent();
        while let Some(d) = dir {
            if d.file_name().is_some_and(|n| n == "target") {
                for profile in &["debug", "release"] {
                    let candidate = d.join(profile).join(name);
                    if candidate.exists() {
                        return Some(candidate);
                    }
                }
            }
            dir = d.parent();
        }
    }

    // 3. Resolve via PATH (system-wide install).
    if let Ok(path) = which::which(name) {
        return Some(path);
    }

    None
}

fn is_child_running(child: &mut std::process::Child) -> bool {
    matches!(child.try_wait(), Ok(None))
}

fn build_status(state: &AppState) -> Result<McpStatus, String> {
    let config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?;
    let mut mcp_guard = state
        .mcp_child
        .lock()
        .map_err(|e| format!("MCP lock error: {}", e))?;

    let (running, pid) = match mcp_guard.as_mut() {
        Some(child) => {
            if is_child_running(child) {
                (true, Some(child.id()))
            } else {
                *mcp_guard = None;
                (false, None)
            }
        }
        None => (false, None),
    };

    Ok(McpStatus {
        enabled: config.mcp.enabled,
        running,
        transport: config.mcp.transport.clone(),
        port: config.mcp.port,
        pid,
        binary_found: find_mcp_binary().is_some(),
    })
}

/// Start the MCP server as a child process. Called from commands and auto-start.
pub fn start_mcp_process(app: &tauri::AppHandle) -> Result<McpStatus, String> {
    let state = app.state::<AppState>();

    let binary = find_mcp_binary().ok_or_else(|| {
        "MCP server binary (zoro-mcp) not found next to the application executable".to_string()
    })?;

    let (transport, port, data_dir) = {
        let config = state
            .config
            .lock()
            .map_err(|e| format!("Config lock error: {}", e))?;
        (
            config.mcp.transport.clone(),
            config.mcp.port,
            state.data_dir.to_string_lossy().to_string(),
        )
    };

    // Stop existing process if any
    {
        let mut guard = state
            .mcp_child
            .lock()
            .map_err(|e| format!("MCP lock error: {}", e))?;
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    let child = std::process::Command::new(&binary)
        .arg("--transport")
        .arg(&transport)
        .arg("--port")
        .arg(port.to_string())
        .arg("--data-dir")
        .arg(&data_dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start MCP server: {}", e))?;

    tracing::info!(
        "MCP server started (PID {}, transport={}, port={})",
        child.id(),
        transport,
        port
    );

    {
        let mut guard = state
            .mcp_child
            .lock()
            .map_err(|e| format!("MCP lock error: {}", e))?;
        *guard = Some(child);
    }

    build_status(&state)
}

fn stop_mcp_process(state: &AppState) -> Result<(), String> {
    let mut guard = state
        .mcp_child
        .lock()
        .map_err(|e| format!("MCP lock error: {}", e))?;
    if let Some(mut child) = guard.take() {
        let _ = child.kill();
        let _ = child.wait();
        tracing::info!("MCP server stopped");
    }
    Ok(())
}

#[tauri::command]
pub async fn get_mcp_status(state: State<'_, AppState>) -> Result<McpStatus, String> {
    build_status(&state)
}

#[tauri::command]
pub async fn update_mcp_config(
    state: State<'_, AppState>,
    input: UpdateMcpConfigInput,
) -> Result<McpStatus, String> {
    {
        let mut config = state
            .config
            .lock()
            .map_err(|e| format!("Config lock error: {}", e))?;
        if let Some(enabled) = input.enabled {
            config.mcp.enabled = enabled;
        }
        if let Some(ref transport) = input.transport {
            config.mcp.transport = transport.clone();
        }
        if let Some(port) = input.port {
            config.mcp.port = port;
        }
        crate::storage::config::save_config(&state.data_dir, &config)
            .map_err(|e| format!("Failed to save config: {}", e))?;
    }
    build_status(&state)
}

#[tauri::command]
pub async fn start_mcp_server(app: tauri::AppHandle) -> Result<McpStatus, String> {
    start_mcp_process(&app)
}

#[tauri::command]
pub async fn stop_mcp_server(state: State<'_, AppState>) -> Result<McpStatus, String> {
    stop_mcp_process(&state)?;
    build_status(&state)
}

#[tauri::command]
pub async fn restart_mcp_server(app: tauri::AppHandle) -> Result<McpStatus, String> {
    let state = app.state::<AppState>();
    stop_mcp_process(&state)?;
    start_mcp_process(&app)
}
