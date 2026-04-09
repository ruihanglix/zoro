// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::path::PathBuf;

use crate::sidecar::find_sidecar_binary;

#[derive(Debug, serde::Serialize)]
pub struct CliStatus {
    pub installed: bool,
    pub path: Option<String>,
    pub sidecar_found: bool,
}

/// Find the bundled `zoro` CLI sidecar binary.
fn find_cli_sidecar() -> Option<PathBuf> {
    find_sidecar_binary("zoro", false)
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
