// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::sync::Arc;

use rmcp::model::*;
use serde_json::json;

use crate::state::AppState;
use zoro_core::models::{Author, PaperMetadata, ReadStatus};
use zoro_core::slug_utils::generate_paper_slug;
use zoro_db::queries::{attachments, notes, papers, search, tags};

/// Helper: build a JSON object from a paper row with all related data.
pub fn paper_to_json(
    row: &papers::PaperRow,
    author_list: Vec<(String, Option<String>, Option<String>)>,
    tag_list: Vec<tags::TagRow>,
    attachment_list: Vec<attachments::AttachmentRow>,
    note_list: Vec<notes::NoteRow>,
) -> serde_json::Value {
    let has_pdf = attachment_list.iter().any(|a| a.file_type == "pdf");
    let has_html = attachment_list.iter().any(|a| a.file_type == "html");

    json!({
        "id": row.id,
        "slug": row.slug,
        "title": row.title,
        "short_title": row.short_title,
        "authors": author_list.iter().map(|(name, aff, _)| json!({
            "name": name,
            "affiliation": aff,
        })).collect::<Vec<_>>(),
        "abstract_text": row.abstract_text,
        "doi": row.doi,
        "arxiv_id": row.arxiv_id,
        "url": row.url,
        "pdf_url": row.pdf_url,
        "html_url": row.html_url,
        "thumbnail_url": row.thumbnail_url,
        "published_date": row.published_date,
        "added_date": row.added_date,
        "modified_date": row.modified_date,
        "source": row.source,
        "read_status": row.read_status,
        "rating": row.rating,
        "tags": tag_list.iter().map(|t| json!({
            "id": t.id,
            "name": t.name,
            "color": t.color,
        })).collect::<Vec<_>>(),
        "attachments": attachment_list.iter().map(|a| json!({
            "id": a.id,
            "filename": a.filename,
            "file_type": a.file_type,
            "file_size": a.file_size,
        })).collect::<Vec<_>>(),
        "has_pdf": has_pdf,
        "has_html": has_html,
        "notes": note_list.iter().map(|n| n.content.clone()).collect::<Vec<_>>(),
        "entry_type": row.entry_type,
        "journal": row.journal,
        "volume": row.volume,
        "issue": row.issue,
        "pages": row.pages,
        "publisher": row.publisher,
        "issn": row.issn,
        "isbn": row.isbn,
    })
}

/// Fetch full paper data from DB and return as JSON.
pub fn get_full_paper(db: &zoro_db::Database, paper_id: &str) -> Result<serde_json::Value, String> {
    let row = papers::get_paper(&db.conn, paper_id).map_err(|e| format!("{}", e))?;
    let author_list = papers::get_paper_authors(&db.conn, paper_id).unwrap_or_default();
    let tag_list = tags::get_paper_tags(&db.conn, paper_id).unwrap_or_default();
    let attachment_list =
        attachments::get_paper_attachments(&db.conn, paper_id).unwrap_or_default();
    let note_list = notes::list_notes(&db.conn, paper_id).unwrap_or_default();
    Ok(paper_to_json(
        &row,
        author_list,
        tag_list,
        attachment_list,
        note_list,
    ))
}

fn sync_after_mutation(state: &AppState, db: &zoro_db::Database, paper_id: Option<&str>) {
    if let Some(pid) = paper_id {
        zoro_storage::sync::sync_paper_metadata(db, &state.data_dir, pid);
    }
    zoro_storage::sync::rebuild_library_index(db, &state.data_dir);
}

