// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::path::PathBuf;

#[derive(Debug, serde::Serialize)]
pub struct CliStatus {
    pub installed: bool,
    pub path: Option<String>,
    pub sidecar_found: bool,
}

/// Find the bundled `zoro` CLI sidecar binary (same logic as find_mcp_binary).
fn find_cli_sidecar() -> Option<PathBuf> {
    let name = if cfg!(windows) { "zoro.exe" } else { "zoro" };

    // 1. Next to the current executable (production bundles)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(name);
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
                    let candidate = d.join(profile).join(name);
                    if candidate.exists() {
                        return Some(candidate);
                    }
                }
            }
            dir = d.parent();
        }
    }

    None
}

/// Check if `zoro` is available on the system PATH.
fn find_installed_cli() -> Option<PathBuf> {
    which::which("zoro").ok()
}

#[tauri::command]
pub async fn check_cli_installed() -> Result<CliStatus, String> {
    let installed_path = find_installed_cli();
    let sidecar = find_cli_sidecar();

    Ok(CliStatus {
        installed: installed_path.is_some(),
        path: installed_path.map(|p| p.to_string_lossy().to_string()),
        sidecar_found: sidecar.is_some(),
    })
}

#[tauri::command]
pub async fn install_cli() -> Result<CliStatus, String> {
    let sidecar = find_cli_sidecar()
        .ok_or_else(|| "CLI sidecar binary (zoro) not found next to the application executable. Try rebuilding with `cargo build -p zoro-cli`.".to_string())?;

    let output = std::process::Command::new(&sidecar)
        .arg("install-cli")
        .output()
        .map_err(|e| format!("Failed to run install-cli: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "install-cli failed: {}{}",
            stdout.trim(),
            stderr.trim()
        ));
    }

    tracing::info!("CLI installed via sidecar");

    // Return updated status
    let installed_path = find_installed_cli();
    Ok(CliStatus {
        installed: installed_path.is_some(),
        path: installed_path.map(|p| p.to_string_lossy().to_string()),
        sidecar_found: true,
    })
}

#[tauri::command]
pub async fn uninstall_cli() -> Result<CliStatus, String> {
    // Try using the sidecar binary to uninstall
    if let Some(sidecar) = find_cli_sidecar() {
        let output = std::process::Command::new(&sidecar)
            .arg("uninstall-cli")
            .output()
            .map_err(|e| format!("Failed to run uninstall-cli: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(format!(
                "uninstall-cli failed: {}{}",
                stdout.trim(),
                stderr.trim()
            ));
        }

        tracing::info!("CLI uninstalled via sidecar");
    } else {
        // Fallback: try removing the symlink directly (Unix)
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            let link = PathBuf::from("/usr/local/bin/zoro");
            if link.symlink_metadata().is_ok() {
                std::fs::remove_file(&link)
                    .map_err(|e| format!("Failed to remove {}: {}", link.display(), e))?;
                tracing::info!("CLI symlink removed directly");
            }
        }

        #[cfg(target_os = "windows")]
        {
            return Err("CLI sidecar not found, cannot uninstall".to_string());
        }
    }

    let installed_path = find_installed_cli();
    Ok(CliStatus {
        installed: installed_path.is_some(),
        path: installed_path.map(|p| p.to_string_lossy().to_string()),
        sidecar_found: find_cli_sidecar().is_some(),
    })
}
