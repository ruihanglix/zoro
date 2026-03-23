// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use serde::{Deserialize, Serialize};

/// Configuration for the local LLM proxy server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    /// Listen address: "127.0.0.1" (local only) or "0.0.0.0" (LAN-accessible)
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,

    /// Listen port (default 29170)
    #[serde(default = "default_port")]
    pub port: u16,

    /// Upstream provider list
    #[serde(default)]
    pub providers: Vec<UpstreamProvider>,

    /// Routing strategy for selecting providers
    #[serde(default)]
    pub routing_strategy: RoutingStrategy,

    /// Maximum retry attempts before giving up (default 3)
    #[serde(default = "default_max_retries")]
    pub max_retries: usize,

    /// Optional bearer token for LAN access control (empty = no auth)
    #[serde(default)]
    pub access_token: String,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            listen_addr: default_listen_addr(),
            port: default_port(),
            providers: Vec::new(),
            routing_strategy: RoutingStrategy::default(),
            max_retries: default_max_retries(),
            access_token: String::new(),
        }
    }
}

fn default_listen_addr() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    29170
}

fn default_max_retries() -> usize {
    3
}

/// An upstream LLM provider that the proxy can forward requests to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamProvider {
    /// Unique identifier (e.g. "openrouter", "groq", "gemini")
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// API base URL (e.g. "https://openrouter.ai/api/v1")
    pub base_url: String,
    /// API key for this provider
    pub api_key: String,
    /// List of model IDs available on this provider
    pub models: Vec<String>,
    /// API format (OpenAI-compatible or Gemini)
    #[serde(default)]
    pub format: ApiFormat,
}

/// API format of an upstream provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ApiFormat {
    /// Standard OpenAI /v1/chat/completions
    #[default]
    OpenAI,
    /// Google Gemini generateContent API — proxy converts on the fly
    Gemini,
}

/// Strategy for selecting which upstream provider to route a request to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum RoutingStrategy {
    /// Try the requested model first; on failure, fall back to any healthy provider
    #[default]
    Auto,
    /// Cycle through all healthy providers in order
    RoundRobin,
    /// Use only the provider that owns the requested model; no fallback
    Manual,
}
