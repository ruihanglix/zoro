// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

pub mod error;
pub mod manifest;
pub mod registry;

pub use error::PluginsError;
pub use manifest::{
    ContributionItem, OverlayContribution, PluginContributions, PluginManifest, SidecarConfig,
};
pub use registry::{PluginEntry, PluginInfo, PluginMode, PluginRegistry, PluginsConfig};
