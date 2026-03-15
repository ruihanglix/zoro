// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::AppState;
use std::path::PathBuf;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use zoro_core::{bibtex, ris};
use zoro_db::queries::{annotations, papers};

fn paper_to_create_input(paper: &zoro_core::models::Paper) -> papers::CreatePaperInput {
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

#[tauri::command]
pub async fn import_bibtex(state: State<'_, AppState>, content: String) -> Result<i32, String> {
    let parsed = bibtex::parse_bibtex(&content).map_err(|e| format!("{}", e))?;
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;

    let mut imported = 0;
    for paper in &parsed {
        let input = paper_to_create_input(paper);

        match papers::insert_paper(&db.conn, &input) {
            Ok(row) => {
                let authors: Vec<(String, Option<String>, Option<String>)> = paper
                    .authors
                    .iter()
                    .map(|a| (a.name.clone(), a.affiliation.clone(), a.orcid.clone()))
                    .collect();
                let _ = papers::set_paper_authors(&db.conn, &row.id, &authors);

                let papers_dir = state.data_dir.join("library/papers");
                let _ = crate::storage::paper_dir::create_paper_dir(&papers_dir, &paper.slug);

                imported += 1;
            }
            Err(e) => tracing::warn!("Failed to import paper '{}': {}", paper.title, e),
        }
    }

    Ok(imported)
}

#[tauri::command]
pub async fn export_bibtex(
    state: State<'_, AppState>,
    paper_ids: Option<Vec<String>>,
) -> Result<String, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;

    let rows = if let Some(ids) = paper_ids {
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
        papers::list_papers(&db.conn, &filter).map_err(|e| format!("{}", e))?
    };

    let core_papers: Vec<zoro_core::models::Paper> = rows
        .iter()
        .map(|row| {
            let authors = papers::get_paper_authors(&db.conn, &row.id).unwrap_or_default();
            row_to_core_paper(row, authors)
        })
        .collect();

    Ok(bibtex::generate_bibtex(&core_papers))
}

#[tauri::command]
pub async fn import_ris(state: State<'_, AppState>, content: String) -> Result<i32, String> {
    let parsed = ris::parse_ris(&content).map_err(|e| format!("{}", e))?;
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;

    let mut imported = 0;
    for paper in &parsed {
        let input = paper_to_create_input(paper);

        match papers::insert_paper(&db.conn, &input) {
            Ok(row) => {
                let authors: Vec<(String, Option<String>, Option<String>)> = paper
                    .authors
                    .iter()
                    .map(|a| (a.name.clone(), a.affiliation.clone(), a.orcid.clone()))
                    .collect();
                let _ = papers::set_paper_authors(&db.conn, &row.id, &authors);

                let papers_dir = state.data_dir.join("library/papers");
                let _ = crate::storage::paper_dir::create_paper_dir(&papers_dir, &paper.slug);

                imported += 1;
            }
            Err(e) => tracing::warn!("Failed to import paper '{}': {}", paper.title, e),
        }
    }

    Ok(imported)
}

#[tauri::command]
pub async fn export_ris(
    state: State<'_, AppState>,
    paper_ids: Option<Vec<String>>,
) -> Result<String, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;

    let rows = if let Some(ids) = paper_ids {
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
        papers::list_papers(&db.conn, &filter).map_err(|e| format!("{}", e))?
    };

    let core_papers: Vec<zoro_core::models::Paper> = rows
        .iter()
        .map(|row| {
            let authors = papers::get_paper_authors(&db.conn, &row.id).unwrap_or_default();
            row_to_core_paper(row, authors)
        })
        .collect();

    Ok(ris::generate_ris(&core_papers))
}

// ═══════════════════════════════════════════════════════════════════════════
// Export annotated PDF / HTML
// ═══════════════════════════════════════════════════════════════════════════

fn resolve_pdf_path(
    state: &AppState,
    paper_id: &str,
    source_file: Option<&str>,
) -> Option<PathBuf> {
    let db = state.db.lock().ok()?;
    let row = papers::get_paper(&db.conn, paper_id).ok()?;
    let paper_dir = state.data_dir.join("library").join(&row.dir_path);
    let filename = source_file.unwrap_or("paper.pdf");
    let path = paper_dir.join(filename);
    if path.exists() {
        return Some(path);
    }
    let atts = zoro_db::queries::attachments::get_paper_attachments(&db.conn, paper_id)
        .unwrap_or_default();
    for att in &atts {
        if att.file_type == "pdf" {
            let att_path = paper_dir.join(&att.relative_path);
            if att_path.exists() {
                return Some(att_path);
            }
        }
    }
    None
}

