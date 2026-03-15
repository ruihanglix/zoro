// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use serde::{Deserialize, Serialize};

/// Plugin manifest parsed from manifest.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    pub icon: Option<String>,
    pub min_host_version: Option<String>,
    pub main: String,
    pub style: Option<String>,
    pub sidecar: Option<SidecarConfig>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub contributions: PluginContributions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarConfig {
    pub command: String,
    pub args: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginContributions {
    #[serde(default)]
    pub reader_sidebar_tabs: Vec<ContributionItem>,
    #[serde(default)]
    pub reader_toolbar_actions: Vec<ContributionItem>,
    #[serde(default)]
    pub reader_overlays: Vec<OverlayContribution>,
    #[serde(default)]
    pub settings_sections: Vec<ContributionItem>,
    #[serde(default)]
    pub sidebar_nav_items: Vec<ContributionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContributionItem {
    pub id: String,
    #[serde(rename = "titleKey")]
    pub title_key: String,
    pub icon: String,
    pub component: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayContribution {
    pub id: String,
    pub trigger: String,
    pub component: String,
}

impl PluginManifest {
    /// Parse manifest from JSON string.
    pub fn from_json(json: &str) -> Result<Self, crate::error::PluginsError> {
        serde_json::from_str(json).map_err(crate::error::PluginsError::Json)
    }

    /// Parse manifest from a file path.
    pub fn from_file(path: &std::path::Path) -> Result<Self, crate::error::PluginsError> {
        let content = std::fs::read_to_string(path)?;
        Self::from_json(&content)
    }

    /// Validate the manifest fields.
    pub fn validate(&self) -> Result<(), crate::error::PluginsError> {
        if self.id.is_empty() {
            return Err(crate::error::PluginsError::InvalidManifest(
                "id is required".to_string(),
            ));
        }
        if self.name.is_empty() {
            return Err(crate::error::PluginsError::InvalidManifest(
                "name is required".to_string(),
            ));
        }
        if self.main.is_empty() {
            return Err(crate::error::PluginsError::InvalidManifest(
                "main entry point is required".to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_manifest() {
        let json = r#"{
            "id": "com.example.test",
            "name": "Test Plugin",
            "version": "0.1.0",
            "description": "A test plugin",
            "main": "dist/index.js",
            "permissions": ["paper:read"],
            "contributions": {
                "reader_sidebar_tabs": [{
                    "id": "test-tab",
                    "titleKey": "testTab",
                    "icon": "FileText",
                    "component": "TestPanel"
                }]
            }
        }"#;
        let manifest = PluginManifest::from_json(json).unwrap();
        assert_eq!(manifest.id, "com.example.test");
        assert_eq!(manifest.contributions.reader_sidebar_tabs.len(), 1);
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_validate_empty_id() {
        let json = r#"{
            "id": "",
            "name": "Test",
            "version": "0.1.0",
            "description": "test",
            "main": "dist/index.js"
        }"#;
        let manifest = PluginManifest::from_json(json).unwrap();
        assert!(manifest.validate().is_err());
    }
}
