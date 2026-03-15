// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::path::Path;
use tracing::{debug, info, warn};

use crate::client::WebDavClient;
use crate::error::WebDavError;

/// Root-level files (paper.pdf, paper.html) that should also be synced.
const ROOT_SYNC_FILES: &[&str] = &["paper.pdf", "paper.html"];

/// Synchronize files (notes, attachments, and primary PDF/HTML) for a single paper
/// between the local filesystem and remote WebDAV.
///
/// This handles:
/// - Root-level `paper.pdf` and `paper.html`
/// - `notes/*.md` files
/// - `attachments/` files
///
/// `max_file_size` controls the upper bound in bytes; 0 means unlimited.
///
/// Conflict resolution: last-write-wins by modification time; the losing
/// version is saved as `{name}.conflict-{device_id}.ext`.
pub async fn sync_small_files_for_paper(
    client: &WebDavClient,
    remote_root: &str,
    device_id: &str,
    slug: &str,
    local_paper_dir: &Path,
    max_file_size: u64,
) -> Result<SyncSmallFilesStats, WebDavError> {
    let mut stats = SyncSmallFilesStats::default();
    let remote_paper_dir = format!(
        "{}/zoro/library/papers/{}",
        remote_root.trim_end_matches('/'),
        slug
    );

    // Sync root-level files (paper.pdf, paper.html)
    sync_root_files(
        client,
        device_id,
        local_paper_dir,
        &remote_paper_dir,
        max_file_size,
        &mut stats,
    )
    .await?;

    // Sync notes/ directory
    let local_notes = local_paper_dir.join("notes");
    let remote_notes = format!("{}/notes", remote_paper_dir);
    if local_notes.exists() {
        sync_directory(
            client,
            device_id,
            &local_notes,
            &remote_notes,
            max_file_size,
            &mut stats,
        )
        .await?;
    } else {
        // Local notes dir doesn't exist — download any remote notes
        download_missing_directory(
            client,
            &local_notes,
            &remote_notes,
            max_file_size,
            &mut stats,
        )
        .await?;
    }

    // Sync files in attachments/ directory
    let local_attachments = local_paper_dir.join("attachments");
    let remote_attachments = format!("{}/attachments", remote_paper_dir);
    if local_attachments.exists() {
        sync_directory(
            client,
            device_id,
            &local_attachments,
            &remote_attachments,
            max_file_size,
            &mut stats,
        )
        .await?;
    } else {
        download_missing_directory(
            client,
            &local_attachments,
            &remote_attachments,
            max_file_size,
            &mut stats,
        )
        .await?;
    }

    if stats.uploaded > 0 || stats.downloaded > 0 || stats.conflicts > 0 {
        info!(
            slug,
            uploaded = stats.uploaded,
            downloaded = stats.downloaded,
            conflicts = stats.conflicts,
            "L2 file sync complete for paper"
        );
    }

    Ok(stats)
}

/// Stats from syncing small files.
#[derive(Debug, Default)]
pub struct SyncSmallFilesStats {
    pub uploaded: u32,
    pub downloaded: u32,
    pub conflicts: u32,
    pub skipped: u32,
}

