// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::sync::Arc;

use rmcp::model::*;
use schemars;
use serde_json::json;

use crate::state::AppState;
use zoro_db::queries::collections;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CreateCollectionInput {
    /// Collection name
    pub name: String,
    /// Parent collection ID for nesting
    pub parent_id: Option<String>,
    /// Description
    pub description: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdateCollectionInput {
    /// Collection ID
    pub id: String,
    /// New name
    pub name: Option<String>,
    /// New parent ID (null to move to root)
    pub parent_id: Option<Option<String>>,
    /// New description (null to clear)
    pub description: Option<Option<String>>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DeleteCollectionInput {
    /// Collection ID
    pub id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AddPaperToCollectionInput {
    /// Paper ID
    pub paper_id: String,
    /// Collection ID
    pub collection_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RemovePaperFromCollectionInput {
    /// Paper ID
    pub paper_id: String,
    /// Collection ID
    pub collection_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetCollectionsForPaperInput {
    /// Paper ID
    pub paper_id: String,
}

fn sync_after_mutation(state: &AppState, db: &zoro_db::Database, paper_id: Option<&str>) {
    if let Some(pid) = paper_id {
        zoro_storage::sync::sync_paper_metadata(db, &state.data_dir, pid);
    }
    zoro_storage::sync::rebuild_library_index(db, &state.data_dir);
}

fn collection_to_json(row: &collections::CollectionRow, paper_count: i64) -> serde_json::Value {
    json!({
        "id": row.id,
        "name": row.name,
        "slug": row.slug,
        "parent_id": row.parent_id,
        "paper_count": paper_count,
        "description": row.description,
    })
}

pub fn tool_create_collection(
    state: &Arc<AppState>,
    input: CreateCollectionInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let row = collections::create_collection(
        &db.conn,
        &input.name,
        input.parent_id.as_deref(),
        input.description.as_deref(),
    )
    .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    sync_after_mutation(state, &db, None);

    let result = collection_to_json(&row, 0);
    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&result).unwrap_or_default(),
    )]))
}

pub fn tool_list_collections(state: &Arc<AppState>) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let rows = collections::list_collections(&db.conn)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    let results: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            let count = collections::get_collection_paper_count(&db.conn, &row.id).unwrap_or(0);
            collection_to_json(row, count)
        })
        .collect();

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&results).unwrap_or_default(),
    )]))
}

pub fn tool_update_collection(
    state: &Arc<AppState>,
    input: UpdateCollectionInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    collections::update_collection(
        &db.conn,
        &input.id,
        input.name.as_deref(),
        input.parent_id.as_ref().map(|p| p.as_deref()),
        input.description.as_ref().map(|d| d.as_deref()),
        None,
    )
    .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    sync_after_mutation(state, &db, None);

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Collection {} updated",
        input.id
    ))]))
}

pub fn tool_delete_collection(
    state: &Arc<AppState>,
    input: DeleteCollectionInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    collections::delete_collection(&db.conn, &input.id)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    sync_after_mutation(state, &db, None);

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Collection {} deleted",
        input.id
    ))]))
}

pub fn tool_add_paper_to_collection(
    state: &Arc<AppState>,
    input: AddPaperToCollectionInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    collections::add_paper_to_collection(&db.conn, &input.paper_id, &input.collection_id)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    sync_after_mutation(state, &db, Some(&input.paper_id));

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Paper {} added to collection {}",
        input.paper_id, input.collection_id
    ))]))
}

pub fn tool_remove_paper_from_collection(
    state: &Arc<AppState>,
    input: RemovePaperFromCollectionInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    collections::remove_paper_from_collection(&db.conn, &input.paper_id, &input.collection_id)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    sync_after_mutation(state, &db, Some(&input.paper_id));

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Paper {} removed from collection {}",
        input.paper_id, input.collection_id
    ))]))
}

pub fn tool_get_collections_for_paper(
    state: &Arc<AppState>,
    input: GetCollectionsForPaperInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let rows = collections::get_collections_for_paper(&db.conn, &input.paper_id)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    let results: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            let count = collections::get_collection_paper_count(&db.conn, &row.id).unwrap_or(0);
            collection_to_json(row, count)
        })
        .collect();

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&results).unwrap_or_default(),
    )]))
}
