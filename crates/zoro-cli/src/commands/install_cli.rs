// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::path::PathBuf;

/// Install the `zoro` CLI binary to system PATH by creating a symlink.
pub fn install_cli() -> Result<(), Box<dyn std::error::Error>> {
    let current_exe = std::env::current_exe()?;

    println!("Installing zoro CLI from: {}", current_exe.display());

    #[cfg(target_os = "macos")]
    {
        install_unix(&current_exe)?;
    }

    #[cfg(target_os = "linux")]
    {
        install_unix(&current_exe)?;
    }

    #[cfg(target_os = "windows")]
    {
        install_windows(&current_exe)?;
    }

    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn install_unix(current_exe: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let link = PathBuf::from("/usr/local/bin/zoro");

    // Remove existing symlink if present
    if link.exists() || link.symlink_metadata().is_ok() {
        std::fs::remove_file(&link).map_err(|e| {
            format!(
                "Failed to remove existing {}: {}. Try running with sudo.",
                link.display(),
                e
            )
        })?;
    }

    std::os::unix::fs::symlink(current_exe, &link).map_err(|e| {
        format!(
            "Failed to create symlink at {}: {}. Try running with sudo.",
            link.display(),
            e
        )
    })?;

    println!("✓ Installed: {} -> {}", link.display(), current_exe.display());
    println!("  You can now use 'zoro' from anywhere in your terminal.");
    Ok(())
}

#[cfg(target_os = "windows")]
fn install_windows(current_exe: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // Copy to %LOCALAPPDATA%/Zoro/bin/zoro.exe
    let local_app_data = std::env::var("LOCALAPPDATA")
        .map_err(|_| "Could not find LOCALAPPDATA environment variable")?;
    let bin_dir = PathBuf::from(&local_app_data).join("Zoro").join("bin");
    std::fs::create_dir_all(&bin_dir)?;

    let dest = bin_dir.join("zoro.exe");
    std::fs::copy(current_exe, &dest)?;

    println!("✓ Copied zoro.exe to: {}", dest.display());
    println!(
        "  Add {} to your PATH if not already present:",
        bin_dir.display()
    );
    println!(
        "  [Environment]::SetEnvironmentVariable('PATH', $env:PATH + ';{}', 'User')",
        bin_dir.display()
    );
    Ok(())
}
