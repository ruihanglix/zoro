// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

//! Free LLM provider definitions.
//! Each provider offers free-tier API access for AI models.

use serde::{Deserialize, Serialize};
use zoro_llm_proxy::ApiFormat;

/// A free LLM provider that offers free-tier API access.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeProvider {
    /// Unique identifier (e.g. "openrouter")
    pub id: String,
    /// Short name (e.g. "OpenRouter")
    pub name: String,
    /// Display name for the UI (e.g. "OpenRouter")
    pub display_name: String,
    /// API base URL (e.g. "https://openrouter.ai/api/v1")
    pub base_url: String,
    /// URL where users can sign up and get an API key
    pub sign_up_url: String,
    /// Expected key prefix for validation hints (e.g. "sk-or-")
    pub key_prefix: String,
    /// Whether this is a "primary" (shown by default) or "secondary" provider
    pub tier: ProviderTier,
    /// API format: OpenAI-compatible or Gemini
    pub format: ApiFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderTier {
    Primary,
    Secondary,
}

/// Get all supported free LLM providers.
pub fn free_providers() -> Vec<FreeProvider> {
    vec![
        FreeProvider {
            id: "openrouter".into(),
            name: "OpenRouter".into(),
            display_name: "OpenRouter".into(),
            base_url: "https://openrouter.ai/api/v1".into(),
            sign_up_url: "https://openrouter.ai/keys".into(),
            key_prefix: "sk-or-".into(),
            tier: ProviderTier::Primary,
            format: ApiFormat::OpenAI,
        },
        FreeProvider {
            id: "groq".into(),
            name: "Groq".into(),
            display_name: "Groq".into(),
            base_url: "https://api.groq.com/openai/v1".into(),
            sign_up_url: "https://console.groq.com/keys".into(),
            key_prefix: "gsk_".into(),
            tier: ProviderTier::Primary,
            format: ApiFormat::OpenAI,
        },
        FreeProvider {
            id: "gemini".into(),
            name: "Gemini".into(),
            display_name: "Google Gemini".into(),
            base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
            sign_up_url: "https://aistudio.google.com/app/apikey".into(),
            key_prefix: "AIza".into(),
            tier: ProviderTier::Primary,
            format: ApiFormat::Gemini,
        },
        FreeProvider {
            id: "github".into(),
            name: "GitHub Models".into(),
            display_name: "GitHub Models".into(),
            base_url: "https://models.github.ai/inference".into(),
            sign_up_url: "https://github.com/settings/tokens".into(),
            key_prefix: "ghp_".into(),
            tier: ProviderTier::Secondary,
            format: ApiFormat::OpenAI,
        },
        FreeProvider {
            id: "mistral".into(),
            name: "Mistral".into(),
            display_name: "Mistral AI".into(),
            base_url: "https://api.mistral.ai/v1".into(),
            sign_up_url: "https://console.mistral.ai/api-keys".into(),
            key_prefix: String::new(),
            tier: ProviderTier::Secondary,
            format: ApiFormat::OpenAI,
        },
        FreeProvider {
            id: "opencode".into(),
            name: "OpenCode".into(),
            display_name: "OpenCode Zen".into(),
            base_url: "https://opencode.ai/zen/v1".into(),
            sign_up_url: "https://opencode.ai/auth".into(),
            key_prefix: String::new(),
            tier: ProviderTier::Secondary,
            format: ApiFormat::OpenAI,
        },
        FreeProvider {
            id: "cerebras".into(),
            name: "Cerebras".into(),
            display_name: "Cerebras".into(),
            base_url: "https://api.cerebras.ai/v1".into(),
            sign_up_url: "https://cloud.cerebras.ai".into(),
            key_prefix: String::new(),
            tier: ProviderTier::Secondary,
            format: ApiFormat::OpenAI,
        },
        FreeProvider {
            id: "sambanova".into(),
            name: "SambaNova".into(),
            display_name: "SambaNova".into(),
            base_url: "https://api.sambanova.ai/v1".into(),
            sign_up_url: "https://cloud.sambanova.ai".into(),
            key_prefix: String::new(),
            tier: ProviderTier::Secondary,
            format: ApiFormat::OpenAI,
        },
    ]
}

/// Convenience constant: call `FREE_PROVIDERS` to get the lazy-initialized list.
pub use once_cell_providers::FREE_PROVIDERS;

mod once_cell_providers {
    use super::*;
    use std::sync::LazyLock;
    /// Pre-built list of free providers (initialized once on first access).
    pub static FREE_PROVIDERS: LazyLock<Vec<FreeProvider>> = LazyLock::new(free_providers);
}
