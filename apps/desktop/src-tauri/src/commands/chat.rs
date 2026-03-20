// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{Emitter, State};

use crate::AppState;

// ── Managed state ────────────────────────────────────────────────────────────

pub struct ChatStateInner {
    pub confirm_tx: tokio::sync::Mutex<Option<tokio::sync::oneshot::Sender<bool>>>,
    pub task_handle: tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

pub struct ChatState {
    pub inner: Arc<ChatStateInner>,
}

impl ChatState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ChatStateInner {
                confirm_tx: tokio::sync::Mutex::new(None),
                task_handle: tokio::sync::Mutex::new(None),
            }),
        }
    }
}

// ── Event types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum ChatUpdate {
    #[serde(rename = "text_chunk")]
    TextChunk { text: String },
    #[serde(rename = "tool_call")]
    ToolCall {
        tool_call_id: String,
        name: String,
        arguments: String,
        needs_confirmation: bool,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_call_id: String,
        name: String,
        result: String,
        is_error: bool,
    },
    #[serde(rename = "done")]
    Done { stop_reason: String },
    #[serde(rename = "error")]
    Error { message: String },
}

// ── Input types ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSendInput {
    pub messages: Vec<ChatHistoryMessage>,
    pub user_message: String,
    #[serde(default)]
    pub images: Vec<ImageInput>,
    pub system_prompt: String,
    pub paper_id: Option<String>,
    pub model: Option<String>,
    /// Select a configured provider by ID. When set, its base_url / api_key
    /// override the default AiConfig values for this message.
    pub provider_id: Option<String>,
    pub confirm_writes: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatHistoryMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInput {
    pub base64_data: String,
    pub mime_type: String,
}

// ── Config response ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetResponse {
    pub name: String,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub models: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatConfigResponse {
    pub active_preset: String,
    pub confirm_tool_calls: bool,
    pub ai_configured: bool,
    pub default_model: String,
    pub presets: Vec<PresetResponse>,
    pub providers: Vec<ProviderInfo>,
}

// ── Write tool names ─────────────────────────────────────────────────────────

const WRITE_TOOLS: &[&str] = &[
    "add_note",
    "add_tag_to_paper",
    "add_paper_to_collection",
    "update_paper_status",
];

fn is_write_tool(name: &str) -> bool {
    WRITE_TOOLS.contains(&name)
}

fn is_tool_choice_error(e: &zoro_ai::error::AiError) -> bool {
    match e {
        zoro_ai::error::AiError::Api { message, .. } => {
            message.contains("tool choice") || message.contains("tool_choice")
        }
        _ => false,
    }
}

// ── Commands ─────────────────────────────────────────────────────────────────

fn build_providers(config: &zoro_core::models::AiConfig) -> Vec<ProviderInfo> {
    let mut providers = Vec::new();
    if !config.base_url.is_empty() {
        let mut default_models = vec![];
        if !config.model.is_empty() {
            default_models.push(config.model.clone());
        }
        providers.push(ProviderInfo {
            id: "default".to_string(),
            name: if config.provider.is_empty() {
                "Default".to_string()
            } else {
                config.provider.clone()
            },
            models: default_models,
        });
    }
    for p in &config.providers {
        providers.push(ProviderInfo {
            id: p.id.clone(),
            name: p.name.clone(),
            models: p.models.clone(),
        });
    }
    providers
}

