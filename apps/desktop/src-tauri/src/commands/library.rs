// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::connector::handlers::{emit_task, BackgroundTaskEvent};
use crate::storage;
use crate::AppState;
use std::path::Path;
use tauri::{Emitter, State};
use zoro_core::models::{AttachmentInfo, Author, PaperMetadata, ReadStatus};
use zoro_core::slug_utils::generate_paper_slug;
use zoro_db::queries::{attachments, citations, collections, notes, papers, tags};
use zoro_db::Database;

/// Sync metadata.json for a paper and rebuild library-index.json.
/// Called after any mutation that affects paper/collection/tag state.
fn sync_after_mutation(state: &AppState, db: &Database, paper_id: Option<&str>) {
    if let Some(pid) = paper_id {
        storage::sync::sync_paper_metadata(db, &state.data_dir, pid);
    }
    storage::sync::rebuild_library_index(db, &state.data_dir);
}

#[derive(Debug, serde::Deserialize)]
pub struct AddPaperInput {
    pub title: String,
    pub short_title: Option<String>,
    pub authors: Vec<AuthorInput>,
    pub abstract_text: Option<String>,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    pub url: Option<String>,
    pub pdf_url: Option<String>,
    pub html_url: Option<String>,
    pub published_date: Option<String>,
    pub source: Option<String>,
    pub tags: Option<Vec<String>>,
    pub extra_json: Option<String>,
    pub entry_type: Option<String>,
    pub journal: Option<String>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub pages: Option<String>,
    pub publisher: Option<String>,
    pub issn: Option<String>,
    pub isbn: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct AuthorInput {
    pub name: String,
    pub affiliation: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct PaperResponse {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub short_title: Option<String>,
    pub authors: Vec<AuthorResponse>,
    pub abstract_text: Option<String>,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    pub url: Option<String>,
    pub pdf_url: Option<String>,
    pub html_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub published_date: Option<String>,
    pub added_date: String,
    pub modified_date: String,
    pub source: Option<String>,
    pub read_status: String,
    pub rating: Option<i32>,
    pub tags: Vec<TagResponse>,
    pub attachments: Vec<AttachmentResponse>,
    pub has_pdf: bool,
    pub has_html: bool,
    pub notes: Vec<String>,
    pub extra_json: Option<String>,
    pub entry_type: Option<String>,
    pub journal: Option<String>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub pages: Option<String>,
    pub publisher: Option<String>,
    pub issn: Option<String>,
    pub isbn: Option<String>,
    pub pdf_downloaded: bool,
    pub html_downloaded: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct AuthorResponse {
    pub name: String,
    pub affiliation: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct TagResponse {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct AttachmentResponse {
    pub id: String,
    pub filename: String,
    pub file_type: String,
    pub file_size: Option<i64>,
    pub source: String,
    pub is_local: bool,
    pub created_date: String,
}

#[derive(Debug, serde::Serialize)]
pub struct CollectionResponse {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub parent_id: Option<String>,
    pub paper_count: i64,
    pub description: Option<String>,
}

pub fn paper_row_to_response(
    row: &papers::PaperRow,
    author_list: Vec<(String, Option<String>, Option<String>)>,
    tag_list: Vec<tags::TagRow>,
    attachment_list: Vec<attachments::AttachmentRow>,
    note_list: Vec<notes::NoteRow>,
    data_dir: &Path,
) -> PaperResponse {
    let paper_dir = data_dir.join("library").join(&row.dir_path);

    // Scan the paper directory for files and merge with DB attachment records.
    let scanned = scan_paper_dir_attachments(&paper_dir, &attachment_list);

    let has_pdf = scanned.iter().any(|a| a.file_type == "pdf");
    // Only consider paper.html (full-text HTML) as having HTML;
    // abs.html (abstract page snapshot) is not useful for the HTML reader.
    let has_html = scanned
        .iter()
        .any(|a| a.file_type == "html" && a.filename != "abs.html");

    PaperResponse {
        id: row.id.clone(),
        slug: row.slug.clone(),
        title: row.title.clone(),
        short_title: row.short_title.clone(),
        authors: author_list
            .into_iter()
            .map(|(name, aff, _)| AuthorResponse {
                name,
                affiliation: aff,
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
        read_status: row.read_status.clone(),
        rating: row.rating,
        tags: tag_list
            .into_iter()
            .map(|t| TagResponse {
                id: t.id,
                name: t.name,
                color: t.color,
            })
            .collect(),
        attachments: scanned,
        has_pdf,
        has_html,
        notes: note_list.iter().map(|n| n.content.clone()).collect(),
        extra_json: row.extra_json.clone(),
        entry_type: row.entry_type.clone(),
        journal: row.journal.clone(),
        volume: row.volume.clone(),
        issue: row.issue.clone(),
        pages: row.pages.clone(),
        publisher: row.publisher.clone(),
        issn: row.issn.clone(),
        isbn: row.isbn.clone(),
        pdf_downloaded: row.pdf_downloaded,
        html_downloaded: row.html_downloaded,
    }
}

/// Scan a paper directory for files and merge with DB-stored attachment records.
/// Files found on disk but not in the DB are included as discovered attachments.
/// DB records whose files don't exist on disk are included with is_local=false.
fn scan_paper_dir_attachments(
    paper_dir: &Path,
    db_attachments: &[attachments::AttachmentRow],
) -> Vec<AttachmentResponse> {
    use std::collections::HashSet;

    let mut result: Vec<AttachmentResponse> = Vec::new();
    let mut seen_filenames: HashSet<String> = HashSet::new();

    // Include DB attachment records, deduplicating by filename (keep first occurrence)
    for a in db_attachments {
        if seen_filenames.contains(&a.filename) {
            continue;
        }
        let local_path = paper_dir.join(&a.relative_path);
        let is_local = local_path.exists();
        seen_filenames.insert(a.filename.clone());
        result.push(AttachmentResponse {
            id: a.id.clone(),
            filename: a.filename.clone(),
            file_type: a.file_type.clone(),
            file_size: a.file_size,
            source: a.source.clone(),
            is_local,
            created_date: a.created_date.clone(),
        });
    }

    // Scan the paper directory for files not already in DB
    let skip_names: HashSet<&str> = ["metadata.json", ".DS_Store", "Thumbs.db"]
        .iter()
        .copied()
        .collect();
    let skip_dirs: HashSet<&str> = ["_babeldoc_temp", "notes"].iter().copied().collect();

    if let Ok(entries) = std::fs::read_dir(paper_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            // Skip subdirectories we handle separately or that are temp dirs
            if path.is_dir() {
                let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if skip_dirs.contains(dir_name) {
                    continue;
                }
                // Scan subdirectories (e.g. attachments/) for user-added files
                if let Ok(sub_entries) = std::fs::read_dir(&path) {
                    for sub_entry in sub_entries.flatten() {
                        let sub_path = sub_entry.path();
                        if sub_path.is_file() {
                            let filename =
                                sub_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                            if skip_names.contains(filename) || filename.starts_with('.') {
                                continue;
                            }
                            // Build relative path from paper_dir
                            let rel = format!(
                                "{}/{}",
                                path.file_name().and_then(|n| n.to_str()).unwrap_or(""),
                                filename
                            );
                            if seen_filenames.contains(filename) {
                                continue;
                            }
                            seen_filenames.insert(filename.to_string());
                            let ext = sub_path
                                .extension()
                                .and_then(|e| e.to_str())
                                .unwrap_or("")
                                .to_lowercase();
                            let file_size =
                                std::fs::metadata(&sub_path).map(|m| m.len() as i64).ok();
                            result.push(AttachmentResponse {
                                id: format!("fs-{}", rel),
                                filename: filename.to_string(),
                                file_type: ext,
                                file_size,
                                source: "filesystem".to_string(),
                                is_local: true,
                                created_date: String::new(),
                            });
                        }
                    }
                }
                continue;
            }

            if !path.is_file() {
                continue;
            }

            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if skip_names.contains(filename) || filename.starts_with('.') {
                continue;
            }
            if seen_filenames.contains(filename) {
                continue;
            }
            seen_filenames.insert(filename.to_string());

            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            let file_size = std::fs::metadata(&path).map(|m| m.len() as i64).ok();

            result.push(AttachmentResponse {
                id: format!("fs-{}", filename),
                filename: filename.to_string(),
                file_type: ext,
                file_size,
                source: "filesystem".to_string(),
                is_local: true,
                created_date: String::new(),
            });
        }
    }

    result
}

#[tauri::command]
pub async fn add_paper(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: AddPaperInput,
) -> Result<PaperResponse, String> {
    let identifier = input
        .doi
        .as_deref()
        .or(input.arxiv_id.as_deref())
        .unwrap_or(&input.title);
    let slug = generate_paper_slug(&input.title, identifier, input.published_date.as_deref());

    let papers_dir = state.data_dir.join("library/papers");
    let paper_dir = storage::paper_dir::create_paper_dir(&papers_dir, &slug)
        .map_err(|e| format!("Failed to create paper directory: {}", e))?;

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
        source: input.source.clone(),
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

    let db = state
        .db
        .lock()
        .map_err(|e| format!("Failed to lock DB: {}", e))?;
    let row = papers::insert_paper(&db.conn, &db_input)
        .map_err(|e| format!("Failed to insert paper: {}", e))?;

    // Set authors
    let authors: Vec<(String, Option<String>, Option<String>)> = input
        .authors
        .iter()
        .map(|a| (a.name.clone(), a.affiliation.clone(), None))
        .collect();
    papers::set_paper_authors(&db.conn, &row.id, &authors)
        .map_err(|e| format!("Failed to set authors: {}", e))?;

    // Add tags
    if let Some(tag_names) = &input.tags {
        for tag_name in tag_names {
            let _ = tags::add_tag_to_paper(&db.conn, &row.id, tag_name, "manual");
        }
    }

    // Write metadata.json
    let tag_list = tags::get_paper_tags(&db.conn, &row.id).unwrap_or_default();
    let paper_title_for_tasks = input.title.clone();
    let metadata = PaperMetadata {
        id: row.id.clone(),
        slug: slug.clone(),
        title: input.title,
        short_title: input.short_title.clone(),
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
        doi: input.doi.clone(),
        arxiv_id: input.arxiv_id.clone(),
        url: input.url,
        pdf_url: input.pdf_url.clone(),
        html_url: input.html_url.clone(),
        thumbnail_url: None,
        published_date: input.published_date,
        added_date: row.added_date.clone(),
        source: input.source,
        tags: tag_list.iter().map(|t| t.name.clone()).collect(),
        collections: Vec::new(),
        attachments: Vec::new(),
        notes: Vec::new(),
        read_status: ReadStatus::Unread,
        rating: None,
        extra: input
            .extra_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_else(|| serde_json::json!({})),
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
    let _ = storage::paper_dir::write_metadata(&paper_dir, &metadata);

    // Download PDF in background if URL provided
    if let Some(ref pdf_url) = input.pdf_url {
        let pdf_path = paper_dir.join("paper.pdf");
        let pdf_url_clone = pdf_url.clone();
        let pid = row.id.clone();
        let db_path = state.data_dir.join("library.db");
        let task_app = app.clone();
        let task_title = paper_title_for_tasks.clone();
        let task_id = format!("pdf-{}", pid);
        emit_task(
            &app,
            &BackgroundTaskEvent {
                task_id: task_id.clone(),
                paper_id: pid.clone(),
                paper_title: task_title.clone(),
                task_type: "pdf-download".into(),
                status: "running".into(),
                message: None,
            },
        );
        tokio::spawn(async move {
            match storage::attachments::download_file(&pdf_url_clone, &pdf_path).await {
                Ok(()) => {
                    let file_size = storage::attachments::get_file_size(&pdf_path);
                    if let Ok(db) = zoro_db::Database::open(&db_path) {
                        let _ = attachments::insert_attachment(
                            &db.conn,
                            &pid,
                            "paper.pdf",
                            "pdf",
                            Some("application/pdf"),
                            file_size,
                            "paper.pdf",
                            "add-paper",
                        );
                    }
                    emit_task(
                        &task_app,
                        &BackgroundTaskEvent {
                            task_id,
                            paper_id: pid,
                            paper_title: task_title,
                            task_type: "pdf-download".into(),
                            status: "completed".into(),
                            message: None,
                        },
                    );
                }
                Err(e) => {
                    emit_task(
                        &task_app,
                        &BackgroundTaskEvent {
                            task_id,
                            paper_id: pid,
                            paper_title: task_title,
                            task_type: "pdf-download".into(),
                            status: "failed".into(),
                            message: Some(format!("{}", e)),
                        },
                    );
                }
            }
        });
    }

    if let Some(ref html_url) = input.html_url {
        let html_path = paper_dir.join("abs.html");
        let html_url_clone = html_url.clone();
        let pid = row.id.clone();
        let db_path = state.data_dir.join("library.db");
        tokio::spawn(async move {
            if let Ok(()) = storage::attachments::download_file(&html_url_clone, &html_path).await {
                let file_size = storage::attachments::get_file_size(&html_path);
                if let Ok(db) = zoro_db::Database::open(&db_path) {
                    let _ = attachments::insert_attachment(
                        &db.conn,
                        &pid,
                        "abs.html",
                        "html",
                        Some("text/html"),
                        file_size,
                        "abs.html",
                        "add-paper",
                    );
                }
            }
        });
    }

    // Background metadata enrichment + PDF resolution (title-based search as fallback)
    {
        let enrich_paper_id = row.id.clone();
        let enrich_doi = input.doi.clone();
        let enrich_arxiv = input.arxiv_id.clone();
        let enrich_pdf_url = input.pdf_url.clone();
        let enrich_db_path = state.data_dir.join("library.db");
        let enrich_paper_dir = paper_dir.clone();
        let enrich_app = app.clone();
        let enrich_title = paper_title_for_tasks.clone();
        let enrich_task_id = format!("enrich-{}", enrich_paper_id);
        emit_task(
            &app,
            &BackgroundTaskEvent {
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
                        if let Ok(current) = papers::get_paper(&db.conn, &enrich_paper_id) {
                            let update = crate::connector::handlers::build_enrichment_update(
                                &current,
                                &enrichment,
                            );
                            let _ = papers::update_paper(&db.conn, &enrich_paper_id, &update);

                            if let Some(ref enrich_authors) = enrichment.authors {
                                if let Ok(existing) =
                                    papers::get_paper_authors(&db.conn, &enrich_paper_id)
                                {
                                    if existing.is_empty() && !enrich_authors.is_empty() {
                                        let tuples: Vec<(String, Option<String>, Option<String>)> =
                                            enrich_authors
                                                .iter()
                                                .map(|(n, a)| (n.clone(), a.clone(), None))
                                                .collect();
                                        let _ = papers::set_paper_authors(
                                            &db.conn,
                                            &enrich_paper_id,
                                            &tuples,
                                        );
                                    }
                                }
                            }
                        }

                        // Download PDF if resolved by enrichment and not already provided
                        if enrich_pdf_url.is_none() {
                            if let Some(ref pdf_url) = enrichment.pdf_url {
                                let pdf_task_id = format!("pdf-{}", enrich_paper_id);
                                emit_task(
                                    &enrich_app,
                                    &BackgroundTaskEvent {
                                        task_id: pdf_task_id.clone(),
                                        paper_id: enrich_paper_id.clone(),
                                        paper_title: enrich_title.clone(),
                                        task_type: "pdf-download".into(),
                                        status: "running".into(),
                                        message: Some("PDF found via OA lookup".into()),
                                    },
                                );
                                let pdf_path = enrich_paper_dir.join("paper.pdf");
                                if !pdf_path.exists() {
                                    match storage::attachments::download_file(pdf_url, &pdf_path)
                                        .await
                                    {
                                        Ok(()) => {
                                            let file_size =
                                                storage::attachments::get_file_size(&pdf_path);
                                            let _ = attachments::insert_attachment(
                                                &db.conn,
                                                &enrich_paper_id,
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
                                                    paper_id: enrich_paper_id.clone(),
                                                    paper_title: enrich_title.clone(),
                                                    task_type: "pdf-download".into(),
                                                    status: "completed".into(),
                                                    message: None,
                                                },
                                            );
                                            let _ =
                                                enrich_app.emit("paper-updated", &enrich_paper_id);
                                        }
                                        Err(e) => {
                                            emit_task(
                                                &enrich_app,
                                                &BackgroundTaskEvent {
                                                    task_id: pdf_task_id,
                                                    paper_id: enrich_paper_id.clone(),
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
                    let resolved_arxiv = enrich_arxiv.as_deref().or(enrichment.arxiv_id.as_deref());
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

                    emit_task(
                        &enrich_app,
                        &BackgroundTaskEvent {
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
                    emit_task(
                        &enrich_app,
                        &BackgroundTaskEvent {
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

    let author_list = papers::get_paper_authors(&db.conn, &row.id).unwrap_or_default();
    let attachment_list = attachments::get_paper_attachments(&db.conn, &row.id).unwrap_or_default();
    let note_list = notes::list_notes(&db.conn, &row.id).unwrap_or_default();
    sync_after_mutation(&state, &db, Some(&row.id));

    Ok(paper_row_to_response(
        &row,
        author_list,
        tag_list,
        attachment_list,
        note_list,
        &state.data_dir,
    ))
}

// --- Local file import ---

#[derive(Debug, serde::Serialize)]
pub struct ImportResult {
    pub imported: Vec<PaperResponse>,
    pub skipped: Vec<ImportSkipped>,
}

#[derive(Debug, serde::Serialize)]
pub struct ImportSkipped {
    pub path: String,
    pub reason: String,
}

/// Import local files (PDFs) into the library.
///
/// For each file:
/// 1. Extract metadata from the PDF (title, DOI, arXiv ID)
/// 2. Create a paper record with the extracted (or filename-derived) title
/// 3. Copy the file into the paper directory
/// 4. Spawn background metadata enrichment if DOI or arXiv ID was found
#[tauri::command]
pub async fn import_local_files(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    file_paths: Vec<String>,
) -> Result<ImportResult, String> {
    let mut imported = Vec::new();
    let mut skipped = Vec::new();

    for file_path_str in &file_paths {
        let file_path = Path::new(file_path_str);

        // Validate the file exists
        if !file_path.exists() {
            skipped.push(ImportSkipped {
                path: file_path_str.clone(),
                reason: "File not found".to_string(),
            });
            continue;
        }

        // Only support PDF files for now
        let extension = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if extension != "pdf" {
            skipped.push(ImportSkipped {
                path: file_path_str.clone(),
                reason: format!("Unsupported file type: .{}", extension),
            });
            continue;
        }

        // Extract metadata from PDF
        let pdf_meta = match zoro_metadata::pdf_extract::extract_pdf_metadata(file_path) {
            Ok(meta) => meta,
            Err(e) => {
                tracing::debug!(
                    "PDF metadata extraction failed for {}: {}",
                    file_path_str,
                    e
                );
                // Still import the file, just with filename as title
                zoro_metadata::pdf_extract::PdfMetadata::default()
            }
        };

        // Derive title: prefer PDF metadata, fall back to filename
        let title = pdf_meta.title.clone().unwrap_or_else(|| {
            file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled")
                .to_string()
        });

        let doi = pdf_meta.doi.clone();
        let arxiv_id = pdf_meta.arxiv_id.clone();

        let identifier = doi.as_deref().or(arxiv_id.as_deref()).unwrap_or(&title);
        let slug = generate_paper_slug(&title, identifier, None);

        // Create paper directory
        let papers_dir = state.data_dir.join("library/papers");
        let paper_dir = storage::paper_dir::create_paper_dir(&papers_dir, &slug)
            .map_err(|e| format!("Failed to create paper directory: {}", e))?;

        // Build author list from PDF metadata
        let authors: Vec<(String, Option<String>, Option<String>)> = pdf_meta
            .authors
            .as_ref()
            .map(|names| names.iter().map(|n| (n.clone(), None, None)).collect())
            .unwrap_or_default();

        // Insert paper into DB
        let db_input = papers::CreatePaperInput {
            slug: slug.clone(),
            title: title.clone(),
            short_title: None,
            abstract_text: None,
            doi: doi.clone(),
            arxiv_id: arxiv_id.clone(),
            url: None,
            pdf_url: None,
            html_url: None,
            thumbnail_url: None,
            published_date: None,
            source: Some("local-import".to_string()),
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

        let db = state
            .db
            .lock()
            .map_err(|e| format!("DB lock error: {}", e))?;
        let row = papers::insert_paper(&db.conn, &db_input)
            .map_err(|e| format!("Failed to insert paper: {}", e))?;

        // Set authors
        if !authors.is_empty() {
            let _ = papers::set_paper_authors(&db.conn, &row.id, &authors);
        }

        // Copy the PDF file into the paper directory
        let dest_path = paper_dir.join("paper.pdf");
        std::fs::copy(file_path, &dest_path)
            .map_err(|e| format!("Failed to copy PDF file: {}", e))?;

        // Record the attachment
        let file_size = storage::attachments::get_file_size(&dest_path);
        let _ = attachments::insert_attachment(
            &db.conn,
            &row.id,
            "paper.pdf",
            "pdf",
            Some("application/pdf"),
            file_size,
            "paper.pdf",
            "local-import",
        );

        // Write metadata.json
        let author_models: Vec<Author> = pdf_meta
            .authors
            .as_ref()
            .map(|names| {
                names
                    .iter()
                    .map(|n| Author {
                        name: n.clone(),
                        affiliation: None,
                        orcid: None,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let metadata = PaperMetadata {
            id: row.id.clone(),
            slug: slug.clone(),
            title: title.clone(),
            short_title: None,
            authors: author_models,
            abstract_text: None,
            doi: doi.clone(),
            arxiv_id: arxiv_id.clone(),
            url: None,
            pdf_url: None,
            html_url: None,
            thumbnail_url: None,
            published_date: None,
            added_date: row.added_date.clone(),
            source: Some("local-import".to_string()),
            tags: Vec::new(),
            collections: Vec::new(),
            attachments: vec![AttachmentInfo {
                filename: "paper.pdf".to_string(),
                attachment_type: "application/pdf".to_string(),
                created: chrono::Utc::now().to_rfc3339(),
            }],
            notes: Vec::new(),
            read_status: ReadStatus::Unread,
            rating: None,
            extra: serde_json::json!({}),
            entry_type: None,
            journal: None,
            volume: None,
            issue: None,
            pages: None,
            publisher: None,
            issn: None,
            isbn: None,
            annotations: Vec::new(),
        };
        let _ = storage::paper_dir::write_metadata(&paper_dir, &metadata);

        // Build response before releasing the DB lock
        let author_list = papers::get_paper_authors(&db.conn, &row.id).unwrap_or_default();
        let tag_list = tags::get_paper_tags(&db.conn, &row.id).unwrap_or_default();
        let attachment_list =
            attachments::get_paper_attachments(&db.conn, &row.id).unwrap_or_default();
        let note_list = notes::list_notes(&db.conn, &row.id).unwrap_or_default();
        sync_after_mutation(&state, &db, Some(&row.id));

        let response = paper_row_to_response(
            &row,
            author_list,
            tag_list,
            attachment_list,
            note_list,
            &state.data_dir,
        );

        // Drop the DB lock before spawning background tasks
        let paper_id = row.id.clone();
        drop(db);

        // Always spawn background metadata enrichment (title-based search as fallback)
        {
            let enrich_paper_id = paper_id.clone();
            let enrich_doi = doi.clone();
            let enrich_arxiv = arxiv_id.clone();
            let enrich_db_path = state.data_dir.join("library.db");
            let enrich_paper_dir = paper_dir.clone();
            let enrich_app = app.clone();
            let enrich_title = title.clone();
            let enrich_task_id = format!("enrich-{}", enrich_paper_id);
            emit_task(
                &app,
                &BackgroundTaskEvent {
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
                        if let Ok(enrich_db) = zoro_db::Database::open(&enrich_db_path) {
                            if let Ok(current) =
                                papers::get_paper(&enrich_db.conn, &enrich_paper_id)
                            {
                                let update = crate::connector::handlers::build_enrichment_update(
                                    &current,
                                    &enrichment,
                                );
                                let _ = papers::update_paper(
                                    &enrich_db.conn,
                                    &enrich_paper_id,
                                    &update,
                                );
                                if let Some(ref enrich_authors) = enrichment.authors {
                                    if let Ok(existing) =
                                        papers::get_paper_authors(&enrich_db.conn, &enrich_paper_id)
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
                                            let _ = papers::set_paper_authors(
                                                &enrich_db.conn,
                                                &enrich_paper_id,
                                                &tuples,
                                            );
                                        }
                                    }
                                }

                                // Download PDF if enrichment resolved one and we don't have it
                                if enrich_doi.is_none() || current.pdf_url.is_none() {
                                    if let Some(ref pdf_url) = enrichment.pdf_url {
                                        let pdf_path = enrich_paper_dir.join("paper.pdf");
                                        if !pdf_path.exists() {
                                            let pdf_task_id = format!("pdf-{}", enrich_paper_id);
                                            emit_task(
                                                &enrich_app,
                                                &BackgroundTaskEvent {
                                                    task_id: pdf_task_id.clone(),
                                                    paper_id: enrich_paper_id.clone(),
                                                    paper_title: enrich_title.clone(),
                                                    task_type: "pdf-download".into(),
                                                    status: "running".into(),
                                                    message: Some("PDF found via OA lookup".into()),
                                                },
                                            );
                                            match storage::attachments::download_file(
                                                pdf_url, &pdf_path,
                                            )
                                            .await
                                            {
                                                Ok(()) => {
                                                    let file_size =
                                                        storage::attachments::get_file_size(
                                                            &pdf_path,
                                                        );
                                                    let _ = attachments::insert_attachment(
                                                        &enrich_db.conn,
                                                        &enrich_paper_id,
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
                                                            paper_id: enrich_paper_id.clone(),
                                                            paper_title: enrich_title.clone(),
                                                            task_type: "pdf-download".into(),
                                                            status: "completed".into(),
                                                            message: None,
                                                        },
                                                    );
                                                    let _ = enrich_app
                                                        .emit("paper-updated", &enrich_paper_id);
                                                }
                                                Err(e) => {
                                                    emit_task(
                                                        &enrich_app,
                                                        &BackgroundTaskEvent {
                                                            task_id: pdf_task_id,
                                                            paper_id: enrich_paper_id.clone(),
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

                        emit_task(
                            &enrich_app,
                            &BackgroundTaskEvent {
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
                        emit_task(
                            &enrich_app,
                            &BackgroundTaskEvent {
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

        imported.push(response);
    }

    Ok(ImportResult { imported, skipped })
}

#[tauri::command]
pub async fn get_paper(state: State<'_, AppState>, id: String) -> Result<PaperResponse, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let row = papers::get_paper(&db.conn, &id).map_err(|e| format!("{}", e))?;
    let author_list = papers::get_paper_authors(&db.conn, &id).unwrap_or_default();
    let tag_list = tags::get_paper_tags(&db.conn, &id).unwrap_or_default();
    let attachment_list = attachments::get_paper_attachments(&db.conn, &id).unwrap_or_default();
    let note_list = notes::list_notes(&db.conn, &id).unwrap_or_default();
    Ok(paper_row_to_response(
        &row,
        author_list,
        tag_list,
        attachment_list,
        note_list,
        &state.data_dir,
    ))
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn list_papers(
    state: State<'_, AppState>,
    collection_id: Option<String>,
    tag_name: Option<String>,
    read_status: Option<String>,
    uncategorized: Option<bool>,
    sort_by: Option<String>,
    sort_order: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<PaperResponse>, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let filter = papers::PaperFilter {
        collection_id,
        tag_name,
        read_status,
        search_query: None,
        uncategorized,
        sort_by,
        sort_order,
        limit,
        offset,
    };
    let rows = papers::list_papers(&db.conn, &filter).map_err(|e| format!("{}", e))?;

    let mut result = Vec::new();
    for row in &rows {
        let author_list = papers::get_paper_authors(&db.conn, &row.id).unwrap_or_default();
        let tag_list = tags::get_paper_tags(&db.conn, &row.id).unwrap_or_default();
        let attachment_list =
            attachments::get_paper_attachments(&db.conn, &row.id).unwrap_or_default();
        let note_list = notes::list_notes(&db.conn, &row.id).unwrap_or_default();
        result.push(paper_row_to_response(
            row,
            author_list,
            tag_list,
            attachment_list,
            note_list,
            &state.data_dir,
        ));
    }
    Ok(result)
}

#[tauri::command]
pub async fn delete_paper(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let row = papers::get_paper(&db.conn, &id).map_err(|e| format!("{}", e))?;
    papers::delete_paper(&db.conn, &id).map_err(|e| format!("{}", e))?;
    // Rebuild index after delete (no paper to sync since it's deleted)
    storage::sync::rebuild_library_index(&db, &state.data_dir);
    drop(db);

    let papers_dir = state.data_dir.join("library/papers");
    let _ = storage::paper_dir::delete_paper_dir(&papers_dir, &row.slug);
    Ok(())
}

#[tauri::command]
pub async fn update_paper_status(
    state: State<'_, AppState>,
    id: String,
    read_status: String,
) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    papers::update_paper_status(&db.conn, &id, &read_status).map_err(|e| format!("{}", e))?;
    sync_after_mutation(&state, &db, Some(&id));
    Ok(())
}

#[tauri::command]
pub async fn update_paper_rating(
    state: State<'_, AppState>,
    id: String,
    rating: Option<i32>,
) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    papers::update_paper_rating(&db.conn, &id, rating).map_err(|e| format!("{}", e))?;
    sync_after_mutation(&state, &db, Some(&id));
    Ok(())
}

#[tauri::command]
pub async fn create_collection(
    state: State<'_, AppState>,
    name: String,
    parent_id: Option<String>,
    description: Option<String>,
) -> Result<CollectionResponse, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let row = collections::create_collection(
        &db.conn,
        &name,
        parent_id.as_deref(),
        description.as_deref(),
    )
    .map_err(|e| format!("{}", e))?;

    // Track change for sync
    {
        let mut changes = std::collections::HashMap::new();
        changes.insert(
            "name".to_string(),
            serde_json::json!({"new_value": row.name}),
        );
        if let Some(ref pid) = row.parent_id {
            changes.insert(
                "parent_id".to_string(),
                serde_json::json!({"new_value": pid}),
            );
        }
        if let Some(ref desc) = row.description {
            changes.insert(
                "description".to_string(),
                serde_json::json!({"new_value": desc}),
            );
        }
        let _ = db.track_change("collection", &row.id, "create", Some(&changes), None);
    }

    sync_after_mutation(&state, &db, None);
    Ok(CollectionResponse {
        id: row.id,
        name: row.name,
        slug: row.slug,
        parent_id: row.parent_id,
        paper_count: 0,
        description: row.description,
    })
}

#[tauri::command]
pub async fn list_collections(
    state: State<'_, AppState>,
) -> Result<Vec<CollectionResponse>, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let rows = collections::list_collections(&db.conn).map_err(|e| format!("{}", e))?;
    let mut result = Vec::new();
    for row in rows {
        let count = collections::get_collection_paper_count(&db.conn, &row.id).unwrap_or(0);
        result.push(CollectionResponse {
            id: row.id,
            name: row.name,
            slug: row.slug,
            parent_id: row.parent_id,
            paper_count: count,
            description: row.description,
        });
    }
    Ok(result)
}

#[tauri::command]
pub async fn delete_collection(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    collections::delete_collection(&db.conn, &id).map_err(|e| format!("{}", e))?;

    // Track change for sync
    let _ = db.track_change("collection", &id, "delete", None, None);

    sync_after_mutation(&state, &db, None);
    Ok(())
}

#[tauri::command]
pub async fn add_paper_to_collection(
    state: State<'_, AppState>,
    paper_id: String,
    collection_id: String,
) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    collections::add_paper_to_collection(&db.conn, &paper_id, &collection_id)
        .map_err(|e| format!("{}", e))?;

    // Track paper-collection association change for sync
    {
        let mut changes = std::collections::HashMap::new();
        changes.insert(
            "paper_id".to_string(),
            serde_json::json!({"new_value": paper_id}),
        );
        changes.insert(
            "collection_id".to_string(),
            serde_json::json!({"new_value": collection_id}),
        );
        let entity_id = format!("{}::{}", paper_id, collection_id);
        let _ = db.track_change(
            "paper_collection",
            &entity_id,
            "create",
            Some(&changes),
            None,
        );
    }

    sync_after_mutation(&state, &db, Some(&paper_id));
    Ok(())
}

#[tauri::command]
pub async fn remove_paper_from_collection(
    state: State<'_, AppState>,
    paper_id: String,
    collection_id: String,
) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    collections::remove_paper_from_collection(&db.conn, &paper_id, &collection_id)
        .map_err(|e| format!("{}", e))?;

    // Track paper-collection disassociation for sync
    {
        let mut changes = std::collections::HashMap::new();
        changes.insert(
            "paper_id".to_string(),
            serde_json::json!({"new_value": paper_id}),
        );
        changes.insert(
            "collection_id".to_string(),
            serde_json::json!({"new_value": collection_id}),
        );
        let entity_id = format!("{}::{}", paper_id, collection_id);
        let _ = db.track_change(
            "paper_collection",
            &entity_id,
            "delete",
            Some(&changes),
            None,
        );
    }

    sync_after_mutation(&state, &db, Some(&paper_id));
    Ok(())
}

#[tauri::command]
pub async fn list_tags(state: State<'_, AppState>) -> Result<Vec<TagResponse>, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let rows = tags::list_tags(&db.conn).map_err(|e| format!("{}", e))?;
    Ok(rows
        .into_iter()
        .map(|t| TagResponse {
            id: t.id,
            name: t.name,
            color: t.color,
        })
        .collect())
}

#[tauri::command]
pub async fn add_tag_to_paper(
    state: State<'_, AppState>,
    paper_id: String,
    tag_name: String,
) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    tags::add_tag_to_paper(&db.conn, &paper_id, &tag_name, "manual")
        .map_err(|e| format!("{}", e))?;

    // Track paper-tag association for sync
    {
        let mut changes = std::collections::HashMap::new();
        changes.insert(
            "paper_id".to_string(),
            serde_json::json!({"new_value": paper_id}),
        );
        changes.insert(
            "tag_name".to_string(),
            serde_json::json!({"new_value": tag_name}),
        );
        let entity_id = format!("{}::{}", paper_id, tag_name);
        let _ = db.track_change("paper_tag", &entity_id, "create", Some(&changes), None);
    }

    sync_after_mutation(&state, &db, Some(&paper_id));
    Ok(())
}

#[tauri::command]
pub async fn remove_tag_from_paper(
    state: State<'_, AppState>,
    paper_id: String,
    tag_name: String,
) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    tags::remove_tag_from_paper(&db.conn, &paper_id, &tag_name).map_err(|e| format!("{}", e))?;

    // Track paper-tag disassociation for sync
    {
        let mut changes = std::collections::HashMap::new();
        changes.insert(
            "paper_id".to_string(),
            serde_json::json!({"new_value": paper_id}),
        );
        changes.insert(
            "tag_name".to_string(),
            serde_json::json!({"new_value": tag_name}),
        );
        let entity_id = format!("{}::{}", paper_id, tag_name);
        let _ = db.track_change("paper_tag", &entity_id, "delete", Some(&changes), None);
    }

    sync_after_mutation(&state, &db, Some(&paper_id));
    Ok(())
}

// --- New commands ---

#[derive(Debug, serde::Deserialize)]
pub struct UpdateCollectionInput {
    pub name: Option<String>,
    pub parent_id: Option<Option<String>>,
    pub description: Option<Option<String>>,
    pub position: Option<i32>,
}

#[tauri::command]
pub async fn update_collection(
    state: State<'_, AppState>,
    id: String,
    input: UpdateCollectionInput,
) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    collections::update_collection(
        &db.conn,
        &id,
        input.name.as_deref(),
        input.parent_id.as_ref().map(|p| p.as_deref()),
        input.description.as_ref().map(|d| d.as_deref()),
        input.position,
    )
    .map_err(|e| format!("{}", e))?;

    // Track change for sync
    {
        let mut changes = std::collections::HashMap::new();
        if let Some(ref name) = input.name {
            changes.insert("name".to_string(), serde_json::json!({"new_value": name}));
        }
        if let Some(ref pid) = input.parent_id {
            changes.insert(
                "parent_id".to_string(),
                serde_json::json!({"new_value": pid}),
            );
        }
        if let Some(ref desc) = input.description {
            changes.insert(
                "description".to_string(),
                serde_json::json!({"new_value": desc}),
            );
        }
        if let Some(pos) = input.position {
            changes.insert(
                "position".to_string(),
                serde_json::json!({"new_value": pos}),
            );
        }
        if !changes.is_empty() {
            let _ = db.track_change("collection", &id, "update", Some(&changes), None);
        }
    }

    sync_after_mutation(&state, &db, None);
    Ok(())
}

#[tauri::command]
pub async fn get_collections_for_paper(
    state: State<'_, AppState>,
    paper_id: String,
) -> Result<Vec<CollectionResponse>, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let rows = collections::get_collections_for_paper(&db.conn, &paper_id)
        .map_err(|e| format!("{}", e))?;
    let mut result = Vec::new();
    for row in rows {
        let count = collections::get_collection_paper_count(&db.conn, &row.id).unwrap_or(0);
        result.push(CollectionResponse {
            id: row.id,
            name: row.name,
            slug: row.slug,
            parent_id: row.parent_id,
            paper_count: count,
            description: row.description,
        });
    }
    Ok(result)
}

#[tauri::command]
pub async fn count_uncategorized_papers(state: State<'_, AppState>) -> Result<i64, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    collections::count_uncategorized_papers(&db.conn).map_err(|e| format!("{}", e))
}

#[derive(Debug, serde::Deserialize)]
pub struct ReorderItem {
    pub id: String,
    pub position: i32,
}

#[tauri::command]
pub async fn reorder_collections(
    state: State<'_, AppState>,
    items: Vec<ReorderItem>,
) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let pairs: Vec<(String, i32)> = items.into_iter().map(|i| (i.id, i.position)).collect();
    collections::reorder_collections(&db.conn, &pairs).map_err(|e| format!("{}", e))?;
    sync_after_mutation(&state, &db, None);
    Ok(())
}

#[tauri::command]
pub async fn delete_tag(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    tags::delete_tag(&db.conn, &id).map_err(|e| format!("{}", e))?;
    sync_after_mutation(&state, &db, None);
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateTagInput {
    pub name: Option<String>,
    pub color: Option<Option<String>>,
}

#[tauri::command]
pub async fn update_tag(
    state: State<'_, AppState>,
    id: String,
    input: UpdateTagInput,
) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    tags::update_tag(
        &db.conn,
        &id,
        input.name.as_deref(),
        input.color.as_ref().map(|c| c.as_deref()),
    )
    .map_err(|e| format!("{}", e))?;
    sync_after_mutation(&state, &db, None);
    Ok(())
}

#[tauri::command]
pub async fn search_tags(
    state: State<'_, AppState>,
    prefix: String,
    limit: Option<i64>,
) -> Result<Vec<TagResponse>, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let rows =
        tags::search_tags(&db.conn, &prefix, limit.unwrap_or(20)).map_err(|e| format!("{}", e))?;
    Ok(rows
        .into_iter()
        .map(|t| TagResponse {
            id: t.id,
            name: t.name,
            color: t.color,
        })
        .collect())
}

/// Deserialize `Option<Option<T>>` correctly so that:
/// - missing field  → `None`       (don't update)
/// - `"field": null` → `Some(None)` (set to NULL)
/// - `"field": "v"`  → `Some(Some("v"))`
fn deserialize_double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    <Option<T> as serde::Deserialize>::deserialize(deserializer).map(Some)
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdatePaperInput {
    pub title: Option<String>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub short_title: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub abstract_text: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub doi: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub arxiv_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub url: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub pdf_url: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub html_url: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub thumbnail_url: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub published_date: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub source: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub extra_json: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub entry_type: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub journal: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub volume: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub issue: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub pages: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub publisher: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub issn: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub isbn: Option<Option<String>>,
}

#[tauri::command]
pub async fn update_paper(
    state: State<'_, AppState>,
    id: String,
    input: UpdatePaperInput,
) -> Result<PaperResponse, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let db_input = papers::UpdatePaperInput {
        title: input.title,
        short_title: input.short_title,
        abstract_text: input.abstract_text,
        doi: input.doi,
        arxiv_id: input.arxiv_id,
        url: input.url,
        pdf_url: input.pdf_url,
        html_url: input.html_url,
        thumbnail_url: input.thumbnail_url,
        published_date: input.published_date,
        source: input.source,
        extra_json: input.extra_json,
        entry_type: input.entry_type,
        journal: input.journal,
        volume: input.volume,
        issue: input.issue,
        pages: input.pages,
        publisher: input.publisher,
        issn: input.issn,
        isbn: input.isbn,
    };
    papers::update_paper(&db.conn, &id, &db_input).map_err(|e| format!("{}", e))?;

    let _ = citations::delete_paper_citation_cache(&db.conn, &id);

    sync_after_mutation(&state, &db, Some(&id));

    let row = papers::get_paper(&db.conn, &id).map_err(|e| format!("{}", e))?;
    let author_list = papers::get_paper_authors(&db.conn, &id).unwrap_or_default();
    let tag_list = tags::get_paper_tags(&db.conn, &id).unwrap_or_default();
    let attachment_list = attachments::get_paper_attachments(&db.conn, &id).unwrap_or_default();
    let note_list = notes::list_notes(&db.conn, &id).unwrap_or_default();
    Ok(paper_row_to_response(
        &row,
        author_list,
        tag_list,
        attachment_list,
        note_list,
        &state.data_dir,
    ))
}

#[tauri::command]
pub async fn update_paper_authors(
    state: State<'_, AppState>,
    paper_id: String,
    author_names: Vec<String>,
) -> Result<PaperResponse, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;

    let authors: Vec<(String, Option<String>, Option<String>)> = author_names
        .into_iter()
        .map(|name| (name, None, None))
        .collect();

    papers::set_paper_authors(&db.conn, &paper_id, &authors)
        .map_err(|e| format!("Failed to set authors: {}", e))?;

    sync_after_mutation(&state, &db, Some(&paper_id));

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

/// Add local files as attachments to an existing paper.
///
/// For each file path:
/// 1. Copy the file into the paper's `attachments/` subdirectory
/// 2. Insert an attachment record in the database
/// 3. Return the updated PaperResponse
#[tauri::command]
pub async fn add_attachment_files(
    state: State<'_, AppState>,
    paper_id: String,
    file_paths: Vec<String>,
) -> Result<PaperResponse, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let row = papers::get_paper(&db.conn, &paper_id).map_err(|e| format!("{}", e))?;
    let paper_dir = state.data_dir.join("library").join(&row.dir_path);
    let attachments_dir = paper_dir.join("attachments");
    std::fs::create_dir_all(&attachments_dir)
        .map_err(|e| format!("Failed to create attachments dir: {}", e))?;

    for file_path_str in &file_paths {
        let src = Path::new(file_path_str);
        if !src.exists() {
            tracing::warn!("Attachment file not found, skipping: {}", file_path_str);
            continue;
        }

        let filename = src
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("attachment")
            .to_string();

        // Avoid overwriting: if a file with the same name already exists, add a suffix
        let mut dest_name = filename.clone();
        let mut dest = attachments_dir.join(&dest_name);
        let mut counter = 1u32;
        while dest.exists() {
            let stem = src
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("attachment");
            let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("");
            dest_name = if ext.is_empty() {
                format!("{} ({})", stem, counter)
            } else {
                format!("{} ({}).{}", stem, counter, ext)
            };
            dest = attachments_dir.join(&dest_name);
            counter += 1;
        }

        std::fs::copy(src, &dest)
            .map_err(|e| format!("Failed to copy file {}: {}", file_path_str, e))?;

        let file_size = storage::attachments::get_file_size(&dest);
        let ext = src
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let file_type = match ext.as_str() {
            "pdf" => "pdf",
            "html" | "htm" => "html",
            "epub" => "epub",
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" => "image",
            _ => "other",
        };
        let mime_type = mime_from_ext(&ext);
        let relative_path = format!("attachments/{}", dest_name);

        let _ = attachments::insert_attachment(
            &db.conn,
            &paper_id,
            &dest_name,
            file_type,
            Some(mime_type),
            file_size,
            &relative_path,
            "user-attachment",
        );
    }

    sync_after_mutation(&state, &db, Some(&paper_id));

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

fn mime_from_ext(ext: &str) -> &'static str {
    match ext {
        "pdf" => "application/pdf",
        "html" | "htm" => "text/html",
        "epub" => "application/epub+zip",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "txt" => "text/plain",
        "json" => "application/json",
        "xml" => "application/xml",
        "csv" => "text/csv",
        "zip" => "application/zip",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        _ => "application/octet-stream",
    }
}

#[tauri::command]
pub async fn get_paper_pdf_path(
    state: State<'_, AppState>,
    paper_id: String,
) -> Result<String, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let row = papers::get_paper(&db.conn, &paper_id).map_err(|e| format!("{}", e))?;
    let paper_dir = state.data_dir.join("library").join(&row.dir_path);

    // Check the default paper.pdf location first
    let pdf_path = paper_dir.join("paper.pdf");
    if pdf_path.exists() {
        return pdf_path
            .to_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Invalid path encoding".to_string());
    }

    // Fall back to PDF attachments from the attachments table
    // (Zotero connector saves PDFs under attachments/{name}.pdf)
    if let Ok(atts) = attachments::get_paper_attachments(&db.conn, &paper_id) {
        for att in &atts {
            if att.file_type == "pdf" {
                let att_path = paper_dir.join(&att.relative_path);
                if att_path.exists() {
                    return att_path
                        .to_str()
                        .map(|s| s.to_string())
                        .ok_or_else(|| "Invalid path encoding".to_string());
                }
            }
        }
    }

    Err(format!("PDF file not found: {}", pdf_path.display()))
}

#[tauri::command]
pub async fn get_paper_html_path(
    state: State<'_, AppState>,
    paper_id: String,
) -> Result<String, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let row = papers::get_paper(&db.conn, &paper_id).map_err(|e| format!("{}", e))?;
    let paper_dir = state.data_dir.join("library").join(&row.dir_path);

    // Only serve full-text HTML (paper.html); abs.html is an abstract page snapshot
    // and should not be used in the HTML reader.
    let html_path = paper_dir.join("paper.html");
    if html_path.exists() {
        return html_path
            .to_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Invalid path encoding".to_string());
    }

    // Fall back to HTML attachments from the attachments table
    if let Ok(atts) = attachments::get_paper_attachments(&db.conn, &paper_id) {
        for att in &atts {
            if att.file_type == "html" {
                let att_path = paper_dir.join(&att.relative_path);
                if att_path.exists() {
                    return att_path
                        .to_str()
                        .map(|s| s.to_string())
                        .ok_or_else(|| "Invalid path encoding".to_string());
                }
            }
        }
    }

    Err(format!("HTML file not found: {}", html_path.display()))
}

/// Resolve the absolute path to a specific file within a paper's directory.
/// Looks in the paper root, then the attachments/ subdirectory.
#[tauri::command]
pub async fn get_paper_file_path(
    state: State<'_, AppState>,
    paper_id: String,
    filename: String,
) -> Result<String, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let row = papers::get_paper(&db.conn, &paper_id).map_err(|e| format!("{}", e))?;
    let paper_dir = state.data_dir.join("library").join(&row.dir_path);

    // Check paper root directory first
    let file_path = paper_dir.join(&filename);
    if file_path.exists() {
        return file_path
            .to_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Invalid path encoding".to_string());
    }

    // Check attachments/ subdirectory
    let att_path = paper_dir.join("attachments").join(&filename);
    if att_path.exists() {
        return att_path
            .to_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Invalid path encoding".to_string());
    }

    Err(format!("File not found: {}", filename))
}

// ── Background filesystem scanner ───────────────────────────────────────

/// Spawn a background task that periodically scans paper directories and
/// reconciles the `pdf_downloaded` / `html_downloaded` flags with what is
/// actually on disk.  This ensures that files added externally (e.g. by an
/// AI agent, a sync tool, or manual copy) are detected automatically.
pub fn start_filesystem_scanner(app: tauri::AppHandle) {
    use std::time::Duration;
    tauri::async_runtime::spawn(async move {
        // Give the app a chance to fully start up
        tokio::time::sleep(Duration::from_secs(15)).await;

        loop {
            run_filesystem_scan(&app);
            // Scan every 60 seconds
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}

/// Single pass: check every paper's directory for pdf/html presence and update
/// the DB flags when they diverge.
fn run_filesystem_scan(app: &tauri::AppHandle) {
    use tauri::Manager;

    let state = app.state::<crate::AppState>();
    let data_dir = state.data_dir.clone();

    let db = match state.db.lock() {
        Ok(db) => db,
        Err(_) => return,
    };

    // Fetch only id + dir_path + current flags to keep memory low
    let rows: Vec<(String, String, bool, bool)> = {
        let mut stmt = match db.conn.prepare(
            "SELECT id, dir_path, COALESCE(pdf_downloaded, 1), COALESCE(html_downloaded, 1) FROM papers",
        ) {
            Ok(s) => s,
            Err(_) => return,
        };
        stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, bool>(2)?,
                row.get::<_, bool>(3)?,
            ))
        })
        .ok()
        .map(|iter| iter.flatten().collect())
        .unwrap_or_default()
    };

    let mut pdf_updated = 0u32;
    let mut html_updated = 0u32;

    for (paper_id, dir_path, db_pdf, db_html) in &rows {
        let paper_dir = data_dir.join("library").join(dir_path);
        if !paper_dir.is_dir() {
            // Directory doesn't exist — if DB says downloaded, fix it
            if *db_pdf {
                let _ = papers::set_pdf_downloaded(&db.conn, paper_id, false);
                pdf_updated += 1;
            }
            if *db_html {
                let _ = papers::set_html_downloaded(&db.conn, paper_id, false);
                html_updated += 1;
            }
            continue;
        }

        // Check for PDF: paper.pdf or any *.pdf in the directory
        let has_pdf_on_disk = paper_dir.join("paper.pdf").exists()
            || std::fs::read_dir(&paper_dir)
                .ok()
                .map(|entries| {
                    entries.flatten().any(|e| {
                        let p = e.path();
                        p.is_file()
                            && p.extension()
                                .and_then(|ext| ext.to_str())
                                .map(|ext| ext.eq_ignore_ascii_case("pdf"))
                                .unwrap_or(false)
                    })
                })
                .unwrap_or(false);

        // Check for HTML: paper.html or paper.*.html
        let has_html_on_disk = paper_dir.join("paper.html").exists()
            || std::fs::read_dir(&paper_dir)
                .ok()
                .map(|entries| {
                    entries.flatten().any(|e| {
                        let p = e.path();
                        if !p.is_file() {
                            return false;
                        }
                        let fname = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        // paper.html, paper.zh.html, etc. but NOT abs.html
                        fname != "abs.html"
                            && fname.ends_with(".html")
                            && fname.starts_with("paper")
                    })
                })
                .unwrap_or(false);

        if has_pdf_on_disk != *db_pdf {
            let _ = papers::set_pdf_downloaded(&db.conn, paper_id, has_pdf_on_disk);
            pdf_updated += 1;
        }
        if has_html_on_disk != *db_html {
            let _ = papers::set_html_downloaded(&db.conn, paper_id, has_html_on_disk);
            html_updated += 1;
        }
    }

    if pdf_updated > 0 || html_updated > 0 {
        tracing::info!(
            "Filesystem scan: updated {} pdf_downloaded, {} html_downloaded flags ({} papers checked)",
            pdf_updated,
            html_updated,
            rows.len(),
        );
        // Notify frontend to refresh
        use tauri::Emitter;
        let _ = app.emit("library-changed", ());
    }
}
