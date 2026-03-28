// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Persistent configuration for the ACP Proxy lab feature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpProxyConfig {
    /// Whether the ACP Proxy feature is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Name of the ACP agent to use (e.g. "claude-agent", "gemini").
    #[serde(default)]
    pub agent_name: String,
    /// Config option ID for the mode selector (from ACP session config).
    #[serde(default)]
    pub mode_config_id: String,
    /// Selected mode value (from ACP session config).
    #[serde(default)]
    pub mode_value: String,
    /// Config option ID for the model selector (from ACP session config).
    #[serde(default)]
    pub model_config_id: String,
    /// Selected model value (from ACP session config).
    #[serde(default)]
    pub model_value: String,
    /// Number of ACP worker sessions (default 3).
    #[serde(default = "default_worker_count")]
    pub worker_count: usize,
    /// Proxy listen port (default 29171).
    #[serde(default = "default_port")]
    pub port: u16,
    /// Whether to listen on 0.0.0.0 (LAN-accessible).
    #[serde(default)]
    pub lan_access: bool,
    /// Optional bearer token for LAN access control (empty = no auth).
    #[serde(default)]
    pub access_token: String,
}

fn default_worker_count() -> usize {
    3
}

fn default_port() -> u16 {
    29171
}

impl Default for AcpProxyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            agent_name: String::new(),
            mode_config_id: String::new(),
            mode_value: String::new(),
            model_config_id: String::new(),
            model_value: String::new(),
            worker_count: default_worker_count(),
            port: default_port(),
            lan_access: false,
            access_token: String::new(),
        }
    }
}

// ── Disk persistence ─────────────────────────────────────────────────────────

fn config_path(data_dir: &Path) -> PathBuf {
    data_dir.join("acp_proxy_config.json")
}

pub fn load_config(data_dir: &Path) -> AcpProxyConfig {
    let path = config_path(data_dir);
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => AcpProxyConfig::default(),
    }
}

pub fn save_config(data_dir: &Path, config: &AcpProxyConfig) {
    let path = config_path(data_dir);
    if let Ok(json) = serde_json::to_string_pretty(config) {
        if let Err(e) = std::fs::write(&path, json) {
            tracing::error!(error = %e, "Failed to save ACP proxy config");
        }
    }
}

// ── Config options cache persistence ─────────────────────────────────────────

fn options_cache_path(data_dir: &Path) -> PathBuf {
    data_dir.join("acp_proxy_options_cache.json")
}

/// Load the per-agent config options cache from disk.
/// Returns a map of agent_name → Vec<ConfigOptionInfo> (as raw JSON values).
pub fn load_options_cache(data_dir: &Path) -> std::collections::HashMap<String, serde_json::Value> {
    let path = options_cache_path(data_dir);
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => std::collections::HashMap::new(),
    }
}

/// Save the per-agent config options cache to disk.
pub fn save_options_cache(
    data_dir: &Path,
    cache: &std::collections::HashMap<String, serde_json::Value>,
) {
    let path = options_cache_path(data_dir);
    if let Ok(json) = serde_json::to_string_pretty(cache) {
        if let Err(e) = std::fs::write(&path, json) {
            tracing::error!(error = %e, "Failed to save ACP proxy options cache");
        }
    }
}
