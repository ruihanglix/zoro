// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::path::PathBuf;

/// Resolve the Zoro data directory from CLI arg, env var, or default ~/.zoro.
pub fn resolve_data_dir(arg: Option<&str>) -> PathBuf {
    if let Some(dir) = arg {
        PathBuf::from(dir)
    } else if let Ok(dir) = std::env::var("ZORO_DATA_DIR") {
        PathBuf::from(dir)
    } else {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".zoro")
    }
}
