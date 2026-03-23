// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

//! `LabService` — the main entry point for managing free LLM providers.
//! Coordinates provider configuration, model list caching, and proxy
//! config generation.

use crate::models::{self, LabModel, ModelCache};
use crate::providers::{FreeProvider, FREE_PROVIDERS};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use zoro_llm_proxy::{RoutingStrategy, UpstreamProvider};

/// Maximum age of cached model lists before considered stale (24 hours).
const MODEL_CACHE_MAX_AGE_SECS: u64 = 24 * 60 * 60;

/// Persistent state for the Lab service.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LabConfig {
    /// Provider ID → API key
    pub provider_keys: HashMap<String, String>,
    /// Set of disabled model IDs (provider_id::model_id)
    pub disabled_models: HashSet<String>,
    /// Routing strategy for the proxy
    pub routing_strategy: RoutingStrategy,
    /// Whether the Lab feature is enabled
    pub enabled: bool,
    /// Proxy listen port
    pub proxy_port: u16,
    /// Whether to listen on 0.0.0.0 (LAN-accessible)
    pub lan_access: bool,
    /// Optional LAN access token
    pub access_token: String,
}

impl LabConfig {
    pub fn default_with_port(port: u16) -> Self {
        Self {
            proxy_port: port,
            enabled: false,
            ..Default::default()
        }
    }
}

/// The main Lab service that manages free LLM providers.
pub struct LabService {
    config: LabConfig,
    model_cache: ModelCache,
    data_dir: PathBuf,
    http_client: reqwest::Client,
}

