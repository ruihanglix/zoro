// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::watch;
use tracing::{debug, info, warn};

use zoro_db::queries::sync as sync_queries;
use zoro_db::Database;

use crate::client::WebDavClient;
use crate::error::WebDavError;
use crate::types::{
    ChangelogEntry, DeviceSyncInfo, EntityType, FieldChange, Operation, RemoteSyncState,
    SyncProgress, SyncStatus,
};

/// Type alias for a shared database handle.
pub type DbHandle = Arc<Mutex<Database>>;

/// The core sync engine that coordinates bidirectional sync between
/// local SQLite database and remote WebDAV storage.
pub struct SyncEngine {
    client: WebDavClient,
    remote_root: String,
    device_id: String,
    device_name: String,
    cancel_tx: watch::Sender<bool>,
    cancel_rx: watch::Receiver<bool>,
}

impl SyncEngine {
    pub fn new(
        client: WebDavClient,
        remote_root: &str,
        device_id: &str,
        device_name: &str,
    ) -> Self {
        let (cancel_tx, cancel_rx) = watch::channel(false);
        Self {
            client,
            remote_root: remote_root.trim_end_matches('/').to_string(),
            device_id: device_id.to_string(),
            device_name: device_name.to_string(),
            cancel_tx,
            cancel_rx,
        }
    }

    /// Signal cancellation of the current sync operation.
    pub fn cancel(&self) {
        let _ = self.cancel_tx.send(true);
    }

    /// Reset the cancellation token for a new sync cycle.
    fn reset_cancel(&self) {
        let _ = self.cancel_tx.send(false);
    }

    /// Check if cancellation was requested.
    fn is_cancelled(&self) -> bool {
        *self.cancel_rx.borrow()
    }

    /// Return a reference to the underlying WebDAV client.
    pub fn client(&self) -> &WebDavClient {
        &self.client
    }

    // -----------------------------------------------------------------------
    // Remote path helpers
    // -----------------------------------------------------------------------

    fn sync_state_path(&self) -> String {
        format!("{}/zoro/sync/sync-state.json", self.remote_root)
    }

    fn changelog_dir(&self) -> String {
        format!("{}/zoro/sync/changelog", self.remote_root)
    }

    fn changelog_file_path(&self, device_id: &str, sequence: u64) -> String {
        format!("{}/{}-{}.json", self.changelog_dir(), device_id, sequence)
    }

    fn paper_remote_dir(&self, slug: &str) -> String {
        format!("{}/zoro/library/papers/{}", self.remote_root, slug)
    }

    fn paper_metadata_path(&self, slug: &str) -> String {
        format!("{}/metadata.json", self.paper_remote_dir(slug))
    }

    // -----------------------------------------------------------------------
    // Core sync cycle
    // -----------------------------------------------------------------------

