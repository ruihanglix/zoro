// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::path::PathBuf;

/// Find a bundled sidecar binary by name.
///
/// Lookup order:
/// 1. Next to the current executable (production bundles).
/// 2. Cargo workspace `target/{debug,release}/` directories (dev mode).
/// 3. Optionally, resolve via system PATH (`check_path = true`).
pub fn find_sidecar_binary(name: &str, check_path: bool) -> Option<PathBuf> {
    let name = if cfg!(windows) && !name.ends_with(".exe") {
        format!("{}.exe", name)
    } else {
        name.to_string()
    };

    // 1. Next to the current executable (production bundles)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(&name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    // 2. Cargo workspace target directories (dev mode)
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent();
        while let Some(d) = dir {
            if d.file_name().is_some_and(|n| n == "target") {
                for profile in &["debug", "release"] {
                    let candidate = d.join(profile).join(&name);
                    if candidate.exists() {
                        return Some(candidate);
                    }
                }
            }
            dir = d.parent();
        }
    }

    // 3. System PATH (optional)
    if check_path {
        if let Ok(path) = which::which(&name) {
            return Some(path);
        }
    }

    None
}