fn resolve_html_path(state: &AppState, paper_id: &str) -> Option<PathBuf> {
    let db = state.db.lock().ok()?;
    let row = papers::get_paper(&db.conn, paper_id).ok()?;
    let paper_dir = state.data_dir.join("library").join(&row.dir_path);
    for name in &["paper.html", "abs.html"] {
        let p = paper_dir.join(name);
        if p.exists() {
            return Some(p);
        }
    }
    let atts = zoro_db::queries::attachments::get_paper_attachments(&db.conn, paper_id)
        .unwrap_or_default();
    for att in &atts {
        if att.file_type == "html" {
            let att_path = paper_dir.join(&att.relative_path);
            if att_path.exists() {
                return Some(att_path);
            }
        }
    }
    None
}

// ─── PDF annotation injection via lopdf ────────────────────────────────────

fn hex_color_to_rgb(hex: &str) -> (f32, f32, f32) {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255) as f32 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(230) as f32 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(143) as f32 / 255.0;
    (r, g, b)
}

/// Get the MediaBox of a page (in PDF points).  Returns (width, height).
fn page_media_box(doc: &lopdf::Document, page_id: lopdf::ObjectId) -> Option<(f64, f64)> {
    let page = doc.get_object(page_id).ok()?.as_dict().ok()?;
    let media_box = page
        .get(b"MediaBox")
        .ok()
        .or_else(|| {
            let parent_ref = page.get(b"Parent").ok()?.as_reference().ok()?;
            let parent = doc.get_object(parent_ref).ok()?.as_dict().ok()?;
            parent.get(b"MediaBox").ok()
        })?
        .as_array()
        .ok()?;
    if media_box.len() < 4 {
        return None;
    }
    let w = obj_to_f64(&media_box[2])? - obj_to_f64(&media_box[0])?;
    let h = obj_to_f64(&media_box[3])? - obj_to_f64(&media_box[1])?;
    Some((w, h))
}

fn obj_to_f64(obj: &lopdf::Object) -> Option<f64> {
    match obj {
        lopdf::Object::Integer(i) => Some(*i as f64),
        lopdf::Object::Real(f) => Some(*f as f64),
        _ => None,
    }
}

/// Convert a stored coordinate (top-left origin, pixel) to PDF coordinate (bottom-left, points).
#[allow(clippy::too_many_arguments)]
fn convert_rect(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    stored_w: f64,
    stored_h: f64,
    page_w: f64,
    page_h: f64,
) -> (f64, f64, f64, f64) {
    let sx = page_w / stored_w;
    let sy = page_h / stored_h;
    let pdf_x1 = x1 * sx;
    let pdf_x2 = x2 * sx;
    // Flip Y axis: PDF origin is bottom-left
    let pdf_y1 = page_h - y2 * sy;
    let pdf_y2 = page_h - y1 * sy;
    (pdf_x1, pdf_y1, pdf_x2, pdf_y2)
}

fn build_quad_points(
    rects: &[serde_json::Value],
    stored_w: f64,
    stored_h: f64,
    page_w: f64,
    page_h: f64,
) -> Vec<lopdf::Object> {
    let mut quads = Vec::new();
    for r in rects {
        let rx1 = r["x1"].as_f64().unwrap_or(0.0);
        let ry1 = r["y1"].as_f64().unwrap_or(0.0);
        let rx2 = r["x2"].as_f64().unwrap_or(0.0);
        let ry2 = r["y2"].as_f64().unwrap_or(0.0);
        let (px1, py1, px2, py2) =
            convert_rect(rx1, ry1, rx2, ry2, stored_w, stored_h, page_w, page_h);
        // QuadPoints order: top-left, top-right, bottom-left, bottom-right
        quads.push(lopdf::Object::Real(px1 as f32));
        quads.push(lopdf::Object::Real(py2 as f32));
        quads.push(lopdf::Object::Real(px2 as f32));
        quads.push(lopdf::Object::Real(py2 as f32));
        quads.push(lopdf::Object::Real(px1 as f32));
        quads.push(lopdf::Object::Real(py1 as f32));
        quads.push(lopdf::Object::Real(px2 as f32));
        quads.push(lopdf::Object::Real(py1 as f32));
    }
    quads
}

