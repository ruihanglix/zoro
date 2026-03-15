// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::sync::Arc;

use rmcp::model::*;
use schemars;
use serde_json::json;

use crate::state::AppState;
use zoro_db::queries::notes;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AddNoteInput {
    /// Paper ID
    pub paper_id: String,
    /// Note content (Markdown supported)
    pub content: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListNotesInput {
    /// Paper ID
    pub paper_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdateNoteInput {
    /// Note ID
    pub id: String,
    /// New content
    pub content: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DeleteNoteInput {
    /// Note ID
    pub id: String,
}

fn note_to_json(row: &notes::NoteRow) -> serde_json::Value {
    json!({
        "id": row.id,
        "paper_id": row.paper_id,
        "content": row.content,
        "created_date": row.created_date,
        "modified_date": row.modified_date,
    })
}

pub fn tool_add_note(
    state: &Arc<AppState>,
    input: AddNoteInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let row = notes::insert_note(&db.conn, &input.paper_id, &input.content)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    let result = note_to_json(&row);
    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&result).unwrap_or_default(),
    )]))
}

pub fn tool_list_notes(
    state: &Arc<AppState>,
    input: ListNotesInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let rows = notes::list_notes(&db.conn, &input.paper_id)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    let results: Vec<serde_json::Value> = rows.iter().map(note_to_json).collect();

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&results).unwrap_or_default(),
    )]))
}

pub fn tool_update_note(
    state: &Arc<AppState>,
    input: UpdateNoteInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let row = notes::update_note(&db.conn, &input.id, &input.content)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    let result = note_to_json(&row);
    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&result).unwrap_or_default(),
    )]))
}

pub fn tool_delete_note(
    state: &Arc<AppState>,
    input: DeleteNoteInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    notes::delete_note(&db.conn, &input.id)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Note {} deleted",
        input.id
    ))]))
}