#[tauri::command]
pub async fn chat_get_config(state: State<'_, AppState>) -> Result<ChatConfigResponse, String> {
    let config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?;
    Ok(ChatConfigResponse {
        active_preset: config.chat.active_preset.clone(),
        confirm_tool_calls: config.chat.confirm_tool_calls,
        ai_configured: !config.ai.api_key.is_empty() && !config.ai.base_url.is_empty(),
        default_model: config.ai.model.clone(),
        presets: config
            .chat
            .presets
            .iter()
            .map(|p| PresetResponse {
                name: p.name.clone(),
                prompt: p.prompt.clone(),
            })
            .collect(),
        providers: build_providers(&config.ai),
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetInput {
    pub name: String,
    pub prompt: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateChatConfigInput {
    pub active_preset: Option<String>,
    pub confirm_tool_calls: Option<bool>,
    pub presets: Option<Vec<PresetInput>>,
}

#[tauri::command]
pub async fn chat_update_config(
    input: UpdateChatConfigInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let data_dir = state.data_dir.clone();
    let mut config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?;
    if let Some(active) = input.active_preset {
        config.chat.active_preset = active;
    }
    if let Some(confirm) = input.confirm_tool_calls {
        config.chat.confirm_tool_calls = confirm;
    }
    if let Some(presets) = input.presets {
        config.chat.presets = presets
            .into_iter()
            .map(|p| zoro_core::models::SystemPromptPreset {
                name: p.name,
                prompt: p.prompt,
            })
            .collect();
    }
    crate::storage::config::save_config(&data_dir, &config)
        .map_err(|e| format!("Save config failed: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn chat_send_message(
    input: ChatSendInput,
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    chat_state: State<'_, ChatState>,
) -> Result<(), String> {
    // Abort any existing streaming task
    {
        let mut handle = chat_state.inner.task_handle.lock().await;
        if let Some(h) = handle.take() {
            h.abort();
        }
    }

    let config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?
        .clone();

    if (config.ai.api_key.is_empty() || config.ai.base_url.is_empty())
        && config.ai.providers.is_empty()
    {
        return Err("AI not configured. Please set API key and base URL in Settings → AI.".into());
    }

    // Resolve provider: use explicit provider_id if given, else fall back to default
    let (base_url, api_key, default_model) = match &input.provider_id {
        Some(pid) if pid != "default" => {
            let p = config
                .ai
                .providers
                .iter()
                .find(|p| p.id == *pid)
                .ok_or_else(|| format!("Unknown provider: {}", pid))?;
            (
                p.base_url.clone(),
                if p.api_key.is_empty() {
                    config.ai.api_key.clone()
                } else {
                    p.api_key.clone()
                },
                p.models.first().cloned().unwrap_or_default(),
            )
        }
        _ => (
            config.ai.base_url.clone(),
            config.ai.api_key.clone(),
            config.ai.model.clone(),
        ),
    };

    if base_url.is_empty() {
        return Err("Selected provider has no base URL configured.".into());
    }

    let model = input.model.unwrap_or(default_model);
    if model.is_empty() {
        return Err("No model configured. Please set a model in Settings → AI.".into());
    }

    // Build system prompt with paper context
    let mut system_prompt = input.system_prompt;
    if let Some(ref paper_id) = input.paper_id {
        let db = state
            .db
            .lock()
            .map_err(|e| format!("DB lock error: {}", e))?;
        match build_paper_context(&db, paper_id, &state.data_dir) {
            Ok(ctx) => {
                system_prompt = format!("{}\n\n{}", system_prompt, ctx);
            }
            Err(e) => {
                tracing::warn!("Failed to build paper context: {}", e);
            }
        }
    }

    // Build API messages
    let mut api_messages: Vec<serde_json::Value> = Vec::new();
    api_messages.push(serde_json::json!({
        "role": "system",
        "content": system_prompt,
    }));

    // Add conversation history (merge consecutive assistant messages)
    let mut pending_assistant = String::new();
    for msg in &input.messages {
        if msg.role == "assistant" {
            if !pending_assistant.is_empty() {
                pending_assistant.push_str("\n\n");
            }
            pending_assistant.push_str(&msg.content);
        } else {
            if !pending_assistant.is_empty() {
                api_messages.push(serde_json::json!({
                    "role": "assistant",
                    "content": pending_assistant,
                }));
                pending_assistant.clear();
            }
            api_messages.push(serde_json::json!({
                "role": "user",
                "content": msg.content,
            }));
        }
    }
    if !pending_assistant.is_empty() {
        api_messages.push(serde_json::json!({
            "role": "assistant",
            "content": pending_assistant,
        }));
    }

    // Add new user message
    if input.images.is_empty() {
        api_messages.push(serde_json::json!({
            "role": "user",
            "content": input.user_message,
        }));
    } else {
        let mut parts = vec![serde_json::json!({"type": "text", "text": input.user_message})];
        for img in &input.images {
            parts.push(serde_json::json!({
                "type": "image_url",
                "image_url": {
                    "url": format!("data:{};base64,{}", img.mime_type, img.base64_data)
                }
            }));
        }
        api_messages.push(serde_json::json!({
            "role": "user",
            "content": parts,
        }));
    }

    let tools = tool_definitions();
    let db = Arc::clone(&state.db);
    let confirm_writes = input.confirm_writes;
    let inner = Arc::clone(&chat_state.inner);
    let handle = app_handle.clone();

    let task = tokio::spawn(async move {
        let client = zoro_ai::streaming::StreamingClient::new(&base_url, &api_key, &model);

        let result = run_chat_loop(
            &client,
            &mut api_messages,
            &tools,
            confirm_writes,
            &db,
            &handle,
            &inner,
        )
        .await;

        match result {
            Ok(reason) => {
                let _ = handle.emit(
                    "chat-update",
                    ChatUpdate::Done {
                        stop_reason: reason,
                    },
                );
            }
            Err(e) => {
                let msg = e.to_string();
                if !msg.contains("cancelled") {
                    let _ = handle.emit(
                        "chat-update",
                        ChatUpdate::Error {
                            message: e.to_string(),
                        },
                    );
                }
            }
        }
    });

    {
        let mut h = chat_state.inner.task_handle.lock().await;
        *h = Some(task);
    }

    Ok(())
}

#[tauri::command]
pub async fn chat_confirm_tool(
    approved: bool,
    chat_state: State<'_, ChatState>,
) -> Result<(), String> {
    let mut tx = chat_state.inner.confirm_tx.lock().await;
    if let Some(sender) = tx.take() {
        let _ = sender.send(approved);
        Ok(())
    } else {
        Err("No pending tool confirmation".into())
    }
}

#[tauri::command]
pub async fn chat_cancel(chat_state: State<'_, ChatState>) -> Result<(), String> {
    let mut handle = chat_state.inner.task_handle.lock().await;
    if let Some(h) = handle.take() {
        h.abort();
    }
    let mut tx = chat_state.inner.confirm_tx.lock().await;
    if let Some(sender) = tx.take() {
        let _ = sender.send(false);
    }
    Ok(())
}

// ── Chat loop ────────────────────────────────────────────────────────────────

async fn run_chat_loop(
    client: &zoro_ai::streaming::StreamingClient,
    api_messages: &mut Vec<serde_json::Value>,
    tools: &[serde_json::Value],
    confirm_writes: bool,
    db: &Arc<std::sync::Mutex<zoro_db::Database>>,
    handle: &tauri::AppHandle,
    chat_inner: &Arc<ChatStateInner>,
) -> Result<String, zoro_ai::error::AiError> {
    const MAX_TOOL_LOOPS: usize = 15;
    let mut tools_enabled = true;

    for _ in 0..MAX_TOOL_LOOPS {
        let active_tools = if tools_enabled { Some(tools) } else { None };
        let handle_clone = handle.clone();
        let result = match client
            .chat_stream(
                api_messages,
                active_tools,
                move |chunk| {
                    let _ = handle_clone.emit(
                        "chat-update",
                        ChatUpdate::TextChunk {
                            text: chunk.to_string(),
                        },
                    );
                },
                None,
            )
            .await
        {
            Ok(r) => r,
            Err(e) if tools_enabled && is_tool_choice_error(&e) => {
                tracing::warn!("Provider does not support tool calling, retrying without tools");
                tools_enabled = false;
                let handle_clone2 = handle.clone();
                client
                    .chat_stream(
                        api_messages,
                        None,
                        move |chunk| {
                            let _ = handle_clone2.emit(
                                "chat-update",
                                ChatUpdate::TextChunk {
                                    text: chunk.to_string(),
                                },
                            );
                        },
                        None,
                    )
                    .await?
            }
            Err(e) => return Err(e),
        };

        if result.tool_calls.is_empty() {
            // No tool calls — add assistant message and return
            if let Some(ref content) = result.content {
                api_messages.push(serde_json::json!({
                    "role": "assistant",
                    "content": content,
                }));
            }
            return Ok(result.finish_reason);
        }

        // Build assistant message with tool_calls
        let tool_calls_json: Vec<serde_json::Value> = result
            .tool_calls
            .iter()
            .map(|tc| {
                serde_json::json!({
                    "id": tc.id,
                    "type": "function",
                    "function": {
                        "name": tc.function_name,
                        "arguments": tc.arguments,
                    }
                })
            })
            .collect();

        let mut assistant_msg = serde_json::json!({
            "role": "assistant",
            "tool_calls": tool_calls_json,
        });
        if let Some(ref content) = result.content {
            assistant_msg["content"] = serde_json::Value::String(content.clone());
        }
        api_messages.push(assistant_msg);

        // Execute each tool call
        for tc in &result.tool_calls {
            let needs_confirmation = confirm_writes && is_write_tool(&tc.function_name);

            let _ = handle.emit(
                "chat-update",
                ChatUpdate::ToolCall {
                    tool_call_id: tc.id.clone(),
                    name: tc.function_name.clone(),
                    arguments: tc.arguments.clone(),
                    needs_confirmation,
                },
            );

            let (tool_result, is_error) = if needs_confirmation {
                // Wait for user confirmation
                let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
                {
                    let mut guard = chat_inner.confirm_tx.lock().await;
                    *guard = Some(tx);
                }

                match rx.await {
                    Ok(true) => execute_tool(db, &tc.function_name, &tc.arguments),
                    Ok(false) => ("Tool call was rejected by the user.".to_string(), false),
                    Err(_) => ("Tool confirmation cancelled.".to_string(), true),
                }
            } else {
                execute_tool(db, &tc.function_name, &tc.arguments)
            };

            let _ = handle.emit(
                "chat-update",
                ChatUpdate::ToolResult {
                    tool_call_id: tc.id.clone(),
                    name: tc.function_name.clone(),
                    result: tool_result.clone(),
                    is_error,
                },
            );

            api_messages.push(serde_json::json!({
                "role": "tool",
                "tool_call_id": tc.id,
                "content": tool_result,
            }));
        }
    }

    Ok("max_tool_loops".to_string())
}

// ── Tool definitions ─────────────────────────────────────────────────────────

fn tool_definitions() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "search_papers",
                "description": "Search the user's paper library using full-text search.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search query" },
                        "limit": { "type": "integer", "description": "Max results to return (default 10)" }
                    },
                    "required": ["query"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "get_paper",
                "description": "Get full details of a specific paper by its ID.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "paper_id": { "type": "string", "description": "The paper ID" }
                    },
                    "required": ["paper_id"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "list_papers",
                "description": "List papers in the library, optionally filtered by collection, tag, or read status.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "collection_id": { "type": "string", "description": "Filter by collection ID" },
                        "tag_name": { "type": "string", "description": "Filter by tag name" },
                        "read_status": { "type": "string", "description": "Filter by status: unread, reading, read, skipped" },
                        "limit": { "type": "integer", "description": "Max results (default 20)" }
                    }
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "list_collections",
                "description": "List all paper collections (folders) in the library.",
                "parameters": { "type": "object", "properties": {} }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "list_tags",
                "description": "List all tags used in the library.",
                "parameters": { "type": "object", "properties": {} }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "list_notes",
                "description": "List all notes for a specific paper.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "paper_id": { "type": "string", "description": "The paper ID" }
                    },
                    "required": ["paper_id"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "list_annotations",
                "description": "List all annotations (highlights, underlines, notes) for a paper.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "paper_id": { "type": "string", "description": "The paper ID" }
                    },
                    "required": ["paper_id"]
                }
            }
        }),
        // Write tools
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "add_note",
                "description": "Add a note to a paper.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "paper_id": { "type": "string", "description": "The paper ID" },
                        "content": { "type": "string", "description": "Note content (markdown supported)" }
                    },
                    "required": ["paper_id", "content"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "add_tag_to_paper",
                "description": "Add a tag to a paper. Creates the tag if it doesn't exist.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "paper_id": { "type": "string", "description": "The paper ID" },
                        "tag_name": { "type": "string", "description": "Tag name" }
                    },
                    "required": ["paper_id", "tag_name"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "add_paper_to_collection",
                "description": "Add a paper to a collection.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "paper_id": { "type": "string", "description": "The paper ID" },
                        "collection_id": { "type": "string", "description": "Collection ID" }
                    },
                    "required": ["paper_id", "collection_id"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "update_paper_status",
                "description": "Update the read status of a paper.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "paper_id": { "type": "string", "description": "The paper ID" },
                        "read_status": {
                            "type": "string",
                            "enum": ["unread", "reading", "read", "skipped"],
                            "description": "New read status"
                        }
                    },
                    "required": ["paper_id", "read_status"]
                }
            }
        }),
    ]
}

