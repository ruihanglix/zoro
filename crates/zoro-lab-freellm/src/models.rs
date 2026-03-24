// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

//! Model list fetching and caching for free LLM providers.

use crate::providers::FreeProvider;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use zoro_llm_proxy::ApiFormat;

/// A model available from a free LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabModel {
    /// Model ID (e.g. "meta-llama/llama-3.1-8b-instruct:free")
    pub id: String,
    /// Human-readable name (may be same as ID)
    pub name: String,
    /// Provider ID that offers this model
    pub provider_id: String,
    /// Whether the user has disabled this model
    #[serde(default)]
    pub disabled: bool,
}

/// Persistent model cache: provider_id → list of model IDs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelCache {
    pub providers: HashMap<String, Vec<String>>,
    /// Timestamp (Unix seconds) of last successful refresh per provider.
    pub last_refresh: HashMap<String, u64>,
}

impl ModelCache {
    pub fn get_models(&self, provider_id: &str) -> &[String] {
        self.providers
            .get(provider_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn set_models(&mut self, provider_id: &str, models: Vec<String>) {
        self.providers.insert(provider_id.to_string(), models);
        self.last_refresh.insert(
            provider_id.to_string(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );
    }

    /// Check if a provider's cache is stale (older than the given max_age seconds).
    pub fn is_stale(&self, provider_id: &str, max_age_secs: u64) -> bool {
        match self.last_refresh.get(provider_id) {
            None => true,
            Some(&ts) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                now.saturating_sub(ts) > max_age_secs
            }
        }
    }
}

/// Fetch model list from a free provider's API.
pub async fn fetch_models(
    client: &reqwest::Client,
    provider: &FreeProvider,
    api_key: &str,
) -> Result<Vec<String>, String> {
    // GitHub Models uses a dedicated catalog API, not the OpenAI /models endpoint.
    if provider.id == "github" {
        return fetch_github_models(client, api_key).await;
    }
    match provider.format {
        ApiFormat::Gemini => fetch_gemini_models(client, provider, api_key).await,
        ApiFormat::OpenAI => fetch_openai_models(client, provider, api_key).await,
    }
}

/// Fetch models from an OpenAI-compatible /v1/models endpoint.
async fn fetch_openai_models(
    client: &reqwest::Client,
    provider: &FreeProvider,
    api_key: &str,
) -> Result<Vec<String>, String> {
    let url = format!("{}/models", provider.base_url.trim_end_matches('/'));

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("API error ({})", resp.status()));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

    let models = body
        .get("data")
        .and_then(|d| d.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    m.get("id")
                        .and_then(|id| id.as_str())
                        .map(|s| s.to_string())
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(models)
}

/// Fetch models from the GitHub Models catalog API.
///
/// GitHub Models doesn't expose a standard OpenAI /models endpoint.
/// Instead it has a catalog API at `https://models.github.ai/catalog/models`
/// that returns a JSON array of model objects. We filter out embedding-only
/// models and only keep those that output text.
async fn fetch_github_models(
    client: &reqwest::Client,
    api_key: &str,
) -> Result<Vec<String>, String> {
    let url = "https://models.github.ai/catalog/models";

    let resp = client
        .get(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2026-03-10")
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("API error ({})", resp.status()));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

    // Response is a JSON array: [ { "id": "openai/gpt-4.1", "supported_output_modalities": ["text"], ... }, ... ]
    let models = body
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    let id = m.get("id").and_then(|v| v.as_str())?;
                    // Skip embedding-only models (output modality is "embeddings", not "text")
                    let output_modalities = m
                        .get("supported_output_modalities")
                        .and_then(|v| v.as_array());
                    if let Some(modalities) = output_modalities {
                        let has_text = modalities.iter().any(|m| m.as_str() == Some("text"));
                        if !has_text {
                            return None;
                        }
                    }
                    Some(id.to_string())
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(models)
}

/// Fetch models from the Google Gemini API.
async fn fetch_gemini_models(
    client: &reqwest::Client,
    provider: &FreeProvider,
    api_key: &str,
) -> Result<Vec<String>, String> {
    let url = format!(
        "{}/models?key={}",
        provider.base_url.trim_end_matches('/'),
        api_key
    );

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("API error ({})", resp.status()));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

    // Gemini returns { models: [ { name: "models/gemini-pro", ... }, ... ] }
    let models = body
        .get("models")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    let name = m.get("name").and_then(|n| n.as_str())?;
                    // Strip the "models/" prefix
                    let id = name.strip_prefix("models/").unwrap_or(name);
                    // Only include models that support generateContent
                    let methods = m
                        .get("supportedGenerationMethods")
                        .and_then(|s| s.as_array())?;
                    let supports_chat = methods
                        .iter()
                        .any(|m| m.as_str() == Some("generateContent"));
                    if supports_chat {
                        Some(id.to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(models)
}
