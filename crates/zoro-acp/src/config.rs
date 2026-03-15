// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::AcpError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: Vec<EnvEntry>,
    /// Binary to check when detecting whether the agent is installed.
    /// Falls back to `command` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detect_command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvEntry {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpConfig {
    #[serde(default = "default_agents")]
    pub agents: Vec<AgentConfig>,
}

fn default_agents() -> Vec<AgentConfig> {
    vec![
        AgentConfig {
            name: "cursor".into(),
            title: "Cursor Agent".into(),
            description: "Anysphere · Cursor IDE built-in agent".into(),
            command: "agent".into(),
            args: vec!["acp".into()],
            env: vec![],
            detect_command: None,
        },
        AgentConfig {
            name: "claude-agent".into(),
            title: "Anthropic Claude Code".into(),
            description: "Anthropic · via Zed ACP adapter".into(),
            command: "npx".into(),
            args: vec![
                "-y".into(),
                "@zed-industries/claude-agent-acp@latest".into(),
            ],
            env: vec![],
            detect_command: Some("claude".into()),
        },
        AgentConfig {
            name: "codex".into(),
            title: "OpenAI Codex CLI".into(),
            description: "OpenAI · via Zed ACP adapter".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@zed-industries/codex-acp@latest".into()],
            env: vec![],
            detect_command: Some("codex".into()),
        },
        AgentConfig {
            name: "gemini".into(),
            title: "Google Gemini CLI".into(),
            description: "Google · AI coding assistant".into(),
            command: "gemini".into(),
            args: vec!["--experimental-acp".into()],
            env: vec![],
            detect_command: None,
        },
        AgentConfig {
            name: "copilot".into(),
            title: "GitHub Copilot CLI".into(),
            description: "GitHub / Microsoft · AI pair programmer".into(),
            command: "copilot".into(),
            args: vec!["--acp".into(), "--stdio".into()],
            env: vec![],
            detect_command: None,
        },
        AgentConfig {
            name: "opencode".into(),
            title: "OpenCode".into(),
            description: "SST · open-source coding agent".into(),
            command: "opencode".into(),
            args: vec!["acp".into()],
            env: vec![],
            detect_command: None,
        },
        AgentConfig {
            name: "openclaw".into(),
            title: "OpenClaw".into(),
            description: "OpenClaw · AI coding agent".into(),
            command: "openclaw".into(),
            args: vec!["acp".into()],
            env: vec![],
            detect_command: None,
        },
    ]
}

impl Default for AcpConfig {
    fn default() -> Self {
        Self {
            agents: default_agents(),
        }
    }
}

pub fn config_path(data_dir: &Path) -> PathBuf {
    data_dir.join("agents.toml")
}

pub fn load_config(data_dir: &Path) -> AcpConfig {
    let path = config_path(data_dir);
    if path.exists() {
        match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str::<AcpConfig>(&content) {
                Ok(cfg) => return cfg,
                Err(e) => {
                    tracing::warn!("Failed to parse agents.toml, using defaults: {}", e);
                }
            },
            Err(e) => {
                tracing::warn!("Failed to read agents.toml, using defaults: {}", e);
            }
        }
    }

    let cfg = AcpConfig::default();
    save_config(data_dir, &cfg).ok();
    cfg
}

pub fn save_config(data_dir: &Path, cfg: &AcpConfig) -> Result<(), AcpError> {
    let path = config_path(data_dir);
    let content = toml::to_string_pretty(cfg).map_err(|e| AcpError::Config(e.to_string()))?;
    std::fs::write(&path, content)?;
    Ok(())
}
