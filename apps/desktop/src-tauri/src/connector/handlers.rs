// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use super::server::ConnectorState;
use axum::{extract::State, Json};
use std::sync::Arc;
use tauri::{Emitter, Manager};

/// Event payload for background task progress, emitted as `"background-task"`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BackgroundTaskEvent {
    pub task_id: String,
    pub paper_id: String,
    pub paper_title: String,
    pub task_type: String,
    pub status: String,
    pub message: Option<String>,
}

/// Emit a background-task event. Errors are silently ignored.
pub fn emit_task(app: &tauri::AppHandle, event: &BackgroundTaskEvent) {
    let _ = app.emit("background-task", event);
}

#[derive(serde::Serialize)]
pub struct PingResponse {
    pub version: String,
    pub name: String,
}

#[derive(serde::Deserialize)]
pub struct SaveItemRequest {
    pub title: String,
    pub short_title: Option<String>,
    pub authors: Option<Vec<String>>,
    pub url: Option<String>,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    pub pdf_url: Option<String>,
    pub html_url: Option<String>,
    pub abstract_text: Option<String>,
    pub tags: Option<Vec<String>>,
    pub entry_type: Option<String>,
    pub journal: Option<String>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub pages: Option<String>,
    pub publisher: Option<String>,
}

#[derive(serde::Serialize)]
pub struct SaveItemResponse {
    pub success: bool,
    pub paper_id: Option<String>,
    pub message: String,
}

#[derive(serde::Deserialize)]
pub struct SaveHtmlRequest {
    pub paper_id: String,
    pub html_content: String,
}

#[derive(serde::Serialize)]
pub struct StatusResponse {
    pub status: String,
    pub current_save: Option<String>,
}

#[derive(serde::Serialize)]
pub struct CollectionItem {
    pub id: String,
    pub name: String,
}

pub async fn ping(_state: State<Arc<ConnectorState>>) -> Json<PingResponse> {
    Json(PingResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
        name: "Zoro".to_string(),
    })
}

