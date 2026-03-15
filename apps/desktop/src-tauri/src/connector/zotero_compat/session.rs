// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use super::types::ZoteroAttachmentProgress;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Tracks the state of a single Zotero Connector save session.
#[derive(Debug, Clone)]
pub struct Session {
    /// Zoro paper IDs created in this session, keyed by Zotero item ID.
    pub item_paper_map: HashMap<String, String>,
    /// Attachment progress, keyed by Zotero attachment ID.
    pub attachment_progress: HashMap<String, AttachmentState>,
    /// Target collection ID (Zoro collection ID, without "C" prefix).
    pub target_collection: Option<String>,
    /// Tags to apply.
    pub tags: Vec<String>,
    /// When this session was created.
    pub created_at: std::time::Instant,
    /// Whether all operations are done.
    pub done: bool,
}

#[derive(Debug, Clone)]
pub struct AttachmentState {
    pub parent_zotero_item_id: String,
    pub progress: AttachmentProgress,
}

#[derive(Debug, Clone)]
pub enum AttachmentProgress {
    /// Progress percentage 0-100
    InProgress(u32),
    /// Completed successfully
    Done,
    /// Failed
    Failed,
}

impl AttachmentProgress {
    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            AttachmentProgress::InProgress(p) => serde_json::Value::Number((*p).into()),
            AttachmentProgress::Done => serde_json::Value::Number(100.into()),
            AttachmentProgress::Failed => serde_json::Value::Bool(false),
        }
    }
}

/// Thread-safe session store.
#[derive(Debug, Clone)]
pub struct SessionStore {
    sessions: Arc<Mutex<HashMap<String, Session>>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a new session.
    pub fn create_session(&self, session_id: &str) {
        let mut sessions = self.sessions.lock().unwrap();
        sessions.insert(
            session_id.to_string(),
            Session {
                item_paper_map: HashMap::new(),
                attachment_progress: HashMap::new(),
                target_collection: None,
                tags: Vec::new(),
                created_at: std::time::Instant::now(),
                done: false,
            },
        );
    }

    /// Register a mapping from Zotero item ID to Zoro paper ID.
    pub fn register_item(&self, session_id: &str, zotero_item_id: &str, paper_id: &str) {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(session_id) {
            session
                .item_paper_map
                .insert(zotero_item_id.to_string(), paper_id.to_string());
        }
    }

    /// Register an attachment that we expect to receive.
    pub fn register_attachment(
        &self,
        session_id: &str,
        attachment_id: &str,
        parent_zotero_item_id: &str,
    ) {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(session_id) {
            session.attachment_progress.insert(
                attachment_id.to_string(),
                AttachmentState {
                    parent_zotero_item_id: parent_zotero_item_id.to_string(),
                    progress: AttachmentProgress::InProgress(0),
                },
            );
        }
    }

    /// Mark an attachment as completed.
    pub fn complete_attachment(&self, session_id: &str, attachment_id: &str) {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(session_id) {
            if let Some(state) = session.attachment_progress.get_mut(attachment_id) {
                state.progress = AttachmentProgress::Done;
            }
            // Check if all attachments are done
            let all_done = session.attachment_progress.values().all(|s| {
                matches!(
                    s.progress,
                    AttachmentProgress::Done | AttachmentProgress::Failed
                )
            });
            if all_done {
                session.done = true;
            }
        }
    }

    /// Mark an attachment as failed.
    pub fn fail_attachment(&self, session_id: &str, attachment_id: &str) {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(session_id) {
            if let Some(state) = session.attachment_progress.get_mut(attachment_id) {
                state.progress = AttachmentProgress::Failed;
            }
            let all_done = session.attachment_progress.values().all(|s| {
                matches!(
                    s.progress,
                    AttachmentProgress::Done | AttachmentProgress::Failed
                )
            });
            if all_done {
                session.done = true;
            }
        }
    }

    /// Look up the Zoro paper ID for a Zotero item ID within a session.
    pub fn get_paper_id(&self, session_id: &str, zotero_item_id: &str) -> Option<String> {
        let sessions = self.sessions.lock().unwrap();
        sessions
            .get(session_id)
            .and_then(|s| s.item_paper_map.get(zotero_item_id).cloned())
    }

    /// Get the first paper ID in a session (for snapshot saves that have one item).
    pub fn get_first_paper_id(&self, session_id: &str) -> Option<String> {
        let sessions = self.sessions.lock().unwrap();
        sessions
            .get(session_id)
            .and_then(|s| s.item_paper_map.values().next().cloned())
    }

    /// Get progress for all items in a session (for sessionProgress endpoint).
    pub fn get_session_progress(
        &self,
        session_id: &str,
    ) -> Option<(Vec<SessionItemProgress>, bool)> {
        let sessions = self.sessions.lock().unwrap();
        let session = sessions.get(session_id)?;

        // Group attachments by their parent Zotero item ID
        let mut items_map: HashMap<String, Vec<ZoteroAttachmentProgress>> = HashMap::new();
        for (att_id, state) in &session.attachment_progress {
            items_map
                .entry(state.parent_zotero_item_id.clone())
                .or_default()
                .push(ZoteroAttachmentProgress {
                    id: att_id.clone(),
                    progress: state.progress.to_json_value(),
                });
        }

        let items: Vec<SessionItemProgress> = session
            .item_paper_map
            .keys()
            .map(|zotero_id| SessionItemProgress {
                id: zotero_id.clone(),
                attachments: items_map.remove(zotero_id).unwrap_or_default(),
            })
            .collect();

        Some((items, session.done))
    }

    /// Update session target collection and tags.
    pub fn update_session(
        &self,
        session_id: &str,
        target: Option<String>,
        tags: Option<Vec<String>>,
    ) {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(session_id) {
            if let Some(t) = target {
                session.target_collection = Some(t);
            }
            if let Some(t) = tags {
                session.tags = t;
            }
        }
    }

    /// Get session data for updateSession handler.
    pub fn get_session(&self, session_id: &str) -> Option<Session> {
        let sessions = self.sessions.lock().unwrap();
        sessions.get(session_id).cloned()
    }

    /// Mark a session as done (no pending attachments).
    pub fn mark_done(&self, session_id: &str) {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(session_id) {
            session.done = true;
        }
    }

    /// Clean up expired sessions (older than 30 minutes).
    pub fn cleanup_expired(&self) {
        let mut sessions = self.sessions.lock().unwrap();
        let cutoff = std::time::Duration::from_secs(30 * 60);
        sessions.retain(|_, s| s.created_at.elapsed() < cutoff);
    }
}

#[derive(Debug)]
pub struct SessionItemProgress {
    pub id: String,
    pub attachments: Vec<ZoteroAttachmentProgress>,
}
