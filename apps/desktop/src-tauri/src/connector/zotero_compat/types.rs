// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use serde::{Deserialize, Serialize};

// ─── Ping ────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[expect(dead_code)]
pub struct ZoteroPingRequest {
    #[serde(rename = "activeURL")]
    pub active_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ZoteroPingResponse {
    pub prefs: ZoteroPrefs,
}

#[derive(Debug, Serialize)]
pub struct ZoteroPrefs {
    #[serde(rename = "downloadAssociatedFiles")]
    pub download_associated_files: bool,
    #[serde(rename = "reportActiveURL")]
    pub report_active_url: bool,
    #[serde(rename = "automaticSnapshots")]
    pub automatic_snapshots: bool,
    #[serde(rename = "supportsAttachmentUpload")]
    pub supports_attachment_upload: bool,
    #[serde(rename = "supportsTagsAutocomplete")]
    pub supports_tags_autocomplete: bool,
    #[serde(rename = "canUserAddNote")]
    pub can_user_add_note: bool,
}

// ─── getSelectedCollection ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[expect(dead_code)]
pub struct GetSelectedCollectionRequest {
    #[serde(rename = "switchToReadableLibrary")]
    pub switch_to_readable_library: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct GetSelectedCollectionResponse {
    pub id: String,
    pub name: String,
    #[serde(rename = "libraryID")]
    pub library_id: i32,
    #[serde(rename = "libraryEditable")]
    pub library_editable: bool,
    #[serde(rename = "filesEditable")]
    pub files_editable: bool,
    pub targets: Vec<CollectionTarget>,
}

#[derive(Debug, Serialize)]
pub struct CollectionTarget {
    pub id: String,
    pub name: String,
    pub level: i32,
    #[serde(rename = "filesEditable")]
    pub files_editable: bool,
}

// ─── saveItems ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[expect(dead_code)]
pub struct ZoteroSaveItemsRequest {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    pub uri: Option<String>,
    pub proxy: Option<serde_json::Value>,
    pub items: Vec<ZoteroItem>,
    pub cookie: Option<String>,
    #[serde(rename = "detailedCookies")]
    pub detailed_cookies: Option<String>,
    #[serde(rename = "singleFile")]
    pub single_file: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ZoteroItem {
    pub id: Option<String>,
    #[serde(rename = "itemType")]
    pub item_type: Option<String>,
    pub title: Option<String>,
    pub creators: Option<Vec<ZoteroCreator>>,
    pub date: Option<String>,
    pub url: Option<String>,
    #[serde(rename = "DOI")]
    pub doi: Option<String>,
    #[serde(rename = "abstractNote")]
    pub abstract_note: Option<String>,
    pub tags: Option<Vec<ZoteroTag>>,
    pub attachments: Option<Vec<ZoteroAttachment>>,
    pub notes: Option<Vec<serde_json::Value>>,
    #[serde(rename = "accessDate")]
    pub access_date: Option<String>,
    // Journal-specific fields
    #[serde(rename = "publicationTitle")]
    pub publication_title: Option<String>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub pages: Option<String>,
    #[serde(rename = "ISSN")]
    pub issn: Option<String>,
    #[serde(rename = "ISBN")]
    pub isbn: Option<String>,
    pub publisher: Option<String>,
    pub place: Option<String>,
    pub language: Option<String>,
    #[serde(rename = "shortTitle")]
    pub short_title: Option<String>,
    #[serde(rename = "journalAbbreviation")]
    pub journal_abbreviation: Option<String>,
    pub rights: Option<String>,
    pub series: Option<String>,
    #[serde(rename = "seriesTitle")]
    pub series_title: Option<String>,
    #[serde(rename = "seriesText")]
    pub series_text: Option<String>,
    #[serde(rename = "numberOfVolumes")]
    pub number_of_volumes: Option<String>,
    pub edition: Option<String>,
    #[serde(rename = "numPages")]
    pub num_pages: Option<String>,
    // Catch-all for any other fields
    #[serde(flatten)]
    pub extra_fields: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ZoteroCreator {
    #[serde(rename = "firstName")]
    pub first_name: Option<String>,
    #[serde(rename = "lastName")]
    pub last_name: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "creatorType")]
    pub creator_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ZoteroTag {
    pub tag: String,
    #[serde(rename = "type")]
    pub tag_type: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ZoteroAttachment {
    pub id: Option<String>,
    #[serde(rename = "parentItem")]
    pub parent_item: Option<String>,
    pub title: Option<String>,
    pub url: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    pub snapshot: Option<bool>,
    pub referrer: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ZoteroSaveItemsResponse {
    pub items: Vec<ZoteroSaveItemResult>,
}

#[derive(Debug, Serialize)]
pub struct ZoteroSaveItemResult {
    pub id: String,
    pub attachments: Vec<ZoteroAttachmentProgress>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ZoteroAttachmentProgress {
    pub id: String,
    pub progress: serde_json::Value, // number or false
}

// ─── saveSnapshot ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[expect(dead_code)]
pub struct ZoteroSaveSnapshotRequest {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    pub url: Option<String>,
    pub referrer: Option<String>,
    pub cookie: Option<String>,
    #[serde(rename = "detailedCookies")]
    pub detailed_cookies: Option<String>,
    pub uri: Option<String>,
    pub title: Option<String>,
    #[serde(rename = "skipSnapshot")]
    pub skip_snapshot: Option<bool>,
    #[serde(rename = "singleFile")]
    pub single_file: Option<bool>,
    pub pdf: Option<bool>,
}

// ─── saveSingleFile ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[expect(dead_code)]
pub struct ZoteroSaveSingleFileRequest {
    pub items: Option<Vec<ZoteroItem>>,
    #[serde(rename = "sessionID")]
    pub session_id: String,
    #[serde(rename = "snapshotContent")]
    pub snapshot_content: String,
    pub url: Option<String>,
    pub title: Option<String>,
}

// ─── saveAttachment metadata (from X-Metadata header) ────────────────────────

#[derive(Debug, Deserialize)]
pub struct AttachmentMetadata {
    pub id: Option<String>,
    pub url: Option<String>,
    #[serde(rename = "contentType")]
    pub content_type: Option<String>,
    #[serde(rename = "parentItemID")]
    pub parent_item_id: Option<String>,
    pub title: Option<String>,
}

// ─── saveStandaloneAttachment response ───────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SaveStandaloneAttachmentResponse {
    #[serde(rename = "canRecognize")]
    pub can_recognize: bool,
}

// ─── sessionProgress ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SessionProgressRequest {
    #[serde(rename = "sessionID")]
    pub session_id: String,
}

#[derive(Debug, Serialize)]
pub struct SessionProgressResponse {
    pub items: Vec<ZoteroSaveItemResult>,
    pub done: bool,
}

// ─── updateSession ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[expect(dead_code)]
pub struct UpdateSessionRequest {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    pub target: Option<String>,
    pub tags: Option<Vec<String>>,
    pub note: Option<String>,
}

// ─── import ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ImportItemResult {
    #[serde(rename = "itemType")]
    pub item_type: String,
    pub title: String,
}

// ─── getTranslators / getTranslatorCode ──────────────────────────────────────

#[derive(Debug, Deserialize)]
#[expect(dead_code)]
pub struct GetTranslatorCodeRequest {
    #[serde(rename = "translatorID")]
    pub translator_id: String,
}

// ─── getRecognizedItem ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[expect(dead_code)]
pub struct GetRecognizedItemRequest {
    #[serde(rename = "sessionID")]
    pub session_id: String,
}

// ─── hasAttachmentResolvers ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[expect(dead_code)]
pub struct HasAttachmentResolversRequest {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    #[serde(rename = "itemID")]
    pub item_id: Option<String>,
}
