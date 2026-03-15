// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::AppState;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, State};

const MAX_HISTORY_BYTES: usize = 1024 * 1024;

pub struct TerminalWriter(Box<dyn Write + Send>);

pub struct TerminalSession {
    paper_id: String,
    writer: Arc<Mutex<TerminalWriter>>,
    master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
    output_history: Arc<Mutex<Vec<u8>>>,
}

pub type TerminalMap = Arc<Mutex<HashMap<String, TerminalSession>>>;

pub fn new_terminal_map() -> TerminalMap {
    Arc::new(Mutex::new(HashMap::new()))
}

#[derive(Clone, serde::Serialize)]
struct TerminalOutputEvent {
    terminal_id: String,
    data: String,
}

#[tauri::command]
pub async fn spawn_terminal(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    paper_id: String,
) -> Result<String, String> {
    // Reuse existing terminal for the same paper
    {
        let terminals = state
            .terminals
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        let map = terminals.lock().map_err(|e| format!("Lock error: {}", e))?;
        for (tid, session) in map.iter() {
            if session.paper_id == paper_id {
                return Ok(tid.clone());
            }
        }
    }

    let cwd = {
        let db = state
            .db
            .lock()
            .map_err(|e| format!("DB lock error: {}", e))?;
        let row = zoro_db::queries::papers::get_paper(&db.conn, &paper_id)
            .map_err(|e| format!("{}", e))?;
        let paper_dir = state.data_dir.join("library").join(&row.dir_path);
        if !paper_dir.exists() {
            return Err(format!(
                "Paper directory not found: {}",
                paper_dir.display()
            ));
        }
        paper_dir
    };

    let terminal_id = uuid::Uuid::new_v4().to_string();
    let output_history: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to open PTY: {}", e))?;

    let mut cmd = if cfg!(windows) {
        let comspec = std::env::var("COMSPEC").unwrap_or_else(|_| "powershell.exe".to_string());
        CommandBuilder::new(comspec)
    } else {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        CommandBuilder::new(shell)
    };
    cmd.cwd(&cwd);

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Failed to spawn shell: {}", e))?;

    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("Failed to get PTY writer: {}", e))?;

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("Failed to get PTY reader: {}", e))?;

    let session = TerminalSession {
        paper_id,
        writer: Arc::new(Mutex::new(TerminalWriter(writer))),
        master: Arc::new(Mutex::new(pair.master)),
        child: Arc::new(Mutex::new(child)),
        output_history: output_history.clone(),
    };

    {
        let terminals = state
            .terminals
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        terminals
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?
            .insert(terminal_id.clone(), session);
    }

    let tid = terminal_id.clone();
    let history_ref = output_history;
    let terminals_ref = state
        .terminals
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?
        .clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if let Ok(mut hist) = history_ref.lock() {
                        hist.extend_from_slice(&buf[..n]);
                        if hist.len() > MAX_HISTORY_BYTES {
                            let excess = hist.len() - MAX_HISTORY_BYTES;
                            hist.drain(..excess);
                        }
                    }
                    let text = String::from_utf8_lossy(&buf[..n]).to_string();
                    let _ = app.emit(
                        "terminal-output",
                        TerminalOutputEvent {
                            terminal_id: tid.clone(),
                            data: text,
                        },
                    );
                }
                Err(_) => break,
            }
        }
        // Clean up after PTY closes
        if let Ok(mut map) = terminals_ref.lock() {
            map.remove(&tid);
        }
    });

    Ok(terminal_id)
}

#[tauri::command]
pub async fn get_terminal_history(
    state: State<'_, AppState>,
    terminal_id: String,
) -> Result<String, String> {
    let terminals = state
        .terminals
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    let map = terminals.lock().map_err(|e| format!("Lock error: {}", e))?;
    let session = map
        .get(&terminal_id)
        .ok_or_else(|| "Terminal not found".to_string())?;
    let hist = session
        .output_history
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    Ok(String::from_utf8_lossy(&hist).to_string())
}

#[tauri::command]
pub async fn write_terminal(
    state: State<'_, AppState>,
    terminal_id: String,
    data: String,
) -> Result<(), String> {
    let terminals = state
        .terminals
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    let map = terminals.lock().map_err(|e| format!("Lock error: {}", e))?;
    let session = map
        .get(&terminal_id)
        .ok_or_else(|| "Terminal not found".to_string())?;
    let mut writer = session
        .writer
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    writer
        .0
        .write_all(data.as_bytes())
        .map_err(|e| format!("Write error: {}", e))?;
    writer
        .0
        .flush()
        .map_err(|e| format!("Flush error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn resize_terminal(
    state: State<'_, AppState>,
    terminal_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let terminals = state
        .terminals
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    let map = terminals.lock().map_err(|e| format!("Lock error: {}", e))?;
    let session = map
        .get(&terminal_id)
        .ok_or_else(|| "Terminal not found".to_string())?;
    let master = session
        .master
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    master
        .resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Resize error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn close_terminal(state: State<'_, AppState>, terminal_id: String) -> Result<(), String> {
    let terminals = state
        .terminals
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    let mut map = terminals.lock().map_err(|e| format!("Lock error: {}", e))?;
    if let Some(session) = map.remove(&terminal_id) {
        if let Ok(mut child) = session.child.lock() {
            let _ = child.kill();
        }
    }
    Ok(())
}
