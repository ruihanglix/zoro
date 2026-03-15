// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{Emitter, State};

use crate::AcpState;
use zoro_acp::config;

// ── Chat session persistence ────────────────────────────────────────────────

fn sessions_dir(data_dir: &std::path::Path) -> std::path::PathBuf {
    data_dir.join("agent-sessions")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSessionFile {
    pub id: String,
    pub agent_name: String,
    pub title: String,
    pub messages: Vec<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSessionMeta {
    pub id: String,
    pub agent_name: String,
    pub title: String,
    pub message_count: usize,
    pub created_at: String,
    pub updated_at: String,
    pub cwd: Option<String>,
}

#[tauri::command]
pub async fn acp_list_chat_sessions(
    state: State<'_, AcpState>,
) -> Result<Vec<ChatSessionMeta>, String> {
    let dir = sessions_dir(&state.data_dir);
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut sessions = Vec::new();
    let entries =
        std::fs::read_dir(&dir).map_err(|e| format!("Failed to read sessions dir: {}", e))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(session) = serde_json::from_str::<ChatSessionFile>(&content) {
                sessions.push(ChatSessionMeta {
                    id: session.id,
                    agent_name: session.agent_name,
                    title: session.title,
                    message_count: session.messages.len(),
                    created_at: session.created_at,
                    updated_at: session.updated_at,
                    cwd: session.cwd,
                });
            }
        }
    }
    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(sessions)
}

#[tauri::command]
pub async fn acp_save_chat_session(
    session: ChatSessionFile,
    state: State<'_, AcpState>,
) -> Result<(), String> {
    let dir = sessions_dir(&state.data_dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create sessions dir: {}", e))?;
    let path = dir.join(format!("{}.json", session.id));
    let content =
        serde_json::to_string_pretty(&session).map_err(|e| format!("Serialize error: {}", e))?;
    std::fs::write(&path, content).map_err(|e| format!("Write error: {}", e))
}

#[tauri::command]
pub async fn acp_load_chat_session(
    session_id: String,
    state: State<'_, AcpState>,
) -> Result<ChatSessionFile, String> {
    let path = sessions_dir(&state.data_dir).join(format!("{}.json", session_id));
    let content = std::fs::read_to_string(&path).map_err(|e| format!("Read error: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Parse error: {}", e))
}

#[tauri::command]
pub async fn acp_delete_chat_session(
    session_id: String,
    state: State<'_, AcpState>,
) -> Result<(), String> {
    let path = sessions_dir(&state.data_dir).join(format!("{}.json", session_id));
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("Delete error: {}", e))?;
    }
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInfoResponse {
    pub name: String,
    pub title: String,
    pub description: String,
    pub has_session: bool,
}

fn command_exists(cmd: &str) -> bool {
    let path = zoro_acp::full_path();
    which::which_in(cmd, Some(path), ".").is_ok()
}

#[tauri::command]
pub async fn acp_list_agents(state: State<'_, AcpState>) -> Result<Vec<AgentInfoResponse>, String> {
    let cfg = config::load_config(&state.data_dir);
    let manager = state.manager.lock().await;

    let mut result = Vec::new();
    for agent in &cfg.agents {
        let check = agent.detect_command.as_deref().unwrap_or(&agent.command);
        if !command_exists(check) {
            continue;
        }
        let has_session = manager.has_session(&agent.name).await;
        result.push(AgentInfoResponse {
            name: agent.name.clone(),
            title: agent.title.clone(),
            description: agent.description.clone(),
            has_session,
        });
    }
    Ok(result)
}