fn inject_pdf_annotations(
    pdf_path: &std::path::Path,
    annotations_list: &[annotations::AnnotationRow],
    output_path: &std::path::Path,
) -> Result<(), String> {
    let mut doc =
        lopdf::Document::load(pdf_path).map_err(|e| format!("Failed to load PDF: {}", e))?;

    // Build a page-number → object-id map.  lopdf::Document::get_pages() returns
    // BTreeMap<u32, ObjectId> where key = 1-based page number.
    let pages = doc.get_pages();

    // Group annotations by page number
    let mut by_page: std::collections::HashMap<u32, Vec<&annotations::AnnotationRow>> =
        std::collections::HashMap::new();
    for ann in annotations_list {
        let page_num = ann.page_number.max(1) as u32;
        by_page.entry(page_num).or_default().push(ann);
    }

    for (page_num, anns) in &by_page {
        let page_id = match pages.get(page_num) {
            Some(id) => *id,
            None => continue,
        };
        let (page_w, page_h) = page_media_box(&doc, page_id).unwrap_or((612.0, 792.0));

        let mut ann_refs: Vec<lopdf::Object> = Vec::new();

        // Collect existing Annots references
        if let Ok(page_obj) = doc.get_object(page_id) {
            if let Ok(dict) = page_obj.as_dict() {
                if let Ok(existing) = dict.get(b"Annots") {
                    if let Ok(arr) = existing.as_array() {
                        ann_refs.extend(arr.iter().cloned());
                    }
                }
            }
        }

        for ann in anns {
            let pos: serde_json::Value =
                serde_json::from_str(&ann.position_json).unwrap_or_default();
            let br = &pos["boundingRect"];
            let stored_w = br["width"].as_f64().unwrap_or(page_w);
            let stored_h = br["height"].as_f64().unwrap_or(page_h);
            let bx1 = br["x1"].as_f64().unwrap_or(0.0);
            let by1 = br["y1"].as_f64().unwrap_or(0.0);
            let bx2 = br["x2"].as_f64().unwrap_or(0.0);
            let by2 = br["y2"].as_f64().unwrap_or(0.0);
            let (pdf_x1, pdf_y1, pdf_x2, pdf_y2) =
                convert_rect(bx1, by1, bx2, by2, stored_w, stored_h, page_w, page_h);

            let (cr, cg, cb) = hex_color_to_rgb(&ann.color);
            let color_arr = vec![
                lopdf::Object::Real(cr),
                lopdf::Object::Real(cg),
                lopdf::Object::Real(cb),
            ];
            let rect_arr = vec![
                lopdf::Object::Real(pdf_x1 as f32),
                lopdf::Object::Real(pdf_y1 as f32),
                lopdf::Object::Real(pdf_x2 as f32),
                lopdf::Object::Real(pdf_y2 as f32),
            ];

            let ann_dict = match ann.annotation_type.as_str() {
                "highlight" | "underline" => {
                    let subtype = if ann.annotation_type == "highlight" {
                        "Highlight"
                    } else {
                        "Underline"
                    };
                    let rects_arr = pos["rects"].as_array();
                    let quad_points = if let Some(rects) = rects_arr {
                        if rects.is_empty() {
                            build_quad_points(
                                std::slice::from_ref(br),
                                stored_w,
                                stored_h,
                                page_w,
                                page_h,
                            )
                        } else {
                            build_quad_points(rects, stored_w, stored_h, page_w, page_h)
                        }
                    } else {
                        build_quad_points(
                            std::slice::from_ref(br),
                            stored_w,
                            stored_h,
                            page_w,
                            page_h,
                        )
                    };

                    let mut dict = lopdf::Dictionary::from_iter(vec![
                        ("Type", lopdf::Object::Name(b"Annot".to_vec())),
                        ("Subtype", lopdf::Object::Name(subtype.as_bytes().to_vec())),
                        ("Rect", lopdf::Object::Array(rect_arr)),
                        ("C", lopdf::Object::Array(color_arr)),
                        ("QuadPoints", lopdf::Object::Array(quad_points)),
                        ("F", lopdf::Object::Integer(4)), // Print flag
                    ]);
                    if let Some(ref text) = ann.selected_text {
                        dict.set(
                            "Contents",
                            lopdf::Object::String(
                                text.as_bytes().to_vec(),
                                lopdf::StringFormat::Literal,
                            ),
                        );
                    }
                    if let Some(ref comment) = ann.comment {
                        dict.set(
                            "T",
                            lopdf::Object::String(b"Zoro".to_vec(), lopdf::StringFormat::Literal),
                        );
                        // Popup note for highlighted text with comment
                        if ann.selected_text.is_some() {
                            dict.set(
                                "Contents",
                                lopdf::Object::String(
                                    comment.as_bytes().to_vec(),
                                    lopdf::StringFormat::Literal,
                                ),
                            );
                        }
                    }
                    dict
                }
                "note" => {
                    let contents = ann
                        .comment
                        .as_deref()
                        .or(ann.selected_text.as_deref())
                        .unwrap_or("");
                    lopdf::Dictionary::from_iter(vec![
                        ("Type", lopdf::Object::Name(b"Annot".to_vec())),
                        ("Subtype", lopdf::Object::Name(b"Text".to_vec())),
                        ("Rect", lopdf::Object::Array(rect_arr)),
                        ("C", lopdf::Object::Array(color_arr)),
                        (
                            "Contents",
                            lopdf::Object::String(
                                contents.as_bytes().to_vec(),
                                lopdf::StringFormat::Literal,
                            ),
                        ),
                        (
                            "T",
                            lopdf::Object::String(b"Zoro".to_vec(), lopdf::StringFormat::Literal),
                        ),
                        ("Name", lopdf::Object::Name(b"Note".to_vec())),
                        ("Open", lopdf::Object::Boolean(false)),
                        ("F", lopdf::Object::Integer(4)),
                    ])
                }
                "ink" => {
                    let ink_strokes = pos["inkStrokes"].as_array();
                    if let Some(strokes) = ink_strokes {
                        let mut ink_list: Vec<lopdf::Object> = Vec::new();
                        for stroke in strokes {
                            let points = stroke["points"].as_array();
                            if let Some(pts) = points {
                                let mut path: Vec<lopdf::Object> = Vec::new();
                                for pt in pts {
                                    let px = pt["x"].as_f64().unwrap_or(0.0);
                                    let py = pt["y"].as_f64().unwrap_or(0.0);
                                    let sx = page_w / stored_w;
                                    let sy = page_h / stored_h;
                                    let pdf_x = px * sx;
                                    let pdf_y = page_h - py * sy;
                                    path.push(lopdf::Object::Real(pdf_x as f32));
                                    path.push(lopdf::Object::Real(pdf_y as f32));
                                }
                                ink_list.push(lopdf::Object::Array(path));
                            }
                        }
                        let stroke_w = pos["inkStrokes"][0]["strokeWidth"].as_f64().unwrap_or(2.0);
                        let mut dict = lopdf::Dictionary::from_iter(vec![
                            ("Type", lopdf::Object::Name(b"Annot".to_vec())),
                            ("Subtype", lopdf::Object::Name(b"Ink".to_vec())),
                            ("Rect", lopdf::Object::Array(rect_arr)),
                            ("C", lopdf::Object::Array(color_arr)),
                            ("InkList", lopdf::Object::Array(ink_list)),
                            ("F", lopdf::Object::Integer(4)),
                        ]);
                        // BS dictionary for border/stroke width
                        let bs = lopdf::Dictionary::from_iter(vec![
                            ("W", lopdf::Object::Real(stroke_w as f32)),
                            ("Type", lopdf::Object::Name(b"Border".to_vec())),
                        ]);
                        dict.set("BS", lopdf::Object::Dictionary(bs));
                        if let Some(ref comment) = ann.comment {
                            dict.set(
                                "Contents",
                                lopdf::Object::String(
                                    comment.as_bytes().to_vec(),
                                    lopdf::StringFormat::Literal,
                                ),
                            );
                        }
                        dict
                    } else {
                        continue;
                    }
                }
                "area" => {
                    let mut dict = lopdf::Dictionary::from_iter(vec![
                        ("Type", lopdf::Object::Name(b"Annot".to_vec())),
                        ("Subtype", lopdf::Object::Name(b"Square".to_vec())),
                        ("Rect", lopdf::Object::Array(rect_arr)),
                        ("C", lopdf::Object::Array(color_arr.clone())),
                        ("F", lopdf::Object::Integer(4)),
                    ]);
                    let ic = color_arr
                        .iter()
                        .map(|o| {
                            if let lopdf::Object::Real(v) = o {
                                lopdf::Object::Real(v + (1.0 - v) * 0.7)
                            } else {
                                o.clone()
                            }
                        })
                        .collect::<Vec<_>>();
                    dict.set("IC", lopdf::Object::Array(ic));
                    let bs = lopdf::Dictionary::from_iter(vec![
                        ("W", lopdf::Object::Real(1.5)),
                        ("Type", lopdf::Object::Name(b"Border".to_vec())),
                    ]);
                    dict.set("BS", lopdf::Object::Dictionary(bs));
                    if let Some(ref comment) = ann.comment {
                        dict.set(
                            "Contents",
                            lopdf::Object::String(
                                comment.as_bytes().to_vec(),
                                lopdf::StringFormat::Literal,
                            ),
                        );
                    }
                    dict
                }
                _ => continue,
            };

            let ann_obj_id = doc.add_object(lopdf::Object::Dictionary(ann_dict));
            ann_refs.push(lopdf::Object::Reference(ann_obj_id));
        }

        // Set the Annots array on the page
        if let Ok(page_obj) = doc.get_object_mut(page_id) {
            if let Ok(dict) = page_obj.as_dict_mut() {
                dict.set("Annots", lopdf::Object::Array(ann_refs));
            }
        }
    }

    doc.save(output_path)
        .map_err(|e| format!("Failed to save annotated PDF: {}", e))?;

    Ok(())
}

