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

/// Uninstall the `zoro` CLI command from system PATH.
pub fn uninstall_cli() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        uninstall_unix()?;
    }

    #[cfg(target_os = "windows")]
    {
        uninstall_windows()?;
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

    // Add bin_dir to user PATH if not already present
    let bin_dir_str = bin_dir.to_string_lossy().to_string();
    let current_user_path = get_user_path_windows()?;

    if current_user_path
        .split(';')
        .any(|p| p.eq_ignore_ascii_case(&bin_dir_str))
    {
        println!("  PATH already contains {}", bin_dir_str);
    } else {
        let new_path = if current_user_path.is_empty() {
            bin_dir_str.clone()
        } else {
            format!("{};{}", current_user_path, bin_dir_str)
        };
        set_user_path_windows(&new_path)?;
        println!("✓ Added {} to user PATH", bin_dir_str);
        println!("  Restart your terminal for the change to take effect.");
    }

    println!("  You can now use 'zoro' from anywhere in your terminal.");
    Ok(())
}

/// Read the user-level PATH from the Windows registry.
#[cfg(target_os = "windows")]
fn get_user_path_windows() -> Result<String, Box<dyn std::error::Error>> {
    let output = std::process::Command::new("reg")
        .args([
            "query",
            "HKCU\\Environment",
            "/v",
            "Path",
        ])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse the REG_SZ or REG_EXPAND_SZ value from reg query output.
    // Format: "    Path    REG_EXPAND_SZ    C:\some\path;C:\other\path"
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Path") || trimmed.starts_with("PATH") {
            // Split by whitespace: ["Path", "REG_EXPAND_SZ", "value..."]
            let parts: Vec<&str> = trimmed.splitn(3, char::is_whitespace).collect();
            if parts.len() >= 3 {
                // The value part may have leading whitespace from the split
                let value_part = parts[2..].join(" ");
                // Strip the type prefix (REG_EXPAND_SZ / REG_SZ) if still present
                let value = value_part
                    .trim()
                    .trim_start_matches("REG_EXPAND_SZ")
                    .trim_start_matches("REG_SZ")
                    .trim();
                return Ok(value.to_string());
            }
        }
    }

    // No user PATH set yet — that's fine
    Ok(String::new())
}

/// Write the user-level PATH via the Windows registry.
#[cfg(target_os = "windows")]
fn set_user_path_windows(new_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let status = std::process::Command::new("reg")
        .args([
            "add",
            "HKCU\\Environment",
            "/v",
            "Path",
            "/t",
            "REG_EXPAND_SZ",
            "/d",
            new_path,
            "/f",
        ])
        .status()?;

    if !status.success() {
        return Err("Failed to update user PATH in registry".into());
    }

    // Broadcast WM_SETTINGCHANGE so running Explorer picks up the change
    // without requiring a full reboot. This uses a small PowerShell snippet.
    let _ = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Add-Type -Namespace Win32 -Name NativeMethods -MemberDefinition '[DllImport(\"user32.dll\", SetLastError = true, CharSet = CharSet.Auto)] public static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam, uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);'; $HWND_BROADCAST = [IntPtr]0xffff; $WM_SETTINGCHANGE = 0x1a; $result = [UIntPtr]::Zero; [Win32.NativeMethods]::SendMessageTimeout($HWND_BROADCAST, $WM_SETTINGCHANGE, [UIntPtr]::Zero, 'Environment', 2, 5000, [ref]$result) | Out-Null",
        ])
        .status();

    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn uninstall_unix() -> Result<(), Box<dyn std::error::Error>> {
    let link = PathBuf::from("/usr/local/bin/zoro");

    if link.symlink_metadata().is_ok() {
        std::fs::remove_file(&link).map_err(|e| {
            format!(
                "Failed to remove {}: {}. Try running with sudo.",
                link.display(),
                e
            )
        })?;
        println!("✓ Removed {}", link.display());
    } else {
        println!("Nothing to uninstall: {} does not exist.", link.display());
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn uninstall_windows() -> Result<(), Box<dyn std::error::Error>> {
    let local_app_data = std::env::var("LOCALAPPDATA")
        .map_err(|_| "Could not find LOCALAPPDATA environment variable")?;
    let bin_dir = PathBuf::from(&local_app_data).join("Zoro").join("bin");
    let dest = bin_dir.join("zoro.exe");

    // Remove binary
    if dest.exists() {
        std::fs::remove_file(&dest)?;
        println!("✓ Removed {}", dest.display());
    }

    // Remove bin_dir from user PATH
    let bin_dir_str = bin_dir.to_string_lossy().to_string();
    let current_user_path = get_user_path_windows()?;

    let new_entries: Vec<&str> = current_user_path
        .split(';')
        .filter(|p| !p.eq_ignore_ascii_case(&bin_dir_str))
        .collect();
    let new_path = new_entries.join(";");

    if new_path != current_user_path {
        set_user_path_windows(&new_path)?;
        println!("✓ Removed {} from user PATH", bin_dir_str);
    }

    // Clean up empty Zoro/bin directory
    if bin_dir.exists() && bin_dir.read_dir().map_or(true, |mut d| d.next().is_none()) {
        let _ = std::fs::remove_dir(&bin_dir);
        let zoro_dir = bin_dir.parent().unwrap();
        if zoro_dir.exists() && zoro_dir.read_dir().map_or(true, |mut d| d.next().is_none()) {
            let _ = std::fs::remove_dir(zoro_dir);
        }
    }

    println!("  Restart your terminal for the change to take effect.");
    Ok(())
}