/// Sync root-level files (paper.pdf, paper.html) for a single paper.
/// These are the primary document files that live directly in the paper directory.
async fn sync_root_files(
    client: &WebDavClient,
    device_id: &str,
    local_paper_dir: &Path,
    remote_paper_dir: &str,
    max_file_size: u64,
    stats: &mut SyncSmallFilesStats,
) -> Result<(), WebDavError> {
    // Ensure remote paper directory exists
    let _ = client.mkcol(remote_paper_dir).await;

    // List remote files at the paper root level
    let remote_entries = match client.list(remote_paper_dir).await {
        Ok(entries) => entries,
        Err(WebDavError::NotFound(_)) => {
            let _ = client.mkcol(remote_paper_dir).await;
            Vec::new()
        }
        Err(e) => return Err(e),
    };

    let remote_files: std::collections::HashMap<String, _> = remote_entries
        .into_iter()
        .filter(|e| !e.is_collection)
        .filter_map(|e| {
            let name = e.href.trim_end_matches('/').rsplit('/').next()?.to_string();
            Some((name, e))
        })
        .collect();

    for &filename in ROOT_SYNC_FILES {
        let local_path = local_paper_dir.join(filename);
        let remote_path = format!("{}/{}", remote_paper_dir, filename);

        let local_exists = local_path.exists();
        let remote_exists = remote_files.contains_key(filename);

        match (local_exists, remote_exists) {
            (true, true) => {
                // Both exist — compare by hash
                let local_metadata = match std::fs::metadata(&local_path) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                if max_file_size > 0 && local_metadata.len() > max_file_size {
                    stats.skipped += 1;
                    continue;
                }

                let local_hash = compute_file_md5(&local_path)?;
                let remote_data = client.get(&remote_path).await?;
                let remote_hash = compute_data_md5(&remote_data);

                if local_hash == remote_hash {
                    continue; // Identical
                }

                let remote_entry = &remote_files[filename];
                let local_newer =
                    is_local_newer(&local_metadata, remote_entry.last_modified.as_deref());

                if local_newer {
                    let conflict_name = make_conflict_name(filename, device_id);
                    let conflict_remote = format!("{}/{}", remote_paper_dir, conflict_name);
                    let _ = client.put(&conflict_remote, remote_data).await;

                    let local_data = std::fs::read(&local_path)?;
                    client.put(&remote_path, local_data).await?;
                    stats.uploaded += 1;
                    stats.conflicts += 1;
                } else {
                    let conflict_name = make_conflict_name(filename, device_id);
                    let conflict_path = local_paper_dir.join(&conflict_name);
                    let _ = std::fs::copy(&local_path, &conflict_path);

                    std::fs::write(&local_path, &remote_data)?;
                    stats.downloaded += 1;
                    stats.conflicts += 1;
                }
            }
            (true, false) => {
                // Local only — upload
                let metadata = match std::fs::metadata(&local_path) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                if max_file_size > 0 && metadata.len() > max_file_size {
                    stats.skipped += 1;
                    continue;
                }
                let local_data = std::fs::read(&local_path)?;
                if let Err(e) = client.put(&remote_path, local_data).await {
                    warn!(filename, error = %e, "Failed to upload root file");
                } else {
                    stats.uploaded += 1;
                }
            }
            (false, true) => {
                // Remote only — download
                match client.get(&remote_path).await {
                    Ok(data) => {
                        if max_file_size > 0 && data.len() as u64 > max_file_size {
                            stats.skipped += 1;
                            continue;
                        }
                        std::fs::write(&local_path, &data)?;
                        stats.downloaded += 1;
                        debug!(filename, "Downloaded root file from remote");
                    }
                    Err(e) => {
                        warn!(filename, error = %e, "Failed to download root file");
                    }
                }
            }
            (false, false) => {
                // Neither exists — nothing to do
            }
        }
    }

    Ok(())
}