impl LabService {
    /// Create a new LabService, loading persisted config and cache from disk.
    pub fn new(data_dir: &Path) -> Self {
        let config = load_lab_config(data_dir);
        let model_cache = load_model_cache(data_dir);

        Self {
            config,
            model_cache,
            data_dir: data_dir.to_path_buf(),
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    // ── Config access ────────────────────────────────────────────────────

    pub fn config(&self) -> &LabConfig {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut LabConfig {
        &mut self.config
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
        self.save_config();
    }

    // ── Provider key management ──────────────────────────────────────────

    /// Set the API key for a provider. Empty string removes the key.
    pub fn set_provider_key(&mut self, provider_id: &str, api_key: String) {
        if api_key.is_empty() {
            self.config.provider_keys.remove(provider_id);
        } else {
            self.config
                .provider_keys
                .insert(provider_id.to_string(), api_key);
        }
        self.save_config();
    }

    pub fn get_provider_key(&self, provider_id: &str) -> Option<&String> {
        self.config.provider_keys.get(provider_id)
    }

    /// Get all providers that have API keys configured.
    pub fn configured_providers(&self) -> Vec<&FreeProvider> {
        FREE_PROVIDERS
            .iter()
            .filter(|p| {
                self.config
                    .provider_keys
                    .get(&p.id)
                    .map(|k| !k.is_empty())
                    .unwrap_or(false)
            })
            .collect()
    }

    pub fn has_configured_providers(&self) -> bool {
        !self.configured_providers().is_empty()
    }

    // ── Model management ─────────────────────────────────────────────────

    /// Get all available models (from cache), respecting disabled state.
    pub fn available_models(&self) -> Vec<LabModel> {
        let mut result = Vec::new();

        for provider in self.configured_providers() {
            for model_id in self.model_cache.get_models(&provider.id) {
                let disabled_key = format!("{}::{}", provider.id, model_id);
                result.push(LabModel {
                    id: model_id.clone(),
                    name: model_id.clone(),
                    provider_id: provider.id.clone(),
                    disabled: self.config.disabled_models.contains(&disabled_key),
                });
            }
        }

        result
    }

    /// Get enabled (non-disabled) models only.
    pub fn enabled_models(&self) -> Vec<LabModel> {
        self.available_models()
            .into_iter()
            .filter(|m| !m.disabled)
            .collect()
    }

    /// Toggle a model's disabled state.
    pub fn set_model_disabled(&mut self, provider_id: &str, model_id: &str, disabled: bool) {
        let key = format!("{}::{}", provider_id, model_id);
        if disabled {
            self.config.disabled_models.insert(key);
        } else {
            self.config.disabled_models.remove(&key);
        }
        self.save_config();
    }

    /// Refresh model lists from all configured providers.
    pub async fn refresh_all_models(&mut self) -> Vec<(String, Result<usize, String>)> {
        let mut results = Vec::new();

        let providers = self
            .configured_providers()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        for provider in &providers {
            let api_key = match self.config.provider_keys.get(&provider.id) {
                Some(k) => k.clone(),
                None => continue,
            };

            let result = models::fetch_models(&self.http_client, provider, &api_key).await;

            match result {
                Ok(model_list) => {
                    let count = model_list.len();
                    // For OpenCode Zen, auto-disable newly discovered models
                    // that don't contain "free" in their ID.
                    self.auto_disable_non_free_models(&provider.id, &model_list);
                    self.model_cache.set_models(&provider.id, model_list);
                    results.push((provider.id.clone(), Ok(count)));
                    tracing::info!(
                        provider = %provider.id,
                        model_count = count,
                        "Refreshed model list"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        provider = %provider.id,
                        error = %e,
                        "Failed to refresh model list"
                    );
                    results.push((provider.id.clone(), Err(e)));
                }
            }
        }

        self.save_model_cache();
        results
    }

    /// Refresh models for a single provider.
    pub async fn refresh_provider_models(&mut self, provider_id: &str) -> Result<usize, String> {
        let provider = FREE_PROVIDERS
            .iter()
            .find(|p| p.id == provider_id)
            .ok_or_else(|| format!("Unknown provider: {}", provider_id))?
            .clone();

        let api_key = self
            .config
            .provider_keys
            .get(provider_id)
            .ok_or_else(|| format!("No API key for provider: {}", provider_id))?
            .clone();

        let model_list = models::fetch_models(&self.http_client, &provider, &api_key).await?;
        let count = model_list.len();
        // For OpenCode Zen, auto-disable newly discovered models
        // that don't contain "free" in their ID.
        self.auto_disable_non_free_models(provider_id, &model_list);
        self.model_cache.set_models(provider_id, model_list);
        self.save_model_cache();

        Ok(count)
    }

    /// Refresh stale model caches (those older than MODEL_CACHE_MAX_AGE_SECS).
    pub async fn refresh_stale_models(&mut self) {
        let stale_providers: Vec<String> = self
            .configured_providers()
            .iter()
            .filter(|p| self.model_cache.is_stale(&p.id, MODEL_CACHE_MAX_AGE_SECS))
            .map(|p| p.id.clone())
            .collect();

        for provider_id in stale_providers {
            if let Err(e) = self.refresh_provider_models(&provider_id).await {
                tracing::warn!(
                    provider = %provider_id,
                    error = %e,
                    "Failed to refresh stale model cache"
                );
            }
        }
    }

    /// For the OpenCode Zen provider, automatically disable newly discovered
    /// models whose ID does not contain "free". Models already known (present
    /// in the current cache) keep their existing enabled/disabled state.
    fn auto_disable_non_free_models(&mut self, provider_id: &str, new_models: &[String]) {
        // This rule only applies to the OpenCode Zen provider.
        if provider_id != "opencode" {
            return;
        }

        let existing: HashSet<&str> = self
            .model_cache
            .get_models(provider_id)
            .iter()
            .map(|s| s.as_str())
            .collect();

        for model_id in new_models {
            // Skip models that already exist in the cache (keep user's setting).
            if existing.contains(model_id.as_str()) {
                continue;
            }
            // New model: disable it if its ID doesn't contain "free".
            let id_lower = model_id.to_lowercase();
            if !id_lower.contains("free") {
                let key = format!("{}::{}", provider_id, model_id);
                self.config.disabled_models.insert(key);
                tracing::debug!(
                    provider = %provider_id,
                    model = %model_id,
                    "Auto-disabled non-free model from OpenCode Zen"
                );
            }
        }

        // Persist the updated disabled_models set.
        self.save_config();
    }

    // ── Proxy config generation ──────────────────────────────────────────

    /// Convert current lab configuration into a list of UpstreamProviders
    /// for the proxy server.
    pub fn to_proxy_providers(&self) -> Vec<UpstreamProvider> {
        let mut result = Vec::new();

        for provider in self.configured_providers() {
            let api_key = match self.config.provider_keys.get(&provider.id) {
                Some(k) if !k.is_empty() => k.clone(),
                _ => continue,
            };

            // Only include enabled models
            let models: Vec<String> = self
                .model_cache
                .get_models(&provider.id)
                .iter()
                .filter(|m| {
                    let key = format!("{}::{}", provider.id, m);
                    !self.config.disabled_models.contains(&key)
                })
                .cloned()
                .collect();

            if models.is_empty() {
                continue;
            }

            result.push(UpstreamProvider {
                id: provider.id.clone(),
                name: provider.name.clone(),
                base_url: provider.base_url.clone(),
                api_key,
                models,
                format: provider.format,
            });
        }

        result
    }

    /// Build a full ProxyConfig from the current lab state.
    pub fn to_proxy_config(&self) -> zoro_llm_proxy::ProxyConfig {
        zoro_llm_proxy::ProxyConfig {
            listen_addr: if self.config.lan_access {
                "0.0.0.0".into()
            } else {
                "127.0.0.1".into()
            },
            port: if self.config.proxy_port > 0 {
                self.config.proxy_port
            } else {
                29170
            },
            providers: self.to_proxy_providers(),
            routing_strategy: self.config.routing_strategy,
            max_retries: 3,
            access_token: self.config.access_token.clone(),
        }
    }

    // ── Routing strategy ─────────────────────────────────────────────────

    pub fn set_routing_strategy(&mut self, strategy: RoutingStrategy) {
        self.config.routing_strategy = strategy;
        self.save_config();
    }

    pub fn routing_strategy(&self) -> RoutingStrategy {
        self.config.routing_strategy
    }

    // ── Proxy settings ───────────────────────────────────────────────────

    pub fn set_proxy_port(&mut self, port: u16) {
        self.config.proxy_port = port;
        self.save_config();
    }

    pub fn set_lan_access(&mut self, enabled: bool) {
        self.config.lan_access = enabled;
        self.save_config();
    }

    pub fn set_access_token(&mut self, token: String) {
        self.config.access_token = token;
        self.save_config();
    }

    // ── Persistence ──────────────────────────────────────────────────────

    fn save_config(&self) {
        save_lab_config(&self.data_dir, &self.config);
    }

    fn save_model_cache(&self) {
        save_model_cache(&self.data_dir, &self.model_cache);
    }
}

// ── Disk persistence helpers ─────────────────────────────────────────────────

fn lab_config_path(data_dir: &Path) -> PathBuf {
    data_dir.join("lab_config.json")
}

fn model_cache_path(data_dir: &Path) -> PathBuf {
    data_dir.join("lab_model_cache.json")
}

fn load_lab_config(data_dir: &Path) -> LabConfig {
    let path = lab_config_path(data_dir);
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => LabConfig::default_with_port(29170),
    }
}

fn save_lab_config(data_dir: &Path, config: &LabConfig) {
    let path = lab_config_path(data_dir);
    if let Ok(json) = serde_json::to_string_pretty(config) {
        if let Err(e) = std::fs::write(&path, json) {
            tracing::error!(error = %e, "Failed to save lab config");
        }
    }
}

fn load_model_cache(data_dir: &Path) -> ModelCache {
    let path = model_cache_path(data_dir);
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => ModelCache::default(),
    }
}

fn save_model_cache(data_dir: &Path, cache: &ModelCache) {
    let path = model_cache_path(data_dir);
    if let Ok(json) = serde_json::to_string_pretty(cache) {
        if let Err(e) = std::fs::write(&path, json) {
            tracing::error!(error = %e, "Failed to save model cache");
        }
    }
}
