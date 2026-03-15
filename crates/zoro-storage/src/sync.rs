// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use zoro_core::models::{AnnotationMetadata, Author, PaperMetadata, ReadStatus};
use zoro_db::queries::{annotations, collections, papers, tags};
use zoro_db::Database;

/// Rebuild metadata.json for a single paper from the current DB state.
pub fn sync_paper_metadata(db: &Database, data_dir: &Path, paper_id: &str) {
    let conn = &db.conn;
    let row = match papers::get_paper(conn, paper_id) {
        Ok(r) => r,
        Err(_) => return,
    };
    let author_list = papers::get_paper_authors(conn, paper_id).unwrap_or_default();
    let tag_list = tags::get_paper_tags(conn, paper_id).unwrap_or_default();
    let collection_list =
        collections::get_collections_for_paper(conn, paper_id).unwrap_or_default();
    let annotation_list = annotations::list_all_annotations(conn, paper_id).unwrap_or_default();

    let read_status = match row.read_status.as_str() {
        "reading" => ReadStatus::Reading,
        "read" => ReadStatus::Read,
        _ => ReadStatus::Unread,
    };

    let metadata = PaperMetadata {
        id: row.id,
        slug: row.slug.clone(),
        title: row.title,
        short_title: row.short_title,
        authors: author_list
            .into_iter()
            .map(|(name, affiliation, orcid)| Author {
                name,
                affiliation,
                orcid,
            })
            .collect(),
        abstract_text: row.abstract_text,
        doi: row.doi,
        arxiv_id: row.arxiv_id,
        url: row.url,
        pdf_url: row.pdf_url,
        html_url: row.html_url,
        thumbnail_url: row.thumbnail_url,
        published_date: row.published_date,
        added_date: row.added_date,
        source: row.source,
        tags: tag_list.iter().map(|t| t.name.clone()).collect(),
        collections: collection_list.iter().map(|c| c.name.clone()).collect(),
        attachments: Vec::new(),
        notes: Vec::new(),
        read_status,
        rating: row.rating.map(|r| r as u8),
        extra: row
            .extra_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_else(|| serde_json::json!({})),
        entry_type: row.entry_type,
        journal: row.journal,
        volume: row.volume,
        issue: row.issue,
        pages: row.pages,
        publisher: row.publisher,
        issn: row.issn,
        isbn: row.isbn,
        annotations: annotation_list
            .into_iter()
            .map(|a| AnnotationMetadata {
                id: a.id,
                annotation_type: a.annotation_type,
                color: a.color,
                comment: a.comment,
                selected_text: a.selected_text,
                position_json: a.position_json,
                page_number: a.page_number,
                source_file: a.source_file,
                created_date: a.created_date,
                modified_date: a.modified_date,
            })
            .collect(),
    };

    let paper_dir = data_dir.join("library").join(&row.dir_path);
    let _ = super::paper_dir::write_metadata(&paper_dir, &metadata);
}

/// Rebuild library-index.json from the current DB state.
/// This file gives agents a global view of the library structure.
pub fn rebuild_library_index(db: &Database, data_dir: &Path) {
    let conn = &db.conn;

    // Get all collections
    let all_collections = collections::list_collections(conn).unwrap_or_default();
    let all_tags = tags::list_tags(conn).unwrap_or_default();
    let total_papers = papers::count_papers(conn).unwrap_or(0);
    let uncategorized_count = collections::count_uncategorized_papers(conn).unwrap_or(0);

    // Build collection tree as a JSON structure
    // First, map each collection to its papers
    let mut collection_papers: HashMap<String, Vec<String>> = HashMap::new();
    for col in &all_collections {
        // Get paper slugs for this collection
        let filter = papers::PaperFilter {
            collection_id: Some(col.id.clone()),
            tag_name: None,
            read_status: None,
            search_query: None,
            uncategorized: None,
            sort_by: Some("title".to_string()),
            sort_order: Some("asc".to_string()),
            limit: Some(10000),
            offset: Some(0),
        };
        let paper_rows = papers::list_papers(conn, &filter).unwrap_or_default();
        let slugs: Vec<String> = paper_rows.iter().map(|p| p.slug.clone()).collect();
        collection_papers.insert(col.id.clone(), slugs);
    }

    // Build tag -> slug mapping
    let mut tag_papers: HashMap<String, Vec<String>> = HashMap::new();
    for tag in &all_tags {
        let filter = papers::PaperFilter {
            collection_id: None,
            tag_name: Some(tag.name.clone()),
            read_status: None,
            search_query: None,
            uncategorized: None,
            sort_by: Some("title".to_string()),
            sort_order: Some("asc".to_string()),
            limit: Some(10000),
            offset: Some(0),
        };
        let paper_rows = papers::list_papers(conn, &filter).unwrap_or_default();
        let slugs: Vec<String> = paper_rows.iter().map(|p| p.slug.clone()).collect();
        tag_papers.insert(tag.name.clone(), slugs);
    }

    // Output a flat structure with parent_id references.
    // This is easier for agents to parse and doesn't require complex tree building.
    let mut flat_collections = Vec::new();
    for col in &all_collections {
        let papers = collection_papers.get(&col.id).cloned().unwrap_or_default();
        flat_collections.push(serde_json::json!({
            "id": col.id,
            "name": col.name,
            "parent_id": col.parent_id,
            "papers": papers,
            "paper_count": papers.len(),
        }));
    }

    // Build tags JSON
    let mut tags_json = serde_json::Map::new();
    for tag in &all_tags {
        let slugs = tag_papers.get(&tag.name).cloned().unwrap_or_default();
        tags_json.insert(
            tag.name.clone(),
            serde_json::json!({
                "paper_count": slugs.len(),
                "papers": slugs,
            }),
        );
    }

    // Get uncategorized paper slugs
    let uncategorized_filter = papers::PaperFilter {
        collection_id: None,
        tag_name: None,
        read_status: None,
        search_query: None,
        uncategorized: Some(true),
        sort_by: Some("title".to_string()),
        sort_order: Some("asc".to_string()),
        limit: Some(10000),
        offset: Some(0),
    };
    let uncategorized_papers = papers::list_papers(conn, &uncategorized_filter)
        .unwrap_or_default()
        .iter()
        .map(|p| p.slug.clone())
        .collect::<Vec<_>>();

    let index = serde_json::json!({
        "version": 1,
        "total_papers": total_papers,
        "uncategorized_count": uncategorized_count,
        "uncategorized_papers": uncategorized_papers,
        "collections": flat_collections,
        "tags": tags_json,
    });

    let index_path = data_dir.join("library-index.json");
    let json = serde_json::to_string_pretty(&index).unwrap_or_default();
    let _ = fs::write(index_path, json);
}