// ── Tool execution ───────────────────────────────────────────────────────────

fn execute_tool(
    db: &Arc<std::sync::Mutex<zoro_db::Database>>,
    name: &str,
    arguments: &str,
) -> (String, bool) {
    let args: serde_json::Value = match serde_json::from_str(arguments) {
        Ok(v) => v,
        Err(e) => return (format!("Invalid arguments: {}", e), true),
    };

    let conn = match db.lock() {
        Ok(db) => db,
        Err(e) => return (format!("DB lock error: {}", e), true),
    };

    match name {
        "search_papers" => {
            let query = match args["query"].as_str() {
                Some(q) => q,
                None => return ("Missing required parameter: query".into(), true),
            };
            let limit = args["limit"].as_i64().unwrap_or(10);
            match zoro_db::queries::search::search_papers(&conn.conn, query, limit) {
                Ok(papers) => {
                    let result: Vec<serde_json::Value> =
                        papers.iter().map(paper_row_to_brief_json).collect();
                    (
                        serde_json::to_string_pretty(&result).unwrap_or_default(),
                        false,
                    )
                }
                Err(e) => (format!("Search failed: {}", e), true),
            }
        }

        "get_paper" => {
            let paper_id = match args["paper_id"].as_str() {
                Some(id) => id,
                None => return ("Missing required parameter: paper_id".into(), true),
            };
            match zoro_db::queries::papers::get_paper(&conn.conn, paper_id) {
                Ok(paper) => {
                    let authors = zoro_db::queries::papers::get_paper_authors(&conn.conn, paper_id)
                        .unwrap_or_default();
                    let tags = zoro_db::queries::tags::get_paper_tags(&conn.conn, paper_id)
                        .unwrap_or_default();
                    let mut json = paper_row_to_json(&paper);
                    json["authors"] = serde_json::json!(authors
                        .iter()
                        .map(|(name, aff, _)| {
                            serde_json::json!({"name": name, "affiliation": aff})
                        })
                        .collect::<Vec<_>>());
                    json["tags"] =
                        serde_json::json!(tags.iter().map(|t| &t.name).collect::<Vec<_>>());
                    (
                        serde_json::to_string_pretty(&json).unwrap_or_default(),
                        false,
                    )
                }
                Err(e) => (format!("Paper not found: {}", e), true),
            }
        }

        "list_papers" => {
            let filter = zoro_db::queries::papers::PaperFilter {
                collection_id: args["collection_id"].as_str().map(|s| s.to_string()),
                tag_name: args["tag_name"].as_str().map(|s| s.to_string()),
                read_status: args["read_status"].as_str().map(|s| s.to_string()),
                search_query: None,
                uncategorized: None,
                sort_by: Some("added_date".to_string()),
                sort_order: Some("desc".to_string()),
                limit: Some(args["limit"].as_i64().unwrap_or(20)),
                offset: None,
            };
            match zoro_db::queries::papers::list_papers(&conn.conn, &filter) {
                Ok(papers) => {
                    let result: Vec<serde_json::Value> =
                        papers.iter().map(paper_row_to_brief_json).collect();
                    (
                        serde_json::to_string_pretty(&result).unwrap_or_default(),
                        false,
                    )
                }
                Err(e) => (format!("List papers failed: {}", e), true),
            }
        }

        "list_collections" => match zoro_db::queries::collections::list_collections(&conn.conn) {
            Ok(cols) => {
                let result: Vec<serde_json::Value> = cols
                    .iter()
                    .map(|c| {
                        serde_json::json!({
                            "id": c.id,
                            "name": c.name,
                            "parent_id": c.parent_id,
                            "description": c.description,
                        })
                    })
                    .collect();
                (
                    serde_json::to_string_pretty(&result).unwrap_or_default(),
                    false,
                )
            }
            Err(e) => (format!("List collections failed: {}", e), true),
        },

        "list_tags" => match zoro_db::queries::tags::list_tags(&conn.conn) {
            Ok(tags) => {
                let result: Vec<serde_json::Value> = tags
                    .iter()
                    .map(|t| serde_json::json!({"id": t.id, "name": t.name, "color": t.color}))
                    .collect();
                (
                    serde_json::to_string_pretty(&result).unwrap_or_default(),
                    false,
                )
            }
            Err(e) => (format!("List tags failed: {}", e), true),
        },

        "list_notes" => {
            let paper_id = match args["paper_id"].as_str() {
                Some(id) => id,
                None => return ("Missing required parameter: paper_id".into(), true),
            };
            match zoro_db::queries::notes::list_notes(&conn.conn, paper_id) {
                Ok(notes) => {
                    let result: Vec<serde_json::Value> = notes
                        .iter()
                        .map(|n| {
                            serde_json::json!({
                                "id": n.id,
                                "content": n.content,
                                "created_date": n.created_date,
                            })
                        })
                        .collect();
                    (
                        serde_json::to_string_pretty(&result).unwrap_or_default(),
                        false,
                    )
                }
                Err(e) => (format!("List notes failed: {}", e), true),
            }
        }

        "list_annotations" => {
            let paper_id = match args["paper_id"].as_str() {
                Some(id) => id,
                None => return ("Missing required parameter: paper_id".into(), true),
            };
            match zoro_db::queries::annotations::list_annotations(&conn.conn, paper_id, None) {
                Ok(anns) => {
                    let result: Vec<serde_json::Value> = anns
                        .iter()
                        .map(|a| {
                            serde_json::json!({
                                "id": a.id,
                                "type": a.annotation_type,
                                "color": a.color,
                                "selected_text": a.selected_text,
                                "comment": a.comment,
                                "page_number": a.page_number,
                            })
                        })
                        .collect();
                    (
                        serde_json::to_string_pretty(&result).unwrap_or_default(),
                        false,
                    )
                }
                Err(e) => (format!("List annotations failed: {}", e), true),
            }
        }

        // Write tools
        "add_note" => {
            let paper_id = match args["paper_id"].as_str() {
                Some(id) => id,
                None => return ("Missing required parameter: paper_id".into(), true),
            };
            let content = match args["content"].as_str() {
                Some(c) => c,
                None => return ("Missing required parameter: content".into(), true),
            };
            match zoro_db::queries::notes::insert_note(&conn.conn, paper_id, content) {
                Ok(note) => (format!("Note added successfully (ID: {})", note.id), false),
                Err(e) => (format!("Add note failed: {}", e), true),
            }
        }

        "add_tag_to_paper" => {
            let paper_id = match args["paper_id"].as_str() {
                Some(id) => id,
                None => return ("Missing required parameter: paper_id".into(), true),
            };
            let tag_name = match args["tag_name"].as_str() {
                Some(n) => n,
                None => return ("Missing required parameter: tag_name".into(), true),
            };
            match zoro_db::queries::tags::add_tag_to_paper(&conn.conn, paper_id, tag_name, "chat") {
                Ok(()) => (format!("Tag '{}' added to paper", tag_name), false),
                Err(e) => (format!("Add tag failed: {}", e), true),
            }
        }

        "add_paper_to_collection" => {
            let paper_id = match args["paper_id"].as_str() {
                Some(id) => id,
                None => return ("Missing required parameter: paper_id".into(), true),
            };
            let collection_id = match args["collection_id"].as_str() {
                Some(id) => id,
                None => return ("Missing required parameter: collection_id".into(), true),
            };
            match zoro_db::queries::collections::add_paper_to_collection(
                &conn.conn,
                paper_id,
                collection_id,
            ) {
                Ok(()) => ("Paper added to collection".to_string(), false),
                Err(e) => (format!("Add to collection failed: {}", e), true),
            }
        }

        "update_paper_status" => {
            let paper_id = match args["paper_id"].as_str() {
                Some(id) => id,
                None => return ("Missing required parameter: paper_id".into(), true),
            };
            let read_status = match args["read_status"].as_str() {
                Some(s) => s,
                None => return ("Missing required parameter: read_status".into(), true),
            };
            match zoro_db::queries::papers::update_paper_status(&conn.conn, paper_id, read_status) {
                Ok(()) => (format!("Paper status updated to '{}'", read_status), false),
                Err(e) => (format!("Update status failed: {}", e), true),
            }
        }

        _ => (format!("Unknown tool: {}", name), true),
    }
}

