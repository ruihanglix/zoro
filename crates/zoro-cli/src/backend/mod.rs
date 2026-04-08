// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

pub mod http;
pub mod local;

use std::path::Path;

use serde::Serialize;

/// Information about a paper, used for display and JSON output.
#[derive(Debug, Clone, Serialize)]
pub struct PaperInfo {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub authors_display: String,
    pub abstract_text: Option<String>,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    pub url: Option<String>,
    pub pdf_url: Option<String>,
    pub published_date: Option<String>,
    pub added_date: String,
    pub read_status: String,
    pub rating: Option<i32>,
    pub tags: Vec<String>,
    pub collections: Vec<String>,
    pub entry_type: Option<String>,
    pub journal: Option<String>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub pages: Option<String>,
    pub publisher: Option<String>,
}

/// Collection information for display.
#[derive(Debug, Clone, Serialize)]
pub struct CollectionInfo {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub paper_count: i64,
}

/// Tag information for display.
#[derive(Debug, Clone, Serialize)]
pub struct TagInfo {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub paper_count: i64,
}

/// Note information for display.
#[derive(Debug, Clone, Serialize)]
pub struct NoteInfo {
    pub id: String,
    pub paper_id: String,
    pub content: String,
    pub created_date: String,
    pub modified_date: String,
}

/// Library status information.
#[derive(Debug, Clone, Serialize)]
pub struct StatusInfo {
    pub mode: String,
    pub data_dir: String,
    pub paper_count: i64,
    pub collection_count: i64,
    pub tag_count: i64,
}

/// Export result.
#[derive(Debug, Clone, Serialize)]
pub struct ExportResult {
    pub format: String,
    pub content: String,
}

/// Backend trait abstracting data access — either local SQLite or HTTP connector.
pub trait Backend {
    fn mode_name(&self) -> &str;
    fn data_dir_display(&self) -> String;

    // Papers
    fn search_papers(&self, query: &str, limit: i64) -> Result<Vec<PaperInfo>, BackendError>;
    fn list_papers(
        &self,
        collection: Option<&str>,
        tag: Option<&str>,
        status: Option<&str>,
        limit: i64,
    ) -> Result<Vec<PaperInfo>, BackendError>;
    fn get_paper(&self, id_or_slug: &str) -> Result<PaperInfo, BackendError>;
    fn add_paper(&self, source: &str) -> Result<PaperInfo, BackendError>;
    fn delete_paper(&self, id_or_slug: &str) -> Result<(), BackendError>;
    fn open_paper(&self, id_or_slug: &str) -> Result<(), BackendError>;

    // Collections
    fn list_collections(&self) -> Result<Vec<CollectionInfo>, BackendError>;
    fn create_collection(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<CollectionInfo, BackendError>;
    fn add_paper_to_collection(
        &self,
        paper_id_or_slug: &str,
        collection_name_or_id: &str,
    ) -> Result<(), BackendError>;
    fn remove_paper_from_collection(
        &self,
        paper_id_or_slug: &str,
        collection_name_or_id: &str,
    ) -> Result<(), BackendError>;

    // Tags
    fn list_tags(&self) -> Result<Vec<TagInfo>, BackendError>;
    fn add_tag_to_paper(&self, paper_id_or_slug: &str, tag_name: &str) -> Result<(), BackendError>;
    fn remove_tag_from_paper(
        &self,
        paper_id_or_slug: &str,
        tag_name: &str,
    ) -> Result<(), BackendError>;

    // Notes
    fn list_notes(&self, paper_id_or_slug: &str) -> Result<Vec<NoteInfo>, BackendError>;
    fn add_note(&self, paper_id_or_slug: &str, content: &str) -> Result<NoteInfo, BackendError>;
    fn delete_note(&self, note_id: &str) -> Result<(), BackendError>;

    // Export
    fn export_paper(
        &self,
        paper_id_or_slug: &str,
        format: &str,
    ) -> Result<ExportResult, BackendError>;

    // Status
    fn status(&self) -> Result<StatusInfo, BackendError>;
}

#[derive(Debug)]
pub struct BackendError(pub String);

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for BackendError {}

impl From<zoro_db::DbError> for BackendError {
    fn from(e: zoro_db::DbError) -> Self {
        BackendError(e.to_string())
    }
}

/// Connect to the best available backend.
/// Tries HTTP connector first (if not forced local), then falls back to direct SQLite.
pub async fn connect(
    data_dir: &Path,
    force_local: bool,
) -> Result<Box<dyn Backend>, Box<dyn std::error::Error>> {
    if !force_local {
        // Try HTTP connector
        if let Ok(resp) = reqwest::get("http://127.0.0.1:23120/connector/ping").await {
            if resp.status().is_success() {
                tracing::info!("Connected to Zoro app via HTTP connector");
                return Ok(Box::new(http::HttpBackend::new(
                    23120,
                    data_dir.to_path_buf(),
                )));
            }
        }
    }

    // Fall back to direct SQLite
    tracing::info!("Using local SQLite backend");

    // Ensure data directory exists
    zoro_storage::init_data_dir(data_dir)?;

    let db_path = data_dir.join("library.db");
    let db = zoro_db::Database::open(&db_path)
        .map_err(|e| format!("Failed to open database at {:?}: {}", db_path, e))?;

    Ok(Box::new(local::LocalBackend::new(
        db,
        data_dir.to_path_buf(),
    )))
}
