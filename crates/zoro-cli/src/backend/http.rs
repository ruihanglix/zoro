// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::path::PathBuf;

use super::{
    Backend, BackendError, CollectionInfo, ExportResult, NoteInfo, PaperInfo, StatusInfo, TagInfo,
};

/// HTTP backend that connects to the Zoro desktop app's connector.
///
/// Phase 1: Only implements endpoints that the connector already supports.
/// Other operations fall back to returning an error with a helpful message.
pub struct HttpBackend {
    port: u16,
    data_dir: PathBuf,
    #[allow(dead_code)]
    base_url: String,
}

impl HttpBackend {
    pub fn new(port: u16, data_dir: PathBuf) -> Self {
        let base_url = format!("http://127.0.0.1:{}/connector", port);
        Self {
            port,
            data_dir,
            base_url,
        }
    }

    fn _not_implemented(&self, op: &str) -> BackendError {
        BackendError(format!(
            "{} is not yet supported via the HTTP connector (port {}). \
             Use --local flag to access the database directly, \
             or wait for the connector API to be extended.",
            op, self.port
        ))
    }
}

impl Backend for HttpBackend {
    fn mode_name(&self) -> &str {
        "HTTP (connector)"
    }

    fn data_dir_display(&self) -> String {
        self.data_dir.display().to_string()
    }

    // Phase 1: Most operations are not available via HTTP yet.
    // The connector only has: ping, status, collections, saveItem, saveHtml.
    // All other operations return a helpful error message suggesting --local.

    fn search_papers(&self, _query: &str, _limit: i64) -> Result<Vec<PaperInfo>, BackendError> {
        Err(self._not_implemented("Paper search"))
    }

    fn list_papers(
        &self,
        _collection: Option<&str>,
        _tag: Option<&str>,
        _status: Option<&str>,
        _limit: i64,
    ) -> Result<Vec<PaperInfo>, BackendError> {
        Err(self._not_implemented("Paper listing"))
    }

    fn get_paper(&self, _id_or_slug: &str) -> Result<PaperInfo, BackendError> {
        Err(self._not_implemented("Get paper"))
    }

    fn add_paper(&self, _source: &str) -> Result<PaperInfo, BackendError> {
        // TODO: Phase 2 — use POST /connector/saveItem
        Err(self._not_implemented("Add paper via HTTP"))
    }

    fn delete_paper(&self, _id_or_slug: &str) -> Result<(), BackendError> {
        Err(self._not_implemented("Delete paper"))
    }

    fn open_paper(&self, _id_or_slug: &str) -> Result<(), BackendError> {
        Err(self._not_implemented("Open paper"))
    }

    fn list_collections(&self) -> Result<Vec<CollectionInfo>, BackendError> {
        // The connector does have GET /connector/collections
        // but for Phase 1, we keep it simple and suggest --local
        Err(self._not_implemented("List collections via HTTP"))
    }

    fn create_collection(
        &self,
        _name: &str,
        _description: Option<&str>,
    ) -> Result<CollectionInfo, BackendError> {
        Err(self._not_implemented("Create collection"))
    }

    fn add_paper_to_collection(
        &self,
        _paper: &str,
        _collection: &str,
    ) -> Result<(), BackendError> {
        Err(self._not_implemented("Add paper to collection"))
    }

    fn remove_paper_from_collection(
        &self,
        _paper: &str,
        _collection: &str,
    ) -> Result<(), BackendError> {
        Err(self._not_implemented("Remove paper from collection"))
    }

    fn list_tags(&self) -> Result<Vec<TagInfo>, BackendError> {
        Err(self._not_implemented("List tags"))
    }

    fn add_tag_to_paper(&self, _paper: &str, _tag: &str) -> Result<(), BackendError> {
        Err(self._not_implemented("Add tag to paper"))
    }

    fn remove_tag_from_paper(&self, _paper: &str, _tag: &str) -> Result<(), BackendError> {
        Err(self._not_implemented("Remove tag from paper"))
    }

    fn list_notes(&self, _paper: &str) -> Result<Vec<NoteInfo>, BackendError> {
        Err(self._not_implemented("List notes"))
    }

    fn add_note(&self, _paper: &str, _content: &str) -> Result<NoteInfo, BackendError> {
        Err(self._not_implemented("Add note"))
    }

    fn delete_note(&self, _note_id: &str) -> Result<(), BackendError> {
        Err(self._not_implemented("Delete note"))
    }

    fn export_paper(&self, _paper: &str, _format: &str) -> Result<ExportResult, BackendError> {
        Err(self._not_implemented("Export paper"))
    }

    fn status(&self) -> Result<StatusInfo, BackendError> {
        // We know the connector is running (we pinged it)
        Ok(StatusInfo {
            mode: self.mode_name().to_string(),
            data_dir: self.data_dir_display(),
            paper_count: -1, // Unknown via HTTP
            collection_count: -1,
            tag_count: -1,
        })
    }
}
