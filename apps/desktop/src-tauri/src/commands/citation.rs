// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::commands::library::{paper_row_to_response, PaperResponse};
use crate::connector::handlers::{build_enrichment_update, emit_task, BackgroundTaskEvent};
use crate::storage;
use crate::AppState;
use tauri::{Emitter, State};
use zoro_db::queries::{attachments, citations, notes, papers, tags};
use zoro_metadata::doi_content_negotiation;

#[derive(Debug, serde::Serialize)]
pub struct CitationResponse {
    pub text: String,
    pub source: CitationSource,
    pub cached: bool,
    pub fetched_date: Option<String>,
    pub http_debug: Option<doi_content_negotiation::HttpDebugInfo>,
}

#[derive(Debug, serde::Serialize)]
pub struct CitationSource {
    pub provider: String,
    pub doi: Option<String>,
    pub request_url: Option<String>,
    pub accept_header: Option<String>,
    pub style: String,
}

/// Enrich a paper's metadata from external APIs (CrossRef, Semantic Scholar, OpenAlex).
/// Runs enrichment in the background and emits progress events.
#[tauri::command]
pub async fn enrich_paper_metadata(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    paper_id: String,
) -> Result<PaperResponse, String> {
    let (row, doi, arxiv_id) = {
        let db = state
            .db
            .lock()
            .map_err(|e| format!("DB lock error: {}", e))?;
        let row = papers::get_paper(&db.conn, &paper_id).map_err(|e| format!("{}", e))?;
        let doi = row.doi.clone();
        let arxiv_id = row.arxiv_id.clone();
        (row, doi, arxiv_id)
    };

    let task_id = format!("enrich-{}", paper_id);
    emit_task(
        &app,
        &BackgroundTaskEvent {
            task_id: task_id.clone(),
            paper_id: paper_id.clone(),
            paper_title: row.title.clone(),
            task_type: "enrichment".into(),
            status: "running".into(),
            message: None,
        },
    );

    // Call enrichment pipeline with title fallback (network calls, do NOT hold DB lock)
    let enrichment = match zoro_metadata::enrich_paper_with_title(
        doi.as_deref(),
        arxiv_id.as_deref(),
        Some(&row.title),
    )
    .await
    {
        Ok(e) => e,
        Err(e) => {
            emit_task(
                &app,
                &BackgroundTaskEvent {
                    task_id,
                    paper_id,
                    paper_title: row.title.clone(),
                    task_type: "enrichment".into(),
                    status: "failed".into(),
                    message: Some(format!("{}", e)),
                },
            );
            return Err(format!("Enrichment failed: {}", e));
        }
    };

    // Update paper with enriched data
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;

    let current = papers::get_paper(&db.conn, &paper_id).map_err(|e| format!("{}", e))?;
    let update = build_enrichment_update(&current, &enrichment);
    papers::update_paper(&db.conn, &paper_id, &update).map_err(|e| format!("{}", e))?;

    let _ = citations::delete_paper_citation_cache(&db.conn, &paper_id);

    // Update authors if enrichment found them and we had none
    if let Some(ref enrich_authors) = enrichment.authors {
        if let Ok(existing) = papers::get_paper_authors(&db.conn, &paper_id) {
            if existing.is_empty() && !enrich_authors.is_empty() {
                let tuples: Vec<(String, Option<String>, Option<String>)> = enrich_authors
                    .iter()
                    .map(|(n, a)| (n.clone(), a.clone(), None))
                    .collect();
                let _ = papers::set_paper_authors(&db.conn, &paper_id, &tuples);
            }
        }
    }

    // Sync metadata file
    crate::storage::sync::sync_paper_metadata(&db, &state.data_dir, &paper_id);
    crate::storage::sync::rebuild_library_index(&db, &state.data_dir);

    emit_task(
        &app,
        &BackgroundTaskEvent {
            task_id: task_id.clone(),
            paper_id: paper_id.clone(),
            paper_title: row.title.clone(),
            task_type: "enrichment".into(),
            status: "completed".into(),
            message: None,
        },
    );

    // Fetch arXiv HTML in background if arXiv ID available
    {
        let resolved_arxiv = current
            .arxiv_id
            .as_deref()
            .or(enrichment.arxiv_id.as_deref());
        if let Some(aid) = resolved_arxiv {
            let html_app = app.clone();
            let html_db_path = state.data_dir.join("library.db");
            let html_pid = paper_id.clone();
            let html_title = row.title.clone();
            let html_aid = aid.to_string();
            let html_paper_dir = state.data_dir.join("library").join(&current.dir_path);
            tokio::spawn(async move {
                crate::connector::handlers::fetch_arxiv_html_background(
                    &html_app,
                    &html_db_path,
                    &html_pid,
                    &html_title,
                    &html_aid,
                    &html_paper_dir,
                )
                .await;
            });
        }
    }

    // Download PDF in background if the local file is missing.
    // Use existing pdf_url from DB first, then fall back to enrichment-resolved URL.
    {
        let resolved_pdf_url = current.pdf_url.as_deref().or(enrichment.pdf_url.as_deref());
        if let Some(pdf_url) = resolved_pdf_url {
            let pdf_task_id = format!("pdf-{}", paper_id);
            let paper_dir = state.data_dir.join("library").join(&current.dir_path);
            let pdf_path = paper_dir.join("paper.pdf");
            if !pdf_path.exists() {
                let pdf_url_clone = pdf_url.to_string();
                let pid = paper_id.clone();
                let title = row.title.clone();
                let dbp = state.data_dir.join("library.db");
                let task_app = app.clone();
                emit_task(
                    &app,
                    &BackgroundTaskEvent {
                        task_id: pdf_task_id.clone(),
                        paper_id: pid.clone(),
                        paper_title: title.clone(),
                        task_type: "pdf-download".into(),
                        status: "running".into(),
                        message: Some("Downloading missing PDF".into()),
                    },
                );
                drop(db);
                tokio::spawn(async move {
                    match storage::attachments::download_file(&pdf_url_clone, &pdf_path).await {
                        Ok(()) => {
                            let file_size = storage::attachments::get_file_size(&pdf_path);
                            if let Ok(db) = zoro_db::Database::open(&dbp) {
                                let _ = attachments::insert_attachment(
                                    &db.conn,
                                    &pid,
                                    "paper.pdf",
                                    "pdf",
                                    Some("application/pdf"),
                                    file_size,
                                    "paper.pdf",
                                    "auto-resolved",
                                );
                            }
                            emit_task(
                                &task_app,
                                &BackgroundTaskEvent {
                                    task_id: pdf_task_id,
                                    paper_id: pid.clone(),
                                    paper_title: title,
                                    task_type: "pdf-download".into(),
                                    status: "completed".into(),
                                    message: None,
                                },
                            );
                            let _ = task_app.emit("paper-updated", &pid);
                        }
                        Err(e) => {
                            emit_task(
                                &task_app,
                                &BackgroundTaskEvent {
                                    task_id: pdf_task_id,
                                    paper_id: pid,
                                    paper_title: title,
                                    task_type: "pdf-download".into(),
                                    status: "failed".into(),
                                    message: Some(format!("{}", e)),
                                },
                            );
                        }
                    }
                });

                // DB was dropped, re-acquire for the response below
                let db = state
                    .db
                    .lock()
                    .map_err(|e| format!("DB lock error: {}", e))?;
                let row = papers::get_paper(&db.conn, &paper_id).map_err(|e| format!("{}", e))?;
                let author_list =
                    papers::get_paper_authors(&db.conn, &paper_id).unwrap_or_default();
                let tag_list = tags::get_paper_tags(&db.conn, &paper_id).unwrap_or_default();
                let attachment_list =
                    attachments::get_paper_attachments(&db.conn, &paper_id).unwrap_or_default();
                let note_list = notes::list_notes(&db.conn, &paper_id).unwrap_or_default();
                return Ok(paper_row_to_response(
                    &row,
                    author_list,
                    tag_list,
                    attachment_list,
                    note_list,
                    &state.data_dir,
                ));
            }
        }
    }

    // Return updated paper
    let row = papers::get_paper(&db.conn, &paper_id).map_err(|e| format!("{}", e))?;
    let author_list = papers::get_paper_authors(&db.conn, &paper_id).unwrap_or_default();
    let tag_list = tags::get_paper_tags(&db.conn, &paper_id).unwrap_or_default();
    let attachment_list =
        attachments::get_paper_attachments(&db.conn, &paper_id).unwrap_or_default();
    let note_list = notes::list_notes(&db.conn, &paper_id).unwrap_or_default();
    Ok(paper_row_to_response(
        &row,
        author_list,
        tag_list,
        attachment_list,
        note_list,
        &state.data_dir,
    ))
}

