// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::sync::Arc;

use rmcp::model::*;
use schemars;
use serde_json::json;

use crate::state::AppState;
use zoro_db::queries::annotations;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AddAnnotationInput {
    /// Paper ID
    pub paper_id: String,
    /// Annotation type: "highlight", "underline", "strikeout", "area"
    pub annotation_type: String,
    /// CSS color string
    pub color: String,
    /// Optional comment
    pub comment: Option<String>,
    /// Selected text (for text annotations)
    pub selected_text: Option<String>,
    /// Base64-encoded image data (for area annotations)
    pub image_data: Option<String>,
    /// Position data as JSON string
    pub position_json: String,
    /// Page number (0-indexed)
    pub page_number: i64,
    /// Source filename (e.g. "paper.pdf", "paper.zh.pdf"); defaults to "paper.pdf"
    pub source_file: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListAnnotationsInput {
    /// Paper ID
    pub paper_id: String,
    /// Source filename to filter by; defaults to "paper.pdf"
    pub source_file: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdateAnnotationInput {
    /// Annotation ID
    pub id: String,
    /// New color
    pub color: Option<String>,
    /// New comment (null to clear)
    pub comment: Option<Option<String>>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DeleteAnnotationInput {
    /// Annotation ID
    pub id: String,
}

fn annotation_to_json(row: &annotations::AnnotationRow) -> serde_json::Value {
    json!({
        "id": row.id,
        "paper_id": row.paper_id,
        "type": row.annotation_type,
        "color": row.color,
        "comment": row.comment,
        "selected_text": row.selected_text,
        "position_json": row.position_json,
        "page_number": row.page_number,
        "created_date": row.created_date,
        "modified_date": row.modified_date,
    })
}

pub fn tool_add_annotation(
    state: &Arc<AppState>,
    input: AddAnnotationInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let row = annotations::insert_annotation(
        &db.conn,
        &input.paper_id,
        &input.annotation_type,
        &input.color,
        input.comment.as_deref(),
        input.selected_text.as_deref(),
        input.image_data.as_deref(),
        &input.position_json,
        input.page_number,
        input.source_file.as_deref(),
    )
    .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    let result = annotation_to_json(&row);
    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&result).unwrap_or_default(),
    )]))
}

pub fn tool_list_annotations(
    state: &Arc<AppState>,
    input: ListAnnotationsInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let rows =
        annotations::list_annotations(&db.conn, &input.paper_id, input.source_file.as_deref())
            .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    let results: Vec<serde_json::Value> = rows.iter().map(annotation_to_json).collect();

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&results).unwrap_or_default(),
    )]))
}

pub fn tool_update_annotation(
    state: &Arc<AppState>,
    input: UpdateAnnotationInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let comment_ref = input.comment.as_ref().map(|c| c.as_deref());
    let row =
        annotations::update_annotation(&db.conn, &input.id, input.color.as_deref(), comment_ref)
            .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    let result = annotation_to_json(&row);
    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&result).unwrap_or_default(),
    )]))
}

pub fn tool_delete_annotation(
    state: &Arc<AppState>,
    input: DeleteAnnotationInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    annotations::delete_annotation(&db.conn, &input.id)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Annotation {} deleted",
        input.id
    ))]))
}
