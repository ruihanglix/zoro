// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::AppState;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;
use zoro_plugins::{PluginInfo, PluginRegistry};

/// Managed state for the plugin system.
pub struct PluginState {
    pub registry: Mutex<PluginRegistry>,
}

#[tauri::command]
pub async fn list_plugins(state: State<'_, PluginState>) -> Result<Vec<PluginInfo>, String> {
    let registry = state
        .registry
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    Ok(registry.list_plugins())
}

#[tauri::command]
pub async fn install_plugin_from_file(
    state: State<'_, PluginState>,
    zcx_path: String,
) -> Result<PluginInfo, String> {
    let mut registry = state
        .registry
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    registry
        .install_from_zcx(&zcx_path)
        .map_err(|e| format!("Install failed: {}", e))
}

#[tauri::command]
pub async fn uninstall_plugin(
    state: State<'_, PluginState>,
    plugin_id: String,
) -> Result<(), String> {
    let mut registry = state
        .registry
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    registry
        .uninstall(&plugin_id)
        .map_err(|e| format!("Uninstall failed: {}", e))
}

#[tauri::command]
pub async fn toggle_plugin(
    state: State<'_, PluginState>,
    plugin_id: String,
    enabled: bool,
) -> Result<(), String> {
    let mut registry = state
        .registry
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    registry
        .toggle_plugin(&plugin_id, enabled)
        .map_err(|e| format!("Toggle failed: {}", e))
}

#[tauri::command]
pub async fn load_dev_plugin(
    state: State<'_, PluginState>,
    folder_path: String,
) -> Result<PluginInfo, String> {
    let mut registry = state
        .registry
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    registry
        .load_dev_plugin(&folder_path)
        .map_err(|e| format!("Load dev plugin failed: {}", e))
}

#[tauri::command]
pub async fn unload_dev_plugin(
    state: State<'_, PluginState>,
    plugin_id: String,
) -> Result<(), String> {
    let mut registry = state
        .registry
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    registry
        .unload_dev_plugin(&plugin_id)
        .map_err(|e| format!("Unload dev plugin failed: {}", e))
}

#[tauri::command]
pub async fn reload_dev_plugin(
    state: State<'_, PluginState>,
    plugin_id: String,
) -> Result<PluginInfo, String> {
    let registry = state
        .registry
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    registry
        .reload_dev_plugin(&plugin_id)
        .map_err(|e| format!("Reload failed: {}", e))
}

// === Plugin Storage (KV) ===

#[tauri::command]
pub async fn plugin_storage_get(
    state: State<'_, AppState>,
    plugin_id: String,
    key: String,
) -> Result<Option<String>, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    zoro_db::queries::plugin_storage::plugin_storage_get(&db.conn, &plugin_id, &key)
        .map_err(|e| format!("Storage get failed: {}", e))
}

#[tauri::command]
pub async fn plugin_storage_set(
    state: State<'_, AppState>,
    plugin_id: String,
    key: String,
    value: String,
) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    zoro_db::queries::plugin_storage::plugin_storage_set(&db.conn, &plugin_id, &key, &value)
        .map_err(|e| format!("Storage set failed: {}", e))
}

#[tauri::command]
pub async fn plugin_storage_delete(
    state: State<'_, AppState>,
    plugin_id: String,
    key: String,
) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    zoro_db::queries::plugin_storage::plugin_storage_delete(&db.conn, &plugin_id, &key)
        .map_err(|e| format!("Storage delete failed: {}", e))
}

