// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PluginsError {
    #[error("Plugin not found: {0}")]
    NotFound(String),

    #[error("Invalid manifest: {0}")]
    InvalidManifest(String),

    #[error("Plugin already installed: {0}")]
    AlreadyInstalled(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML serialization error: {0}")]
    TomlSer(#[from] toml::ser::Error),

    #[error("TOML deserialization error: {0}")]
    TomlDe(#[from] toml::de::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
