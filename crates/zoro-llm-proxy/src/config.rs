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
#[derive(Debug, Serialize, Deserialize)]
pub struct UpstreamProvider {
    /// Unique identifier (e.g. "openrouter", "groq", "gemini")
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// API base URL (e.g. "https://openrouter.ai/api/v1")
    pub base_url: String,
    /// API keys for this provider (supports multiple keys for load balancing)
    #[serde(deserialize_with = "deserialize_api_keys", default)]
    pub api_keys: Vec<String>,
    /// List of model IDs available on this provider
    pub models: Vec<String>,
    /// API format (OpenAI-compatible or Gemini)
    #[serde(default)]
    pub format: ApiFormat,
    /// Round-robin counter for key selection (runtime only, not serialized)
    #[serde(skip, default)]
    key_counter: std::sync::atomic::AtomicUsize,
}

impl Clone for UpstreamProvider {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            name: self.name.clone(),
            base_url: self.base_url.clone(),
            api_keys: self.api_keys.clone(),
            models: self.models.clone(),
            format: self.format,
            key_counter: std::sync::atomic::AtomicUsize::new(
                self.key_counter.load(std::sync::atomic::Ordering::Relaxed),
            ),
        }
    }
}

impl Default for UpstreamProvider {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            base_url: String::new(),
            api_keys: Vec::new(),
            models: Vec::new(),
            format: ApiFormat::default(),
            key_counter: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

impl UpstreamProvider {
    /// Create a new upstream provider.
    pub fn new(
        id: String,
        name: String,
        base_url: String,
        api_keys: Vec<String>,
        models: Vec<String>,
        format: ApiFormat,
    ) -> Self {
        Self {
            id,
            name,
            base_url,
            api_keys,
            models,
            format,
            key_counter: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Select the next API key using round-robin. Panics if `api_keys` is empty.
    pub fn next_api_key(&self) -> &str {
        let keys = &self.api_keys;
        debug_assert!(
            !keys.is_empty(),
            "UpstreamProvider must have at least one API key"
        );
        let idx = self
            .key_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            % keys.len();
        &keys[idx]
    }
}

/// Deserialize `api_keys` from either a single string (legacy) or a list of strings.
fn deserialize_api_keys<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;

    struct ApiKeysVisitor;

    impl<'de> de::Visitor<'de> for ApiKeysVisitor {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or a list of strings")
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            if v.is_empty() {
                Ok(Vec::new())
            } else {
                Ok(vec![v.to_string()])
            }
        }

        fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut keys = Vec::new();
            while let Some(key) = seq.next_element::<String>()? {
                if !key.is_empty() {
                    keys.push(key);
                }
            }
            Ok(keys)
        }
    }

    deserializer.deserialize_any(ApiKeysVisitor)
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