// --- Input schemas for tools ---

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AddPaperInput {
    /// Paper title (required)
    pub title: String,
    /// Short title (user-editable short name)
    pub short_title: Option<String>,
    /// List of authors
    #[serde(default)]
    pub authors: Vec<AuthorInput>,
    /// Abstract text
    pub abstract_text: Option<String>,
    /// DOI identifier
    pub doi: Option<String>,
    /// arXiv identifier
    pub arxiv_id: Option<String>,
    /// Paper URL
    pub url: Option<String>,
    /// PDF download URL
    pub pdf_url: Option<String>,
    /// HTML version URL
    pub html_url: Option<String>,
    /// Publication date (ISO 8601)
    pub published_date: Option<String>,
    /// Source of the paper (e.g. "manual", "arxiv")
    pub source: Option<String>,
    /// Tags to add to the paper
    pub tags: Option<Vec<String>>,
    /// Entry type (e.g. "article", "inproceedings")
    pub entry_type: Option<String>,
    /// Journal name
    pub journal: Option<String>,
    /// Volume number
    pub volume: Option<String>,
    /// Issue number
    pub issue: Option<String>,
    /// Page range
    pub pages: Option<String>,
    /// Publisher name
    pub publisher: Option<String>,
    /// ISSN
    pub issn: Option<String>,
    /// ISBN
    pub isbn: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AuthorInput {
    /// Author name
    pub name: String,
    /// Affiliation
    pub affiliation: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetPaperInput {
    /// Paper ID
    pub id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListPapersInput {
    /// Filter by collection ID
    pub collection_id: Option<String>,
    /// Filter by tag name
    pub tag_name: Option<String>,
    /// Filter by read status: "unread", "reading", or "read"
    pub read_status: Option<String>,
    /// Only show uncategorized papers
    pub uncategorized: Option<bool>,
    /// Sort field: "title", "added_date", "published_date", "rating"
    pub sort_by: Option<String>,
    /// Sort order: "asc" or "desc"
    pub sort_order: Option<String>,
    /// Maximum number of results (default 50)
    pub limit: Option<i64>,
    /// Offset for pagination (default 0)
    pub offset: Option<i64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchPapersInput {
    /// Full-text search query
    pub query: String,
    /// Maximum number of results (default 20)
    pub limit: Option<i64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdatePaperInput {
    /// Paper ID
    pub id: String,
    /// New title
    pub title: Option<String>,
    /// New short title (null to clear)
    pub short_title: Option<Option<String>>,
    /// New abstract text (null to clear)
    pub abstract_text: Option<Option<String>>,
    /// New DOI (null to clear)
    pub doi: Option<Option<String>>,
    /// New arXiv ID (null to clear)
    pub arxiv_id: Option<Option<String>>,
    /// New URL (null to clear)
    pub url: Option<Option<String>>,
    /// New PDF URL (null to clear)
    pub pdf_url: Option<Option<String>>,
    /// New HTML URL (null to clear)
    pub html_url: Option<Option<String>>,
    /// New published date (null to clear)
    pub published_date: Option<Option<String>>,
    /// Entry type (null to clear)
    pub entry_type: Option<Option<String>>,
    /// Journal (null to clear)
    pub journal: Option<Option<String>>,
    /// Volume (null to clear)
    pub volume: Option<Option<String>>,
    /// Issue (null to clear)
    pub issue: Option<Option<String>>,
    /// Pages (null to clear)
    pub pages: Option<Option<String>>,
    /// Publisher (null to clear)
    pub publisher: Option<Option<String>>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdatePaperStatusInput {
    /// Paper ID
    pub id: String,
    /// New read status: "unread", "reading", or "read"
    pub read_status: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdatePaperRatingInput {
    /// Paper ID
    pub id: String,
    /// Rating (1-5), or null to clear
    pub rating: Option<i32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DeletePaperInput {
    /// Paper ID
    pub id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct EnrichPaperInput {
    /// Paper ID (paper must have a DOI or arXiv ID)
    pub paper_id: String,
}

// --- Tool implementations ---

pub fn tool_add_paper(
    state: &Arc<AppState>,
    input: AddPaperInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let identifier = input
        .doi
        .as_deref()
        .or(input.arxiv_id.as_deref())
        .unwrap_or(&input.title);
    let slug = generate_paper_slug(&input.title, identifier, input.published_date.as_deref());

    let papers_dir = state.data_dir.join("library/papers");
    zoro_storage::paper_dir::create_paper_dir(&papers_dir, &slug).map_err(|e| {
        rmcp::ErrorData::internal_error(format!("Failed to create paper dir: {}", e), None)
    })?;

    let db_input = papers::CreatePaperInput {
        slug: slug.clone(),
        title: input.title.clone(),
        short_title: input.short_title.clone(),
        abstract_text: input.abstract_text.clone(),
        doi: input.doi.clone(),
        arxiv_id: input.arxiv_id.clone(),
        url: input.url.clone(),
        pdf_url: input.pdf_url.clone(),
        html_url: input.html_url.clone(),
        thumbnail_url: None,
        published_date: input.published_date.clone(),
        source: input.source.clone().or(Some("mcp".to_string())),
        dir_path: format!("papers/{}", slug),
        extra_json: None,
        entry_type: input.entry_type.clone(),
        journal: input.journal.clone(),
        volume: input.volume.clone(),
        issue: input.issue.clone(),
        pages: input.pages.clone(),
        publisher: input.publisher.clone(),
        issn: input.issn.clone(),
        isbn: input.isbn.clone(),
        added_date: None,
    };

    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let row = papers::insert_paper(&db.conn, &db_input)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("Insert failed: {}", e), None))?;

    // Set authors
    let authors: Vec<(String, Option<String>, Option<String>)> = input
        .authors
        .iter()
        .map(|a| (a.name.clone(), a.affiliation.clone(), None))
        .collect();
    let _ = papers::set_paper_authors(&db.conn, &row.id, &authors);

    // Add tags
    if let Some(tag_names) = &input.tags {
        for tag_name in tag_names {
            let _ = tags::add_tag_to_paper(&db.conn, &row.id, tag_name, "mcp");
        }
    }

    // Write metadata.json
    let tag_list = tags::get_paper_tags(&db.conn, &row.id).unwrap_or_default();
    let paper_dir = state.data_dir.join("library/papers").join(&slug);
    let metadata = PaperMetadata {
        id: row.id.clone(),
        slug: slug.clone(),
        title: input.title,
        short_title: input.short_title,
        authors: input
            .authors
            .iter()
            .map(|a| Author {
                name: a.name.clone(),
                affiliation: a.affiliation.clone(),
                orcid: None,
            })
            .collect(),
        abstract_text: input.abstract_text,
        doi: input.doi,
        arxiv_id: input.arxiv_id,
        url: input.url,
        pdf_url: input.pdf_url,
        html_url: input.html_url,
        thumbnail_url: None,
        published_date: input.published_date,
        added_date: row.added_date.clone(),
        source: input.source.or(Some("mcp".to_string())),
        tags: tag_list.iter().map(|t| t.name.clone()).collect(),
        collections: Vec::new(),
        attachments: Vec::new(),
        notes: Vec::new(),
        read_status: ReadStatus::Unread,
        rating: None,
        extra: serde_json::json!({}),
        entry_type: input.entry_type,
        journal: input.journal,
        volume: input.volume,
        issue: input.issue,
        pages: input.pages,
        publisher: input.publisher,
        issn: input.issn,
        isbn: input.isbn,
        annotations: Vec::new(),
    };
    let _ = zoro_storage::paper_dir::write_metadata(&paper_dir, &metadata);

    let paper_json =
        get_full_paper(&db, &row.id).map_err(|e| rmcp::ErrorData::internal_error(e, None))?;
    sync_after_mutation(state, &db, Some(&row.id));

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&paper_json).unwrap_or_default(),
    )]))
}

pub fn tool_get_paper(
    state: &Arc<AppState>,
    input: GetPaperInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let paper_json =
        get_full_paper(&db, &input.id).map_err(|e| rmcp::ErrorData::internal_error(e, None))?;

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&paper_json).unwrap_or_default(),
    )]))
}