// === Plugin AI — Black-box LLM interface for plugins ===

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginAiChatInput {
    pub messages: Vec<PluginChatMessage>,
    pub model: Option<String>,
    pub provider_id: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginModelInfo {
    pub id: String,
    pub name: String,
    pub models: Vec<String>,
}

/// Resolve the AI provider credentials from the app config.
/// Returns (base_url, api_key, default_model).
fn resolve_plugin_ai_provider(
    config: &zoro_core::models::AppConfig,
    provider_id: Option<&str>,
) -> Result<(String, String, String), String> {
    let ai = &config.ai;

    if (ai.api_key.is_empty() || ai.base_url.is_empty()) && ai.providers.is_empty() {
        return Err("AI not configured. Please set API key and base URL in Settings → AI.".into());
    }

    match provider_id {
        Some(pid) if pid != "default" => {
            let p = ai
                .providers
                .iter()
                .find(|p| p.id == *pid)
                .ok_or_else(|| format!("Unknown provider: {}", pid))?;
            Ok((
                p.base_url.clone(),
                if p.api_key.is_empty() {
                    ai.api_key.clone()
                } else {
                    p.api_key.clone()
                },
                p.models.first().cloned().unwrap_or_default(),
            ))
        }
        _ => Ok((ai.base_url.clone(), ai.api_key.clone(), ai.model.clone())),
    }
}

/// Non-streaming AI chat for plugins.
/// Plugins send messages and receive a complete response — no API keys exposed.
#[tauri::command]
pub async fn plugin_ai_chat(
    state: State<'_, AppState>,
    input: PluginAiChatInput,
) -> Result<String, String> {
    let config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?
        .clone();

    let (base_url, api_key, default_model) =
        resolve_plugin_ai_provider(&config, input.provider_id.as_deref())?;

    if base_url.is_empty() {
        return Err("Selected provider has no base URL configured.".into());
    }

    let model = input.model.unwrap_or(default_model);
    if model.is_empty() {
        return Err("No model configured. Please set a model in Settings → AI.".into());
    }

    // Resolve provider-specific base_url/api_key for special models (e.g. ACP Proxy)
    let resolved = config.ai.resolve_for_model(&model);
    let base_url = if resolved.base_url != config.ai.base_url && !resolved.base_url.is_empty() {
        resolved.base_url
    } else {
        base_url
    };
    let api_key = if resolved.api_key != config.ai.api_key && !resolved.api_key.is_empty() {
        resolved.api_key
    } else {
        api_key
    };

    let temperature = input.temperature.unwrap_or(0.7);

    // Build API messages
    let messages: Vec<zoro_ai::client::ChatMessage> = input
        .messages
        .iter()
        .map(|m| zoro_ai::client::ChatMessage {
            role: m.role.clone(),
            content: m.content.clone(),
        })
        .collect();

    // Use the non-streaming ChatClient with raw messages
    let client = zoro_ai::client::ChatClient::new(&base_url, &api_key, &model);

    // Extract system and user messages for the ChatClient API
    let system_prompt = messages
        .iter()
        .filter(|m| m.role == "system")
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");

    let user_messages: Vec<_> = messages.iter().filter(|m| m.role != "system").collect();

    // For the simple ChatClient, we need system + user prompt.
    // If there are multiple user/assistant messages, concatenate them as context.
    let user_prompt = user_messages
        .iter()
        .map(|m| format!("[{}]: {}", m.role, m.content))
        .collect::<Vec<_>>()
        .join("\n\n");

    let result = client
        .chat(&system_prompt, &user_prompt, temperature, input.max_tokens)
        .await
        .map_err(|e| format!("AI chat failed: {}", e))?;
    Ok(result)
}

/// Streaming AI chat for plugins.
/// Uses Tauri events to stream chunks back, keyed by a unique request_id.
#[tauri::command]
pub async fn plugin_ai_chat_stream(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    input: PluginAiChatInput,
    request_id: String,
) -> Result<(), String> {
    use tauri::Emitter;

    let config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?
        .clone();

    let (base_url, api_key, default_model) =
        resolve_plugin_ai_provider(&config, input.provider_id.as_deref())?;

    if base_url.is_empty() {
        return Err("Selected provider has no base URL configured.".into());
    }

    let model = input.model.unwrap_or(default_model);
    if model.is_empty() {
        return Err("No model configured. Please set a model in Settings → AI.".into());
    }

    // Resolve provider-specific base_url/api_key for special models (e.g. ACP Proxy)
    let resolved = config.ai.resolve_for_model(&model);
    let base_url = if resolved.base_url != config.ai.base_url && !resolved.base_url.is_empty() {
        resolved.base_url
    } else {
        base_url
    };
    let api_key = if resolved.api_key != config.ai.api_key && !resolved.api_key.is_empty() {
        resolved.api_key
    } else {
        api_key
    };

    // Build API messages as serde_json::Value for the streaming client
    let mut api_messages: Vec<serde_json::Value> = Vec::new();
    for msg in &input.messages {
        api_messages.push(serde_json::json!({
            "role": msg.role,
            "content": msg.content,
        }));
    }

    let event_name = format!("plugin-ai-stream-{}", request_id);
    let event_done = format!("plugin-ai-done-{}", request_id);

    let handle = app_handle.clone();
    let event_name_clone = event_name.clone();

    tokio::spawn(async move {
        let client = zoro_ai::streaming::StreamingClient::new(&base_url, &api_key, &model);

        let max_tokens = input.max_tokens;

        let result = client
            .chat_stream(
                &api_messages,
                None,
                |chunk| {
                    let _ = handle.emit(&event_name_clone, chunk);
                },
                max_tokens,
            )
            .await;

        match result {
            Ok(sr) => {
                let content = sr.content.unwrap_or_default();
                let _ = handle.emit(
                    &event_done,
                    serde_json::json!({
                        "ok": true,
                        "content": content,
                    }),
                );
            }
            Err(e) => {
                let _ = handle.emit(
                    &event_done,
                    serde_json::json!({
                        "ok": false,
                        "error": format!("{}", e),
                    }),
                );
            }
        }
    });

    Ok(())
}

/// Return available AI providers and their models to plugins.
/// No API keys or base URLs are exposed — just IDs and model names.
#[tauri::command]
pub async fn plugin_ai_get_models(
    state: State<'_, AppState>,
) -> Result<Vec<PluginModelInfo>, String> {
    let config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?;

    let ai = &config.ai;
    let mut providers = Vec::new();

    // Default provider
    if !ai.base_url.is_empty() {
        let mut models = vec![];
        if !ai.model.is_empty() {
            models.push(ai.model.clone());
        }
        providers.push(PluginModelInfo {
            id: "default".to_string(),
            name: if ai.provider.is_empty() {
                "Default".to_string()
            } else {
                ai.provider.clone()
            },
            models,
        });
    }

    // Additional providers
    for p in &ai.providers {
        providers.push(PluginModelInfo {
            id: p.id.clone(),
            name: p.name.clone(),
            models: p.models.clone(),
        });
    }

    Ok(providers)
}