// ─── HTML annotation injection ─────────────────────────────────────────────

fn strip_html_tags(html: &str) -> String {
    let re = regex::Regex::new(r"<[^>]+>").unwrap();
    re.replace_all(html, "").to_string()
}

/// Find `needle` in `haystack` disambiguated by prefix/suffix context.
/// Returns (start_byte, end_byte) in `haystack`.
fn find_text_with_context(
    haystack: &str,
    needle: &str,
    prefix: &str,
    suffix: &str,
) -> Option<(usize, usize)> {
    if needle.is_empty() {
        return None;
    }
    let mut search_start = 0;
    while let Some(idx) = haystack[search_start..].find(needle) {
        let abs_idx = search_start + idx;
        let end_idx = abs_idx + needle.len();

        let mut score = 0;
        if !prefix.is_empty() {
            let pre_start = abs_idx.saturating_sub(prefix.len() + 10);
            let pre_window = &haystack[pre_start..abs_idx];
            if pre_window.contains(prefix) || prefix.contains(pre_window) {
                score += 1;
            }
        } else {
            score += 1;
        }
        if !suffix.is_empty() {
            let suf_end = (end_idx + suffix.len() + 10).min(haystack.len());
            let suf_window = &haystack[end_idx..suf_end];
            if suf_window.contains(suffix) || suffix.contains(suf_window) {
                score += 1;
            }
        } else {
            score += 1;
        }

        if score >= 1 {
            return Some((abs_idx, end_idx));
        }
        search_start = abs_idx + 1;
    }
    None
}

