// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use super::mapping;
use super::server::ZoteroCompatState;
use super::types::*;
use axum::{
    body::Bytes,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use zoro_core::slug_utils::generate_paper_slug;
use zoro_db::queries::{attachments, collections, papers};

// ─── Helper ──────────────────────────────────────────────────────────────────

/// Build a response with the X-Zotero-Version header.
fn zotero_response<T: serde::Serialize>(body: T) -> Response {
    let mut resp = Json(body).into_response();
    resp.headers_mut()
        .insert("X-Zotero-Version", "7.0.0".parse().unwrap());
    resp
}

fn zotero_ok() -> Response {
    let mut resp = StatusCode::OK.into_response();
    resp.headers_mut()
        .insert("X-Zotero-Version", "7.0.0".parse().unwrap());
    resp
}

fn zotero_error(status: StatusCode, msg: &str) -> Response {
    let mut resp = (status, msg.to_string()).into_response();
    resp.headers_mut()
        .insert("X-Zotero-Version", "7.0.0".parse().unwrap());
    resp
}

// ─── Core Endpoints ──────────────────────────────────────────────────────────

/// POST /connector/ping
pub async fn ping(
    _state: State<Arc<ZoteroCompatState>>,
    _body: Option<Json<ZoteroPingRequest>>,
) -> Response {
    let resp = ZoteroPingResponse {
        prefs: ZoteroPrefs {
            download_associated_files: true,
            report_active_url: false,
            automatic_snapshots: true,
            supports_attachment_upload: true,
            supports_tags_autocomplete: true,
            can_user_add_note: true,
        },
    };
    zotero_response(resp)
}

/// POST /connector/getSelectedCollection
pub async fn get_selected_collection(
    state: State<Arc<ZoteroCompatState>>,
    _body: Option<Json<GetSelectedCollectionRequest>>,
) -> Response {
    let app_state: tauri::State<crate::AppState> = state.app.state();
    let db = match app_state.db.lock() {
        Ok(db) => db,
        Err(e) => return zotero_error(StatusCode::INTERNAL_SERVER_ERROR, &format!("{}", e)),
    };

    let collection_rows = collections::list_collections(&db.conn).unwrap_or_default();

    // Build targets: first entry is "My Library", then all collections
    let mut targets = vec![CollectionTarget {
        id: "L1".to_string(),
        name: "My Library".to_string(),
        level: 0,
        files_editable: true,
    }];

    for c in &collection_rows {
        let level = if c.parent_id.is_some() { 2 } else { 1 };
        targets.push(CollectionTarget {
            id: format!("C{}", c.id),
            name: c.name.clone(),
            level,
            files_editable: true,
        });
    }

    let resp = GetSelectedCollectionResponse {
        id: "L1".to_string(),
        name: "My Library".to_string(),
        library_id: 1,
        library_editable: true,
        files_editable: true,
        targets,
    };
    zotero_response(resp)
}

/// POST /connector/saveItems — the primary save endpoint
pub async fn save_items(
    state: State<Arc<ZoteroCompatState>>,
    Json(req): Json<ZoteroSaveItemsRequest>,
) -> Response {
    let app_state: tauri::State<crate::AppState> = state.app.state();

    // Create session
    state.sessions.create_session(&req.session_id);

    let db = match app_state.db.lock() {
        Ok(db) => db,
        Err(e) => return zotero_error(StatusCode::INTERNAL_SERVER_ERROR, &format!("{}", e)),
    };

    let mut result_items = Vec::new();

    // Collect data for background enrichment tasks (spawned after DB lock is dropped)
    struct EnrichInfo {
        paper_id: String,
        paper_title: String,
        slug: String,
        doi: Option<String>,
        arxiv_id: Option<String>,
    }
    let mut enrich_queue: Vec<EnrichInfo> = Vec::new();

    for item in &req.items {
        let zotero_item_id = item.id.clone().unwrap_or_default();
        let input = mapping::zotero_item_to_paper_input(item);

        let identifier = input
            .doi
            .as_deref()
            .or(input.arxiv_id.as_deref())
            .unwrap_or(&input.title);
        let slug = generate_paper_slug(&input.title, identifier, input.published_date.as_deref());

        let db_input = papers::CreatePaperInput {
            slug: slug.clone(),
            title: input.title.clone(),
            short_title: input.short_title.clone(),
            abstract_text: input.abstract_text.clone(),
            doi: input.doi.clone(),
            arxiv_id: input.arxiv_id.clone(),
            url: input.url.clone(),
            pdf_url: None,
            html_url: None,
            thumbnail_url: None,
            published_date: input.published_date.clone(),
            source: Some("zotero-connector".to_string()),
            dir_path: format!("papers/{}", slug),
            extra_json: input.extra_json.clone(),
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

        match papers::insert_paper(&db.conn, &db_input) {
            Ok(row) => {
                // Create paper directory
                let papers_dir = app_state.data_dir.join("library/papers");
                let _ = crate::storage::paper_dir::create_paper_dir(&papers_dir, &slug);

                // Set authors
                let authors: Vec<(String, Option<String>, Option<String>)> = input
                    .authors
                    .iter()
                    .map(|a| (a.name.clone(), a.affiliation.clone(), None))
                    .collect();
                let _ = papers::set_paper_authors(&db.conn, &row.id, &authors);

                // Note: Zotero tags are stored as labels in extra_json (via mapping),
                // NOT as sidebar tags. Sidebar tags are user-curated only.

                // Register in session
                state
                    .sessions
                    .register_item(&req.session_id, &zotero_item_id, &row.id);

                // Build attachment progress list
                let mut att_progress = Vec::new();
                if let Some(ref atts) = item.attachments {
                    for att in atts {
                        let att_id = att.id.clone().unwrap_or_default();
                        if !att_id.is_empty() {
                            state.sessions.register_attachment(
                                &req.session_id,
                                &att_id,
                                &zotero_item_id,
                            );
                            att_progress.push(ZoteroAttachmentProgress {
                                id: att_id,
                                progress: serde_json::Value::Number(0.into()),
                            });
                        }
                    }
                }

                // Sync metadata
                crate::storage::sync::sync_paper_metadata(&db, &app_state.data_dir, &row.id);

                // Queue for background enrichment
                enrich_queue.push(EnrichInfo {
                    paper_id: row.id.clone(),
                    paper_title: input.title.clone(),
                    slug: slug.clone(),
                    doi: input.doi.clone(),
                    arxiv_id: input.arxiv_id.clone(),
                });

                result_items.push(ZoteroSaveItemResult {
                    id: zotero_item_id,
                    attachments: att_progress,
                });
            }
            Err(e) => {
                tracing::error!("Failed to save Zotero item: {}", e);
                // Still include in results but with empty attachments
                result_items.push(ZoteroSaveItemResult {
                    id: zotero_item_id,
                    attachments: Vec::new(),
                });
            }
        }
    }

    // If no attachments were registered, mark session as done
    let has_attachments = result_items.iter().any(|i| !i.attachments.is_empty());
    if !has_attachments {
        state.sessions.mark_done(&req.session_id);
    }

    // Rebuild library index
    crate::storage::sync::rebuild_library_index(&db, &app_state.data_dir);

    // Notify frontend of saved papers
    let _ = state.app.emit("paper-saved", "batch");

    let db_path = app_state.data_dir.join("library.db");
    let papers_dir = app_state.data_dir.join("library/papers");
    drop(db);

    // Spawn background enrichment tasks for each saved paper
    for info in enrich_queue {
        let app_handle = state.app.clone();
        let db_path = db_path.clone();
        let paper_dir = papers_dir.join(&info.slug);

        let task_id = format!("enrich-{}", info.paper_id);
        crate::connector::handlers::emit_task(
            &app_handle,
            &crate::connector::handlers::BackgroundTaskEvent {
                task_id: task_id.clone(),
                paper_id: info.paper_id.clone(),
                paper_title: info.paper_title.clone(),
                task_type: "enrichment".into(),
                status: "running".into(),
                message: None,
            },
        );

        tokio::spawn(async move {
            match zoro_metadata::enrich_paper_with_title(
                info.doi.as_deref(),
                info.arxiv_id.as_deref(),
                Some(&info.paper_title),
            )
            .await
            {
                Ok(enrichment) => {
                    if let Ok(db) = zoro_db::Database::open(&db_path) {
                        if let Ok(current) =
                            zoro_db::queries::papers::get_paper(&db.conn, &info.paper_id)
                        {
                            let update = crate::connector::handlers::build_enrichment_update(
                                &current,
                                &enrichment,
                            );
                            let _ = zoro_db::queries::papers::update_paper(
                                &db.conn,
                                &info.paper_id,
                                &update,
                            );

                            if let Some(ref enrich_authors) = enrichment.authors {
                                if let Ok(existing) = zoro_db::queries::papers::get_paper_authors(
                                    &db.conn,
                                    &info.paper_id,
                                ) {
                                    if existing.is_empty() && !enrich_authors.is_empty() {
                                        let tuples: Vec<(String, Option<String>, Option<String>)> =
                                            enrich_authors
                                                .iter()
                                                .map(|(n, a)| (n.clone(), a.clone(), None))
                                                .collect();
                                        let _ = zoro_db::queries::papers::set_paper_authors(
                                            &db.conn,
                                            &info.paper_id,
                                            &tuples,
                                        );
                                    }
                                }
                            }
                        }

                        // Download PDF if resolved by enrichment (skip if plugin already uploaded one)
                        let has_pdf_att = zoro_db::queries::attachments::get_paper_attachments(
                            &db.conn,
                            &info.paper_id,
                        )
                        .map(|atts| atts.iter().any(|a| a.file_type == "pdf"))
                        .unwrap_or(false);

                        if !has_pdf_att {
                            if let Some(ref pdf_url) = enrichment.pdf_url {
                                let pdf_path = paper_dir.join("paper.pdf");
                                if !pdf_path.exists() {
                                    let pdf_task_id = format!("pdf-{}", info.paper_id);
                                    crate::connector::handlers::emit_task(
                                        &app_handle,
                                        &crate::connector::handlers::BackgroundTaskEvent {
                                            task_id: pdf_task_id.clone(),
                                            paper_id: info.paper_id.clone(),
                                            paper_title: info.paper_title.clone(),
                                            task_type: "pdf-download".into(),
                                            status: "running".into(),
                                            message: Some("PDF found via OA lookup".into()),
                                        },
                                    );
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
                                            let _ =
                                                zoro_db::queries::attachments::insert_attachment(
                                                    &db.conn,
                                                    &info.paper_id,
                                                    "paper.pdf",
                                                    "pdf",
                                                    Some("application/pdf"),
                                                    file_size,
                                                    "paper.pdf",
                                                    "auto-resolved",
                                                );
                                            crate::connector::handlers::emit_task(
                                                &app_handle,
                                                &crate::connector::handlers::BackgroundTaskEvent {
                                                    task_id: pdf_task_id,
                                                    paper_id: info.paper_id.clone(),
                                                    paper_title: info.paper_title.clone(),
                                                    task_type: "pdf-download".into(),
                                                    status: "completed".into(),
                                                    message: None,
                                                },
                                            );
                                        }
                                        Err(e) => {
                                            crate::connector::handlers::emit_task(
                                                &app_handle,
                                                &crate::connector::handlers::BackgroundTaskEvent {
                                                    task_id: pdf_task_id,
                                                    paper_id: info.paper_id.clone(),
                                                    paper_title: info.paper_title.clone(),
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
                        info.arxiv_id.as_deref().or(enrichment.arxiv_id.as_deref());
                    if let Some(aid) = resolved_arxiv {
                        crate::connector::handlers::fetch_arxiv_html_background(
                            &app_handle,
                            &db_path,
                            &info.paper_id,
                            &info.paper_title,
                            aid,
                            &paper_dir,
                        )
                        .await;
                    }

                    crate::connector::handlers::emit_task(
                        &app_handle,
                        &crate::connector::handlers::BackgroundTaskEvent {
                            task_id,
                            paper_id: info.paper_id.clone(),
                            paper_title: info.paper_title.clone(),
                            task_type: "enrichment".into(),
                            status: "completed".into(),
                            message: None,
                        },
                    );
                    let _ = app_handle.emit("paper-updated", &info.paper_id);
                }
                Err(e) => {
                    crate::connector::handlers::emit_task(
                        &app_handle,
                        &crate::connector::handlers::BackgroundTaskEvent {
                            task_id,
                            paper_id: info.paper_id,
                            paper_title: info.paper_title,
                            task_type: "enrichment".into(),
                            status: "failed".into(),
                            message: Some(format!("{}", e)),
                        },
                    );
                }
            }
        });
    }

    zotero_response(ZoteroSaveItemsResponse {
        items: result_items,
    })
}

/// POST /connector/saveSnapshot
pub async fn save_snapshot(
    state: State<Arc<ZoteroCompatState>>,
    Json(req): Json<ZoteroSaveSnapshotRequest>,
) -> Response {
    let app_state: tauri::State<crate::AppState> = state.app.state();

    state.sessions.create_session(&req.session_id);

    let title = req
        .title
        .clone()
        .unwrap_or_else(|| "Untitled Webpage".to_string());
    let url = req.url.clone().or(req.uri.clone());

    let identifier = url.as_deref().unwrap_or(&title);
    let slug = generate_paper_slug(&title, identifier, None);

    let db = match app_state.db.lock() {
        Ok(db) => db,
        Err(e) => return zotero_error(StatusCode::INTERNAL_SERVER_ERROR, &format!("{}", e)),
    };

    let db_input = papers::CreatePaperInput {
        slug: slug.clone(),
        title: title.clone(),
        short_title: None,
        abstract_text: None,
        doi: None,
        arxiv_id: None,
        url: url.clone(),
        pdf_url: None,
        html_url: None,
        thumbnail_url: None,
        published_date: None,
        source: Some("zotero-connector".to_string()),
        dir_path: format!("papers/{}", slug),
        extra_json: None,
        entry_type: None,
        journal: None,
        volume: None,
        issue: None,
        pages: None,
        publisher: None,
        issn: None,
        isbn: None,
        added_date: None,
    };

    match papers::insert_paper(&db.conn, &db_input) {
        Ok(row) => {
            let papers_dir = app_state.data_dir.join("library/papers");
            let _ = crate::storage::paper_dir::create_paper_dir(&papers_dir, &slug);

            state
                .sessions
                .register_item(&req.session_id, "snapshot", &row.id);

            crate::storage::sync::sync_paper_metadata(&db, &app_state.data_dir, &row.id);
            crate::storage::sync::rebuild_library_index(&db, &app_state.data_dir);

            let _ = state.app.emit("paper-saved", &row.id);

            // Mark done since snapshot content comes via saveSingleFile
            // (the connector will call saveSingleFile separately)
            zotero_response(serde_json::json!({}))
        }
        Err(e) => zotero_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to save snapshot: {}", e),
        ),
    }
}

// ─── Attachment Endpoints ────────────────────────────────────────────────────

/// POST /connector/saveAttachment
pub async fn save_attachment(
    state: State<Arc<ZoteroCompatState>>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let session_id = match params.get("sessionID") {
        Some(id) => id.clone(),
        None => return zotero_error(StatusCode::BAD_REQUEST, "Missing sessionID"),
    };

    // Parse X-Metadata header
    let metadata: AttachmentMetadata = match headers.get("X-Metadata") {
        Some(val) => match val.to_str() {
            Ok(s) => match serde_json::from_str(s) {
                Ok(m) => m,
                Err(e) => {
                    return zotero_error(
                        StatusCode::BAD_REQUEST,
                        &format!("Invalid X-Metadata: {}", e),
                    )
                }
            },
            Err(e) => {
                return zotero_error(
                    StatusCode::BAD_REQUEST,
                    &format!("Invalid X-Metadata header: {}", e),
                )
            }
        },
        None => return zotero_error(StatusCode::BAD_REQUEST, "Missing X-Metadata header"),
    };

    let attachment_id = metadata.id.clone().unwrap_or_default();
    let parent_item_id = match &metadata.parent_item_id {
        Some(id) => id.clone(),
        None => return zotero_error(StatusCode::BAD_REQUEST, "Missing parentItemID in metadata"),
    };

    // Look up the Zoro paper ID
    let paper_id = match state.sessions.get_paper_id(&session_id, &parent_item_id) {
        Some(id) => id,
        None => {
            state.sessions.fail_attachment(&session_id, &attachment_id);
            return zotero_error(StatusCode::NOT_FOUND, "Parent item not found in session");
        }
    };

    let app_state: tauri::State<crate::AppState> = state.app.state();

    // Get paper slug to find directory
    let db = match app_state.db.lock() {
        Ok(db) => db,
        Err(e) => {
            state.sessions.fail_attachment(&session_id, &attachment_id);
            return zotero_error(StatusCode::INTERNAL_SERVER_ERROR, &format!("{}", e));
        }
    };

    let paper = match papers::get_paper(&db.conn, &paper_id) {
        Ok(p) => p,
        Err(e) => {
            state.sessions.fail_attachment(&session_id, &attachment_id);
            return zotero_error(StatusCode::NOT_FOUND, &format!("Paper not found: {}", e));
        }
    };

    // Determine filename and file type from content type
    let content_type = metadata
        .content_type
        .as_deref()
        .or_else(|| headers.get("content-type").and_then(|v| v.to_str().ok()))
        .unwrap_or("application/octet-stream");

    let (filename, file_type) = attachment_filename(content_type, metadata.title.as_deref());

    // Deduplicate: skip if this paper already has an attachment of the same type
    // (e.g. enrichment may have already downloaded a PDF before the plugin pushes one)
    if let Ok(existing) = attachments::get_paper_attachments(&db.conn, &paper_id) {
        if existing.iter().any(|a| a.file_type == file_type) {
            tracing::info!(
                "Skipping duplicate {} attachment for paper {} (already exists)",
                file_type,
                paper_id
            );
            state
                .sessions
                .complete_attachment(&session_id, &attachment_id);
            return zotero_ok();
        }
    }

    // Write file to paper directory
    let paper_dir = app_state.data_dir.join("library/papers").join(&paper.slug);
    let att_dir = paper_dir.join("attachments");
    let _ = std::fs::create_dir_all(&att_dir);
    let file_path = att_dir.join(&filename);

    if let Err(e) = std::fs::write(&file_path, &body) {
        state.sessions.fail_attachment(&session_id, &attachment_id);
        return zotero_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to write file: {}", e),
        );
    }

    let file_size = Some(body.len() as i64);
    let relative_path = format!("attachments/{}", filename);

    // Insert attachment record
    let _ = attachments::insert_attachment(
        &db.conn,
        &paper_id,
        &filename,
        &file_type,
        Some(content_type),
        file_size,
        &relative_path,
        "zotero-connector",
    );

    // Update session progress
    state
        .sessions
        .complete_attachment(&session_id, &attachment_id);

    // Notify frontend so paper list refreshes with new attachment
    let _ = state.app.emit("paper-updated", &paper_id);

    tracing::info!(
        "Saved attachment {} ({} bytes) for paper {}",
        filename,
        body.len(),
        paper_id
    );

    zotero_ok()
}

/// POST /connector/saveStandaloneAttachment
pub async fn save_standalone_attachment(
    state: State<Arc<ZoteroCompatState>>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let session_id = match params.get("sessionID") {
        Some(id) => id.clone(),
        None => return zotero_error(StatusCode::BAD_REQUEST, "Missing sessionID"),
    };

    // Parse X-Metadata header
    let metadata: AttachmentMetadata = match headers.get("X-Metadata") {
        Some(val) => match val.to_str() {
            Ok(s) => match serde_json::from_str(s) {
                Ok(m) => m,
                Err(e) => {
                    return zotero_error(
                        StatusCode::BAD_REQUEST,
                        &format!("Invalid X-Metadata: {}", e),
                    )
                }
            },
            Err(e) => {
                return zotero_error(
                    StatusCode::BAD_REQUEST,
                    &format!("Invalid X-Metadata header: {}", e),
                )
            }
        },
        None => return zotero_error(StatusCode::BAD_REQUEST, "Missing X-Metadata header"),
    };

    let content_type = metadata
        .content_type
        .as_deref()
        .or_else(|| headers.get("content-type").and_then(|v| v.to_str().ok()))
        .unwrap_or("application/octet-stream");

    let is_pdf = content_type.contains("pdf");

    let app_state: tauri::State<crate::AppState> = state.app.state();

    // For PDFs, try to extract metadata (DOI, arXiv ID, title) from the content
    let pdf_meta = if is_pdf {
        // Write to a temp file so lopdf can read it
        let temp_dir = std::env::temp_dir().join("zoro-standalone");
        let _ = std::fs::create_dir_all(&temp_dir);
        let temp_path = temp_dir.join("standalone.pdf");
        if std::fs::write(&temp_path, &body).is_ok() {
            let result = zoro_metadata::pdf_extract::extract_pdf_metadata(&temp_path).ok();
            let _ = std::fs::remove_file(&temp_path);
            let _ = std::fs::remove_dir(&temp_dir);
            result
        } else {
            None
        }
    } else {
        None
    };

    // Derive title: prefer PDF metadata title, fall back to X-Metadata title, then "Untitled"
    let title = pdf_meta
        .as_ref()
        .and_then(|m| m.title.clone())
        .or_else(|| metadata.title.clone())
        .unwrap_or_else(|| "Untitled Document".to_string());

    let doi = pdf_meta.as_ref().and_then(|m| m.doi.clone());
    let arxiv_id = pdf_meta.as_ref().and_then(|m| m.arxiv_id.clone());

    // Create a new paper for this standalone attachment
    let identifier = doi
        .as_deref()
        .or(arxiv_id.as_deref())
        .or(metadata.url.as_deref())
        .unwrap_or(&title);
    let slug = generate_paper_slug(&title, identifier, None);

    let db = match app_state.db.lock() {
        Ok(db) => db,
        Err(e) => return zotero_error(StatusCode::INTERNAL_SERVER_ERROR, &format!("{}", e)),
    };

    let db_input = papers::CreatePaperInput {
        slug: slug.clone(),
        title: title.clone(),
        short_title: None,
        abstract_text: None,
        doi: doi.clone(),
        arxiv_id: arxiv_id.clone(),
        url: metadata.url.clone(),
        pdf_url: None,
        html_url: None,
        thumbnail_url: None,
        published_date: None,
        source: Some("zotero-connector".to_string()),
        dir_path: format!("papers/{}", slug),
        extra_json: None,
        entry_type: None,
        journal: None,
        volume: None,
        issue: None,
        pages: None,
        publisher: None,
        issn: None,
        isbn: None,
        added_date: None,
    };

    match papers::insert_paper(&db.conn, &db_input) {
        Ok(row) => {
            let papers_dir = app_state.data_dir.join("library/papers");
            let paper_dir =
                crate::storage::paper_dir::create_paper_dir(&papers_dir, &slug).unwrap();

            // For PDFs, save as paper.pdf (the primary PDF path); otherwise save to attachments/
            let (filename, file_type, relative_path, file_path) = if is_pdf {
                let fp = paper_dir.join("paper.pdf");
                (
                    "paper.pdf".to_string(),
                    "pdf".to_string(),
                    "paper.pdf".to_string(),
                    fp,
                )
            } else {
                let (fname, ftype) = attachment_filename(content_type, metadata.title.as_deref());
                let att_dir = paper_dir.join("attachments");
                let _ = std::fs::create_dir_all(&att_dir);
                let fp = att_dir.join(&fname);
                let rel = format!("attachments/{}", fname);
                (fname, ftype, rel, fp)
            };

            if let Err(e) = std::fs::write(&file_path, &body) {
                return zotero_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("Failed to write file: {}", e),
                );
            }

            let file_size = Some(body.len() as i64);

            let _ = attachments::insert_attachment(
                &db.conn,
                &row.id,
                &filename,
                &file_type,
                Some(content_type),
                file_size,
                &relative_path,
                "zotero-connector",
            );

            // Set authors from PDF metadata if available
            if let Some(ref pm) = pdf_meta {
                if let Some(ref names) = pm.authors {
                    if !names.is_empty() {
                        let authors: Vec<(String, Option<String>, Option<String>)> =
                            names.iter().map(|n| (n.clone(), None, None)).collect();
                        let _ = papers::set_paper_authors(&db.conn, &row.id, &authors);
                    }
                }
            }

            state.sessions.create_session(&session_id);
            state
                .sessions
                .register_item(&session_id, "standalone", &row.id);
            state.sessions.mark_done(&session_id);

            crate::storage::sync::sync_paper_metadata(&db, &app_state.data_dir, &row.id);
            crate::storage::sync::rebuild_library_index(&db, &app_state.data_dir);

            let _ = state.app.emit("paper-saved", &row.id);

            tracing::info!(
                "Saved standalone attachment {} ({} bytes, doi={:?}, arxiv={:?})",
                filename,
                body.len(),
                doi,
                arxiv_id
            );

            let paper_id = row.id.clone();
            let db_path = app_state.data_dir.join("library.db");
            let app_handle = state.app.clone();
            drop(db);

            // Spawn background metadata enrichment (same as local PDF import)
            {
                let enrich_paper_id = paper_id.clone();
                let enrich_doi = doi.clone();
                let enrich_arxiv = arxiv_id.clone();
                let enrich_db_path = db_path.clone();
                let enrich_paper_dir = paper_dir.clone();
                let enrich_app = app_handle.clone();
                let enrich_title = title.clone();
                let enrich_task_id = format!("enrich-{}", enrich_paper_id);
                crate::connector::handlers::emit_task(
                    &enrich_app,
                    &crate::connector::handlers::BackgroundTaskEvent {
                        task_id: enrich_task_id.clone(),
                        paper_id: enrich_paper_id.clone(),
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
                                    zoro_db::queries::papers::get_paper(&db.conn, &enrich_paper_id)
                                {
                                    let update =
                                        crate::connector::handlers::build_enrichment_update(
                                            &current,
                                            &enrichment,
                                        );
                                    let _ = zoro_db::queries::papers::update_paper(
                                        &db.conn,
                                        &enrich_paper_id,
                                        &update,
                                    );

                                    if let Some(ref enrich_authors) = enrichment.authors {
                                        if let Ok(existing) =
                                            zoro_db::queries::papers::get_paper_authors(
                                                &db.conn,
                                                &enrich_paper_id,
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
                                                    &db.conn,
                                                    &enrich_paper_id,
                                                    &tuples,
                                                );
                                            }
                                        }
                                    }
                                }
                            }

                            // Fetch arXiv HTML if we have an arXiv ID
                            let resolved_arxiv =
                                enrich_arxiv.as_deref().or(enrichment.arxiv_id.as_deref());
                            if let Some(aid) = resolved_arxiv {
                                crate::connector::handlers::fetch_arxiv_html_background(
                                    &enrich_app,
                                    &enrich_db_path,
                                    &enrich_paper_id,
                                    &enrich_title,
                                    aid,
                                    &enrich_paper_dir,
                                )
                                .await;
                            }

                            crate::connector::handlers::emit_task(
                                &enrich_app,
                                &crate::connector::handlers::BackgroundTaskEvent {
                                    task_id: enrich_task_id,
                                    paper_id: enrich_paper_id.clone(),
                                    paper_title: enrich_title,
                                    task_type: "enrichment".into(),
                                    status: "completed".into(),
                                    message: None,
                                },
                            );
                            let _ = enrich_app.emit("paper-updated", &enrich_paper_id);
                        }
                        Err(e) => {
                            crate::connector::handlers::emit_task(
                                &enrich_app,
                                &crate::connector::handlers::BackgroundTaskEvent {
                                    task_id: enrich_task_id,
                                    paper_id: enrich_paper_id,
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

            zotero_response(SaveStandaloneAttachmentResponse {
                can_recognize: true,
            })
        }
        Err(e) => zotero_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to create paper: {}", e),
        ),
    }
}

/// POST /connector/saveSingleFile
pub async fn save_single_file(
    state: State<Arc<ZoteroCompatState>>,
    Json(req): Json<ZoteroSaveSingleFileRequest>,
) -> Response {
    let app_state: tauri::State<crate::AppState> = state.app.state();

    // Find the paper from the session
    let paper_id = match state.sessions.get_first_paper_id(&req.session_id) {
        Some(id) => id,
        None => return zotero_error(StatusCode::NOT_FOUND, "No paper found for this session"),
    };

    let db = match app_state.db.lock() {
        Ok(db) => db,
        Err(e) => return zotero_error(StatusCode::INTERNAL_SERVER_ERROR, &format!("{}", e)),
    };

    let paper = match papers::get_paper(&db.conn, &paper_id) {
        Ok(p) => p,
        Err(e) => return zotero_error(StatusCode::NOT_FOUND, &format!("Paper not found: {}", e)),
    };

    // Write snapshot HTML to paper directory
    let paper_dir = app_state.data_dir.join("library/papers").join(&paper.slug);
    let html_path = paper_dir.join("abs.html");

    if let Err(e) = std::fs::write(&html_path, &req.snapshot_content) {
        return zotero_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to write snapshot: {}", e),
        );
    }

    let file_size = Some(req.snapshot_content.len() as i64);

    let _ = attachments::insert_attachment(
        &db.conn,
        &paper_id,
        "abs.html",
        "html",
        Some("text/html"),
        file_size,
        "abs.html",
        "zotero-connector",
    );

    crate::storage::sync::sync_paper_metadata(&db, &app_state.data_dir, &paper_id);

    // Notify frontend so paper list refreshes with new attachment
    let _ = state.app.emit("paper-updated", &paper_id);

    tracing::info!(
        "Saved SingleFile snapshot ({} bytes) for paper {}",
        req.snapshot_content.len(),
        paper_id
    );

    zotero_ok()
}

// ─── Session Endpoints ───────────────────────────────────────────────────────

/// POST /connector/sessionProgress
pub async fn session_progress(
    state: State<Arc<ZoteroCompatState>>,
    Json(req): Json<SessionProgressRequest>,
) -> Response {
    match state.sessions.get_session_progress(&req.session_id) {
        Some((items, done)) => {
            let result_items: Vec<ZoteroSaveItemResult> = items
                .into_iter()
                .map(|i| ZoteroSaveItemResult {
                    id: i.id,
                    attachments: i.attachments,
                })
                .collect();
            zotero_response(SessionProgressResponse {
                items: result_items,
                done,
            })
        }
        None => zotero_response(SessionProgressResponse {
            items: Vec::new(),
            done: true,
        }),
    }
}

/// POST /connector/updateSession
pub async fn update_session(
    state: State<Arc<ZoteroCompatState>>,
    Json(req): Json<UpdateSessionRequest>,
) -> Response {
    let app_state: tauri::State<crate::AppState> = state.app.state();

    // Parse target: "C{id}" -> collection id, "L{id}" -> library (no-op)
    let collection_id = req
        .target
        .as_ref()
        .and_then(|t| t.strip_prefix('C').map(|stripped| stripped.to_string()));

    // Update session store
    state
        .sessions
        .update_session(&req.session_id, collection_id.clone(), req.tags.clone());

    // Apply changes to papers in this session
    if let Some(session) = state.sessions.get_session(&req.session_id) {
        let db = match app_state.db.lock() {
            Ok(db) => db,
            Err(e) => return zotero_error(StatusCode::INTERNAL_SERVER_ERROR, &format!("{}", e)),
        };

        for paper_id in session.item_paper_map.values() {
            // Add to collection if specified
            if let Some(ref cid) = collection_id {
                let _ = collections::add_paper_to_collection(&db.conn, paper_id, cid);
            }

            // Add labels to extra_json if tags specified (NOT as sidebar tags).
            // Sidebar tags are user-curated only; Zotero tags become labels in metadata.
            if let Some(ref tag_names) = req.tags {
                if !tag_names.is_empty() {
                    if let Ok(paper_row) = papers::get_paper(&db.conn, paper_id) {
                        let mut extra: serde_json::Value = paper_row
                            .extra_json
                            .as_deref()
                            .and_then(|s| serde_json::from_str(s).ok())
                            .unwrap_or_else(|| serde_json::json!({}));
                        // Merge new labels with existing ones
                        let existing: Vec<String> = extra
                            .get("labels")
                            .and_then(|v| serde_json::from_value(v.clone()).ok())
                            .unwrap_or_default();
                        let mut merged = existing;
                        for tag in tag_names {
                            if !merged.contains(tag) {
                                merged.push(tag.clone());
                            }
                        }
                        if let serde_json::Value::Object(ref mut map) = extra {
                            map.insert("labels".to_string(), serde_json::json!(merged));
                        }
                        let update = papers::UpdatePaperInput {
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
                            extra_json: Some(serde_json::to_string(&extra).ok()),
                            entry_type: None,
                            journal: None,
                            volume: None,
                            issue: None,
                            pages: None,
                            publisher: None,
                            issn: None,
                            isbn: None,
                        };
                        let _ = papers::update_paper(&db.conn, paper_id, &update);
                    }
                }
            }

            crate::storage::sync::sync_paper_metadata(&db, &app_state.data_dir, paper_id);
        }

        crate::storage::sync::rebuild_library_index(&db, &app_state.data_dir);
    }

    zotero_ok()
}

// ─── Import Endpoint ─────────────────────────────────────────────────────────

/// POST /connector/import
pub async fn import(
    state: State<Arc<ZoteroCompatState>>,
    Query(_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let body_str = match String::from_utf8(body.to_vec()) {
        Ok(s) => s,
        Err(e) => return zotero_error(StatusCode::BAD_REQUEST, &format!("Invalid UTF-8: {}", e)),
    };

    // Parse based on content type
    let parsed_papers = if content_type.contains("bibtex") {
        zoro_core::bibtex::parse_bibtex(&body_str)
    } else if content_type.contains("research-info-systems") || content_type.contains("ris") {
        zoro_core::ris::parse_ris(&body_str)
    } else {
        // Try BibTeX first, then RIS
        zoro_core::bibtex::parse_bibtex(&body_str).or_else(|_| zoro_core::ris::parse_ris(&body_str))
    };

    let papers_list = match parsed_papers {
        Ok(p) => p,
        Err(e) => {
            return zotero_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Parse error: {}", e),
            )
        }
    };

    let app_state: tauri::State<crate::AppState> = state.app.state();
    let db = match app_state.db.lock() {
        Ok(db) => db,
        Err(e) => return zotero_error(StatusCode::INTERNAL_SERVER_ERROR, &format!("{}", e)),
    };

    let mut results = Vec::new();

    for paper in &papers_list {
        let identifier = paper
            .doi
            .as_deref()
            .or(paper.arxiv_id.as_deref())
            .unwrap_or(&paper.title);
        let slug = generate_paper_slug(&paper.title, identifier, paper.published_date.as_deref());

        let db_input = papers::CreatePaperInput {
            slug: slug.clone(),
            title: paper.title.clone(),
            short_title: paper.short_title.clone(),
            abstract_text: paper.abstract_text.clone(),
            doi: paper.doi.clone(),
            arxiv_id: paper.arxiv_id.clone(),
            url: paper.url.clone(),
            pdf_url: None,
            html_url: None,
            thumbnail_url: None,
            published_date: paper.published_date.clone(),
            source: Some("import".to_string()),
            dir_path: format!("papers/{}", slug),
            extra_json: if paper.extra == serde_json::json!({}) {
                None
            } else {
                serde_json::to_string(&paper.extra).ok()
            },
            entry_type: paper.entry_type.clone(),
            journal: paper.journal.clone(),
            volume: paper.volume.clone(),
            issue: paper.issue.clone(),
            pages: paper.pages.clone(),
            publisher: paper.publisher.clone(),
            issn: paper.issn.clone(),
            isbn: paper.isbn.clone(),
            added_date: None,
        };

        if let Ok(row) = papers::insert_paper(&db.conn, &db_input) {
            let papers_dir = app_state.data_dir.join("library/papers");
            let _ = crate::storage::paper_dir::create_paper_dir(&papers_dir, &slug);

            let authors: Vec<(String, Option<String>, Option<String>)> = paper
                .authors
                .iter()
                .map(|a| (a.name.clone(), a.affiliation.clone(), None))
                .collect();
            let _ = papers::set_paper_authors(&db.conn, &row.id, &authors);

            crate::storage::sync::sync_paper_metadata(&db, &app_state.data_dir, &row.id);

            results.push(ImportItemResult {
                item_type: "journalArticle".to_string(),
                title: paper.title.clone(),
            });
        }
    }

    crate::storage::sync::rebuild_library_index(&db, &app_state.data_dir);

    if !results.is_empty() {
        let _ = state.app.emit("paper-saved", "import");
    }

    zotero_response(results)
}

// ─── Stub Endpoints ──────────────────────────────────────────────────────────

/// POST /connector/getTranslators — return empty array
pub async fn get_translators(_state: State<Arc<ZoteroCompatState>>) -> Response {
    tracing::debug!("Zotero compat: getTranslators called (stub, returning empty array)");
    zotero_response(serde_json::json!([]))
}

/// POST /connector/getTranslatorCode — return 404
pub async fn get_translator_code(
    _state: State<Arc<ZoteroCompatState>>,
    _body: Option<Json<GetTranslatorCodeRequest>>,
) -> Response {
    tracing::debug!("Zotero compat: getTranslatorCode called (stub, returning 404)");
    zotero_error(StatusCode::NOT_FOUND, "Translator not found")
}

/// POST /connector/delaySync — no-op
pub async fn delay_sync(_state: State<Arc<ZoteroCompatState>>) -> Response {
    tracing::debug!("Zotero compat: delaySync called (stub, no-op)");
    zotero_ok()
}

/// GET /connector/proxies — return empty object
pub async fn proxies(_state: State<Arc<ZoteroCompatState>>) -> Response {
    tracing::debug!("Zotero compat: proxies called (stub, returning empty object)");
    zotero_response(serde_json::json!({}))
}

/// GET /connector/getClientHostnames — return empty array
pub async fn get_client_hostnames(_state: State<Arc<ZoteroCompatState>>) -> Response {
    tracing::debug!("Zotero compat: getClientHostnames called (stub, returning empty array)");
    zotero_response(serde_json::json!([]))
}

/// POST /connector/getRecognizedItem — return null (not supported)
pub async fn get_recognized_item(
    _state: State<Arc<ZoteroCompatState>>,
    _body: Option<Json<GetRecognizedItemRequest>>,
) -> Response {
    tracing::debug!("Zotero compat: getRecognizedItem called (stub, returning null)");
    zotero_response(serde_json::Value::Null)
}

/// POST /connector/hasAttachmentResolvers — return false
pub async fn has_attachment_resolvers(
    _state: State<Arc<ZoteroCompatState>>,
    _body: Option<Json<HasAttachmentResolversRequest>>,
) -> Response {
    tracing::debug!("Zotero compat: hasAttachmentResolvers called (stub, returning false)");
    zotero_response(false)
}

/// POST /connector/installStyle — no-op
pub async fn install_style(_state: State<Arc<ZoteroCompatState>>) -> Response {
    tracing::debug!("Zotero compat: installStyle called (stub, no-op)");
    zotero_ok()
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Determine filename and file_type from MIME type.
fn attachment_filename(content_type: &str, title: Option<&str>) -> (String, String) {
    let (ext, file_type) = if content_type.contains("pdf") {
        ("pdf", "pdf")
    } else if content_type.contains("epub") {
        ("epub", "epub")
    } else if content_type.contains("html") {
        ("html", "html")
    } else {
        ("bin", "other")
    };

    // Sanitize title for filename
    let base = title
        .map(|t| {
            let sanitized: String = t
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '-' || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .take(50)
                .collect();
            sanitized
        })
        .unwrap_or_else(|| "attachment".to_string());

    (format!("{}.{}", base, ext), file_type.to_string())
}