/// Search multiple external APIs for metadata candidates matching a query.
/// Returns a list of candidates for the user to pick from.
#[tauri::command]
pub async fn search_metadata_candidates(
    params: zoro_metadata::MetadataSearchParams,
) -> Result<Vec<zoro_metadata::MetadataCandidate>, String> {
    Ok(zoro_metadata::search_metadata_candidates(&params).await)
}

/// Apply a selected metadata candidate to a paper.
/// Runs full enrichment using the candidate's DOI or arXiv ID,
/// then updates the paper's metadata.
#[tauri::command]
pub async fn apply_metadata_candidate(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    paper_id: String,
    doi: Option<String>,
    arxiv_id: Option<String>,
) -> Result<PaperResponse, String> {
    // If the candidate has a DOI or arXiv, run the full enrichment pipeline
    let enrichment = zoro_metadata::enrich_paper(doi.as_deref(), arxiv_id.as_deref())
        .await
        .map_err(|e| format!("Enrichment failed: {}", e))?;

    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;

    let current = papers::get_paper(&db.conn, &paper_id).map_err(|e| format!("{}", e))?;

    // Build update — for manual apply we FORCE-overwrite DOI/arXiv even if they exist
    let mut update = build_enrichment_update(&current, &enrichment);
    if doi.is_some() {
        update.doi = doi.map(Some);
    }
    if arxiv_id.is_some() {
        update.arxiv_id = arxiv_id.clone().map(Some);
    }

    papers::update_paper(&db.conn, &paper_id, &update).map_err(|e| format!("{}", e))?;
    let _ = citations::delete_paper_citation_cache(&db.conn, &paper_id);

    // Update authors if enrichment found them
    if let Some(ref enrich_authors) = enrichment.authors {
        if !enrich_authors.is_empty() {
            let tuples: Vec<(String, Option<String>, Option<String>)> = enrich_authors
                .iter()
                .map(|(n, a)| (n.clone(), a.clone(), None))
                .collect();
            let _ = papers::set_paper_authors(&db.conn, &paper_id, &tuples);
        }
    }

    // Sync metadata file
    crate::storage::sync::sync_paper_metadata(&db, &state.data_dir, &paper_id);
    crate::storage::sync::rebuild_library_index(&db, &state.data_dir);

    let _ = app.emit("paper-updated", &paper_id);

    let row = papers::get_paper(&db.conn, &paper_id).map_err(|e| format!("{}", e))?;
    let author_list = papers::get_paper_authors(&db.conn, &paper_id).unwrap_or_default();
    let tag_list = tags::get_paper_tags(&db.conn, &paper_id).unwrap_or_default();
    let attachment_list =
        attachments::get_paper_attachments(&db.conn, &paper_id).unwrap_or_default();
    let note_list = notes::list_notes(&db.conn, &paper_id).unwrap_or_default();
    Ok(paper_row_to_response(
        &row,
        author_list,
        tag_list,
        attachment_list,
        note_list,
        &state.data_dir,
    ))
}