// ── Paper context ────────────────────────────────────────────────────────────

fn build_paper_context(
    db: &zoro_db::Database,
    paper_id: &str,
    data_dir: &std::path::Path,
) -> Result<String, String> {
    let paper =
        zoro_db::queries::papers::get_paper(&db.conn, paper_id).map_err(|e| format!("{}", e))?;

    let authors =
        zoro_db::queries::papers::get_paper_authors(&db.conn, paper_id).unwrap_or_default();

    let tags = zoro_db::queries::tags::get_paper_tags(&db.conn, paper_id).unwrap_or_default();

    let notes = zoro_db::queries::notes::list_notes(&db.conn, paper_id).unwrap_or_default();

    let annotations = zoro_db::queries::annotations::list_annotations(&db.conn, paper_id, None)
        .unwrap_or_default();

    let mut ctx = String::from("## Current Paper Context\n\n");

    ctx.push_str(&format!("**Title:** {}\n", paper.title));

    if !authors.is_empty() {
        let author_names: Vec<&str> = authors.iter().map(|(name, _, _)| name.as_str()).collect();
        ctx.push_str(&format!("**Authors:** {}\n", author_names.join(", ")));
    }

    if let Some(ref date) = paper.published_date {
        ctx.push_str(&format!("**Published:** {}\n", date));
    }
    if let Some(ref doi) = paper.doi {
        ctx.push_str(&format!("**DOI:** {}\n", doi));
    }
    if let Some(ref arxiv_id) = paper.arxiv_id {
        ctx.push_str(&format!("**arXiv ID:** {}\n", arxiv_id));
    }
    ctx.push_str(&format!("**Read Status:** {}\n", paper.read_status));
    ctx.push_str(&format!("**Paper ID:** {}\n", paper.id));

    if !tags.is_empty() {
        let tag_names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
        ctx.push_str(&format!("**Tags:** {}\n", tag_names.join(", ")));
    }

    if let Some(ref abstract_text) = paper.abstract_text {
        ctx.push_str(&format!("\n### Abstract\n{}\n", abstract_text));
    }

    // Try to read paper HTML content and include as full text context.
    // This allows the AI to answer questions about the paper's actual content,
    // not just the title/abstract.
    let paper_dir = data_dir.join("library").join(&paper.dir_path);
    let html_path = paper_dir.join("paper.html");
    if html_path.exists() {
        if let Ok(html_content) = std::fs::read_to_string(&html_path) {
            let plain_text = strip_html_tags(&html_content);
            if !plain_text.is_empty() {
                // Cap at ~60k chars to avoid exceeding context window limits
                const MAX_CONTENT_CHARS: usize = 60_000;
                let truncated = if plain_text.len() > MAX_CONTENT_CHARS {
                    format!(
                        "{}\n\n[Content truncated — showing first ~60k characters]",
                        &plain_text[..MAX_CONTENT_CHARS]
                    )
                } else {
                    plain_text
                };
                ctx.push_str(&format!("\n### Full Text\n{}\n", truncated));
            }
        }
    }

    if !notes.is_empty() {
        ctx.push_str("\n### Notes\n");
        for note in &notes {
            ctx.push_str(&format!("- {}\n", note.content.replace('\n', " ")));
        }
    }

    if !annotations.is_empty() {
        ctx.push_str("\n### Annotations\n");
        for ann in &annotations {
            let mut parts = Vec::new();
            parts.push(format!(
                "[Page {}, {}]",
                ann.page_number, ann.annotation_type
            ));
            if let Some(ref text) = ann.selected_text {
                parts.push(format!("\"{}\"", text));
            }
            if let Some(ref comment) = ann.comment {
                if !comment.is_empty() {
                    parts.push(format!("Comment: {}", comment));
                }
            }
            ctx.push_str(&format!("- {}\n", parts.join(" — ")));
        }
    }

    Ok(ctx)
}

