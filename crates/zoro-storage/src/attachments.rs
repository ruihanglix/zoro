// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::fs;
use std::path::Path;

/// Download a file from URL and save to the paper directory.
pub async fn download_file(
    url: &str,
    save_path: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header("User-Agent", "Zoro/0.1.0")
        .send()
        .await?;

    let bytes = response.bytes().await?;
    fs::write(save_path, &bytes)?;
    tracing::info!("Downloaded {} bytes to {:?}", bytes.len(), save_path);
    Ok(())
}

/// Get the size of a file in bytes.
pub fn get_file_size(path: &Path) -> Option<i64> {
    fs::metadata(path).ok().map(|m| m.len() as i64)
}