/// Get a formatted citation for a paper.
///
/// Supported styles: "bibtex", "apa", "ieee", "mla", "chicago", "vancouver",
/// "harvard", "nature", "science", "ris"
///
/// "ris" is generated locally; all other styles require a DOI
/// and use DOI content negotiation (results are cached per paper+style).
///
/// Returns `CitationResponse` with the citation text, provenance metadata, and cache status.
#[tauri::command]
pub async fn get_formatted_citation(
    state: State<'_, AppState>,
    paper_id: String,
    style: String,
) -> Result<CitationResponse, String> {
    let (row, authors) = {
        let db = state
            .db
            .lock()
            .map_err(|e| format!("DB lock error: {}", e))?;
        let row = papers::get_paper(&db.conn, &paper_id).map_err(|e| format!("{}", e))?;
        let authors = papers::get_paper_authors(&db.conn, &paper_id).unwrap_or_default();
        (row, authors)
    };

    if style == "ris" {
        let paper = row_to_core_paper(&row, authors);
        let text = zoro_core::ris::generate_ris(&[paper]);
        return Ok(CitationResponse {
            text,
            source: CitationSource {
                provider: "Local".into(),
                doi: row.doi.clone(),
                request_url: None,
                accept_header: None,
                style: "ris".into(),
            },
            cached: false,
            fetched_date: None,
            http_debug: None,
        });
    }

    // Check cache
    if let Some(cached) = check_citation_cache(&state, &paper_id, &style)? {
        return Ok(cached);
    }

    let doi = row
        .doi
        .as_deref()
        .ok_or_else(|| "No DOI available for citation lookup".to_string())?;

    let response = fetch_citation_from_doi(doi, &style).await?;

    save_citation_cache(&state, &paper_id, &response)?;

    Ok(response)
}

