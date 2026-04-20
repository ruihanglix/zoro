// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::AiError;
use serde::{Deserialize, Serialize};
use zoro_core::models::ApiFormat;

/// A generic chat completions client supporting OpenAI and Anthropic formats.
pub struct ChatClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
    format: ApiFormat,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

// Anthropic request/response types
#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    text: Option<String>,
}

impl ChatClient {
    /// Create a new client. `base_url` should be the API root
    /// (e.g. "https://api.openai.com/v1"). A trailing slash is tolerated.
    pub fn new(
        client: reqwest::Client,
        base_url: &str,
        api_key: &str,
        model: &str,
        format: ApiFormat,
    ) -> Self {
        let base_url = base_url.trim_end_matches('/').to_string();
        Self {
            http: client,
            base_url,
            api_key: api_key.to_string(),
            model: model.to_string(),
            format,
        }
    }

    /// Send a chat completion request and return the assistant's reply text.
    pub async fn chat(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        temperature: f32,
        max_tokens: Option<u32>,
    ) -> Result<String, AiError> {
        let result = match self.format {
            ApiFormat::Anthropic => self.chat_anthropic(system_prompt, user_prompt, temperature, max_tokens).await,
            _ => self.chat_openai(system_prompt, user_prompt, temperature, max_tokens).await,
        }?;
        Ok(strip_thinking_tags(&result))
    }

    async fn chat_openai(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        temperature: f32,
        max_tokens: Option<u32>,
    ) -> Result<String, AiError> {
        let mut messages = Vec::new();
        if !system_prompt.is_empty() {
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            });
        }
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: user_prompt.to_string(),
        });

        let request = ChatRequest {
            model: self.model.clone(),
            messages,
            temperature,
            max_tokens,
        };

        let url = format!("{}/chat/completions", self.base_url);

        tracing::debug!(
            url = %url,
            model = %request.model,
            temperature = %request.temperature,
            message_count = %request.messages.len(),
            "LLM request"
        );
        for (i, msg) in request.messages.iter().enumerate() {
            tracing::debug!(
                index = i,
                role = %msg.role,
                content_len = msg.content.len(),
                content = %msg.content,
                "LLM request message"
            );
        }

        // Retry once on 429 (rate limit)
        let mut attempts = 0;
        loop {
            attempts += 1;
            let resp = self
                .http
                .post(&url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .json(&request)
                .timeout(std::time::Duration::from_secs(120))
                .send()
                .await?;

            let status = resp.status().as_u16();

            if status == 429 && attempts < 3 {
                let wait = std::time::Duration::from_secs(2u64.pow(attempts));
                tracing::warn!("Rate limited (429), retrying in {:?}...", wait);
                tokio::time::sleep(wait).await;
                continue;
            }

            if !resp.status().is_success() {
                let body = resp.text().await.unwrap_or_default();
                tracing::debug!(status = status, body = %body, "LLM response error");
                return Err(AiError::Api {
                    status,
                    message: body,
                });
            }

            let raw_body = resp.text().await?;
            tracing::debug!(status = status, body_len = raw_body.len(), body = %raw_body, "LLM response");

            let body: ChatResponse = serde_json::from_str(&raw_body)?;
            let content = body
                .choices
                .into_iter()
                .next()
                .map(|c| c.message.content)
                .ok_or(AiError::EmptyResponse)?;

            return Ok(content.trim().to_string());
        }
    }

    async fn chat_anthropic(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        temperature: f32,
        max_tokens: Option<u32>,
    ) -> Result<String, AiError> {
        let url = format!("{}/messages", self.base_url);

        let request = AnthropicRequest {
            model: self.model.clone(),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: user_prompt.to_string(),
            }],
            max_tokens: max_tokens.unwrap_or(4096),
            system: if system_prompt.is_empty() {
                None
            } else {
                Some(system_prompt.to_string())
            },
            temperature: if temperature == 0.0 { None } else { Some(temperature) },
        };

        tracing::debug!(
            url = %url,
            model = %request.model,
            max_tokens = %request.max_tokens,
            "Anthropic LLM request"
        );

        // Retry once on 429 (rate limit)
        let mut attempts = 0;
        loop {
            attempts += 1;
            let resp = self
                .http
                .post(&url)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .header("Content-Type", "application/json")
                .json(&request)
                .timeout(std::time::Duration::from_secs(120))
                .send()
                .await?;

            let status = resp.status().as_u16();

            if status == 429 && attempts < 3 {
                let wait = std::time::Duration::from_secs(2u64.pow(attempts));
                tracing::warn!("Anthropic rate limited (429), retrying in {:?}...", wait);
                tokio::time::sleep(wait).await;
                continue;
            }

            if !resp.status().is_success() {
                let body = resp.text().await.unwrap_or_default();
                tracing::debug!(status = status, body = %body, "Anthropic response error");
                return Err(AiError::Api {
                    status,
                    message: body,
                });
            }

            let raw_body = resp.text().await?;
            tracing::debug!(status = status, body_len = raw_body.len(), body = %raw_body, "Anthropic response");

            let body: AnthropicResponse = serde_json::from_str(&raw_body)?;
            let content = body
                .content
                .into_iter()
                .find(|b| b.block_type == "text")
                .and_then(|b| b.text)
                .ok_or(AiError::EmptyResponse)?;

            return Ok(content.trim().to_string());
        }
    }

    /// Simple connectivity test: send a minimal request and check for a valid response.
    pub async fn test_connection(&self) -> Result<String, AiError> {
        let reply = self.chat("", "Reply with exactly: OK", 0.0, None).await?;
        Ok(reply)
    }
}

/// Strip `<think>...</think>` blocks from reasoning model output.
/// Many reasoning models (DeepSeek, MiniMax, Qwen, etc.) wrap chain-of-thought
/// in these tags. Callers always want the final answer only.
fn strip_thinking_tags(text: &str) -> String {
    let mut result = text.to_string();
    // Repeatedly strip all <think>...</think> blocks (handles nested/multiple)
    while let Some(start) = result.find("<think>") {
        if let Some(end) = result[start..].find("</think>") {
            result = format!(
                "{}{}",
                &result[..start],
                &result[start + end + "</think>".len()..],
            );
        } else {
            // Unclosed <think> tag — strip from <think> to end
            result = result[..start].to_string();
            break;
        }
    }
    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_url_normalization() {
        let client = ChatClient::new(
            reqwest::Client::new(),
            "https://api.openai.com/v1/",
            "key",
            "model",
            ApiFormat::OpenAI,
        );
        assert_eq!(client.base_url, "https://api.openai.com/v1");
    }

    #[test]
    fn test_strip_thinking_tags() {
        assert_eq!(
            strip_thinking_tags("<think>reasoning here</think>\nAnswer"),
            "Answer"
        );
        assert_eq!(
            strip_thinking_tags("<think>\nlong\nreasoning\n</think>\n\nFinal answer"),
            "Final answer"
        );
        assert_eq!(strip_thinking_tags("No tags here"), "No tags here");
        assert_eq!(
            strip_thinking_tags("<think>unclosed tag"),
            ""
        );
    }
}