/// Build annotated HTML with inline styles for highlights/underlines,
/// note markers, ink SVG overlays, and a footnote section for comments.
fn inject_html_annotations(
    html_content: &str,
    annotations_list: &[annotations::AnnotationRow],
) -> String {
    let annotation_css = r#"<style>
mark.zr-export-highlight { padding: 0; margin: 0; }
mark.zr-export-underline { background: transparent; padding: 0; margin: 0; }
sup.zr-note-ref { font-size: 0.7em; vertical-align: super; margin-left: 1px;
  background: #ffd700; border-radius: 3px; padding: 0 3px; cursor: default; }
.zr-notes-section { margin-top: 2em; padding-top: 1em; border-top: 2px solid #ccc; }
.zr-notes-section h2 { font-size: 1.2em; margin-bottom: 0.5em; }
.zr-note-entry { margin: 0.6em 0; padding: 0.4em 0.6em; border-left: 3px solid #ffd700;
  background: #fffbe6; font-size: 0.9em; }
.zr-note-entry .zr-note-quote { color: #666; font-style: italic; margin-bottom: 0.3em; }
.zr-note-entry .zr-note-comment { color: #333; }
.zr-ink-overlay { position: absolute; top: 0; left: 0; width: 100%;
  pointer-events: none; z-index: 9990; overflow: visible; }
</style>"#;

    // Separate text-based annotations from ink annotations
    let mut text_annotations: Vec<&annotations::AnnotationRow> = Vec::new();
    let mut ink_annotations: Vec<&annotations::AnnotationRow> = Vec::new();
    let mut note_entries: Vec<(usize, String, String, String)> = Vec::new(); // (index, quote, comment, color)

    for ann in annotations_list {
        let pos: serde_json::Value = serde_json::from_str(&ann.position_json).unwrap_or_default();
        if pos["format"].as_str() != Some("html") {
            continue;
        }
        match ann.annotation_type.as_str() {
            "highlight" | "underline" => text_annotations.push(ann),
            "note" => text_annotations.push(ann),
            "ink" => ink_annotations.push(ann),
            _ => {}
        }
    }

    // Strip tags to get plain text for searching
    let plain_text = strip_html_tags(html_content);

    // For each text annotation, find position in plain text, then map back to HTML
    // We process annotations by collecting replacements, then apply them in reverse
    // order to avoid offset shifts.
    struct Replacement {
        plain_start: usize,
        plain_end: usize,
        ann_type: String,
        color: String,
        comment: Option<String>,
        quote: String,
    }

    let mut replacements: Vec<Replacement> = Vec::new();

    for ann in &text_annotations {
        let pos: serde_json::Value = serde_json::from_str(&ann.position_json).unwrap_or_default();
        let quote = pos["textQuote"].as_str().unwrap_or("");
        let prefix = pos["textPrefix"].as_str().unwrap_or("");
        let suffix = pos["textSuffix"].as_str().unwrap_or("");

        if quote.is_empty() {
            continue;
        }

        if let Some((start, end)) = find_text_with_context(&plain_text, quote, prefix, suffix) {
            replacements.push(Replacement {
                plain_start: start,
                plain_end: end,
                ann_type: ann.annotation_type.clone(),
                color: ann.color.clone(),
                comment: ann.comment.clone(),
                quote: quote.to_string(),
            });
        }
    }

    // Sort by start position descending so we can replace from end to start
    replacements.sort_by(|a, b| b.plain_start.cmp(&a.plain_start));

    // Build a mapping from plain text offset → HTML offset
    // Walk the HTML to build char-by-char index mapping
    let html_bytes = html_content.as_bytes();
    let mut plain_to_html: Vec<usize> = Vec::with_capacity(plain_text.len() + 1);
    let mut html_idx = 0;
    let mut in_tag = false;
    for byte in html_bytes {
        if *byte == b'<' {
            in_tag = true;
            html_idx += 1;
            continue;
        }
        if *byte == b'>' {
            in_tag = false;
            html_idx += 1;
            continue;
        }
        if in_tag {
            html_idx += 1;
            continue;
        }
        plain_to_html.push(html_idx);
        html_idx += 1;
    }
    plain_to_html.push(html_idx); // sentinel for end

    let mut result = html_content.to_string();
    let mut note_idx = 0usize;

    for rep in &replacements {
        if rep.plain_start >= plain_to_html.len() || rep.plain_end > plain_to_html.len() {
            continue;
        }
        let html_start = plain_to_html[rep.plain_start];
        let html_end = if rep.plain_end < plain_to_html.len() {
            plain_to_html[rep.plain_end]
        } else {
            result.len()
        };
        if html_start >= html_end || html_end > result.len() {
            continue;
        }

        let original = &result[html_start..html_end];

        let wrapped = match rep.ann_type.as_str() {
            "highlight" => {
                if let Some(ref comment) = rep.comment {
                    note_idx += 1;
                    note_entries.push((
                        note_idx,
                        rep.quote.clone(),
                        comment.clone(),
                        rep.color.clone(),
                    ));
                    format!(
                        "<mark class=\"zr-export-highlight\" style=\"background-color:{};\" \
                         title=\"{}\">{}<sup class=\"zr-note-ref\">{}</sup></mark>",
                        rep.color,
                        html_escape(comment),
                        original,
                        note_idx,
                    )
                } else {
                    format!(
                        "<mark class=\"zr-export-highlight\" \
                         style=\"background-color:{};\">{}</mark>",
                        rep.color, original,
                    )
                }
            }
            "underline" => {
                let extra = if let Some(ref comment) = rep.comment {
                    note_idx += 1;
                    note_entries.push((
                        note_idx,
                        rep.quote.clone(),
                        comment.clone(),
                        rep.color.clone(),
                    ));
                    format!("<sup class=\"zr-note-ref\">{}</sup>", note_idx)
                } else {
                    String::new()
                };
                format!(
                    "<mark class=\"zr-export-underline\" \
                     style=\"border-bottom:2px solid {};\">{}{}</mark>",
                    rep.color, original, extra,
                )
            }
            "note" => {
                note_idx += 1;
                let comment = rep.comment.as_deref().unwrap_or("");
                note_entries.push((
                    note_idx,
                    rep.quote.clone(),
                    comment.to_string(),
                    rep.color.clone(),
                ));
                format!(
                    "{}<sup class=\"zr-note-ref\" style=\"background:{};\" \
                     title=\"{}\">{}</sup>",
                    original,
                    rep.color,
                    html_escape(comment),
                    note_idx,
                )
            }
            _ => original.to_string(),
        };

        result.replace_range(html_start..html_end, &wrapped);
    }

    // Re-sort note entries by index for display
    note_entries.sort_by_key(|(idx, _, _, _)| *idx);

    // Build notes section
    let notes_html = if note_entries.is_empty() {
        String::new()
    } else {
        let mut s = String::from("<div class=\"zr-notes-section\"><h2>Notes &amp; Comments</h2>\n");
        for (idx, quote, comment, color) in &note_entries {
            s.push_str(&format!(
                "<div class=\"zr-note-entry\" style=\"border-left-color:{color};\">\
                 <div class=\"zr-note-quote\">[{idx}] \"{quote}\"</div>\
                 <div class=\"zr-note-comment\">{comment}</div></div>\n",
                color = color,
                idx = idx,
                quote = html_escape(quote),
                comment = html_escape(comment),
            ));
        }
        s.push_str("</div>");
        s
    };

    // Build ink SVG overlay
    let ink_svg = if ink_annotations.is_empty() {
        String::new()
    } else {
        let mut svg_paths = String::new();
        let mut max_h: f64 = 0.0;
        for ann in &ink_annotations {
            let pos: serde_json::Value =
                serde_json::from_str(&ann.position_json).unwrap_or_default();
            let content_h = pos["contentHeight"].as_f64().unwrap_or(5000.0);
            if content_h > max_h {
                max_h = content_h;
            }
            if let Some(strokes) = pos["inkStrokes"].as_array() {
                for stroke in strokes {
                    if let Some(points) = stroke["points"].as_array() {
                        let sw = stroke["strokeWidth"].as_f64().unwrap_or(2.0);
                        let mut d = String::new();
                        for (i, pt) in points.iter().enumerate() {
                            let x = pt["x"].as_f64().unwrap_or(0.0);
                            let y = pt["y"].as_f64().unwrap_or(0.0);
                            if i == 0 {
                                d.push_str(&format!("M {} {}", x, y));
                            } else {
                                d.push_str(&format!(" L {} {}", x, y));
                            }
                        }
                        svg_paths.push_str(&format!(
                            "<path d=\"{}\" fill=\"none\" stroke=\"{}\" \
                             stroke-width=\"{}\" stroke-linecap=\"round\" \
                             stroke-linejoin=\"round\"/>\n",
                            d, ann.color, sw,
                        ));
                    }
                }
            }
        }
        format!(
            "<svg class=\"zr-ink-overlay\" style=\"height:{}px;\" \
             xmlns=\"http://www.w3.org/2000/svg\">\n{}</svg>",
            max_h, svg_paths,
        )
    };

    // Inject CSS into <head>, notes + ink before </body>
    if let Some(head_end) = result.find("</head>") {
        result.insert_str(head_end, annotation_css);
    } else if let Some(body_start) = result.find("<body") {
        result.insert_str(body_start, &format!("<head>{}</head>", annotation_css));
    } else {
        result = format!("<head>{}</head>{}", annotation_css, result);
    }

    let body_content = format!("{}{}", ink_svg, notes_html);
    if !body_content.is_empty() {
        if let Some(body_end) = result.rfind("</body>") {
            result.insert_str(body_end, &body_content);
        } else {
            result.push_str(&body_content);
        }
    }

    result
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ─── Tauri commands ────────────────────────────────────────────────────────

#[tauri::command]
pub async fn export_annotated_pdf(
    state: State<'_, AppState>,
    app: AppHandle,
    paper_id: String,
    source_file: Option<String>,
) -> Result<(), String> {
    let pdf_path = resolve_pdf_path(&state, &paper_id, source_file.as_deref())
        .ok_or_else(|| "No PDF file found for this paper.".to_string())?;

    let sf = source_file.as_deref().unwrap_or("paper.pdf");
    let all_annotations = {
        let db = state
            .db
            .lock()
            .map_err(|e| format!("DB lock error: {}", e))?;
        annotations::list_annotations(&db.conn, &paper_id, Some(sf))
            .map_err(|e| format!("Failed to list annotations: {}", e))?
    };

    if all_annotations.is_empty() {
        return Err("No annotations to export for this paper.".to_string());
    }

    let default_name = pdf_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
        + "_annotated.pdf";

    let dest = app
        .dialog()
        .file()
        .add_filter("PDF", &["pdf"])
        .set_file_name(&default_name)
        .set_title("Export Annotated PDF")
        .blocking_save_file();

    let dest_path = match dest {
        Some(fp) => PathBuf::from(fp.to_string()),
        None => return Ok(()), // user cancelled
    };

    inject_pdf_annotations(&pdf_path, &all_annotations, &dest_path)?;

    tracing::info!(
        "Exported annotated PDF with {} annotations to {:?}",
        all_annotations.len(),
        dest_path
    );
    Ok(())
}

#[tauri::command]
pub async fn export_annotated_html(
    state: State<'_, AppState>,
    app: AppHandle,
    paper_id: String,
) -> Result<(), String> {
    let html_path = resolve_html_path(&state, &paper_id)
        .ok_or_else(|| "No HTML file found for this paper.".to_string())?;

    let sf = html_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let all_annotations = {
        let db = state
            .db
            .lock()
            .map_err(|e| format!("DB lock error: {}", e))?;
        annotations::list_annotations(&db.conn, &paper_id, Some(&sf))
            .map_err(|e| format!("Failed to list annotations: {}", e))?
    };

    if all_annotations.is_empty() {
        return Err("No annotations to export for this paper.".to_string());
    }

    let html_content = std::fs::read_to_string(&html_path)
        .map_err(|e| format!("Failed to read HTML file: {}", e))?;

    let annotated = inject_html_annotations(&html_content, &all_annotations);

    let default_name = html_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
        + "_annotated.html";

    let dest = app
        .dialog()
        .file()
        .add_filter("HTML", &["html", "htm"])
        .set_file_name(&default_name)
        .set_title("Export Annotated HTML")
        .blocking_save_file();

    let dest_path = match dest {
        Some(fp) => PathBuf::from(fp.to_string()),
        None => return Ok(()), // user cancelled
    };

    std::fs::write(&dest_path, annotated)
        .map_err(|e| format!("Failed to write annotated HTML: {}", e))?;

    tracing::info!(
        "Exported annotated HTML with {} annotations to {:?}",
        all_annotations.len(),
        dest_path
    );
    Ok(())
}

#[tauri::command]
pub async fn export_pdf(
    state: State<'_, AppState>,
    app: AppHandle,
    paper_id: String,
    source_file: Option<String>,
) -> Result<(), String> {
    let pdf_path = resolve_pdf_path(&state, &paper_id, source_file.as_deref())
        .ok_or_else(|| "No PDF file found for this paper.".to_string())?;

    let default_name = pdf_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let dest = app
        .dialog()
        .file()
        .add_filter("PDF", &["pdf"])
        .set_file_name(&default_name)
        .set_title("Export PDF")
        .blocking_save_file();

    let dest_path = match dest {
        Some(fp) => PathBuf::from(fp.to_string()),
        None => return Ok(()), // user cancelled
    };

    std::fs::copy(&pdf_path, &dest_path).map_err(|e| format!("Failed to copy PDF: {}", e))?;

    tracing::info!("Exported PDF to {:?}", dest_path);
    Ok(())
}

#[tauri::command]
pub async fn export_html(
    state: State<'_, AppState>,
    app: AppHandle,
    paper_id: String,
) -> Result<(), String> {
    let html_path = resolve_html_path(&state, &paper_id)
        .ok_or_else(|| "No HTML file found for this paper.".to_string())?;

    let default_name = html_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let dest = app
        .dialog()
        .file()
        .add_filter("HTML", &["html", "htm"])
        .set_file_name(&default_name)
        .set_title("Export HTML")
        .blocking_save_file();

    let dest_path = match dest {
        Some(fp) => PathBuf::from(fp.to_string()),
        None => return Ok(()), // user cancelled
    };

    std::fs::copy(&html_path, &dest_path).map_err(|e| format!("Failed to copy HTML: {}", e))?;

    tracing::info!("Exported HTML to {:?}", dest_path);
    Ok(())
}

/// Open the system file manager at the paper's directory.
#[tauri::command]
pub async fn show_paper_folder(state: State<'_, AppState>, paper_id: String) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let row = papers::get_paper(&db.conn, &paper_id).map_err(|e| format!("{}", e))?;
    let paper_dir = state.data_dir.join("library").join(&row.dir_path);

    if !paper_dir.exists() {
        return Err(format!("Paper directory not found: {:?}", paper_dir));
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&paper_dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder in Finder: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        // Canonicalize to get an absolute path with native backslashes;
        // explorer.exe fails (opens Documents) if the path contains forward slashes.
        let canonical = paper_dir
            .canonicalize()
            .unwrap_or_else(|_| paper_dir.clone());
        std::process::Command::new("explorer")
            .arg(&canonical)
            .spawn()
            .map_err(|e| format!("Failed to open folder in Explorer: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&paper_dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    Ok(())
}

/// Open the system file manager and select/highlight the given attachment file.
#[tauri::command]
pub async fn show_attachment_in_folder(
    state: State<'_, AppState>,
    paper_id: String,
    filename: String,
) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let row = papers::get_paper(&db.conn, &paper_id).map_err(|e| format!("{}", e))?;
    let paper_dir = state.data_dir.join("library").join(&row.dir_path);

    // Resolve the actual file path (check root then attachments/)
    let file_path = {
        let root = paper_dir.join(&filename);
        if root.exists() {
            root
        } else {
            let att = paper_dir.join("attachments").join(&filename);
            if att.exists() {
                att
            } else {
                return Err(format!("File not found: {}", filename));
            }
        }
    };

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(&file_path)
            .spawn()
            .map_err(|e| format!("Failed to reveal in Finder: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        // Canonicalize to get an absolute path with native backslashes.
        // explorer.exe requires `/select,<path>` as a single argument;
        // passing them as two separate args causes it to open the default
        // Documents folder instead of the actual path.
        let canonical = file_path
            .canonicalize()
            .unwrap_or_else(|_| file_path.clone());
        let select_arg = format!("/select,{}", canonical.display());
        std::process::Command::new("explorer")
            .arg(&select_arg)
            .spawn()
            .map_err(|e| format!("Failed to reveal in Explorer: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        // Try xdg-open on the parent folder; most Linux file managers
        // don't support selecting a specific file.
        if let Some(parent) = file_path.parent() {
            std::process::Command::new("xdg-open")
                .arg(parent)
                .spawn()
                .map_err(|e| format!("Failed to open folder: {}", e))?;
        }
    }

    Ok(())
}