pub async fn save_item(
    state: State<Arc<ConnectorState>>,
    Json(req): Json<SaveItemRequest>,
) -> Json<SaveItemResponse> {
    // Access the Tauri app state to save the item
    let app_state: tauri::State<crate::AppState> = state.app.state();

    let input = crate::commands::library::AddPaperInput {
        title: req.title,
        short_title: req.short_title.clone(),
        authors: req
            .authors
            .unwrap_or_default()
            .into_iter()
            .map(|name| crate::commands::library::AuthorInput {
                name,
                affiliation: None,
            })
            .collect(),
        abstract_text: req.abstract_text,
        doi: req.doi,
        arxiv_id: req.arxiv_id,
        url: req.url,
        pdf_url: req.pdf_url,
        html_url: req.html_url,
        published_date: None,
        source: Some("browser-extension".to_string()),
        tags: req.tags,
        entry_type: req.entry_type,
        journal: req.journal,
        volume: req.volume,
        issue: req.issue,
        pages: req.pages,
        publisher: req.publisher,
        issn: None,
        isbn: None,
        extra_json: None,
    };

    let identifier = input
        .doi
        .as_deref()
        .or(input.arxiv_id.as_deref())
        .unwrap_or(&input.title);
    let slug = zoro_core::slug_utils::generate_paper_slug(&input.title, identifier, None);

    let db = match app_state.db.lock() {
        Ok(db) => db,
        Err(e) => {
            return Json(SaveItemResponse {
                success: false,
                paper_id: None,
                message: format!("DB lock error: {}", e),
            })
        }
    };

    // Clone values needed for background tasks before moving into db_input
    let bg_doi = input.doi.clone();
    let bg_arxiv = input.arxiv_id.clone();
    let bg_pdf_url = input.pdf_url.clone();
    let bg_html_url = input.html_url.clone();

    let db_input = zoro_db::queries::papers::CreatePaperInput {
        slug: slug.clone(),
        title: input.title.clone(),
        short_title: input.short_title.clone(),
        abstract_text: input.abstract_text,
        doi: input.doi,
        arxiv_id: input.arxiv_id,
        url: input.url,
        pdf_url: input.pdf_url,
        html_url: input.html_url,
        thumbnail_url: None,
        published_date: None,
        source: Some("browser-extension".to_string()),
        dir_path: format!("papers/{}", slug),
        extra_json: None,
        entry_type: input.entry_type,
        journal: input.journal,
        volume: input.volume,
        issue: input.issue,
        pages: input.pages,
        publisher: input.publisher,
        issn: input.issn,
        isbn: input.isbn,
        added_date: None,
    };

    match zoro_db::queries::papers::insert_paper(&db.conn, &db_input) {
        Ok(row) => {
            let papers_dir = app_state.data_dir.join("library/papers");
            let paper_dir = papers_dir.join(&slug);
            let _ = crate::storage::paper_dir::create_paper_dir(&papers_dir, &slug);

            // Notify frontend of new paper
            let _ = state.app.emit("paper-saved", &row.id);

            let paper_id = row.id.clone();
            let paper_title = db_input.title.clone();
            let db_path = app_state.data_dir.join("library.db");
            let paper_dir_clone = paper_dir.clone();
            let app_handle = state.app.clone();
            drop(db);

            // Download PDF in background if URL was provided
            if let Some(ref pdf_url) = bg_pdf_url {
                let pdf_path = paper_dir.join("paper.pdf");
                let pdf_url_clone = pdf_url.clone();
                let pid = paper_id.clone();
                let dbp = db_path.clone();
                let app = app_handle.clone();
                let title = paper_title.clone();
                let task_id = format!("pdf-{}", pid);
                emit_task(
                    &app,
                    &BackgroundTaskEvent {
                        task_id: task_id.clone(),
                        paper_id: pid.clone(),
                        paper_title: title.clone(),
                        task_type: "pdf-download".into(),
                        status: "running".into(),
                        message: None,
                    },
                );
                tokio::spawn(async move {
                    match crate::storage::attachments::download_file(&pdf_url_clone, &pdf_path)
                        .await
                    {
                        Ok(()) => {
                            let file_size = crate::storage::attachments::get_file_size(&pdf_path);
                            if let Ok(db) = zoro_db::Database::open(&dbp) {
                                let _ = zoro_db::queries::attachments::insert_attachment(
                                    &db.conn,
                                    &pid,
                                    "paper.pdf",
                                    "pdf",
                                    Some("application/pdf"),
                                    file_size,
                                    "paper.pdf",
                                    "browser-extension",
                                );
                            }
                            emit_task(
                                &app,
                                &BackgroundTaskEvent {
                                    task_id,
                                    paper_id: pid.clone(),
                                    paper_title: title,
                                    task_type: "pdf-download".into(),
                                    status: "completed".into(),
                                    message: None,
                                },
                            );
                            let _ = app.emit("paper-updated", &pid);
                        }
                        Err(e) => {
                            emit_task(
                                &app,
                                &BackgroundTaskEvent {
                                    task_id,
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
            }

            // Download HTML in background if URL was provided
            if let Some(ref html_url) = bg_html_url {
                let html_path = paper_dir.join("abs.html");
                let html_url_clone = html_url.clone();
                let pid = paper_id.clone();
                let dbp = db_path.clone();
                let html_app = app_handle.clone();
                tokio::spawn(async move {
                    if let Ok(()) =
                        crate::storage::attachments::download_file(&html_url_clone, &html_path)
                            .await
                    {
                        let file_size = crate::storage::attachments::get_file_size(&html_path);
                        if let Ok(db) = zoro_db::Database::open(&dbp) {
                            let _ = zoro_db::queries::attachments::insert_attachment(
                                &db.conn,
                                &pid,
                                "abs.html",
                                "html",
                                Some("text/html"),
                                file_size,
                                "abs.html",
                                "browser-extension",
                            );
                        }
                        let _ = html_app.emit("paper-updated", &pid);
                    }
                });
            }

            // Background enrichment + PDF resolution (title-based search as fallback)
            {
                let enrich_id = paper_id.clone();
                let enrich_doi = bg_doi;
                let enrich_arxiv = bg_arxiv;
                let enrich_db_path = db_path.clone();
                let enrich_pdf_url = bg_pdf_url.clone();
                let enrich_paper_dir = paper_dir_clone;
                let enrich_app = app_handle.clone();
                let enrich_title = paper_title.clone();
                let enrich_task_id = format!("enrich-{}", enrich_id);
                emit_task(
                    &enrich_app,
                    &BackgroundTaskEvent {
                        task_id: enrich_task_id.clone(),
                        paper_id: enrich_id.clone(),
                        paper_title: enrich_title.clone(),
                        task_type: "enrichment".into(),
                        status: "running".into(),
                        message: None,
                    },
                );
                tokio::spawn(async move {
                    match zoro_metadata::enrich_paper_with_title(
                        enrich_doi.as_deref(),
                        enrich_arxiv.as_deref(),
                        Some(&enrich_title),
                    )
                    .await
                    {
                        Ok(enrichment) => {
                            if let Ok(db) = zoro_db::Database::open(&enrich_db_path) {
                                if let Ok(current) =
                                    zoro_db::queries::papers::get_paper(&db.conn, &enrich_id)
                                {
                                    let update =
                                        crate::connector::handlers::build_enrichment_update(
                                            &current,
                                            &enrichment,
                                        );
                                    let _ = zoro_db::queries::papers::update_paper(
                                        &db.conn, &enrich_id, &update,
                                    );

                                    if let Some(ref enrich_authors) = enrichment.authors {
                                        if let Ok(existing) =
                                            zoro_db::queries::papers::get_paper_authors(
                                                &db.conn, &enrich_id,
                                            )
                                        {
                                            if existing.is_empty() && !enrich_authors.is_empty() {
                                                let tuples: Vec<(
                                                    String,
                                                    Option<String>,
                                                    Option<String>,
                                                )> = enrich_authors
                                                    .iter()
                                                    .map(|(n, a)| (n.clone(), a.clone(), None))
                                                    .collect();
                                                let _ = zoro_db::queries::papers::set_paper_authors(
                                                    &db.conn, &enrich_id, &tuples,
                                                );
                                            }
                                        }
                                    }
                                }

                                // Download PDF if resolved and not already provided
                                if enrich_pdf_url.is_none() {
                                    if let Some(ref pdf_url) = enrichment.pdf_url {
                                        let pdf_task_id = format!("pdf-{}", enrich_id);
                                        emit_task(
                                            &enrich_app,
                                            &BackgroundTaskEvent {
                                                task_id: pdf_task_id.clone(),
                                                paper_id: enrich_id.clone(),
                                                paper_title: enrich_title.clone(),
                                                task_type: "pdf-download".into(),
                                                status: "running".into(),
                                                message: Some("PDF found via OA lookup".into()),
                                            },
                                        );
                                        let pdf_path = enrich_paper_dir.join("paper.pdf");
                                        if !pdf_path.exists() {
                                            match crate::storage::attachments::download_file(
                                                pdf_url, &pdf_path,
                                            )
                                            .await
                                            {
                                                Ok(()) => {
                                                    let file_size =
                                                        crate::storage::attachments::get_file_size(
                                                            &pdf_path,
                                                        );
                                                    let _ = zoro_db::queries::attachments::insert_attachment(
                                                        &db.conn,
                                                        &enrich_id,
                                                        "paper.pdf",
                                                        "pdf",
                                                        Some("application/pdf"),
                                                        file_size,
                                                        "paper.pdf",
                                                        "auto-resolved",
                                                    );
                                                    emit_task(
                                                        &enrich_app,
                                                        &BackgroundTaskEvent {
                                                            task_id: pdf_task_id,
                                                            paper_id: enrich_id.clone(),
                                                            paper_title: enrich_title.clone(),
                                                            task_type: "pdf-download".into(),
                                                            status: "completed".into(),
                                                            message: None,
                                                        },
                                                    );
                                                }
                                                Err(e) => {
                                                    emit_task(
                                                        &enrich_app,
                                                        &BackgroundTaskEvent {
                                                            task_id: pdf_task_id,
                                                            paper_id: enrich_id.clone(),
                                                            paper_title: enrich_title.clone(),
                                                            task_type: "pdf-download".into(),
                                                            status: "failed".into(),
                                                            message: Some(format!("{}", e)),
                                                        },
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // Fetch arXiv HTML if we have an arXiv ID
                            let resolved_arxiv =
                                enrich_arxiv.as_deref().or(enrichment.arxiv_id.as_deref());
                            if let Some(aid) = resolved_arxiv {
                                fetch_arxiv_html_background(
                                    &enrich_app,
                                    &enrich_db_path,
                                    &enrich_id,
                                    &enrich_title,
                                    aid,
                                    &enrich_paper_dir,
                                )
                                .await;
                            }

                            emit_task(
                                &enrich_app,
                                &BackgroundTaskEvent {
                                    task_id: enrich_task_id,
                                    paper_id: enrich_id.clone(),
                                    paper_title: enrich_title,
                                    task_type: "enrichment".into(),
                                    status: "completed".into(),
                                    message: None,
                                },
                            );
                            let _ = enrich_app.emit("paper-updated", &enrich_id);
                        }
                        Err(e) => {
                            emit_task(
                                &enrich_app,
                                &BackgroundTaskEvent {
                                    task_id: enrich_task_id,
                                    paper_id: enrich_id,
                                    paper_title: enrich_title,
                                    task_type: "enrichment".into(),
                                    status: "failed".into(),
                                    message: Some(format!("{}", e)),
                                },
                            );
                        }
                    }
                });
            }

            Json(SaveItemResponse {
                success: true,
                paper_id: Some(paper_id),
                message: "Paper saved successfully".to_string(),
            })
        }
        Err(e) => Json(SaveItemResponse {
            success: false,
            paper_id: None,
            message: format!("Failed to save paper: {}", e),
        }),
    }
}

/// Build an update input from enrichment results.
///
/// Most fields are only filled in when the current paper has `None`. But `entry_type`
/// is always overwritten when the enrichment provides a value — APIs like CrossRef
/// and Semantic Scholar are more authoritative than browser-extension guesses.
/// Similarly, `pdf_url` is updated if the enrichment resolved one and the paper
/// didn't already have it.
pub fn build_enrichment_update(
    current: &zoro_db::queries::papers::PaperRow,
    enrichment: &zoro_metadata::EnrichmentResult,
) -> zoro_db::queries::papers::UpdatePaperInput {
    zoro_db::queries::papers::UpdatePaperInput {
        title: None,
        short_title: None,
        abstract_text: if current.abstract_text.is_none() {
            enrichment.abstract_text.as_ref().map(|v| Some(v.clone()))
        } else {
            None
        },
        doi: if current.doi.is_none() {
            enrichment.doi.as_ref().map(|v| Some(v.clone()))
        } else {
            None
        },
        arxiv_id: if current.arxiv_id.is_none() {
            enrichment.arxiv_id.as_ref().map(|v| Some(v.clone()))
        } else {
            None
        },
        url: None,
        pdf_url: if current.pdf_url.is_none() {
            enrichment.pdf_url.as_ref().map(|v| Some(v.clone()))
        } else {
            None
        },
        html_url: None,
        thumbnail_url: None,
        published_date: if current.published_date.is_none() {
            enrichment.published_date.as_ref().map(|v| Some(v.clone()))
        } else {
            None
        },
        source: None,
        extra_json: None,
        // entry_type: always prefer the authoritative API value
        entry_type: enrichment.entry_type.as_ref().map(|v| Some(v.clone())),
        journal: if current.journal.is_none() {
            enrichment.journal.as_ref().map(|v| Some(v.clone()))
        } else {
            None
        },
        volume: if current.volume.is_none() {
            enrichment.volume.as_ref().map(|v| Some(v.clone()))
        } else {
            None
        },
        issue: if current.issue.is_none() {
            enrichment.issue.as_ref().map(|v| Some(v.clone()))
        } else {
            None
        },
        pages: if current.pages.is_none() {
            enrichment.pages.as_ref().map(|v| Some(v.clone()))
        } else {
            None
        },
        publisher: if current.publisher.is_none() {
            enrichment.publisher.as_ref().map(|v| Some(v.clone()))
        } else {
            None
        },
        issn: if current.issn.is_none() {
            enrichment.issn.as_ref().map(|v| Some(v.clone()))
        } else {
            None
        },
        isbn: if current.isbn.is_none() {
            enrichment.isbn.as_ref().map(|v| Some(v.clone()))
        } else {
            None
        },
    }
}

/// Fetch arXiv HTML in the background, clean it, and record the attachment.
/// Emits `background-task` events for progress.  Does nothing if abs.html already exists.
pub async fn fetch_arxiv_html_background(
    app: &tauri::AppHandle,
    db_path: &std::path::Path,
    paper_id: &str,
    paper_title: &str,
    arxiv_id: &str,
    paper_dir: &std::path::Path,
) {
    let html_path = paper_dir.join("paper.html");
    if html_path.exists() {
        return;
    }

    let task_id = format!("html-{}", paper_id);
    emit_task(
        app,
        &BackgroundTaskEvent {
            task_id: task_id.clone(),
            paper_id: paper_id.to_string(),
            paper_title: paper_title.to_string(),
            task_type: "html-download".into(),
            status: "running".into(),
            message: Some("Fetching arXiv HTML".into()),
        },
    );

    match zoro_arxiv::fetch::fetch_and_save(arxiv_id, &html_path).await {
        Ok(()) => {
            let _ = zoro_arxiv::clean::clean_html_file(&html_path, &[]).await;
            let file_size = crate::storage::attachments::get_file_size(&html_path);
            if let Ok(db) = zoro_db::Database::open(db_path) {
                // Remove any stale HTML attachment records first
                if let Ok(atts) =
                    zoro_db::queries::attachments::get_paper_attachments(&db.conn, paper_id)
                {
                    for att in &atts {
                        if att.file_type == "html" {
                            let _ =
                                zoro_db::queries::attachments::delete_attachment(&db.conn, &att.id);
                        }
                    }
                }
                let _ = zoro_db::queries::attachments::insert_attachment(
                    &db.conn,
                    paper_id,
                    "paper.html",
                    "html",
                    Some("text/html"),
                    file_size,
                    "paper.html",
                    "auto-enrichment",
                );
            }
            emit_task(
                app,
                &BackgroundTaskEvent {
                    task_id,
                    paper_id: paper_id.to_string(),
                    paper_title: paper_title.to_string(),
                    task_type: "html-download".into(),
                    status: "completed".into(),
                    message: None,
                },
            );
            let _ = app.emit("paper-updated", paper_id);
        }
        Err(e) => {
            tracing::debug!(
                arxiv_id = %arxiv_id,
                error = %e,
                "arXiv HTML fetch failed during enrichment"
            );
            emit_task(
                app,
                &BackgroundTaskEvent {
                    task_id,
                    paper_id: paper_id.to_string(),
                    paper_title: paper_title.to_string(),
                    task_type: "html-download".into(),
                    status: "failed".into(),
                    message: Some(format!("{}", e)),
                },
            );
        }
    }
}

pub async fn save_html(
    state: State<Arc<ConnectorState>>,
    Json(req): Json<SaveHtmlRequest>,
) -> Json<SaveItemResponse> {
    let app_state: tauri::State<crate::AppState> = state.app.state();

    let db = match app_state.db.lock() {
        Ok(db) => db,
        Err(e) => {
            return Json(SaveItemResponse {
                success: false,
                paper_id: None,
                message: format!("DB lock error: {}", e),
            })
        }
    };

    match zoro_db::queries::papers::get_paper(&db.conn, &req.paper_id) {
        Ok(paper) => {
            let paper_dir = app_state.data_dir.join("library/papers").join(&paper.slug);
            let html_path = paper_dir.join("abs.html");
            if let Err(e) = std::fs::write(&html_path, &req.html_content) {
                return Json(SaveItemResponse {
                    success: false,
                    paper_id: Some(req.paper_id),
                    message: format!("Failed to write HTML: {}", e),
                });
            }
            Json(SaveItemResponse {
                success: true,
                paper_id: Some(req.paper_id),
                message: "HTML saved successfully".to_string(),
            })
        }
        Err(e) => Json(SaveItemResponse {
            success: false,
            paper_id: None,
            message: format!("Paper not found: {}", e),
        }),
    }
}

pub async fn status(_state: State<Arc<ConnectorState>>) -> Json<StatusResponse> {
    Json(StatusResponse {
        status: "ready".to_string(),
        current_save: None,
    })
}

pub async fn list_collections(state: State<Arc<ConnectorState>>) -> Json<Vec<CollectionItem>> {
    let app_state: tauri::State<crate::AppState> = state.app.state();
    let db = match app_state.db.lock() {
        Ok(db) => db,
        Err(_) => return Json(Vec::new()),
    };

    let collections = zoro_db::queries::collections::list_collections(&db.conn).unwrap_or_default();

    Json(
        collections
            .into_iter()
            .map(|c| CollectionItem {
                id: c.id,
                name: c.name,
            })
            .collect(),
    )
}
