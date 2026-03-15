// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::sync::Arc;

use rmcp::model::*;
use schemars;

use crate::state::AppState;
use zoro_core::{bibtex, models, ris};
use zoro_db::queries::papers;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ImportBibtexInput {
    /// BibTeX content to import
    pub content: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ExportBibtexInput {
    /// Paper IDs to export (null = export all)
    pub paper_ids: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ImportRisInput {
    /// RIS content to import
    pub content: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ExportRisInput {
    /// Paper IDs to export (null = export all)
    pub paper_ids: Option<Vec<String>>,
}

fn paper_to_create_input(paper: &models::Paper) -> papers::CreatePaperInput {
    let extra_json = if paper.extra == serde_json::json!({}) {
        None
    } else {
        serde_json::to_string(&paper.extra).ok()
    };

    papers::CreatePaperInput {
        slug: paper.slug.clone(),
        title: paper.title.clone(),
        short_title: paper.short_title.clone(),
        abstract_text: paper.abstract_text.clone(),
        doi: paper.doi.clone(),
        arxiv_id: paper.arxiv_id.clone(),
        url: paper.url.clone(),
        pdf_url: paper.pdf_url.clone(),
        html_url: paper.html_url.clone(),
        thumbnail_url: paper.thumbnail_url.clone(),
        published_date: paper.published_date.clone(),
        source: Some("import".to_string()),
        dir_path: format!("papers/{}", paper.slug),
        extra_json,
        entry_type: paper.entry_type.clone(),
        journal: paper.journal.clone(),
        volume: paper.volume.clone(),
        issue: paper.issue.clone(),
        pages: paper.pages.clone(),
        publisher: paper.publisher.clone(),
        issn: paper.issn.clone(),
        isbn: paper.isbn.clone(),
        added_date: None,
    }
}

fn row_to_core_paper(
    row: &papers::PaperRow,
    authors: Vec<(String, Option<String>, Option<String>)>,
) -> models::Paper {
    models::Paper {
        id: row.id.clone(),
        slug: row.slug.clone(),
        title: row.title.clone(),
        short_title: row.short_title.clone(),
        authors: authors
            .into_iter()
            .map(|(name, aff, orcid)| models::Author {
                name,
                affiliation: aff,
                orcid,
            })
            .collect(),
        abstract_text: row.abstract_text.clone(),
        doi: row.doi.clone(),
        arxiv_id: row.arxiv_id.clone(),
        url: row.url.clone(),
        pdf_url: row.pdf_url.clone(),
        html_url: row.html_url.clone(),
        thumbnail_url: row.thumbnail_url.clone(),
        published_date: row.published_date.clone(),
        added_date: row.added_date.clone(),
        modified_date: row.modified_date.clone(),
        source: row.source.clone(),
        tags: Vec::new(),
        collections: Vec::new(),
        attachments: Vec::new(),
        notes: Vec::new(),
        read_status: models::ReadStatus::Unread,
        rating: row.rating.map(|r| r as u8),
        extra: row
            .extra_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_else(|| serde_json::json!({})),
        entry_type: row.entry_type.clone(),
        journal: row.journal.clone(),
        volume: row.volume.clone(),
        issue: row.issue.clone(),
        pages: row.pages.clone(),
        publisher: row.publisher.clone(),
        issn: row.issn.clone(),
        isbn: row.isbn.clone(),
    }
}

pub fn tool_import_bibtex(
    state: &Arc<AppState>,
    input: ImportBibtexInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let parsed = bibtex::parse_bibtex(&input.content)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let mut imported = 0;
    for paper in &parsed {
        let db_input = paper_to_create_input(paper);
        match papers::insert_paper(&db.conn, &db_input) {
            Ok(row) => {
                let authors: Vec<(String, Option<String>, Option<String>)> = paper
                    .authors
                    .iter()
                    .map(|a| (a.name.clone(), a.affiliation.clone(), a.orcid.clone()))
                    .collect();
                let _ = papers::set_paper_authors(&db.conn, &row.id, &authors);

                let papers_dir = state.data_dir.join("library/papers");
                let _ = zoro_storage::paper_dir::create_paper_dir(&papers_dir, &paper.slug);
                imported += 1;
            }
            Err(e) => tracing::warn!("Failed to import paper '{}': {}", paper.title, e),
        }
    }

    zoro_storage::sync::rebuild_library_index(&db, &state.data_dir);

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Imported {} papers from BibTeX (out of {} entries parsed)",
        imported,
        parsed.len()
    ))]))
}

pub fn tool_export_bibtex(
    state: &Arc<AppState>,
    input: ExportBibtexInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let rows = if let Some(ids) = input.paper_ids {
        ids.iter()
            .filter_map(|id| papers::get_paper(&db.conn, id).ok())
            .collect()
    } else {
        let filter = papers::PaperFilter {
            collection_id: None,
            tag_name: None,
            read_status: None,
            search_query: None,
            uncategorized: None,
            sort_by: None,
            sort_order: None,
            limit: Some(10000),
            offset: Some(0),
        };
        papers::list_papers(&db.conn, &filter)
            .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?
    };

    let core_papers: Vec<models::Paper> = rows
        .iter()
        .map(|row| {
            let authors = papers::get_paper_authors(&db.conn, &row.id).unwrap_or_default();
            row_to_core_paper(row, authors)
        })
        .collect();

    Ok(CallToolResult::success(vec![Content::text(
        bibtex::generate_bibtex(&core_papers),
    )]))
}

pub fn tool_import_ris(
    state: &Arc<AppState>,
    input: ImportRisInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let parsed = ris::parse_ris(&input.content)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let mut imported = 0;
    for paper in &parsed {
        let db_input = paper_to_create_input(paper);
        match papers::insert_paper(&db.conn, &db_input) {
            Ok(row) => {
                let authors: Vec<(String, Option<String>, Option<String>)> = paper
                    .authors
                    .iter()
                    .map(|a| (a.name.clone(), a.affiliation.clone(), a.orcid.clone()))
                    .collect();
                let _ = papers::set_paper_authors(&db.conn, &row.id, &authors);

                let papers_dir = state.data_dir.join("library/papers");
                let _ = zoro_storage::paper_dir::create_paper_dir(&papers_dir, &paper.slug);
                imported += 1;
            }
            Err(e) => tracing::warn!("Failed to import paper '{}': {}", paper.title, e),
        }
    }

    zoro_storage::sync::rebuild_library_index(&db, &state.data_dir);

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Imported {} papers from RIS (out of {} entries parsed)",
        imported,
        parsed.len()
    ))]))
}

pub fn tool_export_ris(
    state: &Arc<AppState>,
    input: ExportRisInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let rows = if let Some(ids) = input.paper_ids {
        ids.iter()
            .filter_map(|id| papers::get_paper(&db.conn, id).ok())
            .collect()
    } else {
        let filter = papers::PaperFilter {
            collection_id: None,
            tag_name: None,
            read_status: None,
            search_query: None,
            uncategorized: None,
            sort_by: None,
            sort_order: None,
            limit: Some(10000),
            offset: Some(0),
        };
        papers::list_papers(&db.conn, &filter)
            .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?
    };

    let core_papers: Vec<models::Paper> = rows
        .iter()
        .map(|row| {
            let authors = papers::get_paper_authors(&db.conn, &row.id).unwrap_or_default();
            row_to_core_paper(row, authors)
        })
        .collect();

    Ok(CallToolResult::success(vec![Content::text(
        ris::generate_ris(&core_papers),
    )]))
}