#[tauri::command]
pub async fn acp_start_session(
    agent_name: String,
    cwd: Option<String>,
    app_handle: tauri::AppHandle,
    state: State<'_, AcpState>,
) -> Result<String, String> {
    let cfg = config::load_config(&state.data_dir);
    let agent_config = cfg
        .agents
        .iter()
        .find(|a| a.name == agent_name)
        .ok_or_else(|| format!("Agent not found: {}", agent_name))?
        .clone();

    let manager = state.manager.lock().await;

    if manager.has_session(&agent_name).await {
        manager
            .stop_session(&agent_name)
            .await
            .map_err(|e| format!("Failed to stop existing session: {}", e))?;
    }

    let handle = app_handle.clone();
    let session_id = manager
        .start_session(&agent_config, cwd, move |update| {
            let _ = handle.emit("acp-session-update", &update);
        })
        .await
        .map_err(|e| format!("Failed to start agent session: {}", e))?;

    Ok(session_id)
}

#[tauri::command]
pub async fn acp_get_paper_dir(
    paper_id: String,
    state: State<'_, crate::AppState>,
    acp_state: State<'_, AcpState>,
) -> Result<String, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let row =
        zoro_db::queries::papers::get_paper(&db.conn, &paper_id).map_err(|e| format!("{}", e))?;
    let paper_dir = acp_state.data_dir.join("library").join(&row.dir_path);
    Ok(paper_dir.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn acp_set_config_option(
    agent_name: String,
    config_id: String,
    value: String,
    state: State<'_, AcpState>,
) -> Result<Vec<zoro_acp::ConfigOptionInfo>, String> {
    let manager = state.manager.lock().await;
    manager
        .set_config_option(&agent_name, &config_id, &value)
        .await
        .map_err(|e| format!("Set config option failed: {}", e))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInput {
    pub base64_data: String,
    pub mime_type: String,
}

#[tauri::command]
pub async fn acp_send_prompt(
    agent_name: String,
    message: String,
    images: Option<Vec<ImageInput>>,
    app_handle: tauri::AppHandle,
    state: State<'_, AcpState>,
) -> Result<(), String> {
    let image_attachments: Vec<zoro_acp::ImageAttachment> = images
        .unwrap_or_default()
        .into_iter()
        .map(|img| zoro_acp::ImageAttachment {
            base64_data: img.base64_data,
            mime_type: img.mime_type,
        })
        .collect();

    let manager_ref = Arc::clone(&state.manager);
    let handle = app_handle.clone();

    // Run prompt in background so the command returns immediately;
    // streaming updates arrive via the "acp-session-update" event.
    tokio::spawn(async move {
        let mgr = manager_ref.lock().await;
        match mgr
            .send_prompt(&agent_name, &message, image_attachments)
            .await
        {
            Ok(stop_reason) => {
                let _ = handle.emit(
                    "acp-session-update",
                    &zoro_acp::AgentUpdate::PromptDone {
                        session_id: String::new(),
                        stop_reason,
                    },
                );
            }
            Err(e) => {
                let _ = handle.emit(
                    "acp-session-update",
                    &zoro_acp::AgentUpdate::Error {
                        session_id: String::new(),
                        message: e.to_string(),
                    },
                );
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn acp_cancel_prompt(
    agent_name: String,
    state: State<'_, AcpState>,
) -> Result<(), String> {
    let manager = state.manager.lock().await;
    manager
        .cancel_prompt(&agent_name)
        .await
        .map_err(|e| format!("Cancel failed: {}", e))
}

#[tauri::command]
pub async fn acp_stop_session(
    agent_name: String,
    state: State<'_, AcpState>,
) -> Result<(), String> {
    let manager = state.manager.lock().await;
    manager
        .stop_session(&agent_name)
        .await
        .map_err(|e| format!("Stop session failed: {}", e))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveAgentConfigInput {
    pub name: String,
    pub title: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[tauri::command]
pub async fn acp_save_agent_config(
    agents: Vec<SaveAgentConfigInput>,
    state: State<'_, AcpState>,
) -> Result<(), String> {
    let cfg = zoro_acp::AcpConfig {
        agents: agents
            .into_iter()
            .map(|a| zoro_acp::AgentConfig {
                name: a.name,
                title: a.title,
                description: String::new(),
                command: a.command,
                args: a.args,
                env: vec![],
                detect_command: None,
            })
            .collect(),
    };
    config::save_config(&state.data_dir, &cfg).map_err(|e| format!("Save config failed: {}", e))
}
