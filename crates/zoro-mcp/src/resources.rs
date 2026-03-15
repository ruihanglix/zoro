// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::fs;
use std::sync::Arc;

use rmcp::model::*;
use rmcp::ErrorData as McpError;
use serde_json::json;

use crate::state::AppState;
use crate::tools::papers::get_full_paper;
use zoro_db::queries::{collections, papers, tags};

/// List available MCP resources.
pub fn list_resources(state: &Arc<AppState>) -> Result<ListResourcesResult, McpError> {
    let mut resources = vec![RawResource::new("zoro://library-index", "Library Index")
        .with_description(
            "Overview of the library: total papers, collections, and tags with paper counts",
        )
        .with_mime_type("application/json")
        .no_annotation()];

    // Add individual paper resources
    let db = state
        .db
        .lock()
        .map_err(|e| McpError::internal_error(format!("DB lock error: {}", e), None))?;

    let filter = papers::PaperFilter {
        collection_id: None,
        tag_name: None,
        read_status: None,
        search_query: None,
        uncategorized: None,
        sort_by: Some("added_date".to_string()),
        sort_order: Some("desc".to_string()),
        limit: Some(100),
        offset: Some(0),
    };

    let rows = papers::list_papers(&db.conn, &filter).unwrap_or_default();
    for row in &rows {
        resources.push(
            RawResource::new(format!("zoro://paper/{}", row.id), &row.title)
                .with_description(format!(
                    "Paper metadata: {}",
                    row.doi
                        .as_deref()
                        .or(row.arxiv_id.as_deref())
                        .unwrap_or(&row.slug)
                ))
                .with_mime_type("application/json")
                .no_annotation(),
        );
    }

    Ok(ListResourcesResult {
        resources,
        next_cursor: None,
        meta: None,
    })
}

/// Read a specific resource by URI.
pub fn read_resource(state: &Arc<AppState>, uri: &str) -> Result<ReadResourceResult, McpError> {
    if uri == "zoro://library-index" {
        return read_library_index(state, uri);
    }

    if let Some(paper_id) = uri.strip_prefix("zoro://paper/") {
        return read_paper_resource(state, paper_id, uri);
    }

    Err(McpError::resource_not_found(
        "resource_not_found",
        Some(json!({ "uri": uri })),
    ))
}

fn read_library_index(state: &Arc<AppState>, uri: &str) -> Result<ReadResourceResult, McpError> {
    // Try to read from the pre-built file first
    let index_path = state.data_dir.join("library-index.json");
    if let Ok(content) = fs::read_to_string(&index_path) {
        return Ok(ReadResourceResult::new(vec![ResourceContents::text(
            content, uri,
        )]));
    }

    // Fallback: build it on the fly
    let db = state
        .db
        .lock()
        .map_err(|e| McpError::internal_error(format!("DB lock error: {}", e), None))?;

    let total_papers = papers::count_papers(&db.conn).unwrap_or(0);
    let all_collections = collections::list_collections(&db.conn).unwrap_or_default();
    let all_tags = tags::list_tags(&db.conn).unwrap_or_default();
    let uncategorized = collections::count_uncategorized_papers(&db.conn).unwrap_or(0);

    let cols: Vec<serde_json::Value> = all_collections
        .iter()
        .map(|c| {
            let count = collections::get_collection_paper_count(&db.conn, &c.id).unwrap_or(0);
            json!({
                "id": c.id,
                "name": c.name,
                "parent_id": c.parent_id,
                "paper_count": count,
            })
        })
        .collect();

    let tag_objs: Vec<serde_json::Value> = all_tags
        .iter()
        .map(|t| {
            json!({
                "id": t.id,
                "name": t.name,
                "color": t.color,
            })
        })
        .collect();

    let index = json!({
        "total_papers": total_papers,
        "uncategorized_count": uncategorized,
        "collections": cols,
        "tags": tag_objs,
    });

    Ok(ReadResourceResult::new(vec![ResourceContents::text(
        serde_json::to_string_pretty(&index).unwrap_or_default(),
        uri,
    )]))
}

fn read_paper_resource(
    state: &Arc<AppState>,
    paper_id: &str,
    uri: &str,
) -> Result<ReadResourceResult, McpError> {
    let db = state
        .db
        .lock()
        .map_err(|e| McpError::internal_error(format!("DB lock error: {}", e), None))?;

    let paper_json = get_full_paper(&db, paper_id)
        .map_err(|e| McpError::resource_not_found(e.to_string(), None))?;

    Ok(ReadResourceResult::new(vec![ResourceContents::text(
        serde_json::to_string_pretty(&paper_json).unwrap_or_default(),
        uri,
    )]))
}
