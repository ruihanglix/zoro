// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::AiError;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::pin::pin;

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
}

impl StreamingClient {
    pub fn new(base_url: &str, api_key: &str, model: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
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
            .header("Content-Type", "application/json")
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
