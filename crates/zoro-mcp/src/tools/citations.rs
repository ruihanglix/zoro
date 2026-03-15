// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::sync::Arc;

use rmcp::model::*;
use schemars;

use crate::state::AppState;
use zoro_core::models;
use zoro_db::queries::papers;
use zoro_metadata::doi_content_negotiation;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetFormattedCitationInput {
    /// Paper ID
    pub paper_id: String,
    /// Citation style: "bibtex", "ris", "apa", "ieee", "mla", "chicago"
    pub style: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetPaperBibtexInput {
    /// Paper ID
    pub paper_id: String,
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

fn generate_local_citation(paper: &models::Paper, style: &str) -> String {
    let authors_str = if paper.authors.is_empty() {
        "Unknown".to_string()
    } else {
        match paper.authors.len() {
            1 => paper.authors[0].name.clone(),
            2 => format!("{} & {}", paper.authors[0].name, paper.authors[1].name),
            _ => format!("{} et al.", paper.authors[0].name),
        }
    };

    let year = paper
        .published_date
        .as_deref()
        .and_then(|d| d.get(..4))
        .unwrap_or("n.d.");

    let title = &paper.title;
    let journal = paper.journal.as_deref().unwrap_or("");
    let volume = paper.volume.as_deref().unwrap_or("");
    let issue = paper.issue.as_deref().unwrap_or("");
    let pages = paper.pages.as_deref().unwrap_or("");
    let doi = paper.doi.as_deref().unwrap_or("");

    match style {
        "apa" => {
            let mut c = format!("{} ({}). {}", authors_str, year, title);
            if !journal.is_empty() {
                c.push_str(&format!(". {}", journal));
                if !volume.is_empty() {
                    c.push_str(&format!(", {}", volume));
                    if !issue.is_empty() {
                        c.push_str(&format!("({})", issue));
                    }
                }
                if !pages.is_empty() {
                    c.push_str(&format!(", {}", pages));
                }
            }
            c.push('.');
            if !doi.is_empty() {
                c.push_str(&format!(" https://doi.org/{}", doi));
            }
            c
        }
        "ieee" => {
            let mut c = format!("{}, \"{}\"", authors_str, title);
            if !journal.is_empty() {
                c.push_str(&format!(", {}", journal));
            }
            if !volume.is_empty() {
                c.push_str(&format!(", vol. {}", volume));
            }
            if !issue.is_empty() {
                c.push_str(&format!(", no. {}", issue));
            }
            if !pages.is_empty() {
                c.push_str(&format!(", pp. {}", pages));
            }
            c.push_str(&format!(", {}.", year));
            if !doi.is_empty() {
                c.push_str(&format!(" doi: {}.", doi));
            }
            c
        }
        _ => {
            // Generic/Chicago-ish fallback
            let mut c = format!("{} ({}). \"{}\"", authors_str, year, title);
            if !journal.is_empty() {
                c.push_str(&format!(". {}", journal));
                if !volume.is_empty() {
                    c.push_str(&format!(" {}", volume));
                }
                if !pages.is_empty() {
                    c.push_str(&format!(": {}", pages));
                }
            }
            c.push('.');
            if !doi.is_empty() {
                c.push_str(&format!(" https://doi.org/{}", doi));
            }
            c
        }
    }
}

pub async fn tool_get_formatted_citation(
    state: &Arc<AppState>,
    input: GetFormattedCitationInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let (row, authors) = {
        let db = state
            .db
            .lock()
            .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;
        let row = papers::get_paper(&db.conn, &input.paper_id)
            .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;
        let authors = papers::get_paper_authors(&db.conn, &input.paper_id).unwrap_or_default();
        (row, authors)
    };

    let paper = row_to_core_paper(&row, authors);

    if input.style == "bibtex" {
        return Ok(CallToolResult::success(vec![Content::text(
            zoro_core::bibtex::generate_bibtex(&[paper]),
        )]));
    }

    if input.style == "ris" {
        return Ok(CallToolResult::success(vec![Content::text(
            zoro_core::ris::generate_ris(&[paper]),
        )]));
    }

    // Try DOI content negotiation first
    if let Some(ref doi) = row.doi {
        let csl_style = doi_content_negotiation::normalize_style_name(&input.style);
        if let Ok(citation) =
            doi_content_negotiation::fetch_formatted_citation(doi, csl_style).await
        {
            return Ok(CallToolResult::success(vec![Content::text(citation)]));
        }
    }

    // Fallback: generate locally
    Ok(CallToolResult::success(vec![Content::text(
        generate_local_citation(&paper, &input.style),
    )]))
}

pub async fn tool_get_paper_bibtex(
    state: &Arc<AppState>,
    input: GetPaperBibtexInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let (row, authors) = {
        let db = state
            .db
            .lock()
            .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;
        let row = papers::get_paper(&db.conn, &input.paper_id)
            .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;
        let authors = papers::get_paper_authors(&db.conn, &input.paper_id).unwrap_or_default();
        (row, authors)
    };

    // Try DOI content negotiation for richer BibTeX
    if let Some(ref doi) = row.doi {
        if let Ok(bibtex) = doi_content_negotiation::fetch_bibtex(doi).await {
            return Ok(CallToolResult::success(vec![Content::text(bibtex)]));
        }
    }

    // Fallback: generate locally
    let paper = row_to_core_paper(&row, authors);
    Ok(CallToolResult::success(vec![Content::text(
        zoro_core::bibtex::generate_bibtex(&[paper]),
    )]))
}