pub fn tool_list_papers(
    state: &Arc<AppState>,
    input: ListPapersInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let filter = papers::PaperFilter {
        collection_id: input.collection_id,
        tag_name: input.tag_name,
        read_status: input.read_status,
        search_query: None,
        uncategorized: input.uncategorized,
        sort_by: input.sort_by,
        sort_order: input.sort_order,
        limit: Some(input.limit.unwrap_or(50)),
        offset: Some(input.offset.unwrap_or(0)),
    };

    let rows = papers::list_papers(&db.conn, &filter)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    let results: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            let author_list = papers::get_paper_authors(&db.conn, &row.id).unwrap_or_default();
            let tag_list = tags::get_paper_tags(&db.conn, &row.id).unwrap_or_default();
            let attachment_list =
                attachments::get_paper_attachments(&db.conn, &row.id).unwrap_or_default();
            let note_list = notes::list_notes(&db.conn, &row.id).unwrap_or_default();
            paper_to_json(row, author_list, tag_list, attachment_list, note_list)
        })
        .collect();

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&results).unwrap_or_default(),
    )]))
}

pub fn tool_search_papers(
    state: &Arc<AppState>,
    input: SearchPapersInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let limit = input.limit.unwrap_or(20);
    let rows = search::search_papers(&db.conn, &input.query, limit)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    let results: Vec<serde_json::Value> = rows
        .iter()
        .filter_map(|row| get_full_paper(&db, &row.id).ok())
        .collect();

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&results).unwrap_or_default(),
    )]))
}