/// Sync a local directory against a remote WebDAV directory.
/// Files are compared by MD5 hash; conflicts use last-write-wins.
async fn sync_directory(
    client: &WebDavClient,
    device_id: &str,
    local_dir: &Path,
    remote_dir: &str,
    max_file_size: u64,
    stats: &mut SyncSmallFilesStats,
) -> Result<(), WebDavError> {
    // Ensure remote directory exists
    let _ = client.mkcol(remote_dir).await;

    // List remote files
    let remote_entries = match client.list(remote_dir).await {
        Ok(entries) => entries,
        Err(WebDavError::NotFound(_)) => {
            let _ = client.mkcol(remote_dir).await;
            Vec::new()
        }
        Err(e) => return Err(e),
    };

    let remote_files: std::collections::HashMap<String, _> = remote_entries
        .into_iter()
        .filter(|e| !e.is_collection)
        .filter_map(|e| {
            let name = e.href.trim_end_matches('/').rsplit('/').next()?.to_string();
            // Skip conflict files
            if name.contains(".conflict-") {
                return None;
            }
            Some((name, e))
        })
        .collect();

    // Scan local files
    let local_entries = match std::fs::read_dir(local_dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };

    let mut seen_remote: std::collections::HashSet<String> = std::collections::HashSet::new();

    for entry in local_entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        // Skip conflict files and files too large for L2 sync
        if filename.contains(".conflict-") {
            continue;
        }
        let metadata = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if max_file_size > 0 && metadata.len() > max_file_size {
            stats.skipped += 1;
            continue;
        }

        seen_remote.insert(filename.clone());
        let remote_path = format!("{}/{}", remote_dir, filename);

        if let Some(remote_entry) = remote_files.get(&filename) {
            // File exists both locally and remotely — compare by hash
            let local_hash = compute_file_md5(&path)?;
            let remote_data = client.get(&remote_path).await?;
            let remote_hash = compute_data_md5(&remote_data);

            if local_hash == remote_hash {
                // Identical, skip
                continue;
            }

            // Conflict: use local modification time vs remote last-modified
            // We use last-write-wins; assume local is newer if remote has no date
            let local_newer = is_local_newer(&metadata, remote_entry.last_modified.as_deref());

            if local_newer {
                // Upload local, backup remote as conflict file
                let conflict_name = make_conflict_name(&filename, device_id);
                let conflict_remote = format!("{}/{}", remote_dir, conflict_name);
                let _ = client.put(&conflict_remote, remote_data).await;

                let local_data = std::fs::read(&path)?;
                client.put(&remote_path, local_data).await?;
                stats.uploaded += 1;
                stats.conflicts += 1;
            } else {
                // Download remote, backup local as conflict file
                let conflict_name = make_conflict_name(&filename, device_id);
                let conflict_path = local_dir.join(&conflict_name);
                let _ = std::fs::copy(&path, &conflict_path);

                std::fs::write(&path, &remote_data)?;
                stats.downloaded += 1;
                stats.conflicts += 1;
            }
        } else {
            // File exists locally but not remotely — upload
            let local_data = std::fs::read(&path)?;
            if let Err(e) = client.put(&remote_path, local_data).await {
                warn!(filename, error = %e, "Failed to upload file");
            } else {
                stats.uploaded += 1;
            }
        }
    }

    // Download files that exist remotely but not locally
    for filename in remote_files.keys() {
        if seen_remote.contains(filename) {
            continue;
        }

        let remote_path = format!("{}/{}", remote_dir, filename);
        let local_path = local_dir.join(filename);

        match client.get(&remote_path).await {
            Ok(data) => {
                if max_file_size > 0 && data.len() as u64 > max_file_size {
                    stats.skipped += 1;
                    continue;
                }
                std::fs::create_dir_all(local_dir)?;
                std::fs::write(&local_path, &data)?;
                stats.downloaded += 1;
                debug!(filename, "Downloaded remote file");
            }
            Err(e) => {
                warn!(filename, error = %e, "Failed to download remote file");
            }
        }
    }

    Ok(())
}

/// Download all files from a remote directory to a local directory that
/// doesn't exist yet.
async fn download_missing_directory(
    client: &WebDavClient,
    local_dir: &Path,
    remote_dir: &str,
    max_file_size: u64,
    stats: &mut SyncSmallFilesStats,
) -> Result<(), WebDavError> {
    let remote_entries = match client.list(remote_dir).await {
        Ok(entries) => entries,
        Err(WebDavError::NotFound(_)) => return Ok(()),
        Err(e) => return Err(e),
    };

    for entry in &remote_entries {
        if entry.is_collection {
            continue;
        }
        let filename = match entry.href.trim_end_matches('/').rsplit('/').next() {
            Some(n) => n.to_string(),
            None => continue,
        };
        if filename.contains(".conflict-") {
            continue;
        }

        let remote_path = format!("{}/{}", remote_dir, filename);
        match client.get(&remote_path).await {
            Ok(data) => {
                if max_file_size > 0 && data.len() as u64 > max_file_size {
                    stats.skipped += 1;
                    continue;
                }
                std::fs::create_dir_all(local_dir)?;
                std::fs::write(local_dir.join(&filename), &data)?;
                stats.downloaded += 1;
            }
            Err(e) => {
                warn!(filename, error = %e, "Failed to download file");
            }
        }
    }
    Ok(())
}

