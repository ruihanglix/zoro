// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

pub mod attachments;
pub mod paper_dir;
pub mod sync;

use std::fs;
use std::path::Path;

/// Initialize the data directory structure.
pub fn init_data_dir(data_dir: &Path) -> Result<(), std::io::Error> {
    fs::create_dir_all(data_dir.join("library/papers"))?;
    fs::create_dir_all(data_dir.join("library/collections"))?;
    fs::create_dir_all(data_dir.join("subscriptions/cache"))?;
    fs::create_dir_all(data_dir.join("cache/thumbnails"))?;
    fs::create_dir_all(data_dir.join("exports"))?;
    fs::create_dir_all(data_dir.join("logs"))?;

    let config_path = data_dir.join("config.toml");
    if !config_path.exists() {
        let default_config = r#"[general]
data_dir = "~/.zoro"
language = "en"

[connector]
port = 23120
enabled = true
zotero_compat_enabled = true
zotero_compat_port = 23119

[subscriptions]
poll_interval_minutes = 60

[ai]
provider = ""
api_key = ""

[ai.task_model_defaults]
quick_translation = ""
normal_translation = ""
heavy_translation = ""
glossary_extraction = ""

[sync]
enabled = false
url = ""
username = ""
password = ""
remote_path = "/"
interval_minutes = 5
device_id = ""
device_name = ""
"#;
        fs::write(&config_path, default_config)?;
    }

    Ok(())
}