pub fn tool_update_paper(
    state: &Arc<AppState>,
    input: UpdatePaperInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let db_input = papers::UpdatePaperInput {
        title: input.title,
        short_title: input.short_title,
        abstract_text: input.abstract_text,
        doi: input.doi,
        arxiv_id: input.arxiv_id,
        url: input.url,
        pdf_url: input.pdf_url,
        html_url: input.html_url,
        thumbnail_url: None,
        published_date: input.published_date,
        source: None,
        extra_json: None,
        entry_type: input.entry_type,
        journal: input.journal,
        volume: input.volume,
        issue: input.issue,
        pages: input.pages,
        publisher: input.publisher,
        issn: None,
        isbn: None,
    };

    papers::update_paper(&db.conn, &input.id, &db_input)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    sync_after_mutation(state, &db, Some(&input.id));

    let paper_json =
        get_full_paper(&db, &input.id).map_err(|e| rmcp::ErrorData::internal_error(e, None))?;

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&paper_json).unwrap_or_default(),
    )]))
}

pub fn tool_update_paper_status(
    state: &Arc<AppState>,
    input: UpdatePaperStatusInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    papers::update_paper_status(&db.conn, &input.id, &input.read_status)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    sync_after_mutation(state, &db, Some(&input.id));

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Paper {} status updated to '{}'",
        input.id, input.read_status
    ))]))
}

pub fn tool_update_paper_rating(
    state: &Arc<AppState>,
    input: UpdatePaperRatingInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    papers::update_paper_rating(&db.conn, &input.id, input.rating)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    sync_after_mutation(state, &db, Some(&input.id));

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Paper {} rating updated to {:?}",
        input.id, input.rating
    ))]))
}

pub fn tool_delete_paper(
    state: &Arc<AppState>,
    input: DeletePaperInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let row = papers::get_paper(&db.conn, &input.id)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    papers::delete_paper(&db.conn, &input.id)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    zoro_storage::sync::rebuild_library_index(&db, &state.data_dir);
    drop(db);

    let papers_dir = state.data_dir.join("library/papers");
    let _ = zoro_storage::paper_dir::delete_paper_dir(&papers_dir, &row.slug);

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Paper '{}' deleted",
        row.title
    ))]))
}

pub async fn tool_enrich_paper(
    state: &Arc<AppState>,
    input: EnrichPaperInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    // Read DOI/arXiv from DB
    let (doi, arxiv_id) = {
        let db = state
            .db
            .lock()
            .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;
        let row = papers::get_paper(&db.conn, &input.paper_id)
            .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;
        (row.doi.clone(), row.arxiv_id.clone())
    };

    if doi.is_none() && arxiv_id.is_none() {
        return Err(rmcp::ErrorData::invalid_params(
            "Paper has no DOI or arXiv ID to enrich from",
            None,
        ));
    }

    // Call enrichment pipeline (network calls, no DB lock held)
    let enrichment = zoro_metadata::enrich_paper(doi.as_deref(), arxiv_id.as_deref())
        .await
        .map_err(|e| rmcp::ErrorData::internal_error(format!("Enrichment failed: {}", e), None))?;

    // Update paper with enriched data (only fill missing fields)
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let row = papers::get_paper(&db.conn, &input.paper_id)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    let update = papers::UpdatePaperInput {
        title: None,
        short_title: None,
        abstract_text: if row.abstract_text.is_none() {
            enrichment.abstract_text.map(Some)
        } else {
            None
        },
        doi: if row.doi.is_none() {
            enrichment.doi.map(Some)
        } else {
            None
        },
        arxiv_id: None,
        url: None,
        pdf_url: None,
        html_url: None,
        thumbnail_url: None,
        published_date: if row.published_date.is_none() {
            enrichment.published_date.map(Some)
        } else {
            None
        },
        source: None,
        extra_json: None,
        entry_type: if row.entry_type.is_none() {
            enrichment.entry_type.map(Some)
        } else {
            None
        },
        journal: if row.journal.is_none() {
            enrichment.journal.map(Some)
        } else {
            None
        },
        volume: if row.volume.is_none() {
            enrichment.volume.map(Some)
        } else {
            None
        },
        issue: if row.issue.is_none() {
            enrichment.issue.map(Some)
        } else {
            None
        },
        pages: if row.pages.is_none() {
            enrichment.pages.map(Some)
        } else {
            None
        },
        publisher: if row.publisher.is_none() {
            enrichment.publisher.map(Some)
        } else {
            None
        },
        issn: if row.issn.is_none() {
            enrichment.issn.map(Some)
        } else {
            None
        },
        isbn: if row.isbn.is_none() {
            enrichment.isbn.map(Some)
        } else {
            None
        },
    };

    papers::update_paper(&db.conn, &input.paper_id, &update)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    sync_after_mutation(state, &db, Some(&input.paper_id));

    let paper_json = get_full_paper(&db, &input.paper_id)
        .map_err(|e| rmcp::ErrorData::internal_error(e, None))?;

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&paper_json).unwrap_or_default(),
    )]))
}
