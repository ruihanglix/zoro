// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::PluginsError;
use crate::manifest::PluginManifest;

/// Runtime information about a loaded plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub manifest: PluginManifest,
    pub mode: PluginMode,
    pub path: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginMode {
    Installed,
    Dev,
}

/// Persistent plugin entry stored in plugins.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEntry {
    pub id: String,
    pub enabled: bool,
    pub mode: PluginMode,
    pub path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginsConfig {
    #[serde(default)]
    pub plugins: Vec<PluginEntry>,
}

/// The plugin registry manages all installed and dev plugins.
pub struct PluginRegistry {
    data_dir: PathBuf,
    config: PluginsConfig,
}

impl PluginRegistry {
    /// Create a new registry. `data_dir` is typically `~/.zoro`.
    pub fn new(data_dir: PathBuf) -> Self {
        let config = Self::load_config(&data_dir).unwrap_or_default();
        Self { data_dir, config }
    }

    /// Path to the plugins.toml config file.
    fn config_path(data_dir: &Path) -> PathBuf {
        data_dir.join("plugins").join("plugins.toml")
    }

    /// Path to the installed plugins directory.
    fn installed_dir(data_dir: &Path) -> PathBuf {
        data_dir.join("plugins").join("installed")
    }

    /// Load config from disk.
    fn load_config(data_dir: &Path) -> Result<PluginsConfig, PluginsError> {
        let path = Self::config_path(data_dir);
        if !path.exists() {
            return Ok(PluginsConfig::default());
        }
        let content = std::fs::read_to_string(&path)?;
        let config: PluginsConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save config to disk.
    fn save_config(&self) -> Result<(), PluginsError> {
        let path = Self::config_path(&self.data_dir);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(&self.config)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// List all registered plugins with their manifests.
    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        self.config
            .plugins
            .iter()
            .filter_map(|entry| {
                let plugin_path = PathBuf::from(&entry.path);
                let manifest_path = plugin_path.join("manifest.json");
                let manifest = PluginManifest::from_file(&manifest_path).ok()?;
                Some(PluginInfo {
                    manifest,
                    mode: entry.mode.clone(),
                    path: entry.path.clone(),
                    enabled: entry.enabled,
                })
            })
            .collect()
    }

    /// Install a plugin from a .zcx file (zip archive).
    pub fn install_from_zcx(&mut self, zcx_path: &str) -> Result<PluginInfo, PluginsError> {
        // Read zip file
        let data = std::fs::read(zcx_path)?;
        let cursor = std::io::Cursor::new(data);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| PluginsError::InvalidManifest(format!("Invalid .zcx archive: {}", e)))?;

        // Extract manifest.json first to get plugin ID
        let manifest_content = {
            let mut manifest_file = archive.by_name("manifest.json").map_err(|_| {
                PluginsError::InvalidManifest("manifest.json not found in .zcx archive".to_string())
            })?;
            let mut content = String::new();
            std::io::Read::read_to_string(&mut manifest_file, &mut content)?;
            content
        };

        let manifest = PluginManifest::from_json(&manifest_content)?;
        manifest.validate()?;

        // Check if already installed
        if self.config.plugins.iter().any(|p| p.id == manifest.id) {
            // Remove old installation
            self.uninstall(&manifest.id)?;
        }

        // Extract to installed directory
        let install_dir = Self::installed_dir(&self.data_dir).join(&manifest.id);
        std::fs::create_dir_all(&install_dir)?;

        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| PluginsError::Io(std::io::Error::other(e)))?;
            let out_path = install_dir.join(file.name());

            if file.is_dir() {
                std::fs::create_dir_all(&out_path)?;
            } else {
                if let Some(parent) = out_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut out_file = std::fs::File::create(&out_path)?;
                std::io::copy(&mut file, &mut out_file)?;
            }
        }

        let path_str = install_dir.to_string_lossy().to_string();
        let entry = PluginEntry {
            id: manifest.id.clone(),
            enabled: true,
            mode: PluginMode::Installed,
            path: path_str.clone(),
        };
        self.config.plugins.push(entry);
        self.save_config()?;

        Ok(PluginInfo {
            manifest,
            mode: PluginMode::Installed,
            path: path_str,
            enabled: true,
        })
    }

    /// Uninstall a plugin by ID.
    pub fn uninstall(&mut self, plugin_id: &str) -> Result<(), PluginsError> {
        let entry = self
            .config
            .plugins
            .iter()
            .find(|p| p.id == plugin_id)
            .ok_or_else(|| PluginsError::NotFound(plugin_id.to_string()))?
            .clone();

        // Remove installed files (only for non-dev plugins)
        if matches!(entry.mode, PluginMode::Installed) {
            let plugin_dir = PathBuf::from(&entry.path);
            if plugin_dir.exists() {
                let _ = std::fs::remove_dir_all(&plugin_dir);
            }
        }

        self.config.plugins.retain(|p| p.id != plugin_id);
        self.save_config()?;
        Ok(())
    }

    /// Toggle a plugin's enabled state.
    pub fn toggle_plugin(&mut self, plugin_id: &str, enabled: bool) -> Result<(), PluginsError> {
        let entry = self
            .config
            .plugins
            .iter_mut()
            .find(|p| p.id == plugin_id)
            .ok_or_else(|| PluginsError::NotFound(plugin_id.to_string()))?;
        entry.enabled = enabled;
        self.save_config()?;
        Ok(())
    }

    /// Load a dev plugin from a folder path.
    pub fn load_dev_plugin(&mut self, folder_path: &str) -> Result<PluginInfo, PluginsError> {
        let plugin_dir = PathBuf::from(folder_path);
        let manifest_path = plugin_dir.join("manifest.json");

        if !manifest_path.exists() {
            return Err(PluginsError::InvalidManifest(format!(
                "manifest.json not found in {}",
                folder_path
            )));
        }

        let manifest = PluginManifest::from_file(&manifest_path)?;
        manifest.validate()?;

        // Remove existing entry with same ID if any
        self.config.plugins.retain(|p| p.id != manifest.id);

        let entry = PluginEntry {
            id: manifest.id.clone(),
            enabled: true,
            mode: PluginMode::Dev,
            path: folder_path.to_string(),
        };
        self.config.plugins.push(entry);
        self.save_config()?;

        Ok(PluginInfo {
            manifest,
            mode: PluginMode::Dev,
            path: folder_path.to_string(),
            enabled: true,
        })
    }

    /// Unload a dev plugin (remove from registry but don't delete files).
    pub fn unload_dev_plugin(&mut self, plugin_id: &str) -> Result<(), PluginsError> {
        let exists = self.config.plugins.iter().any(|p| p.id == plugin_id);
        if !exists {
            return Err(PluginsError::NotFound(plugin_id.to_string()));
        }
        self.config.plugins.retain(|p| p.id != plugin_id);
        self.save_config()?;
        Ok(())
    }

    /// Reload a dev plugin (re-read manifest from disk).
    pub fn reload_dev_plugin(&self, plugin_id: &str) -> Result<PluginInfo, PluginsError> {
        let entry = self
            .config
            .plugins
            .iter()
            .find(|p| p.id == plugin_id)
            .ok_or_else(|| PluginsError::NotFound(plugin_id.to_string()))?;

        let plugin_path = PathBuf::from(&entry.path);
        let manifest_path = plugin_path.join("manifest.json");
        let manifest = PluginManifest::from_file(&manifest_path)?;

        Ok(PluginInfo {
            manifest,
            mode: entry.mode.clone(),
            path: entry.path.clone(),
            enabled: entry.enabled,
        })
    }
}
