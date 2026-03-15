// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a WebDAV resource entry from PROPFIND response
#[derive(Debug, Clone)]
pub struct DavResource {
    pub href: String,
    pub is_collection: bool,
    pub content_length: Option<u64>,
    pub last_modified: Option<String>,
    pub etag: Option<String>,
    pub content_type: Option<String>,
}

/// Download progress callback info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub slug: String,
    pub filename: String,
    pub downloaded: u64,
    pub total: Option<u64>,
}

/// Upload progress callback info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadProgress {
    pub slug: String,
    pub filename: String,
    pub uploaded: u64,
    pub total: u64,
}

/// Sync state stored on the remote WebDAV server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteSyncState {
    pub version: u32,
    pub devices: HashMap<String, DeviceSyncInfo>,
    /// Optimistic lock: incremented on each write
    pub lock_version: u64,
}

impl Default for RemoteSyncState {
    fn default() -> Self {
        Self {
            version: 1,
            devices: HashMap::new(),
            lock_version: 0,
        }
    }
}

/// Per-device sync info stored in sync-state.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSyncInfo {
    pub device_id: String,
    pub device_name: String,
    pub last_sync_time: Option<String>,
    pub last_sequence: u64,
}

/// A single changelog entry that is synced between devices
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogEntry {
    pub id: String,
    pub sequence: u64,
    pub device_id: String,
    pub entity_type: EntityType,
    pub entity_id: String,
    pub operation: Operation,
    pub field_changes: Option<HashMap<String, FieldChange>>,
    pub file_info: Option<FileChangeInfo>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Paper,
    Collection,
    Tag,
    Note,
    Attachment,
    Annotation,
    PaperCollection,
    PaperTag,
    ReaderState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    Create,
    Update,
    Delete,
}

/// Describes a field-level change for merge resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldChange {
    pub old_value: Option<serde_json::Value>,
    pub new_value: serde_json::Value,
    /// Timestamp of when this field was changed — used for conflict resolution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_date: Option<String>,
}

/// Describes a file-level change (for attachments, PDFs, notes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangeInfo {
    pub paper_slug: String,
    pub filename: String,
    pub file_hash: Option<String>,
    pub file_size: Option<u64>,
}

/// Sync status for frontend display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    pub enabled: bool,
    pub syncing: bool,
    pub last_sync_time: Option<String>,
    pub last_error: Option<String>,
    pub progress: Option<SyncProgress>,
    pub devices: Vec<DeviceSyncInfo>,
}

/// Sync progress info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProgress {
    pub phase: String,
    pub current: u64,
    pub total: u64,
    pub message: String,
}