/// Strip HTML tags and return plain text content.
/// Handles common elements: collapses whitespace, converts block elements to
/// newlines, and removes <script>/<style> blocks entirely.
fn strip_html_tags(html: &str) -> String {
    // Remove <script> and <style> blocks (case-insensitive)
    let mut result = html.to_string();
    // Simple iterative removal of script/style blocks
    for tag in &["script", "style"] {
        loop {
            let open = format!("<{}", tag);
            let close = format!("</{}>", tag);
            if let Some(start) = result.to_lowercase().find(&open) {
                if let Some(end) = result.to_lowercase()[start..].find(&close) {
                    let end_abs = start + end + close.len();
                    result.replace_range(start..end_abs, " ");
                    continue;
                }
            }
            break;
        }
    }

    // Replace block-level tags with newlines
    let block_tags = [
        "<br", "<p ", "<p>", "</p>", "<div", "</div>", "<h1", "<h2", "<h3", "<h4", "<h5", "<h6",
        "</h1>", "</h2>", "</h3>", "</h4>", "</h5>", "</h6>", "<li", "</li>", "<tr", "</tr>",
    ];
    let lower = result.to_lowercase();
    let mut output = String::with_capacity(result.len());
    let chars: Vec<char> = result.chars().collect();
    let lower_chars: Vec<char> = lower.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '<' {
            // Check if this is a block-level tag
            let remaining: String = lower_chars[i..].iter().take(10).collect();
            let is_block = block_tags.iter().any(|t| remaining.starts_with(t));
            // Skip to end of tag
            if let Some(end) = chars[i..].iter().position(|&c| c == '>') {
                if is_block {
                    output.push('\n');
                }
                i += end + 1;
            } else {
                i += 1;
            }
        } else {
            output.push(chars[i]);
            i += 1;
        }
    }

    // Decode common HTML entities
    let output = output
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");

    // Collapse multiple whitespace/newlines
    let mut collapsed = String::with_capacity(output.len());
    let mut prev_newline = false;
    let mut prev_space = false;
    for ch in output.chars() {
        if ch == '\n' {
            if !prev_newline {
                collapsed.push('\n');
            }
            prev_newline = true;
            prev_space = false;
        } else if ch.is_whitespace() {
            if !prev_space && !prev_newline {
                collapsed.push(' ');
            }
            prev_space = true;
        } else {
            collapsed.push(ch);
            prev_newline = false;
            prev_space = false;
        }
    }

    collapsed.trim().to_string()
}
// ── JSON helpers ─────────────────────────────────────────────────────────────

fn paper_row_to_brief_json(p: &zoro_db::queries::papers::PaperRow) -> serde_json::Value {
    serde_json::json!({
        "id": p.id,
        "title": p.title,
        "abstract": p.abstract_text,
        "published_date": p.published_date,
        "read_status": p.read_status,
        "doi": p.doi,
        "arxiv_id": p.arxiv_id,
    })
}

fn paper_row_to_json(p: &zoro_db::queries::papers::PaperRow) -> serde_json::Value {
    serde_json::json!({
        "id": p.id,
        "title": p.title,
        "short_title": p.short_title,
        "abstract": p.abstract_text,
        "doi": p.doi,
        "arxiv_id": p.arxiv_id,
        "url": p.url,
        "pdf_url": p.pdf_url,
        "html_url": p.html_url,
        "published_date": p.published_date,
        "added_date": p.added_date,
        "source": p.source,
        "read_status": p.read_status,
        "rating": p.rating,
        "entry_type": p.entry_type,
        "journal": p.journal,
        "volume": p.volume,
        "issue": p.issue,
        "pages": p.pages,
        "publisher": p.publisher,
    })
}
