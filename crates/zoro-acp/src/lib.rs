// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

pub mod config;
pub mod error;
pub mod manager;

pub use config::{AcpConfig, AgentConfig};
pub use error::AcpError;
pub use manager::{AcpManager, AgentUpdate, ConfigOptionInfo, ImageAttachment};

/// Build a PATH that includes common user-level bin directories.
/// macOS GUI apps get a minimal PATH that misses these.
pub fn full_path() -> String {
    let base = std::env::var("PATH").unwrap_or_default();
    let home = std::env::var("HOME").unwrap_or_default();
    if home.is_empty() {
        return base;
    }

    let extra_dirs = [
        format!("{home}/.opencode/bin"),
        format!("{home}/.local/bin"),
        format!("{home}/.cargo/bin"),
        format!("{home}/.nvm/current/bin"),
        "/usr/local/bin".into(),
        "/opt/homebrew/bin".into(),
    ];

    let mut parts: Vec<&str> = base.split(':').collect();
    for dir in &extra_dirs {
        if !parts.contains(&dir.as_str()) && std::path::Path::new(dir).is_dir() {
            parts.push(dir);
        }
    }
    parts.join(":")
}