/// Get BibTeX for a single paper via DOI content negotiation.
/// Returns an error if the paper has no DOI or the fetch fails.
/// Results are cached per paper.
///
/// Returns `CitationResponse` with BibTeX text, provenance metadata, and cache status.
#[tauri::command]
pub async fn get_paper_bibtex(
    state: State<'_, AppState>,
    paper_id: String,
) -> Result<CitationResponse, String> {
    // Check cache
    if let Some(cached) = check_citation_cache(&state, &paper_id, "bibtex")? {
        return Ok(cached);
    }

    let doi = {
        let db = state
            .db
            .lock()
            .map_err(|e| format!("DB lock error: {}", e))?;
        let row = papers::get_paper(&db.conn, &paper_id).map_err(|e| format!("{}", e))?;
        row.doi
            .ok_or_else(|| "No DOI available for BibTeX lookup".to_string())?
    };

    let response = fetch_citation_from_doi(&doi, "bibtex").await?;

    save_citation_cache(&state, &paper_id, &response)?;

    Ok(response)
}

fn check_citation_cache(
    state: &State<'_, AppState>,
    paper_id: &str,
    style: &str,
) -> Result<Option<CitationResponse>, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let cached = citations::get_cached_citation(&db.conn, paper_id, style)
        .map_err(|e| format!("Cache read error: {}", e))?;
    Ok(cached.map(|c| CitationResponse {
        text: c.text,
        source: CitationSource {
            provider: c.provider,
            doi: c.doi,
            request_url: c.request_url,
            accept_header: c.accept_header,
            style: c.style,
        },
        cached: true,
        fetched_date: Some(c.fetched_date),
        http_debug: None,
    }))
}

fn save_citation_cache(
    state: &State<'_, AppState>,
    paper_id: &str,
    response: &CitationResponse,
) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let _ = citations::upsert_citation_cache(
        &db.conn,
        &citations::CitationCacheInput {
            paper_id,
            style: &response.source.style,
            text: &response.text,
            provider: &response.source.provider,
            doi: response.source.doi.as_deref(),
            request_url: response.source.request_url.as_deref(),
            accept_header: response.source.accept_header.as_deref(),
        },
    );
    Ok(())
}

async fn fetch_citation_from_doi(doi: &str, style: &str) -> Result<CitationResponse, String> {
    if style == "bibtex" {
        let (text, debug) = doi_content_negotiation::fetch_bibtex_debug(doi)
            .await
            .map_err(|e| format!("Failed to fetch BibTeX: {}", e))?;
        return Ok(CitationResponse {
            text,
            source: CitationSource {
                provider: "DOI Content Negotiation".into(),
                doi: Some(doi.to_string()),
                request_url: Some(debug.request_url.clone()),
                accept_header: Some("application/x-bibtex".into()),
                style: "bibtex".into(),
            },
            cached: false,
            fetched_date: None,
            http_debug: Some(debug),
        });
    }

    let csl_style = doi_content_negotiation::normalize_style_name(style);
    let (text, debug) = doi_content_negotiation::fetch_formatted_citation_debug(doi, csl_style)
        .await
        .map_err(|e| format!("Failed to fetch citation: {}", e))?;
    let accept = format!("text/x-bibliography; style={}", csl_style);
    Ok(CitationResponse {
        text,
        source: CitationSource {
            provider: "DOI Content Negotiation".into(),
            doi: Some(doi.to_string()),
            request_url: Some(debug.request_url.clone()),
            accept_header: Some(accept),
            style: style.to_string(),
        },
        cached: false,
        fetched_date: None,
        http_debug: Some(debug),
    })
}

fn row_to_core_paper(
    row: &papers::PaperRow,
    authors: Vec<(String, Option<String>, Option<String>)>,
) -> zoro_core::models::Paper {
    zoro_core::models::Paper {
        id: row.id.clone(),
        slug: row.slug.clone(),
        title: row.title.clone(),
        short_title: row.short_title.clone(),
        authors: authors
            .into_iter()
            .map(|(name, aff, orcid)| zoro_core::models::Author {
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
        read_status: zoro_core::models::ReadStatus::Unread,
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
