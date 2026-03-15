// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::AiError;
use serde::{Deserialize, Serialize};

/// A generic OpenAI-compatible chat completions client.
pub struct ChatClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
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

impl ChatClient {
    /// Create a new client. `base_url` should be the API root
    /// (e.g. "https://api.openai.com/v1"). A trailing slash is tolerated.
    pub fn new(base_url: &str, api_key: &str, model: &str) -> Self {
        let base_url = base_url.trim_end_matches('/').to_string();
        Self {
            http: reqwest::Client::new(),
            base_url,
            api_key: api_key.to_string(),
            model: model.to_string(),
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
                // Rate limited — wait and retry
                let wait = std::time::Duration::from_secs(2u64.pow(attempts));
                tracing::warn!("Rate limited (429), retrying in {:?}...", wait);
                tokio::time::sleep(wait).await;
                continue;
            }

            if !resp.status().is_success() {
                let body = resp.text().await.unwrap_or_default();
                tracing::debug!(
                    status = status,
                    body = %body,
                    "LLM response error"
                );
                return Err(AiError::Api {
                    status,
                    message: body,
                });
            }

            let raw_body = resp.text().await?;
            tracing::debug!(
                status = status,
                body_len = raw_body.len(),
                body = %raw_body,
                "LLM response"
            );

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

    /// Simple connectivity test: send a minimal request and check for a valid response.
    pub async fn test_connection(&self) -> Result<String, AiError> {
        let reply = self.chat("", "Reply with exactly: OK", 0.0, None).await?;
        Ok(reply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_url_normalization() {
        let client = ChatClient::new("https://api.openai.com/v1/", "key", "model");
        assert_eq!(client.base_url, "https://api.openai.com/v1");
    }
}
