// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::fs;
use std::path::Path;
use zoro_core::models::AppConfig;

/// Supported native-language codes (must match the `<select>` options in settings UI).
const SUPPORTED_NATIVE_LANGS: &[&str] = &[
    "zh", "ja", "ko", "es", "fr", "de", "pt", "ru", "ar", "it", "vi", "th", "id", "hi", "en",
];

/// Detect the user's system locale and return a supported native-language code,
/// or an empty string if the locale doesn't match any supported language.
fn detect_system_native_lang() -> String {
    let locale = sys_locale::get_locale().unwrap_or_default();
    if locale.is_empty() {
        return String::new();
    }
    // Locale strings look like "zh-CN", "en-US", "ja", "pt-BR", etc.
    // Extract the primary language subtag (before the first '-' or '_').
    let primary = locale.split(['-', '_']).next().unwrap_or("").to_lowercase();
    if SUPPORTED_NATIVE_LANGS.contains(&primary.as_str()) {
        primary
    } else {
        String::new()
    }
}

/// Load config from config.toml, falling back to defaults on any error.
/// When `native_lang` is empty (not yet configured by the user), it is
/// automatically populated from the system locale so that translation
/// features work out of the box.
pub fn load_config(data_dir: &Path) -> AppConfig {
    let config_path = data_dir.join("config.toml");
    let mut config = match fs::read_to_string(&config_path) {
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
    };

    // Default native_lang to system locale when not explicitly configured.
    if config.general.native_lang.is_empty() {
        config.general.native_lang = detect_system_native_lang();
    }

    config
}

/// Save config to config.toml.
pub fn save_config(data_dir: &Path, config: &AppConfig) -> Result<(), std::io::Error> {
    let config_path = data_dir.join("config.toml");
    let content = toml::to_string_pretty(config)
        .map_err(|e| std::io::Error::other(format!("TOML serialize error: {}", e)))?;
    fs::write(&config_path, content)
}