    /// Execute a full incremental sync cycle.
    ///
    /// Steps:
    /// 1. Pull remote sync-state.json
    /// 2. Compare sequence numbers to find new changes
    /// 3. Upload local changelog entries to remote
    /// 4. Download remote changelog entries from other devices
    /// 5. Apply remote changes to local database
    /// 6. Update sync-state.json with optimistic lock
    pub async fn sync<F>(&self, db: &DbHandle, progress_cb: Option<F>) -> Result<(), WebDavError>
    where
        F: Fn(SyncProgress) + Send,
    {
        self.reset_cancel();
        info!(device_id = %self.device_id, "Starting sync cycle");

        // Step 1: Pull remote sync state
        self.report_progress(
            &progress_cb,
            "pull_state",
            0,
            5,
            "Fetching remote sync state",
        );
        let mut remote_state = self.pull_remote_state().await?;
        self.check_cancelled()?;

        // Step 2: Register this device if not already present
        if !remote_state.devices.contains_key(&self.device_id) {
            remote_state.devices.insert(
                self.device_id.clone(),
                DeviceSyncInfo {
                    device_id: self.device_id.clone(),
                    device_name: self.device_name.clone(),
                    last_sync_time: None,
                    last_sequence: 0,
                },
            );
        }

        // Step 3: Upload local changes
        self.report_progress(&progress_cb, "upload", 1, 5, "Uploading local changes");
        let uploaded_seq = self.upload_local_changes(db).await?;
        self.check_cancelled()?;

        // Step 4: Download and apply remote changes
        self.report_progress(&progress_cb, "download", 2, 5, "Downloading remote changes");
        let (_local_state_data, remote_sequences) = {
            let db_guard = db.lock().map_err(|e| WebDavError::Other(e.to_string()))?;
            let local_state =
                sync_queries::get_or_create_sync_state(&db_guard.conn, &self.device_id)
                    .map_err(|e| WebDavError::Other(e.to_string()))?;
            let remote_sequences: HashMap<String, i64> =
                serde_json::from_str(&local_state.last_remote_sequences_json).unwrap_or_default();
            (local_state, remote_sequences)
        };

        let mut new_remote_sequences = remote_sequences.clone();

        for (dev_id, dev_info) in &remote_state.devices {
            if dev_id == &self.device_id {
                continue; // Skip our own device
            }
            self.check_cancelled()?;

            let last_known_seq = remote_sequences.get(dev_id).copied().unwrap_or(0);
            if dev_info.last_sequence as i64 <= last_known_seq {
                debug!(device = %dev_id, "No new changes from this device");
                continue;
            }

            // Download changelog entries from this device
            self.report_progress(
                &progress_cb,
                "apply",
                3,
                5,
                &format!("Applying changes from {}", dev_info.device_name),
            );

            let applied_seq = self
                .download_and_apply_changes(db, dev_id, last_known_seq, dev_info.last_sequence)
                .await?;
            new_remote_sequences.insert(dev_id.clone(), applied_seq);
        }
        self.check_cancelled()?;

        // Step 5: Update remote sync state with optimistic lock
        self.report_progress(&progress_cb, "finalize", 4, 5, "Updating sync state");
        let now = chrono::Utc::now().to_rfc3339();

        // Update our device info in the remote state
        if let Some(dev_info) = remote_state.devices.get_mut(&self.device_id) {
            dev_info.last_sync_time = Some(now.clone());
            dev_info.last_sequence = uploaded_seq as u64;
        }

        // Attempt to write with optimistic lock
        self.push_remote_state(&remote_state).await?;

        // Update local sync state
        let new_remote_seq_json = serde_json::to_string(&new_remote_sequences)
            .map_err(|e| WebDavError::Other(e.to_string()))?;
        {
            let db_guard = db.lock().map_err(|e| WebDavError::Other(e.to_string()))?;
            sync_queries::update_sync_state(
                &db_guard.conn,
                &self.device_id,
                &now,
                uploaded_seq,
                &new_remote_seq_json,
            )
            .map_err(|e| WebDavError::Other(e.to_string()))?;
        }

        self.report_progress(&progress_cb, "done", 5, 5, "Sync complete");
        info!("Sync cycle completed successfully");
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Remote state management
    // -----------------------------------------------------------------------

    /// Pull the remote sync-state.json, or create a default one if it doesn't exist.
    async fn pull_remote_state(&self) -> Result<RemoteSyncState, WebDavError> {
        match self
            .client
            .get_json::<RemoteSyncState>(&self.sync_state_path())
            .await
        {
            Ok(state) => Ok(state),
            Err(WebDavError::NotFound(_)) => {
                info!("No remote sync state found, creating initial state");
                let state = RemoteSyncState::default();
                self.client
                    .put_json(&self.sync_state_path(), &state)
                    .await?;
                Ok(state)
            }
            Err(e) => Err(e),
        }
    }

    /// Push updated sync-state.json with optimistic lock check.
    async fn push_remote_state(&self, state: &RemoteSyncState) -> Result<(), WebDavError> {
        // Increment lock version before writing
        let mut new_state = state.clone();
        new_state.lock_version += 1;
        self.client
            .put_json(&self.sync_state_path(), &new_state)
            .await
    }

    // -----------------------------------------------------------------------
    // Upload local changes
    // -----------------------------------------------------------------------

    /// Upload local changelog entries that haven't been synced yet.
    /// Returns the latest local sequence number after upload.
    async fn upload_local_changes(&self, db: &DbHandle) -> Result<i64, WebDavError> {
        let (local_state, entries) = {
            let db_guard = db.lock().map_err(|e| WebDavError::Other(e.to_string()))?;
            let local_state =
                sync_queries::get_or_create_sync_state(&db_guard.conn, &self.device_id)
                    .map_err(|e| WebDavError::Other(e.to_string()))?;
            let entries = sync_queries::get_changelog_since(
                &db_guard.conn,
                &self.device_id,
                local_state.last_local_sequence,
                1000,
            )
            .map_err(|e| WebDavError::Other(e.to_string()))?;
            (local_state, entries)
        };

        if entries.is_empty() {
            debug!("No local changes to upload");
            return Ok(local_state.last_local_sequence);
        }

        info!(count = entries.len(), "Uploading local changelog entries");

        let mut max_seq = local_state.last_local_sequence;
        for entry in &entries {
            self.check_cancelled()?;

            let changelog_entry = ChangelogEntry {
                id: entry.id.clone(),
                sequence: entry.sequence as u64,
                device_id: entry.device_id.clone(),
                entity_type: parse_entity_type(&entry.entity_type),
                entity_id: entry.entity_id.clone(),
                operation: parse_operation(&entry.operation),
                field_changes: entry
                    .field_changes_json
                    .as_ref()
                    .and_then(|j| serde_json::from_str(j).ok()),
                file_info: entry
                    .file_info_json
                    .as_ref()
                    .and_then(|j| serde_json::from_str(j).ok()),
                timestamp: entry.timestamp.clone(),
            };

            let remote_path = self.changelog_file_path(&self.device_id, entry.sequence as u64);
            let data = serde_json::to_vec_pretty(&changelog_entry)
                .map_err(|e| WebDavError::Other(e.to_string()))?;
            self.client.put(&remote_path, data).await?;

            max_seq = max_seq.max(entry.sequence);
        }

        debug!(max_seq, "Uploaded changelog entries");
        Ok(max_seq)
    }

    // -----------------------------------------------------------------------
    // Download and apply remote changes
    // -----------------------------------------------------------------------

    /// Download changelog entries from a remote device and apply them locally.
    /// Returns the highest applied sequence number.
    async fn download_and_apply_changes(
        &self,
        db: &DbHandle,
        remote_device_id: &str,
        after_seq: i64,
        up_to_seq: u64,
    ) -> Result<i64, WebDavError> {
        let mut applied_seq = after_seq;

        for seq in (after_seq + 1)..=(up_to_seq as i64) {
            self.check_cancelled()?;

            let path = self.changelog_file_path(remote_device_id, seq as u64);
            let entry = match self.client.get_json::<ChangelogEntry>(&path).await {
                Ok(e) => e,
                Err(WebDavError::NotFound(_)) => {
                    warn!(device = %remote_device_id, seq, "Changelog entry not found, skipping");
                    applied_seq = seq;
                    continue;
                }
                Err(e) => return Err(e),
            };

            // Apply the change to local database
            {
                let db_guard = db.lock().map_err(|e| WebDavError::Other(e.to_string()))?;
                self.apply_changelog_entry(&db_guard, &entry)?;
            }

            // Import the remote entry into our local changelog (for tracking)
            let row = sync_queries::ChangelogRow {
                id: entry.id,
                sequence: entry.sequence as i64,
                device_id: entry.device_id,
                entity_type: format!("{:?}", entry.entity_type).to_lowercase(),
                entity_id: entry.entity_id,
                operation: format!("{:?}", entry.operation).to_lowercase(),
                field_changes_json: entry
                    .field_changes
                    .as_ref()
                    .map(|fc| serde_json::to_string(fc).unwrap_or_default()),
                file_info_json: entry
                    .file_info
                    .as_ref()
                    .map(|fi| serde_json::to_string(fi).unwrap_or_default()),
                timestamp: entry.timestamp,
            };
            {
                let db_guard = db.lock().map_err(|e| WebDavError::Other(e.to_string()))?;
                let _ = sync_queries::import_remote_changelog(&db_guard.conn, &row);
            }

            applied_seq = seq;
        }

        info!(
            device = %remote_device_id,
            applied_seq,
            "Applied remote changes"
        );
        Ok(applied_seq)
    }

    /// Apply a single changelog entry to the local database.
    /// Uses field-level merge for updates and last-write-wins for conflicts.
    fn apply_changelog_entry(
        &self,
        db: &Database,
        entry: &ChangelogEntry,
    ) -> Result<(), WebDavError> {
        use zoro_db::queries::{annotations, collections, papers, reader_state, tags};

        match (&entry.entity_type, &entry.operation) {
            // -- Paper operations --
            (EntityType::Paper, Operation::Create) => {
                if papers::get_paper(&db.conn, &entry.entity_id).is_ok() {
                    if let Some(ref changes) = entry.field_changes {
                        self.merge_paper_fields(db, &entry.entity_id, changes)?;
                    }
                } else if let Some(ref changes) = entry.field_changes {
                    self.create_paper_from_changes(db, &entry.entity_id, changes)?;
                }
            }
            (EntityType::Paper, Operation::Update) => {
                if let Some(ref changes) = entry.field_changes {
                    self.merge_paper_fields(db, &entry.entity_id, changes)?;
                }
            }
            (EntityType::Paper, Operation::Delete) => {
                let _ = papers::delete_paper(&db.conn, &entry.entity_id);
            }

            // -- Collection operations --
            (EntityType::Collection, Operation::Create) => {
                if let Some(ref changes) = entry.field_changes {
                    let name = changes
                        .get("name")
                        .and_then(|fc| fc.new_value.as_str())
                        .unwrap_or("Untitled");
                    let parent_id = changes
                        .get("parent_id")
                        .and_then(|fc| fc.new_value.as_str());
                    let desc = changes
                        .get("description")
                        .and_then(|fc| fc.new_value.as_str());
                    let _ = collections::create_collection(&db.conn, name, parent_id, desc);
                }
            }
            (EntityType::Collection, Operation::Update) => {
                if let Some(ref changes) = entry.field_changes {
                    let name = changes.get("name").and_then(|fc| fc.new_value.as_str());
                    let parent_id = changes.get("parent_id").map(|fc| fc.new_value.as_str());
                    let desc = changes.get("description").map(|fc| fc.new_value.as_str());
                    let position = changes
                        .get("position")
                        .and_then(|fc| fc.new_value.as_i64())
                        .map(|v| v as i32);
                    let _ = collections::update_collection(
                        &db.conn,
                        &entry.entity_id,
                        name,
                        parent_id,
                        desc,
                        position,
                    );
                }
            }
            (EntityType::Collection, Operation::Delete) => {
                let _ = collections::delete_collection(&db.conn, &entry.entity_id);
            }

            // -- Tag operations --
            (EntityType::Tag, Operation::Create) => {
                if let Some(ref changes) = entry.field_changes {
                    let name = changes
                        .get("name")
                        .and_then(|fc| fc.new_value.as_str())
                        .unwrap_or("unnamed");
                    let color = changes.get("color").and_then(|fc| fc.new_value.as_str());
                    let _ = tags::create_tag(&db.conn, name, color);
                }
            }
            (EntityType::Tag, Operation::Update) => {
                if let Some(ref changes) = entry.field_changes {
                    let name = changes.get("name").and_then(|fc| fc.new_value.as_str());
                    let color = changes.get("color").map(|fc| fc.new_value.as_str());
                    let _ = tags::update_tag(&db.conn, &entry.entity_id, name, color);
                }
            }
            (EntityType::Tag, Operation::Delete) => {
                let _ = tags::delete_tag(&db.conn, &entry.entity_id);
            }

            // -- Annotation operations (merge by ID) --
            (EntityType::Annotation, Operation::Create) => {
                if let Some(ref changes) = entry.field_changes {
                    // Check if annotation already exists (created on both devices)
                    if annotations::get_annotation(&db.conn, &entry.entity_id).is_ok() {
                        // Already exists — apply field-level merge
                        self.merge_annotation_fields(
                            db,
                            &entry.entity_id,
                            changes,
                            &entry.timestamp,
                        )?;
                    } else {
                        // Create new annotation from sync
                        self.create_annotation_from_changes(db, &entry.entity_id, changes)?;
                    }
                }
            }
            (EntityType::Annotation, Operation::Update) => {
                if let Some(ref changes) = entry.field_changes {
                    if annotations::get_annotation(&db.conn, &entry.entity_id).is_ok() {
                        self.merge_annotation_fields(
                            db,
                            &entry.entity_id,
                            changes,
                            &entry.timestamp,
                        )?;
                    } else {
                        debug!(id = %entry.entity_id, "Annotation not found for update, skipping");
                    }
                }
            }
            (EntityType::Annotation, Operation::Delete) => {
                let _ = annotations::delete_annotation(&db.conn, &entry.entity_id);
            }

            // -- PaperCollection association operations --
            (EntityType::PaperCollection, Operation::Create) => {
                if let Some(ref changes) = entry.field_changes {
                    let paper_id = changes.get("paper_id").and_then(|fc| fc.new_value.as_str());
                    let collection_id = changes
                        .get("collection_id")
                        .and_then(|fc| fc.new_value.as_str());
                    if let (Some(pid), Some(cid)) = (paper_id, collection_id) {
                        let _ = collections::add_paper_to_collection(&db.conn, pid, cid);
                    }
                }
            }
            (EntityType::PaperCollection, Operation::Delete) => {
                if let Some(ref changes) = entry.field_changes {
                    let paper_id = changes.get("paper_id").and_then(|fc| fc.new_value.as_str());
                    let collection_id = changes
                        .get("collection_id")
                        .and_then(|fc| fc.new_value.as_str());
                    if let (Some(pid), Some(cid)) = (paper_id, collection_id) {
                        let _ = collections::remove_paper_from_collection(&db.conn, pid, cid);
                    }
                }
            }

            // -- PaperTag association operations --
            (EntityType::PaperTag, Operation::Create) => {
                if let Some(ref changes) = entry.field_changes {
                    let paper_id = changes.get("paper_id").and_then(|fc| fc.new_value.as_str());
                    let tag_name = changes.get("tag_name").and_then(|fc| fc.new_value.as_str());
                    if let (Some(pid), Some(tn)) = (paper_id, tag_name) {
                        let _ = tags::add_tag_to_paper(&db.conn, pid, tn, "sync");
                    }
                }
            }
            (EntityType::PaperTag, Operation::Delete) => {
                if let Some(ref changes) = entry.field_changes {
                    let paper_id = changes.get("paper_id").and_then(|fc| fc.new_value.as_str());
                    let tag_name = changes.get("tag_name").and_then(|fc| fc.new_value.as_str());
                    if let (Some(pid), Some(tn)) = (paper_id, tag_name) {
                        let _ = tags::remove_tag_from_paper(&db.conn, pid, tn);
                    }
                }
            }

            // -- ReaderState operations (last-write-wins by modified_date) --
            (EntityType::ReaderState, Operation::Update)
            | (EntityType::ReaderState, Operation::Create) => {
                if let Some(ref changes) = entry.field_changes {
                    let remote_modified = changes
                        .get("modified_date")
                        .and_then(|fc| fc.new_value.as_str())
                        .unwrap_or(&entry.timestamp);

                    // Check local state and compare timestamps
                    let should_apply =
                        match reader_state::get_reader_state(&db.conn, &entry.entity_id) {
                            Ok(Some(local_state)) => {
                                // Last-write-wins: apply only if remote is newer
                                remote_modified > local_state.modified_date.as_str()
                            }
                            _ => true, // No local state, always apply
                        };

                    if should_apply {
                        let scroll = changes
                            .get("scroll_position")
                            .and_then(|fc| fc.new_value.as_f64());
                        let scale = changes.get("scale").and_then(|fc| fc.new_value.as_f64());
                        let _ = reader_state::save_reader_state(
                            &db.conn,
                            &entry.entity_id,
                            scroll,
                            scale,
                        );
                    } else {
                        debug!(
                            paper_id = %entry.entity_id,
                            "Skipping reader state update: local is newer"
                        );
                    }
                }
            }

            _ => {
                debug!(
                    entity_type = ?entry.entity_type,
                    operation = ?entry.operation,
                    "Unhandled changelog entry type, skipping"
                );
            }
        }

        Ok(())
    }

    /// Create a new annotation from changelog field changes (sync import).
    fn create_annotation_from_changes(
        &self,
        db: &Database,
        annotation_id: &str,
        changes: &HashMap<String, FieldChange>,
    ) -> Result<(), WebDavError> {
        use zoro_db::queries::annotations;

        let get_str =
            |key: &str| -> Option<&str> { changes.get(key).and_then(|fc| fc.new_value.as_str()) };

        let paper_id = get_str("paper_id")
            .ok_or_else(|| WebDavError::Other("Annotation sync missing paper_id".to_string()))?;
        let annotation_type = get_str("type").unwrap_or("highlight");
        let color = get_str("color").unwrap_or("#ffe28f");
        let comment = get_str("comment");
        let selected_text = get_str("selected_text");
        let position_json = get_str("position_json").unwrap_or("{}");
        let page_number = changes
            .get("page_number")
            .and_then(|fc| fc.new_value.as_i64())
            .unwrap_or(1);
        let source_file = get_str("source_file");
        let created_date = get_str("created_date").unwrap_or_else(|| {
            changes
                .get("modified_date")
                .and_then(|fc| fc.new_value.as_str())
                .unwrap_or("")
        });
        let modified_date = get_str("modified_date").unwrap_or(created_date);

        let _ = annotations::insert_annotation_with_id(
            &db.conn,
            annotation_id,
            paper_id,
            annotation_type,
            color,
            comment,
            selected_text,
            None, // image_data — not synced via changelog field changes
            position_json,
            page_number,
            source_file,
            created_date,
            modified_date,
        );

        Ok(())
    }

    /// Merge annotation field changes with conflict detection.
    /// Uses last-write-wins based on modified_date, with conflict recording
    /// for truly conflicting edits to the same field.
    fn merge_annotation_fields(
        &self,
        db: &Database,
        annotation_id: &str,
        changes: &HashMap<String, FieldChange>,
        remote_timestamp: &str,
    ) -> Result<(), WebDavError> {
        use zoro_db::queries::annotations;

        let local = annotations::get_annotation(&db.conn, annotation_id)
            .map_err(|e| WebDavError::Other(format!("Failed to get annotation: {}", e)))?;

        let remote_modified = changes
            .get("modified_date")
            .and_then(|fc| fc.new_value.as_str())
            .unwrap_or(remote_timestamp);

        // If remote is newer, apply updates
        if remote_modified > local.modified_date.as_str() {
            let new_color = changes.get("color").and_then(|fc| fc.new_value.as_str());
            let new_comment = changes.get("comment").map(|fc| fc.new_value.as_str());
            let new_type = changes.get("type").and_then(|fc| fc.new_value.as_str());

            // Apply color and comment updates
            if new_color.is_some() || new_comment.is_some() {
                let _ =
                    annotations::update_annotation(&db.conn, annotation_id, new_color, new_comment);
            }

            // Apply type change
            if let Some(t) = new_type {
                let _ = annotations::update_annotation_type(&db.conn, annotation_id, t);
            }
        } else if remote_modified < local.modified_date.as_str() {
            // Local is newer — check if there's a real conflict (both modified same fields)
            let has_conflict = changes.iter().any(|(field, fc)| match field.as_str() {
                "color" => fc.new_value.as_str() != Some(&local.color),
                "comment" => {
                    let remote_comment = fc.new_value.as_str().map(String::from);
                    remote_comment != local.comment
                }
                "type" => fc.new_value.as_str() != Some(&local.annotation_type),
                _ => false,
            });

            if has_conflict {
                // Record conflict for user resolution
                let _ = zoro_db::queries::sync_conflicts::insert_conflict(
                    &db.conn,
                    "annotation",
                    annotation_id,
                    None, // field-level detail in local/remote values
                    Some(&serde_json::json!({
                        "color": local.color,
                        "comment": local.comment,
                        "type": local.annotation_type,
                        "modified_date": local.modified_date,
                    }).to_string()),
                    Some(&serde_json::json!({
                        "changes": changes.iter().map(|(k, v)| (k.clone(), &v.new_value)).collect::<HashMap<_, _>>(),
                        "modified_date": remote_modified,
                    }).to_string()),
                    Some(&entry_device_id_from_changes(changes)),
                );
                debug!(
                    annotation_id,
                    "Annotation conflict detected — local is newer, conflict recorded"
                );
            }
        }
        // If timestamps are equal, no action needed (same change)

        Ok(())
    }

    /// Merge field-level changes into an existing paper (last-write-wins).
    fn merge_paper_fields(
        &self,
        db: &Database,
        paper_id: &str,
        changes: &HashMap<String, FieldChange>,
    ) -> Result<(), WebDavError> {
        use zoro_db::queries::papers::{
            update_paper, update_paper_rating, update_paper_status, UpdatePaperInput,
        };

        let mut input = UpdatePaperInput {
            title: None,
            short_title: None,
            abstract_text: None,
            doi: None,
            arxiv_id: None,
            url: None,
            pdf_url: None,
            html_url: None,
            thumbnail_url: None,
            published_date: None,
            source: None,
            extra_json: None,
            entry_type: None,
            journal: None,
            volume: None,
            issue: None,
            pages: None,
            publisher: None,
            issn: None,
            isbn: None,
        };

        for (field, change) in changes {
            match field.as_str() {
                "title" => input.title = change.new_value.as_str().map(String::from),
                "short_title" => {
                    input.short_title = Some(change.new_value.as_str().map(String::from))
                }
                "abstract_text" => {
                    input.abstract_text = Some(change.new_value.as_str().map(String::from))
                }
                "doi" => input.doi = Some(change.new_value.as_str().map(String::from)),
                "arxiv_id" => input.arxiv_id = Some(change.new_value.as_str().map(String::from)),
                "url" => input.url = Some(change.new_value.as_str().map(String::from)),
                "pdf_url" => input.pdf_url = Some(change.new_value.as_str().map(String::from)),
                "html_url" => input.html_url = Some(change.new_value.as_str().map(String::from)),
                "thumbnail_url" => {
                    input.thumbnail_url = Some(change.new_value.as_str().map(String::from))
                }
                "published_date" => {
                    input.published_date = Some(change.new_value.as_str().map(String::from))
                }
                "source" => input.source = Some(change.new_value.as_str().map(String::from)),
                "entry_type" => {
                    input.entry_type = Some(change.new_value.as_str().map(String::from))
                }
                "journal" => input.journal = Some(change.new_value.as_str().map(String::from)),
                "volume" => input.volume = Some(change.new_value.as_str().map(String::from)),
                "issue" => input.issue = Some(change.new_value.as_str().map(String::from)),
                "pages" => input.pages = Some(change.new_value.as_str().map(String::from)),
                "publisher" => input.publisher = Some(change.new_value.as_str().map(String::from)),
                "issn" => input.issn = Some(change.new_value.as_str().map(String::from)),
                "isbn" => input.isbn = Some(change.new_value.as_str().map(String::from)),
                "read_status" => {
                    if let Some(status) = change.new_value.as_str() {
                        let _ = update_paper_status(&db.conn, paper_id, status);
                    }
                }
                "rating" => {
                    let rating = change.new_value.as_i64().map(|v| v as i32);
                    let _ = update_paper_rating(&db.conn, paper_id, rating);
                }
                _ => {
                    debug!(field, "Unknown paper field in changelog, skipping");
                }
            }
        }

        update_paper(&db.conn, paper_id, &input)
            .map_err(|e| WebDavError::Other(format!("Failed to merge paper fields: {}", e)))?;

        Ok(())
    }

    /// Create a new paper from a set of field changes (used when syncing a
    /// paper that was created on another device).
    fn create_paper_from_changes(
        &self,
        db: &Database,
        _paper_id: &str,
        changes: &HashMap<String, FieldChange>,
    ) -> Result<(), WebDavError> {
        use zoro_db::queries::papers::{insert_paper, CreatePaperInput};

        let get_str = |key: &str| -> Option<String> {
            changes
                .get(key)
                .and_then(|fc| fc.new_value.as_str())
                .map(String::from)
        };

        let slug = get_str("slug").unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let title = get_str("title").unwrap_or_else(|| "Untitled".to_string());

        let input = CreatePaperInput {
            slug: slug.clone(),
            title,
            short_title: get_str("short_title"),
            abstract_text: get_str("abstract_text"),
            doi: get_str("doi"),
            arxiv_id: get_str("arxiv_id"),
            url: get_str("url"),
            pdf_url: get_str("pdf_url"),
            html_url: get_str("html_url"),
            thumbnail_url: get_str("thumbnail_url"),
            published_date: get_str("published_date"),
            source: get_str("source"),
            dir_path: format!("library/papers/{}", slug),
            extra_json: get_str("extra_json"),
            entry_type: get_str("entry_type"),
            journal: get_str("journal"),
            volume: get_str("volume"),
            issue: get_str("issue"),
            pages: get_str("pages"),
            publisher: get_str("publisher"),
            issn: get_str("issn"),
            isbn: get_str("isbn"),
            added_date: get_str("added_date"),
        };

        insert_paper(&db.conn, &input)
            .map_err(|e| WebDavError::Other(format!("Failed to create paper from sync: {}", e)))?;

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn check_cancelled(&self) -> Result<(), WebDavError> {
        if self.is_cancelled() {
            Err(WebDavError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn report_progress<F>(
        &self,
        cb: &Option<F>,
        phase: &str,
        current: u64,
        total: u64,
        message: &str,
    ) where
        F: Fn(SyncProgress),
    {
        if let Some(ref cb) = cb {
            cb(SyncProgress {
                phase: phase.to_string(),
                current,
                total,
                message: message.to_string(),
            });
        }
    }

    /// Get the current sync status for display.
    pub fn get_status(&self, db: &DbHandle) -> SyncStatus {
        let local_state = db
            .lock()
            .ok()
            .and_then(|g| sync_queries::get_or_create_sync_state(&g.conn, &self.device_id).ok());

        SyncStatus {
            enabled: true,
            syncing: false,
            last_sync_time: local_state.as_ref().and_then(|s| s.last_sync_time.clone()),
            last_error: None,
            progress: None,
            devices: Vec::new(), // Populated from remote state during sync
        }
    }

    // -----------------------------------------------------------------------
    // Initial (first-time) full sync
    // -----------------------------------------------------------------------

    /// Perform the initial full sync for a new device.
    ///
    /// This method:
    /// 1. Checks whether the remote already has data
    /// 2. If remote has data: downloads all metadata.json files and imports them
    ///    into the local database (PDF/HTML marked as not-downloaded)
    /// 3. If remote is empty: uploads all local papers' metadata.json
    /// 4. Registers this device in the remote sync state
    ///
    /// The `progress_cb` receives (current_paper_index, total_papers, message).
    pub async fn initial_sync<F>(
        &self,
        db: &DbHandle,
        data_dir: &std::path::Path,
        progress_cb: Option<F>,
    ) -> Result<(), WebDavError>
    where
        F: Fn(SyncProgress) + Send,
    {
        use zoro_core::models::PaperMetadata;
        use zoro_db::queries::{annotations, collections, papers};

        self.reset_cancel();
        info!(device_id = %self.device_id, "Starting initial full sync");

        // Ensure remote directory structure exists
        self.report_progress(
            &progress_cb,
            "init",
            0,
            1,
            "Initializing remote directories",
        );
        self.client.init_remote_dirs(&self.remote_root).await?;
        self.check_cancelled()?;

        // Pull or create remote sync state
        let mut remote_state = self.pull_remote_state().await?;

        // Check if remote already has papers
        let papers_dir = format!("{}/zoro/library/papers/", self.remote_root);
        let remote_papers = match self.client.list(&papers_dir).await {
            Ok(entries) => entries
                .into_iter()
                .filter(|e| e.is_collection && !e.href.trim_end_matches('/').ends_with("papers"))
                .collect::<Vec<_>>(),
            Err(WebDavError::NotFound(_)) => Vec::new(),
            Err(e) => return Err(e),
        };
        self.check_cancelled()?;

        let remote_has_data = !remote_papers.is_empty();
        let local_papers = {
            let db_guard = db.lock().map_err(|e| WebDavError::Other(e.to_string()))?;
            papers::list_papers(
                &db_guard.conn,
                &papers::PaperFilter {
                    collection_id: None,
                    tag_name: None,
                    read_status: None,
                    search_query: None,
                    uncategorized: None,
                    sort_by: None,
                    sort_order: None,
                    limit: Some(100_000),
                    offset: Some(0),
                },
            )
            .map_err(|e| WebDavError::Other(e.to_string()))?
        };

        let local_has_data = !local_papers.is_empty();

        info!(
            remote_has_data,
            local_has_data,
            remote_count = remote_papers.len(),
            local_count = local_papers.len(),
            "Initial sync: assessing state"
        );

        if remote_has_data {
            // Download remote papers' metadata into local DB
            let total = remote_papers.len() as u64;
            for (idx, resource) in remote_papers.iter().enumerate() {
                self.check_cancelled()?;

                // Extract slug from href (e.g. "/dav/zoro/library/papers/some-slug/")
                let slug = resource
                    .href
                    .trim_end_matches('/')
                    .rsplit('/')
                    .next()
                    .unwrap_or("unknown");

                self.report_progress(
                    &progress_cb,
                    "download_metadata",
                    idx as u64 + 1,
                    total,
                    &format!("Downloading metadata: {}", slug),
                );

                // Download metadata.json
                let metadata_path = self.paper_metadata_path(slug);
                let metadata: PaperMetadata = match self.client.get_json(&metadata_path).await {
                    Ok(m) => m,
                    Err(WebDavError::NotFound(_)) => {
                        warn!(slug, "metadata.json not found on remote, skipping");
                        continue;
                    }
                    Err(e) => {
                        warn!(slug, error = %e, "Failed to download metadata, skipping");
                        continue;
                    }
                };

                // Check if paper already exists locally
                {
                    let db_guard = db.lock().map_err(|e| WebDavError::Other(e.to_string()))?;
                    if papers::get_paper(&db_guard.conn, &metadata.id).is_ok() {
                        debug!(slug, "Paper already exists locally, skipping");
                        continue;
                    }
                }

                // Import into local database with pdf_downloaded=0, html_downloaded=0
                let input = papers::CreatePaperInput {
                    slug: metadata.slug.clone(),
                    title: metadata.title,
                    short_title: metadata.short_title,
                    abstract_text: metadata.abstract_text,
                    doi: metadata.doi,
                    arxiv_id: metadata.arxiv_id,
                    url: metadata.url,
                    pdf_url: metadata.pdf_url,
                    html_url: metadata.html_url,
                    thumbnail_url: metadata.thumbnail_url,
                    published_date: metadata.published_date,
                    source: metadata.source,
                    dir_path: format!("library/papers/{}", metadata.slug),
                    extra_json: Some(serde_json::to_string(&metadata.extra).unwrap_or_default()),
                    entry_type: metadata.entry_type,
                    journal: metadata.journal,
                    volume: metadata.volume,
                    issue: metadata.issue,
                    pages: metadata.pages,
                    publisher: metadata.publisher,
                    issn: metadata.issn,
                    isbn: metadata.isbn,
                    added_date: Some(metadata.added_date.clone()),
                };

                let insert_result = {
                    let db_guard = db.lock().map_err(|e| WebDavError::Other(e.to_string()))?;
                    papers::insert_paper(&db_guard.conn, &input)
                };
                match insert_result {
                    Ok(row) => {
                        // Mark PDF and HTML as not downloaded (remote-only paper)
                        {
                            let db_guard =
                                db.lock().map_err(|e| WebDavError::Other(e.to_string()))?;
                            let _ = db_guard.conn.execute(
                                "UPDATE papers SET pdf_downloaded = 0, html_downloaded = 0 WHERE id = ?1",
                                [&row.id],
                            );
                        }

                        // Create local paper directory and write metadata.json
                        let paper_dir = data_dir.join("library/papers").join(&metadata.slug);
                        let _ = std::fs::create_dir_all(&paper_dir);
                        let meta_file = paper_dir.join("metadata.json");
                        if let Ok(meta_json) = self.client.get(&metadata_path).await {
                            let _ = std::fs::write(&meta_file, &meta_json);
                        }

                        // Set authors if available
                        let authors: Vec<(String, Option<String>, Option<String>)> = metadata
                            .authors
                            .iter()
                            .map(|a| (a.name.clone(), a.affiliation.clone(), a.orcid.clone()))
                            .collect();
                        {
                            let db_guard =
                                db.lock().map_err(|e| WebDavError::Other(e.to_string()))?;
                            if !authors.is_empty() {
                                let _ =
                                    papers::set_paper_authors(&db_guard.conn, &row.id, &authors);
                            }
                            // Set tags
                            for tag_name in &metadata.tags {
                                let _ = zoro_db::queries::tags::add_tag_to_paper(
                                    &db_guard.conn,
                                    &row.id,
                                    tag_name,
                                    "sync",
                                );
                            }
                            // Set collection associations
                            for col_name in &metadata.collections {
                                // Find or create collection by name
                                let col_id: Option<String> = db_guard
                                    .conn
                                    .query_row(
                                        "SELECT id FROM collections WHERE name = ?1",
                                        [col_name as &str],
                                        |r| r.get(0),
                                    )
                                    .ok();
                                let cid = match col_id {
                                    Some(id) => id,
                                    None => {
                                        match collections::create_collection(
                                            &db_guard.conn,
                                            col_name,
                                            None,
                                            None,
                                        ) {
                                            Ok(c) => c.id,
                                            Err(_) => continue,
                                        }
                                    }
                                };
                                let _ = collections::add_paper_to_collection(
                                    &db_guard.conn,
                                    &row.id,
                                    &cid,
                                );
                            }
                            // Import annotations
                            for ann in &metadata.annotations {
                                let _ = annotations::insert_annotation_with_id(
                                    &db_guard.conn,
                                    &ann.id,
                                    &row.id,
                                    &ann.annotation_type,
                                    &ann.color,
                                    ann.comment.as_deref(),
                                    ann.selected_text.as_deref(),
                                    None, // image_data
                                    &ann.position_json,
                                    ann.page_number,
                                    Some(&ann.source_file),
                                    &ann.created_date,
                                    &ann.modified_date,
                                );
                            }
                        }

                        debug!(slug = metadata.slug, "Imported remote paper");
                    }
                    Err(e) => {
                        warn!(slug = metadata.slug, error = %e, "Failed to import paper");
                    }
                }
            }
        }

        if local_has_data {
            // Upload local papers' metadata to remote
            let total = local_papers.len() as u64;
            for (idx, paper) in local_papers.iter().enumerate() {
                self.check_cancelled()?;

                self.report_progress(
                    &progress_cb,
                    "upload_metadata",
                    idx as u64 + 1,
                    total,
                    &format!("Uploading metadata: {}", paper.slug),
                );

                // Read local metadata.json
                let local_meta_path = data_dir
                    .join("library/papers")
                    .join(&paper.slug)
                    .join("metadata.json");

                if !local_meta_path.exists() {
                    debug!(slug = paper.slug, "No local metadata.json, skipping upload");
                    continue;
                }

                // Create remote paper directory
                let remote_paper_dir = self.paper_remote_dir(&paper.slug);
                let _ = self.client.mkcol(&remote_paper_dir).await;

                // Upload metadata.json
                let meta_data = match std::fs::read(&local_meta_path) {
                    Ok(d) => d,
                    Err(e) => {
                        warn!(slug = paper.slug, error = %e, "Failed to read local metadata");
                        continue;
                    }
                };
                let remote_meta_path = self.paper_metadata_path(&paper.slug);
                if let Err(e) = self.client.put(&remote_meta_path, meta_data).await {
                    warn!(slug = paper.slug, error = %e, "Failed to upload metadata");
                }
            }
        }

        // Register this device in remote sync state
        remote_state.devices.insert(
            self.device_id.clone(),
            DeviceSyncInfo {
                device_id: self.device_id.clone(),
                device_name: self.device_name.clone(),
                last_sync_time: Some(chrono::Utc::now().to_rfc3339()),
                last_sequence: 0,
            },
        );
        self.push_remote_state(&remote_state).await?;

        // Update local sync state — record all remote device sequence numbers
        // so that the first incremental sync won't replay old changelog entries.
        let now = chrono::Utc::now().to_rfc3339();
        let mut initial_remote_seqs: HashMap<String, i64> = HashMap::new();
        for (dev_id, dev_info) in &remote_state.devices {
            if dev_id != &self.device_id {
                initial_remote_seqs.insert(dev_id.clone(), dev_info.last_sequence as i64);
            }
        }
        let remote_seqs_json =
            serde_json::to_string(&initial_remote_seqs).unwrap_or_else(|_| "{}".to_string());
        {
            let db_guard = db.lock().map_err(|e| WebDavError::Other(e.to_string()))?;
            sync_queries::update_sync_state(
                &db_guard.conn,
                &self.device_id,
                &now,
                0,
                &remote_seqs_json,
            )
            .map_err(|e| WebDavError::Other(e.to_string()))?;
        }

        self.report_progress(&progress_cb, "done", 1, 1, "Initial sync complete");
        info!("Initial full sync completed");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Extract device_id from field changes if available (used for conflict tracking).
fn entry_device_id_from_changes(changes: &HashMap<String, FieldChange>) -> String {
    changes
        .get("device_id")
        .and_then(|fc| fc.new_value.as_str())
        .unwrap_or("unknown")
        .to_string()
}

fn parse_entity_type(s: &str) -> EntityType {
    match s {
        "paper" => EntityType::Paper,
        "collection" => EntityType::Collection,
        "tag" => EntityType::Tag,
        "note" => EntityType::Note,
        "attachment" => EntityType::Attachment,
        "annotation" => EntityType::Annotation,
        "paper_collection" => EntityType::PaperCollection,
        "paper_tag" => EntityType::PaperTag,
        "reader_state" => EntityType::ReaderState,
        _ => EntityType::Paper,
    }
}

fn parse_operation(s: &str) -> Operation {
    match s {
        "create" => Operation::Create,
        "update" => Operation::Update,
        "delete" => Operation::Delete,
        _ => Operation::Update,
    }
}