/// Compute MD5 hash of a file on disk.
fn compute_file_md5(path: &Path) -> Result<String, WebDavError> {
    use md5::{Digest, Md5};
    let data = std::fs::read(path)?;
    let hash = Md5::digest(&data);
    Ok(format!("{:x}", hash))
}

/// Compute MD5 hash of in-memory data.
fn compute_data_md5(data: &[u8]) -> String {
    use md5::{Digest, Md5};
    let hash = Md5::digest(data);
    format!("{:x}", hash)
}

/// Determine if the local file is newer than the remote file.
fn is_local_newer(local_metadata: &std::fs::Metadata, remote_last_modified: Option<&str>) -> bool {
    let local_modified = match local_metadata.modified() {
        Ok(t) => t,
        Err(_) => return true, // If we can't read local time, assume local is newer
    };

    let remote_time = match remote_last_modified {
        Some(s) => {
            // WebDAV dates are typically in RFC 2822 format
            chrono::DateTime::parse_from_rfc2822(s)
                .or_else(|_| chrono::DateTime::parse_from_rfc3339(s))
                .ok()
        }
        None => return true,
    };

    match remote_time {
        Some(rt) => {
            let local_dt: chrono::DateTime<chrono::Utc> = local_modified.into();
            local_dt > rt.with_timezone(&chrono::Utc)
        }
        None => true,
    }
}

/// Generate a conflict backup filename.
/// e.g. "notes.md" -> "notes.conflict-device123.md"
fn make_conflict_name(filename: &str, device_id: &str) -> String {
    let short_id = if device_id.len() > 8 {
        &device_id[..8]
    } else {
        device_id
    };

    if let Some(dot_pos) = filename.rfind('.') {
        let (name, ext) = filename.split_at(dot_pos);
        format!("{}.conflict-{}{}", name, short_id, ext)
    } else {
        format!("{}.conflict-{}", filename, short_id)
    }
}

/// Synchronize all small files for all papers that have local directories.
pub async fn sync_all_small_files(
    client: &WebDavClient,
    remote_root: &str,
    device_id: &str,
    data_dir: &Path,
    max_file_size: u64,
) -> Result<SyncSmallFilesStats, WebDavError> {
    let papers_dir = data_dir.join("library/papers");
    let mut total_stats = SyncSmallFilesStats::default();

    if !papers_dir.exists() {
        return Ok(total_stats);
    }

    let entries = match std::fs::read_dir(&papers_dir) {
        Ok(e) => e,
        Err(_) => return Ok(total_stats),
    };

    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let slug = match entry.file_name().into_string() {
            Ok(s) => s,
            Err(_) => continue,
        };

        let paper_stats = sync_small_files_for_paper(
            client,
            remote_root,
            device_id,
            &slug,
            &entry.path(),
            max_file_size,
        )
        .await?;

        total_stats.uploaded += paper_stats.uploaded;
        total_stats.downloaded += paper_stats.downloaded;
        total_stats.conflicts += paper_stats.conflicts;
        total_stats.skipped += paper_stats.skipped;
    }

    if total_stats.uploaded > 0 || total_stats.downloaded > 0 {
        info!(
            uploaded = total_stats.uploaded,
            downloaded = total_stats.downloaded,
            conflicts = total_stats.conflicts,
            "L2 small file sync complete"
        );
    }

    Ok(total_stats)
}
