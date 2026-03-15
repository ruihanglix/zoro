// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::AppState;
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, State};
use zoro_core::slug_utils::generate_paper_slug;
use zoro_db::queries::{annotations, collections, notes, papers};

// ═══════════════════════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoteroScanResult {
    pub valid: bool,
    pub error: Option<String>,
    pub total_items: usize,
    pub total_collections: usize,
    pub total_tags: usize,
    pub total_attachments: usize,
    pub total_notes: usize,
    pub total_annotations: usize,
    /// Number of PDF attachments whose local files are missing (still in cloud).
    pub cloud_attachments: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoteroImportProgress {
    pub phase: String,
    pub current: usize,
    pub total: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoteroImportResult {
    pub papers_imported: usize,
    pub papers_skipped: usize,
    pub collections_imported: usize,
    pub notes_imported: usize,
    pub attachments_copied: usize,
    pub attachments_missing: usize,
    pub annotations_imported: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoteroImportOptions {
    pub zotero_dir: String,
    pub import_collections: bool,
    pub import_notes: bool,
    pub import_attachments: bool,
    pub import_annotations: bool,
}

// Internal intermediate types for Zotero data
#[allow(dead_code)]
struct ZoteroItemData {
    item_id: i64,
    item_type: String,
    fields: HashMap<String, String>,
    creators: Vec<ZoteroCreatorData>,
    tags: Vec<String>,
    attachment_keys: Vec<ZoteroAttachmentData>,
    note_texts: Vec<String>,
    collection_keys: Vec<String>,
    date_added: Option<String>,
}

struct ZoteroCreatorData {
    first_name: Option<String>,
    last_name: Option<String>,
    name: Option<String>,
    creator_type: String,
}

#[allow(dead_code)]
struct ZoteroAttachmentData {
    key: String,
    content_type: Option<String>,
    path: Option<String>,
    link_mode: i64,
}

struct ZoteroCollectionData {
    key: String,
    name: String,
    parent_key: Option<String>,
}

struct ZoteroAnnotationData {
    annotation_type: String,
    text: Option<String>,
    comment: Option<String>,
    color: Option<String>,
    page_label: Option<String>,
    position: Option<String>,
    parent_item_id: i64,
}

// ═══════════════════════════════════════════════════════════════════════════
// Tauri Commands
// ═══════════════════════════════════════════════════════════════════════════

/// Auto-detect the default Zotero data directory.
#[tauri::command]
pub async fn detect_zotero_dir() -> Result<Option<String>, String> {
    let home = dirs_home().ok_or("Cannot determine home directory")?;

    // Platform-specific defaults
    let candidates = vec![
        home.join("Zotero"),
        #[cfg(target_os = "macos")]
        home.join("Library/Application Support/Zotero/Profiles"),
    ];

    for dir in &candidates {
        if dir.join("zotero.sqlite").exists() {
            return Ok(Some(dir.to_string_lossy().to_string()));
        }
    }

    // macOS: check Zotero profile directory for a redirect
    #[cfg(target_os = "macos")]
    {
        let profiles_dir = home.join("Library/Application Support/Zotero/Profiles");
        if profiles_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&profiles_dir) {
                for entry in entries.flatten() {
                    let prefs_path = entry.path().join("prefs.js");
                    if prefs_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&prefs_path) {
                            for line in content.lines() {
                                if line.contains("extensions.zotero.dataDir")
                                    || line.contains("extensions.zotero.lastDataDir")
                                {
                                    // Extract path from: user_pref("...", "/path/to/dir");
                                    if let Some(start) = line.rfind('"') {
                                        let before = &line[..start];
                                        if let Some(second) = before.rfind('"') {
                                            let path_str = &line[second + 1..start];
                                            let p = PathBuf::from(path_str);
                                            if p.join("zotero.sqlite").exists() {
                                                return Ok(Some(p.to_string_lossy().to_string()));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Fallback: just check ~/Zotero
    let default = home.join("Zotero");
    if default.join("zotero.sqlite").exists() {
        return Ok(Some(default.to_string_lossy().to_string()));
    }

    Ok(None)
}

/// Validate that a directory is a valid Zotero data directory.
#[tauri::command]
pub async fn validate_zotero_dir(path: String) -> Result<bool, String> {
    let dir = PathBuf::from(&path);
    if !dir.exists() {
        return Err("Directory does not exist".to_string());
    }
    if !dir.join("zotero.sqlite").exists() {
        return Err("zotero.sqlite not found in this directory".to_string());
    }
    if !dir.join("storage").exists() {
        return Err(
            "storage/ directory not found — this may not be a Zotero data directory".to_string(),
        );
    }
    Ok(true)
}

/// Scan a Zotero library and return statistics without importing.
#[tauri::command]
pub async fn scan_zotero_library(path: String) -> Result<ZoteroScanResult, String> {
    let db_path = PathBuf::from(&path).join("zotero.sqlite");
    let conn = open_zotero_db(&db_path)?;

    // Count top-level items (not attachments/notes)
    let total_items: usize =
        conn.query_row(
            "SELECT COUNT(*) FROM items i
             JOIN itemTypes it ON i.itemTypeID = it.itemTypeID
             WHERE it.typeName NOT IN ('attachment', 'note', 'annotation')
             AND i.itemID NOT IN (SELECT itemID FROM deletedItems)",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| format!("Failed to count items: {}", e))? as usize;

    let total_collections: usize = conn
        .query_row(
            "SELECT COUNT(*) FROM collections
             WHERE collectionID NOT IN (SELECT collectionID FROM deletedCollections WHERE 1=0)",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0) as usize;

    let total_tags: usize = conn
        .query_row("SELECT COUNT(DISTINCT tagID) FROM itemTags", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap_or(0) as usize;

    let total_attachments: usize = conn
        .query_row(
            "SELECT COUNT(*) FROM itemAttachments ia
             JOIN items i ON ia.itemID = i.itemID
             WHERE i.itemID NOT IN (SELECT itemID FROM deletedItems)
             AND ia.contentType LIKE 'application/pdf%'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0) as usize;

    let total_notes: usize = conn
        .query_row(
            "SELECT COUNT(*) FROM itemNotes inn
             JOIN items i ON inn.itemID = i.itemID
             WHERE i.itemID NOT IN (SELECT itemID FROM deletedItems)
             AND inn.parentItemID IS NOT NULL",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0) as usize;

    // Zotero 6+ annotations
    let total_annotations: usize = conn
        .query_row(
            "SELECT COUNT(*) FROM itemAnnotations ia
             JOIN items i ON ia.itemID = i.itemID
             WHERE i.itemID NOT IN (SELECT itemID FROM deletedItems)",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0) as usize;

    // Count PDF attachments whose local file is missing (still in cloud).
    // For imported files (linkMode 0 or 1), the file lives in storage/<KEY>/.
    // We check whether that directory exists and contains at least one file.
    let storage_dir = PathBuf::from(&path).join("storage");
    let cloud_attachments: usize = {
        let mut stmt = conn
            .prepare(
                "SELECT i.key, ia.path, ia.linkMode
                 FROM itemAttachments ia
                 JOIN items i ON ia.itemID = i.itemID
                 WHERE i.itemID NOT IN (SELECT itemID FROM deletedItems)
                 AND ia.contentType LIKE 'application/pdf%'
                 AND ia.linkMode IN (0, 1)",
            )
            .map_err(|e| format!("Failed to query cloud attachments: {}", e))?;

        let rows: Vec<(String, Option<String>)> = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
            })
            .map_err(|e| format!("{}", e))?
            .filter_map(|r| r.ok())
            .collect();

        let mut missing = 0usize;
        for (key, path_hint) in &rows {
            let att_dir = storage_dir.join(key);
            if !att_dir.exists() {
                missing += 1;
                continue;
            }
            // Directory exists — check if the expected file is present
            let file_found = if let Some(p) = path_hint {
                let filename = p.strip_prefix("storage:").unwrap_or(p);
                att_dir.join(filename).is_file()
            } else {
                false
            };
            if !file_found {
                // Fallback: scan for any non-hidden file
                let has_file = std::fs::read_dir(&att_dir)
                    .ok()
                    .map(|entries| {
                        entries.flatten().any(|e| {
                            let p = e.path();
                            p.is_file()
                                && p.file_name()
                                    .and_then(|n| n.to_str())
                                    .map(|n| !n.starts_with('.'))
                                    .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false);
                if !has_file {
                    missing += 1;
                }
            }
        }
        missing
    };

    Ok(ZoteroScanResult {
        valid: true,
        error: None,
        total_items,
        total_collections,
        total_tags,
        total_attachments,
        total_notes,
        total_annotations,
        cloud_attachments,
    })
}

/// Import the Zotero library into Zoro.
#[tauri::command]
pub async fn import_zotero_library(
    state: State<'_, AppState>,
    app: AppHandle,
    options: ZoteroImportOptions,
) -> Result<ZoteroImportResult, String> {
    let zotero_dir = PathBuf::from(&options.zotero_dir);
    let db_path = zotero_dir.join("zotero.sqlite");
    let storage_dir = zotero_dir.join("storage");

    let zconn = open_zotero_db(&db_path)?;

    let mut result = ZoteroImportResult {
        papers_imported: 0,
        papers_skipped: 0,
        collections_imported: 0,
        notes_imported: 0,
        attachments_copied: 0,
        attachments_missing: 0,
        annotations_imported: 0,
        errors: Vec::new(),
    };

    // ── Phase 1: Read all Zotero data ───────────────────────────────────
    emit_progress(&app, "reading", 0, 1, "Reading Zotero database...");

    let zotero_items = read_zotero_items(&zconn).map_err(|e| e.to_string())?;
    let zotero_collections = if options.import_collections {
        read_zotero_collections(&zconn).map_err(|e| e.to_string())?
    } else {
        Vec::new()
    };

    let total = zotero_items.len();
    emit_progress(&app, "reading", 1, 1, &format!("Found {} items", total));

    // Get Zoro DB connection
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;

    // ── Phase 2: Import collections ─────────────────────────────────────
    // Maps Zotero collection key → Zoro collection ID
    let mut collection_map: HashMap<String, String> = HashMap::new();

    if options.import_collections && !zotero_collections.is_empty() {
        emit_progress(
            &app,
            "collections",
            0,
            zotero_collections.len(),
            "Importing collections...",
        );

        // Topological sort: create root collections first, then children.
        // We iterate in rounds until all collections are created or no progress is made.
        let mut remaining: Vec<&ZoteroCollectionData> = zotero_collections.iter().collect();
        let mut made_progress = true;

        while !remaining.is_empty() && made_progress {
            made_progress = false;
            let mut still_remaining = Vec::new();

            for zc in remaining {
                let parent_id: Option<&str> = match &zc.parent_key {
                    None => None, // Root collection
                    Some(pk) => {
                        // Parent must already be created
                        match collection_map.get(pk) {
                            Some(id) => Some(id.as_str()),
                            None => {
                                // Parent not created yet — defer to next round
                                still_remaining.push(zc);
                                continue;
                            }
                        }
                    }
                };

                match collections::create_collection(&db.conn, &zc.name, parent_id, None) {
                    Ok(row) => {
                        collection_map.insert(zc.key.clone(), row.id);
                        result.collections_imported += 1;
                        made_progress = true;
                    }
                    Err(e) => {
                        result
                            .errors
                            .push(format!("Collection '{}': {}", zc.name, e));
                        made_progress = true; // Still counts as progress (error handled)
                    }
                }
            }

            remaining = still_remaining;
        }

        // Any remaining collections had unresolvable parent references
        for zc in remaining {
            // Create as root collection as fallback
            match collections::create_collection(&db.conn, &zc.name, None, None) {
                Ok(row) => {
                    collection_map.insert(zc.key.clone(), row.id);
                    result.collections_imported += 1;
                }
                Err(e) => {
                    result
                        .errors
                        .push(format!("Collection '{}': {}", zc.name, e));
                }
            }
        }

        emit_progress(
            &app,
            "collections",
            zotero_collections.len(),
            zotero_collections.len(),
            &format!("Imported {} collections", result.collections_imported),
        );
    }

    // ── Phase 3: Import papers ──────────────────────────────────────────
    let papers_dir = state.data_dir.join("library/papers");

    for (idx, item) in zotero_items.iter().enumerate() {
        if idx % 10 == 0 {
            emit_progress(
                &app,
                "papers",
                idx,
                total,
                &format!("Importing paper {}/{}...", idx + 1, total),
            );
        }

        // Check for duplicates by DOI or arXiv ID or URL
        let doi = item.fields.get("DOI").cloned();
        let url = item.fields.get("url").cloned();
        let arxiv_id = extract_arxiv_from_fields(&item.fields);
        let title = item
            .fields
            .get("title")
            .cloned()
            .unwrap_or_else(|| "Untitled".to_string());

        if is_duplicate(
            &db.conn,
            doi.as_deref(),
            arxiv_id.as_deref(),
            url.as_deref(),
        ) {
            result.papers_skipped += 1;
            continue;
        }

        // Build identifier for slug
        let identifier = doi
            .as_deref()
            .or(arxiv_id.as_deref())
            .or(url.as_deref())
            .unwrap_or(&title);
        let published_date = item.fields.get("date").cloned();
        let slug = generate_paper_slug(&title, identifier, published_date.as_deref());

        // Build extra_json with labels (tags) and Zotero metadata
        let mut extra = serde_json::Map::new();
        if !item.tags.is_empty() {
            extra.insert("labels".to_string(), serde_json::json!(item.tags));
        }
        extra.insert(
            "zotero_item_type".to_string(),
            serde_json::json!(item.item_type),
        );
        // Store non-author creators
        let non_authors: Vec<&ZoteroCreatorData> = item
            .creators
            .iter()
            .filter(|c| c.creator_type != "author")
            .collect();
        if !non_authors.is_empty() {
            let creator_vals: Vec<serde_json::Value> = non_authors
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "creatorType": c.creator_type,
                        "firstName": c.first_name,
                        "lastName": c.last_name,
                        "name": c.name,
                    })
                })
                .collect();
            extra.insert(
                "zotero_creators".to_string(),
                serde_json::json!(creator_vals),
            );
        }

        let extra_json = serde_json::to_string(&serde_json::Value::Object(extra)).ok();

        let entry_type = map_zotero_item_type(&item.item_type);

        let input = papers::CreatePaperInput {
            slug: slug.clone(),
            title: title.clone(),
            short_title: item.fields.get("shortTitle").cloned(),
            abstract_text: item.fields.get("abstractNote").cloned(),
            doi,
            arxiv_id,
            url,
            pdf_url: None,
            html_url: None,
            thumbnail_url: None,
            published_date,
            source: Some("zotero-import".to_string()),
            dir_path: format!("papers/{}", slug),
            extra_json,
            entry_type: Some(entry_type),
            journal: item.fields.get("publicationTitle").cloned(),
            volume: item.fields.get("volume").cloned(),
            issue: item.fields.get("issue").cloned(),
            pages: item.fields.get("pages").cloned(),
            publisher: item.fields.get("publisher").cloned(),
            issn: item.fields.get("ISSN").cloned(),
            isbn: item.fields.get("ISBN").cloned(),
            added_date: item.date_added.clone(),
        };

        match papers::insert_paper(&db.conn, &input) {
            Ok(row) => {
                let paper_id = row.id.clone();

                // Set authors
                let authors: Vec<(String, Option<String>, Option<String>)> = item
                    .creators
                    .iter()
                    .filter(|c| c.creator_type == "author")
                    .map(|c| {
                        let name = format_creator_name(c);
                        (name, None, None)
                    })
                    .collect();
                let _ = papers::set_paper_authors(&db.conn, &paper_id, &authors);

                // Create paper directory
                let _ = crate::storage::paper_dir::create_paper_dir(&papers_dir, &slug);
                let paper_dir = papers_dir.join(&slug);

                // Add to collections
                for ck in &item.collection_keys {
                    if let Some(coll_id) = collection_map.get(ck) {
                        let _ = collections::add_paper_to_collection(&db.conn, &paper_id, coll_id);
                    }
                }

                // Copy attachments (PDFs, snapshots)
                if options.import_attachments {
                    let mut first_pdf = true;
                    for att in &item.attachment_keys {
                        // linkMode: 0 = imported_file, 1 = imported_url, 2 = linked_file, 3 = linked_url
                        if att.link_mode > 2 {
                            continue; // linked_url — no local file
                        }

                        // Resolve the source file path based on linkMode
                        let src_file: Option<PathBuf> = if att.link_mode == 2 {
                            // linked_file: path is an absolute (or relative) file path
                            att.path.as_ref().and_then(|p| {
                                let pb = PathBuf::from(p);
                                if pb.is_file() {
                                    Some(pb)
                                } else {
                                    // Try relative to zotero_dir
                                    let relative = zotero_dir.join(p);
                                    if relative.is_file() {
                                        Some(relative)
                                    } else {
                                        None
                                    }
                                }
                            })
                        } else {
                            // imported_file / imported_url: file in storage/<KEY>/
                            let att_dir = storage_dir.join(&att.key);
                            if !att_dir.exists() {
                                None
                            } else {
                                // Try to use path hint from Zotero (format: "storage:filename.pdf")
                                let from_path_hint = att.path.as_ref().and_then(|p| {
                                    let filename = p.strip_prefix("storage:").unwrap_or(p);
                                    let candidate = att_dir.join(filename);
                                    if candidate.is_file() {
                                        Some(candidate)
                                    } else {
                                        None
                                    }
                                });
                                // Fallback: scan directory for first non-hidden file
                                from_path_hint.or_else(|| find_attachment_file(&att_dir))
                            }
                        };

                        let src_file = match src_file {
                            Some(f) => f,
                            None => {
                                // Local file missing — only mark pdf_downloaded=0 if we
                                // haven't already successfully copied a PDF for this paper.
                                // This avoids a later missing attachment overwriting the
                                // successful copy of an earlier one.
                                let is_pdf = att
                                    .content_type
                                    .as_deref()
                                    .map(|ct| ct.contains("pdf"))
                                    .unwrap_or(false);
                                if is_pdf && first_pdf {
                                    // No PDF has been copied yet for this paper — mark it
                                    // as not downloaded so the UI shows a cloud badge.
                                    let _ = papers::set_pdf_downloaded(&db.conn, &paper_id, false);
                                    result.attachments_missing += 1;
                                }
                                continue;
                            }
                        };

                        let is_pdf = att
                            .content_type
                            .as_deref()
                            .map(|ct| ct.contains("pdf"))
                            .unwrap_or(false);

                        let dest_name = if is_pdf && first_pdf {
                            first_pdf = false;
                            "paper.pdf".to_string()
                        } else {
                            src_file
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string()
                        };

                        let dest_path = if is_pdf && dest_name == "paper.pdf" {
                            paper_dir.join(&dest_name)
                        } else {
                            let att_dest = paper_dir.join("attachments");
                            let _ = std::fs::create_dir_all(&att_dest);
                            att_dest.join(&dest_name)
                        };

                        match std::fs::copy(&src_file, &dest_path) {
                            Ok(_) => {
                                result.attachments_copied += 1;
                                // Register in DB if not main PDF
                                if dest_name != "paper.pdf" {
                                    let relative = format!("attachments/{}", dest_name);
                                    let file_type = if is_pdf {
                                        "pdf"
                                    } else if dest_name.ends_with(".html")
                                        || dest_name.ends_with(".htm")
                                    {
                                        "html"
                                    } else {
                                        "other"
                                    };
                                    let _ = register_attachment(
                                        &db.conn,
                                        &paper_id,
                                        &dest_name,
                                        file_type,
                                        &relative,
                                        src_file.metadata().ok().map(|m| m.len() as i64),
                                    );
                                }
                            }
                            Err(e) => {
                                result
                                    .errors
                                    .push(format!("Copy attachment for '{}': {}", title, e));
                            }
                        }
                    }
                }

                // Import notes (HTML → stored as-is)
                if options.import_notes && !item.note_texts.is_empty() {
                    for note_html in &item.note_texts {
                        // Convert basic HTML to markdown-ish text, or store raw
                        let note_content = strip_html_for_note(note_html);
                        if !note_content.trim().is_empty() {
                            match notes::insert_note(&db.conn, &paper_id, &note_content) {
                                Ok(_) => result.notes_imported += 1,
                                Err(e) => {
                                    result.errors.push(format!("Note for '{}': {}", title, e));
                                }
                            }
                        }
                    }
                }

                result.papers_imported += 1;
            }
            Err(e) => {
                result.errors.push(format!("Paper '{}': {}", title, e));
            }
        }
    }

    emit_progress(
        &app,
        "papers",
        total,
        total,
        &format!("Imported {} papers", result.papers_imported),
    );

    // ── Phase 4: Import annotations ─────────────────────────────────────
    if options.import_annotations {
        let zotero_annotations = read_zotero_annotations(&zconn).unwrap_or_default();

        if !zotero_annotations.is_empty() {
            emit_progress(
                &app,
                "annotations",
                0,
                zotero_annotations.len(),
                "Importing annotations...",
            );

            // Build a mapping from Zotero parent attachment itemID → Zoro paper ID
            // We need to find which Zoro paper each annotation belongs to
            let ann_parent_map = build_annotation_parent_map(&zconn, &db.conn).unwrap_or_default();

            for (idx, ann) in zotero_annotations.iter().enumerate() {
                if idx % 50 == 0 {
                    emit_progress(
                        &app,
                        "annotations",
                        idx,
                        zotero_annotations.len(),
                        &format!(
                            "Importing annotation {}/{}...",
                            idx + 1,
                            zotero_annotations.len()
                        ),
                    );
                }

                if let Some(paper_id) = ann_parent_map.get(&ann.parent_item_id) {
                    let ann_type = match ann.annotation_type.as_str() {
                        "highlight" => "highlight",
                        "underline" => "underline",
                        "note" => "note",
                        "ink" => "ink",
                        "image" => "area",
                        _ => "highlight",
                    };
                    let color = ann.color.as_deref().unwrap_or("#ffe28f");
                    let page_number = ann
                        .page_label
                        .as_deref()
                        .and_then(|p| p.parse::<i64>().ok())
                        .unwrap_or(1);

                    // Build position_json from Zotero's position data
                    let position_json = build_annotation_position(&ann.position, page_number);

                    match annotations::insert_annotation(
                        &db.conn,
                        paper_id,
                        ann_type,
                        color,
                        ann.comment.as_deref(),
                        ann.text.as_deref(),
                        None,
                        &position_json,
                        page_number,
                        Some("paper.pdf"),
                    ) {
                        Ok(_) => result.annotations_imported += 1,
                        Err(e) => {
                            if result.errors.len() < 50 {
                                result.errors.push(format!("Annotation: {}", e));
                            }
                        }
                    }
                }
            }

            emit_progress(
                &app,
                "annotations",
                zotero_annotations.len(),
                zotero_annotations.len(),
                &format!("Imported {} annotations", result.annotations_imported),
            );
        }
    }

    // ── Phase 5: Done ───────────────────────────────────────────────────
    emit_progress(&app, "done", 1, 1, "Import complete!");

    tracing::info!(
        "Zotero import complete: {} papers, {} collections, {} notes, {} attachments ({} missing), {} annotations, {} skipped, {} errors",
        result.papers_imported,
        result.collections_imported,
        result.notes_imported,
        result.attachments_copied,
        result.attachments_missing,
        result.annotations_imported,
        result.papers_skipped,
        result.errors.len(),
    );

    Ok(result)
}

// ═══════════════════════════════════════════════════════════════════════════
// Helper functions
// ═══════════════════════════════════════════════════════════════════════════

fn dirs_home() -> Option<PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from)
}

fn open_zotero_db(path: &Path) -> Result<Connection, String> {
    // Open in read-only mode — we never write to Zotero's database
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("Failed to open Zotero database: {}", e))
}

fn emit_progress(app: &AppHandle, phase: &str, current: usize, total: usize, message: &str) {
    let _ = app.emit(
        "zotero-import-progress",
        ZoteroImportProgress {
            phase: phase.to_string(),
            current,
            total,
            message: message.to_string(),
        },
    );
}

/// Read all top-level items (papers) from Zotero's EAV schema.
fn read_zotero_items(conn: &Connection) -> Result<Vec<ZoteroItemData>, String> {
    // Get all non-deleted, non-attachment, non-note items
    let mut stmt = conn
        .prepare(
            "SELECT i.itemID, it.typeName, i.key, i.dateAdded
             FROM items i
             JOIN itemTypes it ON i.itemTypeID = it.itemTypeID
             WHERE it.typeName NOT IN ('attachment', 'note', 'annotation')
             AND i.itemID NOT IN (SELECT itemID FROM deletedItems)",
        )
        .map_err(|e| format!("Failed to query items: {}", e))?;

    let items: Vec<(i64, String, String, Option<String>)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(|e| format!("{}", e))?
        .filter_map(|r| r.ok())
        .collect();

    let mut result = Vec::with_capacity(items.len());

    for (item_id, item_type, _key, date_added) in &items {
        // Read EAV fields
        let fields = read_item_fields(conn, *item_id);

        // Read creators
        let creators = read_item_creators(conn, *item_id);

        // Read tags
        let tags = read_item_tags(conn, *item_id);

        // Read attachment keys (child attachments)
        let attachment_keys = read_item_attachments(conn, *item_id);

        // Read child notes
        let note_texts = read_item_notes(conn, *item_id);

        // Read collection memberships
        let collection_keys = read_item_collections(conn, *item_id);

        result.push(ZoteroItemData {
            item_id: *item_id,
            item_type: item_type.clone(),
            fields,
            creators,
            tags,
            attachment_keys,
            note_texts,
            collection_keys,
            date_added: date_added.clone(),
        });
    }

    Ok(result)
}

fn read_item_fields(conn: &Connection, item_id: i64) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut stmt = conn
        .prepare(
            "SELECT f.fieldName, idv.value
             FROM itemData id
             JOIN fields f ON id.fieldID = f.fieldID
             JOIN itemDataValues idv ON id.valueID = idv.valueID
             WHERE id.itemID = ?1",
        )
        .unwrap();

    let rows = stmt
        .query_map(params![item_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .unwrap();

    for row in rows.flatten() {
        map.insert(row.0, row.1);
    }
    map
}

fn read_item_creators(conn: &Connection, item_id: i64) -> Vec<ZoteroCreatorData> {
    let mut stmt = conn
        .prepare(
            "SELECT c.firstName, c.lastName, ct.creatorType
             FROM itemCreators ic
             JOIN creators c ON ic.creatorID = c.creatorID
             JOIN creatorTypes ct ON ic.creatorTypeID = ct.creatorTypeID
             WHERE ic.itemID = ?1
             ORDER BY ic.orderIndex",
        )
        .unwrap();

    stmt.query_map(params![item_id], |row| {
        let first: Option<String> = row.get(0)?;
        let last: Option<String> = row.get(1)?;
        let ctype: String = row.get(2)?;
        // Zotero uses firstName/lastName for most creators; for
        // institutional names, lastName holds the full name and firstName is empty.
        let name = if first.as_deref().unwrap_or("").is_empty()
            && !last.as_deref().unwrap_or("").is_empty()
        {
            last.clone()
        } else {
            None
        };
        Ok(ZoteroCreatorData {
            first_name: first,
            last_name: last,
            name,
            creator_type: ctype,
        })
    })
    .unwrap()
    .filter_map(|r| r.ok())
    .collect()
}

fn read_item_tags(conn: &Connection, item_id: i64) -> Vec<String> {
    let mut stmt = conn
        .prepare(
            "SELECT t.name FROM itemTags it
             JOIN tags t ON it.tagID = t.tagID
             WHERE it.itemID = ?1",
        )
        .unwrap();

    stmt.query_map(params![item_id], |row| row.get::<_, String>(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
}

fn read_item_attachments(conn: &Connection, item_id: i64) -> Vec<ZoteroAttachmentData> {
    let mut stmt = conn
        .prepare(
            "SELECT i.key, ia.contentType, ia.path, ia.linkMode
             FROM itemAttachments ia
             JOIN items i ON ia.itemID = i.itemID
             WHERE ia.parentItemID = ?1
             AND i.itemID NOT IN (SELECT itemID FROM deletedItems)",
        )
        .unwrap();

    stmt.query_map(params![item_id], |row| {
        Ok(ZoteroAttachmentData {
            key: row.get(0)?,
            content_type: row.get(1)?,
            path: row.get(2)?,
            link_mode: row.get(3)?,
        })
    })
    .unwrap()
    .filter_map(|r| r.ok())
    .collect()
}

fn read_item_notes(conn: &Connection, item_id: i64) -> Vec<String> {
    let mut stmt = conn
        .prepare(
            "SELECT inn.note FROM itemNotes inn
             JOIN items i ON inn.itemID = i.itemID
             WHERE inn.parentItemID = ?1
             AND i.itemID NOT IN (SELECT itemID FROM deletedItems)",
        )
        .unwrap();

    stmt.query_map(params![item_id], |row| row.get::<_, String>(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
}

fn read_item_collections(conn: &Connection, item_id: i64) -> Vec<String> {
    let mut stmt = conn
        .prepare(
            "SELECT c.key FROM collectionItems ci
             JOIN collections c ON ci.collectionID = c.collectionID
             WHERE ci.itemID = ?1",
        )
        .unwrap();

    stmt.query_map(params![item_id], |row| row.get::<_, String>(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
}

fn read_zotero_collections(conn: &Connection) -> Result<Vec<ZoteroCollectionData>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT c.key, c.collectionName, pc.key
             FROM collections c
             LEFT JOIN collections pc ON c.parentCollectionID = pc.collectionID",
        )
        .map_err(|e| format!("Failed to query collections: {}", e))?;

    let rows: Vec<ZoteroCollectionData> = stmt
        .query_map([], |row| {
            Ok(ZoteroCollectionData {
                key: row.get(0)?,
                name: row.get(1)?,
                parent_key: row.get(2)?,
            })
        })
        .map_err(|e| format!("{}", e))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}

fn read_zotero_annotations(conn: &Connection) -> Result<Vec<ZoteroAnnotationData>, String> {
    // Zotero 6+ stores annotations in itemAnnotations table
    let has_table: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='itemAnnotations'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;

    if !has_table {
        return Ok(Vec::new());
    }

    let mut stmt = conn
        .prepare(
            "SELECT ia.type, ia.text, ia.comment, ia.color, ia.pageLabel,
                    ia.position, ia.parentItemID
             FROM itemAnnotations ia
             JOIN items i ON ia.itemID = i.itemID
             WHERE i.itemID NOT IN (SELECT itemID FROM deletedItems)",
        )
        .map_err(|e| format!("Failed to query annotations: {}", e))?;

    let rows: Vec<ZoteroAnnotationData> = stmt
        .query_map([], |row| {
            Ok(ZoteroAnnotationData {
                annotation_type: row.get::<_, String>(0).unwrap_or_default(),
                text: row.get(1)?,
                comment: row.get(2)?,
                color: row.get(3)?,
                page_label: row.get(4)?,
                position: row.get(5)?,
                parent_item_id: row.get(6)?,
            })
        })
        .map_err(|e| format!("{}", e))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}

/// Build a mapping from Zotero attachment itemID to Zoro paper ID.
/// Annotations in Zotero are children of attachments, which are children of items.
fn build_annotation_parent_map(
    zconn: &Connection,
    zoro_conn: &Connection,
) -> Result<HashMap<i64, String>, String> {
    let mut map = HashMap::new();

    // For each attachment that has annotations, find its parent item's DOI/title,
    // then look up the corresponding Zoro paper.
    let mut stmt = zconn
        .prepare(
            "SELECT DISTINCT ia.parentItemID, att.parentItemID
             FROM itemAnnotations ia
             JOIN itemAttachments att ON ia.parentItemID = att.itemID
             JOIN items i ON att.parentItemID = i.itemID
             WHERE i.itemID NOT IN (SELECT itemID FROM deletedItems)",
        )
        .map_err(|e| format!("{}", e))?;

    let pairs: Vec<(i64, i64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| format!("{}", e))?
        .filter_map(|r| r.ok())
        .collect();

    for (att_item_id, parent_item_id) in &pairs {
        // Read parent item's identifiers
        let fields = read_item_fields(zconn, *parent_item_id);
        let doi = fields.get("DOI");
        let title = fields.get("title");
        let url = fields.get("url");
        let arxiv_id = extract_arxiv_from_fields(&fields);

        // Try to find matching Zoro paper
        if let Some(paper_id) = find_zoro_paper(
            zoro_conn,
            doi.map(|s| s.as_str()),
            arxiv_id.as_deref(),
            url.map(|s| s.as_str()),
            title.map(|s| s.as_str()),
        ) {
            map.insert(*att_item_id, paper_id);
        }
    }

    Ok(map)
}

/// Find a Zoro paper by DOI, arXiv ID, URL, or title match.
fn find_zoro_paper(
    conn: &Connection,
    doi: Option<&str>,
    arxiv_id: Option<&str>,
    url: Option<&str>,
    title: Option<&str>,
) -> Option<String> {
    // Try DOI first
    if let Some(doi) = doi {
        if let Ok(id) = conn.query_row(
            "SELECT id FROM papers WHERE doi = ?1",
            params![doi],
            |row| row.get::<_, String>(0),
        ) {
            return Some(id);
        }
    }
    // Try arXiv ID
    if let Some(arxiv) = arxiv_id {
        if let Ok(id) = conn.query_row(
            "SELECT id FROM papers WHERE arxiv_id = ?1",
            params![arxiv],
            |row| row.get::<_, String>(0),
        ) {
            return Some(id);
        }
    }
    // Try URL
    if let Some(url) = url {
        if let Ok(id) = conn.query_row(
            "SELECT id FROM papers WHERE url = ?1",
            params![url],
            |row| row.get::<_, String>(0),
        ) {
            return Some(id);
        }
    }
    // Try title (exact match, source = zotero-import)
    if let Some(title) = title {
        if let Ok(id) = conn.query_row(
            "SELECT id FROM papers WHERE title = ?1 AND source = 'zotero-import'",
            params![title],
            |row| row.get::<_, String>(0),
        ) {
            return Some(id);
        }
    }
    None
}

fn is_duplicate(
    conn: &Connection,
    doi: Option<&str>,
    arxiv_id: Option<&str>,
    url: Option<&str>,
) -> bool {
    if let Some(doi) = doi {
        if !doi.is_empty()
            && conn
                .query_row(
                    "SELECT COUNT(*) FROM papers WHERE doi = ?1",
                    params![doi],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap_or(0)
                > 0
        {
            return true;
        }
    }
    if let Some(arxiv_id) = arxiv_id {
        if !arxiv_id.is_empty()
            && conn
                .query_row(
                    "SELECT COUNT(*) FROM papers WHERE arxiv_id = ?1",
                    params![arxiv_id],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap_or(0)
                > 0
        {
            return true;
        }
    }
    if let Some(url) = url {
        if !url.is_empty()
            && conn
                .query_row(
                    "SELECT COUNT(*) FROM papers WHERE url = ?1",
                    params![url],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap_or(0)
                > 0
        {
            return true;
        }
    }
    false
}

fn extract_arxiv_from_fields(fields: &HashMap<String, String>) -> Option<String> {
    // Check URL for arxiv pattern
    if let Some(url) = fields.get("url") {
        let patterns = ["arxiv.org/abs/", "arxiv.org/pdf/"];
        for pattern in &patterns {
            if let Some(pos) = url.find(pattern) {
                let id_start = pos + pattern.len();
                if let Some(id) = url[id_start..].split(&['?', '#', '/'][..]).next() {
                    if !id.is_empty() {
                        return Some(id.to_string());
                    }
                }
            }
        }
    }
    // Check extra field
    if let Some(extra) = fields.get("extra") {
        for line in extra.lines() {
            let line = line.trim();
            if let Some(id) = line.strip_prefix("arXiv:") {
                return Some(id.trim().to_string());
            }
            if let Some(id) = line.strip_prefix("arxiv:") {
                return Some(id.trim().to_string());
            }
        }
    }
    None
}

fn format_creator_name(creator: &ZoteroCreatorData) -> String {
    if let Some(ref name) = creator.name {
        return name.clone();
    }
    match (&creator.first_name, &creator.last_name) {
        (Some(first), Some(last)) if !first.is_empty() && !last.is_empty() => {
            format!("{} {}", first, last)
        }
        (None, Some(last)) | (Some(_), Some(last)) => last.clone(),
        (Some(first), None) => first.clone(),
        (None, None) => "Unknown".to_string(),
    }
}

fn map_zotero_item_type(zotero_type: &str) -> String {
    match zotero_type {
        "journalArticle" => "article",
        "bookSection" => "incollection",
        "conferencePaper" => "inproceedings",
        "book" => "book",
        "thesis" => "thesis",
        "report" => "techreport",
        "patent" => "patent",
        "webpage" => "webpage",
        "preprint" => "article",
        _ => "article",
    }
    .to_string()
}

/// Find the first file in a Zotero storage/<KEY>/ directory.
fn find_attachment_file(dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            // Skip hidden files
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if !name.starts_with('.') {
                    return Some(path);
                }
            }
        }
    }
    None
}

fn register_attachment(
    conn: &Connection,
    paper_id: &str,
    filename: &str,
    file_type: &str,
    relative_path: &str,
    file_size: Option<i64>,
) -> Result<(), String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO attachments (id, paper_id, filename, file_type, mime_type, file_size, relative_path, created_date, modified_date, source)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'zotero-import')",
        rusqlite::params![id, paper_id, filename, file_type, Option::<String>::None, file_size, relative_path, now, now],
    )
    .map_err(|e| format!("{}", e))?;
    Ok(())
}

/// Build a position_json compatible with Zoro's annotation format from Zotero's position data.
///
/// Zotero stores positions in raw PDF coordinate space (origin at bottom-left,
/// units in PDF points). We set `usePdfCoordinates: true` so that
/// react-pdf-highlighter uses `viewport.convertToViewportRectangle` to convert
/// them into screen coordinates at render time—no manual Y-flip or page-size
/// guessing is needed.
fn build_annotation_position(position: &Option<String>, page_number: i64) -> String {
    if let Some(ref pos_str) = position {
        if let Ok(pos) = serde_json::from_str::<serde_json::Value>(pos_str) {
            // Zotero 6 position format: {"pageIndex":0,"rects":[[x1,y1,x2,y2],...]}
            let page_index = pos["pageIndex"].as_i64().unwrap_or(0);
            let actual_page = if page_number > 0 {
                page_number
            } else {
                page_index + 1
            };

            if let Some(rects) = pos["rects"].as_array() {
                if !rects.is_empty() {
                    let mut min_x1 = f64::MAX;
                    let mut min_y1 = f64::MAX;
                    let mut max_x2 = f64::MIN;
                    let mut max_y2 = f64::MIN;

                    let zoro_rects: Vec<serde_json::Value> = rects
                        .iter()
                        .filter_map(|r| {
                            let arr = r.as_array()?;
                            if arr.len() < 4 {
                                return None;
                            }
                            let x1 = arr[0].as_f64()?;
                            let y1 = arr[1].as_f64()?;
                            let x2 = arr[2].as_f64()?;
                            let y2 = arr[3].as_f64()?;
                            min_x1 = min_x1.min(x1);
                            min_y1 = min_y1.min(y1);
                            max_x2 = max_x2.max(x2);
                            max_y2 = max_y2.max(y2);
                            Some(serde_json::json!({
                                "x1": x1, "y1": y1, "x2": x2, "y2": y2,
                                "width": 1.0, "height": 1.0, "pageNumber": actual_page
                            }))
                        })
                        .collect();

                    return serde_json::json!({
                        "boundingRect": {
                            "x1": min_x1, "y1": min_y1,
                            "x2": max_x2, "y2": max_y2,
                            "width": 1.0, "height": 1.0,
                            "pageNumber": actual_page
                        },
                        "rects": zoro_rects,
                        "pageNumber": actual_page,
                        "usePdfCoordinates": true
                    })
                    .to_string();
                }
            }
        }
    }

    // Fallback: minimal position with standard PDF page size
    serde_json::json!({
        "boundingRect": {
            "x1": 0, "y1": 0, "x2": 100, "y2": 20,
            "width": 612, "height": 792,
            "pageNumber": page_number
        },
        "rects": [],
        "pageNumber": page_number
    })
    .to_string()
}

/// Strip HTML tags from Zotero note content, producing plain text / basic markdown.
fn strip_html_for_note(html: &str) -> String {
    // Remove the Zotero-specific wrapper div
    let content = html
        .replace("<div class=\"zotero-note znv1\">", "")
        .replace("</div>", "\n");

    // Basic HTML → Markdown conversions
    let content = content
        .replace("<p>", "")
        .replace("</p>", "\n\n")
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("<strong>", "**")
        .replace("</strong>", "**")
        .replace("<b>", "**")
        .replace("</b>", "**")
        .replace("<em>", "*")
        .replace("</em>", "*")
        .replace("<i>", "*")
        .replace("</i>", "*")
        .replace("<h1>", "# ")
        .replace("</h1>", "\n")
        .replace("<h2>", "## ")
        .replace("</h2>", "\n")
        .replace("<h3>", "### ")
        .replace("</h3>", "\n")
        .replace("<li>", "- ")
        .replace("</li>", "\n")
        .replace("<ul>", "")
        .replace("</ul>", "\n")
        .replace("<ol>", "")
        .replace("</ol>", "\n")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&nbsp;", " ");

    // Strip remaining HTML tags
    let re = regex::Regex::new(r"<[^>]+>").unwrap();
    let content = re.replace_all(&content, "").to_string();

    // Clean up excessive whitespace
    let lines: Vec<&str> = content.lines().collect();
    let mut cleaned = String::new();
    let mut prev_empty = false;
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !prev_empty {
                cleaned.push('\n');
                prev_empty = true;
            }
        } else {
            cleaned.push_str(trimmed);
            cleaned.push('\n');
            prev_empty = false;
        }
    }

    cleaned.trim().to_string()
}
