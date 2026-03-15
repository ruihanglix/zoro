// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::path::PathBuf;
use std::sync::Mutex;
use zoro_core::models::AppConfig;
use zoro_db::Database;

/// Shared state for the MCP server, analogous to the desktop app's AppState.
pub struct AppState {
    pub db: Mutex<Database>,
    pub data_dir: PathBuf,
}

impl AppState {
    pub fn new(db: Database, data_dir: PathBuf) -> Self {
        Self {
            db: Mutex::new(db),
            data_dir,
        }
    }

    /// Load config from config.toml in the data directory.
    /// Falls back to defaults on any error.
    pub fn load_config(&self) -> AppConfig {
        let config_path = self.data_dir.join("config.toml");
        match std::fs::read_to_string(&config_path) {
            Ok(content) => match toml::from_str::<AppConfig>(&content) {
                Ok(config) => config,
                Err(e) => {
                    tracing::warn!("Failed to parse config.toml, using defaults: {}", e);
                    AppConfig::default()
                }
            },
            Err(e) => {
                tracing::warn!("Failed to read config.toml, using defaults: {}", e);
                AppConfig::default()
            }
        }
    }
}
