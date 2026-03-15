// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::fs;
use std::path::{Path, PathBuf};
use zoro_core::models::PaperMetadata;

/// Create the directory structure for a paper.
pub fn create_paper_dir(papers_dir: &Path, slug: &str) -> Result<PathBuf, std::io::Error> {
    let paper_dir = papers_dir.join(slug);
    fs::create_dir_all(paper_dir.join("attachments"))?;
    fs::create_dir_all(paper_dir.join("notes"))?;
    Ok(paper_dir)
}

/// Write metadata.json to a paper directory.
pub fn write_metadata(
    paper_dir: &Path,
    metadata: &PaperMetadata,
) -> Result<(), Box<dyn std::error::Error>> {
    let metadata_path = paper_dir.join("metadata.json");
    let json = serde_json::to_string_pretty(metadata)?;
    fs::write(metadata_path, json)?;
    Ok(())
}

/// Read metadata.json from a paper directory.
#[allow(dead_code)]
pub fn read_metadata(paper_dir: &Path) -> Result<PaperMetadata, Box<dyn std::error::Error>> {
    let metadata_path = paper_dir.join("metadata.json");
    let json = fs::read_to_string(metadata_path)?;
    let metadata: PaperMetadata = serde_json::from_str(&json)?;
    Ok(metadata)
}

/// Delete a paper directory entirely.
pub fn delete_paper_dir(papers_dir: &Path, slug: &str) -> Result<(), std::io::Error> {
    let paper_dir = papers_dir.join(slug);
    if paper_dir.exists() {
        fs::remove_dir_all(paper_dir)?;
    }
    Ok(())
}

/// List all paper directories.
#[allow(dead_code)]
pub fn list_paper_dirs(papers_dir: &Path) -> Result<Vec<String>, std::io::Error> {
    let mut slugs = Vec::new();
    if papers_dir.exists() {
        for entry in fs::read_dir(papers_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    slugs.push(name.to_string());
                }
            }
        }
    }
    Ok(slugs)
}
