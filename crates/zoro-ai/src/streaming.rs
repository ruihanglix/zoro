// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::AiError;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::pin;
use zoro_core::models::ApiFormat;

/// Result of a streaming chat completion.
#[derive(Debug)]
pub struct StreamResult {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCallResult>,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub id: String,
    pub function_name: String,
    pub arguments: String,
}

#[derive(Debug, Deserialize)]
struct ChunkResponse {
    choices: Vec<ChunkChoice>,
}

#[derive(Debug, Deserialize)]
struct ChunkChoice {
    delta: ChunkDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChunkDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ChunkToolCall>>,
}

#[derive(Debug, Deserialize)]
struct ChunkToolCall {
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<ChunkFunction>,
}

#[derive(Debug, Deserialize)]
struct ChunkFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

struct ToolCallAccumulator {
    id: String,
    name: String,
    arguments: String,
}

pub struct StreamingClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
    format: ApiFormat,
    custom_headers: HashMap<String, String>,
}

impl StreamingClient {
    pub fn new(
        client: reqwest::Client,
        base_url: &str,
        api_key: &str,
        model: &str,
        format: ApiFormat,
        custom_headers: HashMap<String, String>,
    ) -> Self {
        Self {
            http: client,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
            format,
            custom_headers,
        }
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// Stream a chat completion. Calls `on_chunk` for each text delta.
    /// Returns the accumulated content and any tool calls.
    pub async fn chat_stream(
        &self,
        messages: &[serde_json::Value],
        tools: Option<&[serde_json::Value]>,
        on_chunk: impl Fn(&str),
        max_tokens: Option<u32>,
    ) -> Result<StreamResult, AiError> {
        match self.format {
            ApiFormat::Anthropic => {
                self.chat_stream_anthropic(messages, tools, on_chunk, max_tokens)
                    .await
            }
            _ => {
                self.chat_stream_openai(messages, tools, on_chunk, max_tokens)
                    .await
            }
        }
    }

    async fn chat_stream_openai(
        &self,
        messages: &[serde_json::Value],
        tools: Option<&[serde_json::Value]>,
        on_chunk: impl Fn(&str),
        max_tokens: Option<u32>,
    ) -> Result<StreamResult, AiError> {
        let url = format!("{}/chat/completions", self.base_url);

        let mut body = serde_json::json!({
            "model": &self.model,
            "messages": messages,
            "stream": true,
        });

        if let Some(tools) = tools {
            if !tools.is_empty() {
                body["tools"] = serde_json::Value::Array(tools.to_vec());
            }
        }

        if let Some(max) = max_tokens {
            body["max_tokens"] = serde_json::json!(max);
        }

        tracing::debug!(
            url = %url,
            model = %self.model,
            msg_count = messages.len(),
            "chat_stream request"
        );

        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json");
        let resp = self
            .custom_headers
            .iter()
            .fold(resp, |r, (k, v)| r.header(k.as_str(), v.as_str()));
        let resp = resp
            .json(&body)
            .timeout(std::time::Duration::from_secs(300))
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body_text = resp.text().await.unwrap_or_default();
            return Err(AiError::Api {
                status,
                message: body_text,
            });
        }

        let mut stream = pin!(resp.bytes_stream());
        let mut buffer = String::new();
        let mut accumulated_text = String::new();
        let mut tool_accumulators: Vec<ToolCallAccumulator> = Vec::new();
        let mut finish_reason: Option<String> = None;

        while let Some(chunk_result) = stream.next().await {
            let bytes = chunk_result.map_err(AiError::Http)?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim_end().to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                if line.is_empty() || line.starts_with(':') {
                    continue;
                }

                let data = match line.strip_prefix("data: ") {
                    Some(d) => d.trim(),
                    None => continue,
                };

                if data == "[DONE]" {
                    return Ok(build_result(
                        accumulated_text,
                        tool_accumulators,
                        finish_reason,
                    ));
                }

                // Check for error objects in the stream (some providers send errors as SSE data)
                if let Ok(obj) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(err) = obj.get("error") {
                        let msg = err
                            .get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("Unknown streaming error");
                        return Err(AiError::Api {
                            status: 400,
                            message: msg.to_string(),
                        });
                    }
                }

                let chunk: ChunkResponse = match serde_json::from_str(data) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!(error = %e, data = %data, "Failed to parse SSE chunk");
                        continue;
                    }
                };

                if let Some(choice) = chunk.choices.first() {
                    if let Some(ref content) = choice.delta.content {
                        accumulated_text.push_str(content);
                        on_chunk(content);
                    }

                    if let Some(ref delta_tool_calls) = choice.delta.tool_calls {
                        for dtc in delta_tool_calls {
                            while tool_accumulators.len() <= dtc.index {
                                tool_accumulators.push(ToolCallAccumulator {
                                    id: String::new(),
                                    name: String::new(),
                                    arguments: String::new(),
                                });
                            }
                            let acc = &mut tool_accumulators[dtc.index];
                            if let Some(ref id) = dtc.id {
                                acc.id.clone_from(id);
                            }
                            if let Some(ref func) = dtc.function {
                                if let Some(ref name) = func.name {
                                    acc.name.clone_from(name);
                                }
                                if let Some(ref args) = func.arguments {
                                    acc.arguments.push_str(args);
                                }
                            }
                        }
                    }

                    if let Some(ref reason) = choice.finish_reason {
                        finish_reason = Some(reason.clone());
                    }
                }
            }
        }

        Ok(build_result(
            accumulated_text,
            tool_accumulators,
            finish_reason,
        ))
    }

    async fn chat_stream_anthropic(
        &self,
        messages: &[serde_json::Value],
        tools: Option<&[serde_json::Value]>,
        on_chunk: impl Fn(&str),
        max_tokens: Option<u32>,
    ) -> Result<StreamResult, AiError> {
        let url = format!("{}/messages", self.base_url);

        // Separate system messages from user/assistant messages
        let mut system_parts: Vec<String> = Vec::new();
        let mut anthropic_messages: Vec<serde_json::Value> = Vec::new();

        for msg in messages {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            if role == "system" {
                if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                    system_parts.push(content.to_string());
                }
            } else if role == "tool" {
                // Convert OpenAI tool result to Anthropic tool_result content block
                let tool_call_id = msg
                    .get("tool_call_id")
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                let content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");
                anthropic_messages.push(serde_json::json!({
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": tool_call_id,
                        "content": content,
                    }],
                }));
            } else if role == "assistant" {
                // Check if this assistant message has tool_calls (OpenAI format)
                if let Some(tool_calls) = msg.get("tool_calls").and_then(|tc| tc.as_array()) {
                    let mut content_blocks: Vec<serde_json::Value> = Vec::new();
                    // Include text if present
                    if let Some(text) = msg.get("content").and_then(|c| c.as_str()) {
                        if !text.is_empty() {
                            content_blocks.push(serde_json::json!({
                                "type": "text",
                                "text": text,
                            }));
                        }
                    }
                    // Convert tool_calls to tool_use blocks
                    for tc in tool_calls {
                        let func = tc.get("function").unwrap_or(tc);
                        content_blocks.push(serde_json::json!({
                            "type": "tool_use",
                            "id": tc.get("id").and_then(|i| i.as_str()).unwrap_or(""),
                            "name": func.get("name").and_then(|n| n.as_str()).unwrap_or(""),
                            "input": serde_json::from_str::<serde_json::Value>(
                                func.get("arguments").and_then(|a| a.as_str()).unwrap_or("{}")
                            ).unwrap_or(serde_json::json!({})),
                        }));
                    }
                    anthropic_messages.push(serde_json::json!({
                        "role": "assistant",
                        "content": content_blocks,
                    }));
                } else {
                    // Plain text assistant message — handle content as array or string
                    let content = msg.get("content").cloned().unwrap_or(serde_json::json!(""));
                    anthropic_messages.push(serde_json::json!({
                        "role": "assistant",
                        "content": content,
                    }));
                }
            } else {
                // user messages — convert image_url format if needed
                let content = convert_content_for_anthropic(msg.get("content"));
                anthropic_messages.push(serde_json::json!({
                    "role": "user",
                    "content": content,
                }));
            }
        }

        let mut body = serde_json::json!({
            "model": &self.model,
            "messages": anthropic_messages,
            "max_tokens": max_tokens.unwrap_or(4096),
            "stream": true,
        });

        if !system_parts.is_empty() {
            body["system"] = serde_json::json!(system_parts.join("\n"));
        }

        // Convert OpenAI tool definitions to Anthropic format
        if let Some(tools) = tools {
            if !tools.is_empty() {
                let anthropic_tools: Vec<serde_json::Value> = tools
                    .iter()
                    .filter_map(|tool| {
                        let func = tool.get("function")?;
                        Some(serde_json::json!({
                            "name": func.get("name")?,
                            "description": func.get("description").unwrap_or(&serde_json::Value::Null),
                            "input_schema": func.get("parameters").unwrap_or(&serde_json::json!({"type": "object", "properties": {}})),
                        }))
                    })
                    .collect();
                if !anthropic_tools.is_empty() {
                    body["tools"] = serde_json::json!(anthropic_tools);
                }
            }
        }

        tracing::debug!(
            url = %url,
            model = %self.model,
            msg_count = anthropic_messages.len(),
            "chat_stream_anthropic request"
        );

        let resp = self
            .http
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json");
        let resp = self
            .custom_headers
            .iter()
            .fold(resp, |r, (k, v)| r.header(k.as_str(), v.as_str()));
        let resp = resp
            .json(&body)
            .timeout(std::time::Duration::from_secs(300))
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body_text = resp.text().await.unwrap_or_default();
            return Err(AiError::Api {
                status,
                message: body_text,
            });
        }

        // Parse Anthropic SSE stream
        let mut stream = pin!(resp.bytes_stream());
        let mut buffer = String::new();
        let mut accumulated_text = String::new();
        let mut tool_accumulators: Vec<ToolCallAccumulator> = Vec::new();
        let mut finish_reason: Option<String> = None;
        let mut current_event = String::new();
        // Track which tool_use block we're currently accumulating
        let mut current_tool_index: Option<usize> = None;

        while let Some(chunk_result) = stream.next().await {
            let bytes = chunk_result.map_err(AiError::Http)?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim_end().to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                // Track event type
                if let Some(event_type) = line.strip_prefix("event: ") {
                    current_event = event_type.trim().to_string();
                    continue;
                }

                // Skip non-data lines
                let data = match line.strip_prefix("data: ") {
                    Some(d) => d.trim(),
                    None => continue,
                };

                if data.is_empty() {
                    continue;
                }

                let parsed: serde_json::Value = match serde_json::from_str(data) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                match current_event.as_str() {
                    "content_block_start" => {
                        // Check if this is a tool_use block
                        if let Some(content_block) = parsed.get("content_block") {
                            if content_block.get("type").and_then(|t| t.as_str())
                                == Some("tool_use")
                            {
                                let id = content_block
                                    .get("id")
                                    .and_then(|i| i.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let name = content_block
                                    .get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                tool_accumulators.push(ToolCallAccumulator {
                                    id,
                                    name,
                                    arguments: String::new(),
                                });
                                current_tool_index = Some(tool_accumulators.len() - 1);
                            } else {
                                current_tool_index = None;
                            }
                        }
                    }
                    "content_block_delta" => {
                        if let Some(delta) = parsed.get("delta") {
                            let delta_type =
                                delta.get("type").and_then(|t| t.as_str()).unwrap_or("");
                            match delta_type {
                                "text_delta" => {
                                    if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                        accumulated_text.push_str(text);
                                        on_chunk(text);
                                    }
                                }
                                "input_json_delta" => {
                                    if let Some(idx) = current_tool_index {
                                        if let Some(partial) =
                                            delta.get("partial_json").and_then(|p| p.as_str())
                                        {
                                            tool_accumulators[idx].arguments.push_str(partial);
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    "content_block_stop" => {
                        current_tool_index = None;
                    }
                    "message_delta" => {
                        if let Some(delta) = parsed.get("delta") {
                            if let Some(stop_reason) =
                                delta.get("stop_reason").and_then(|r| r.as_str())
                            {
                                finish_reason = Some(match stop_reason {
                                    "end_turn" => "stop".to_string(),
                                    "tool_use" => "tool_calls".to_string(),
                                    "max_tokens" => "length".to_string(),
                                    other => other.to_string(),
                                });
                            }
                        }
                    }
                    "message_stop" => {
                        return Ok(build_result(
                            accumulated_text,
                            tool_accumulators,
                            finish_reason,
                        ));
                    }
                    "error" => {
                        let msg = parsed
                            .get("error")
                            .and_then(|e| e.get("message"))
                            .and_then(|m| m.as_str())
                            .unwrap_or("Unknown Anthropic streaming error");
                        return Err(AiError::Api {
                            status: 400,
                            message: msg.to_string(),
                        });
                    }
                    _ => {}
                }
            }
        }

        Ok(build_result(
            accumulated_text,
            tool_accumulators,
            finish_reason,
        ))
    }
}

fn build_result(
    text: String,
    accumulators: Vec<ToolCallAccumulator>,
    finish_reason: Option<String>,
) -> StreamResult {
    StreamResult {
        content: if text.is_empty() { None } else { Some(text) },
        tool_calls: accumulators
            .into_iter()
            .map(|tc| ToolCallResult {
                id: tc.id,
                function_name: tc.name,
                arguments: tc.arguments,
            })
            .collect(),
        finish_reason: finish_reason.unwrap_or_else(|| "stop".to_string()),
    }
}

/// Convert OpenAI content (string or array with image_url) to Anthropic format.
fn convert_content_for_anthropic(content: Option<&serde_json::Value>) -> serde_json::Value {
    let Some(content) = content else {
        return serde_json::json!("");
    };

    // If it's a plain string, return as-is
    if content.is_string() {
        return content.clone();
    }

    // If it's an array (multimodal), convert image_url blocks to Anthropic image blocks
    if let Some(parts) = content.as_array() {
        let converted: Vec<serde_json::Value> = parts
            .iter()
            .filter_map(|part| {
                let part_type = part.get("type").and_then(|t| t.as_str())?;
                match part_type {
                    "text" => Some(part.clone()),
                    "image_url" => {
                        let url = part
                            .get("image_url")
                            .and_then(|iu| iu.get("url"))
                            .and_then(|u| u.as_str())?;
                        // Parse data:image/TYPE;base64,DATA
                        if let Some(rest) = url.strip_prefix("data:") {
                            let parts: Vec<&str> = rest.splitn(2, ',').collect();
                            if parts.len() == 2 {
                                let media_info = parts[0]; // e.g. "image/png;base64"
                                let data = parts[1];
                                let media_type =
                                    media_info.split(';').next().unwrap_or("image/png");
                                return Some(serde_json::json!({
                                    "type": "image",
                                    "source": {
                                        "type": "base64",
                                        "media_type": media_type,
                                        "data": data,
                                    }
                                }));
                            }
                        }
                        // Non-data URL — Anthropic supports URL source too
                        Some(serde_json::json!({
                            "type": "image",
                            "source": {
                                "type": "url",
                                "url": url,
                            }
                        }))
                    }
                    _ => Some(part.clone()),
                }
            })
            .collect();
        return serde_json::json!(converted);
    }

    content.clone()
}
