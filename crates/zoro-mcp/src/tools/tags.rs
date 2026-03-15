// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::sync::Arc;

use rmcp::model::*;
use schemars;
use serde_json::json;

use crate::state::AppState;
use zoro_db::queries::tags;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchTagsInput {
    /// Prefix to search for
    pub prefix: String,
    /// Maximum number of results (default 20)
    pub limit: Option<i64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AddTagToPaperInput {
    /// Paper ID
    pub paper_id: String,
    /// Tag name (will be created if it doesn't exist)
    pub tag_name: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RemoveTagFromPaperInput {
    /// Paper ID
    pub paper_id: String,
    /// Tag name
    pub tag_name: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdateTagInput {
    /// Tag ID
    pub id: String,
    /// New tag name
    pub name: Option<String>,
    /// New color (null to clear)
    pub color: Option<Option<String>>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DeleteTagInput {
    /// Tag ID
    pub id: String,
}

fn sync_after_mutation(state: &AppState, db: &zoro_db::Database, paper_id: Option<&str>) {
    if let Some(pid) = paper_id {
        zoro_storage::sync::sync_paper_metadata(db, &state.data_dir, pid);
    }
    zoro_storage::sync::rebuild_library_index(db, &state.data_dir);
}

fn tag_to_json(row: &tags::TagRow) -> serde_json::Value {
    json!({
        "id": row.id,
        "name": row.name,
        "color": row.color,
    })
}

pub fn tool_list_tags(state: &Arc<AppState>) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let rows = tags::list_tags(&db.conn)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    let results: Vec<serde_json::Value> = rows.iter().map(tag_to_json).collect();

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&results).unwrap_or_default(),
    )]))
}

pub fn tool_search_tags(
    state: &Arc<AppState>,
    input: SearchTagsInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let rows = tags::search_tags(&db.conn, &input.prefix, input.limit.unwrap_or(20))
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    let results: Vec<serde_json::Value> = rows.iter().map(tag_to_json).collect();

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&results).unwrap_or_default(),
    )]))
}

pub fn tool_add_tag_to_paper(
    state: &Arc<AppState>,
    input: AddTagToPaperInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    tags::add_tag_to_paper(&db.conn, &input.paper_id, &input.tag_name, "mcp")
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    sync_after_mutation(state, &db, Some(&input.paper_id));

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Tag '{}' added to paper {}",
        input.tag_name, input.paper_id
    ))]))
}

pub fn tool_remove_tag_from_paper(
    state: &Arc<AppState>,
    input: RemoveTagFromPaperInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    tags::remove_tag_from_paper(&db.conn, &input.paper_id, &input.tag_name)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    sync_after_mutation(state, &db, Some(&input.paper_id));

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Tag '{}' removed from paper {}",
        input.tag_name, input.paper_id
    ))]))
}

pub fn tool_update_tag(
    state: &Arc<AppState>,
    input: UpdateTagInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    tags::update_tag(
        &db.conn,
        &input.id,
        input.name.as_deref(),
        input.color.as_ref().map(|c| c.as_deref()),
    )
    .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    sync_after_mutation(state, &db, None);

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Tag {} updated",
        input.id
    ))]))
}

pub fn tool_delete_tag(
    state: &Arc<AppState>,
    input: DeleteTagInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    tags::delete_tag(&db.conn, &input.id)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    sync_after_mutation(state, &db, None);

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Tag {} deleted",
        input.id
    ))]))
}
