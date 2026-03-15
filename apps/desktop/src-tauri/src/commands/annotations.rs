// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::AppState;
use std::collections::HashMap;
use tauri::State;
use zoro_db::queries::annotations;

#[derive(Debug, serde::Serialize)]
pub struct AnnotationResponse {
    pub id: String,
    pub paper_id: String,
    #[serde(rename = "type")]
    pub annotation_type: String,
    pub color: String,
    pub comment: Option<String>,
    pub selected_text: Option<String>,
    pub image_data: Option<String>,
    pub position_json: String,
    pub page_number: i64,
    pub created_date: String,
    pub modified_date: String,
}

fn row_to_response(row: &annotations::AnnotationRow) -> AnnotationResponse {
    AnnotationResponse {
        id: row.id.clone(),
        paper_id: row.paper_id.clone(),
        annotation_type: row.annotation_type.clone(),
        color: row.color.clone(),
        comment: row.comment.clone(),
        selected_text: row.selected_text.clone(),
        image_data: row.image_data.clone(),
        position_json: row.position_json.clone(),
        page_number: row.page_number,
        created_date: row.created_date.clone(),
        modified_date: row.modified_date.clone(),
    }
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn add_annotation(
    state: State<'_, AppState>,
    paper_id: String,
    annotation_type: String,
    color: String,
    comment: Option<String>,
    selected_text: Option<String>,
    image_data: Option<String>,
    position_json: String,
    page_number: i64,
    source_file: Option<String>,
) -> Result<AnnotationResponse, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let row = annotations::insert_annotation(
        &db.conn,
        &paper_id,
        &annotation_type,
        &color,
        comment.as_deref(),
        selected_text.as_deref(),
        image_data.as_deref(),
        &position_json,
        page_number,
        source_file.as_deref(),
    )
    .map_err(|e| format!("{}", e))?;

    // Track change for sync
    let mut changes = HashMap::new();
    changes.insert(
        "paper_id".to_string(),
        serde_json::json!({"new_value": row.paper_id}),
    );
    changes.insert(
        "type".to_string(),
        serde_json::json!({"new_value": row.annotation_type}),
    );
    changes.insert(
        "color".to_string(),
        serde_json::json!({"new_value": row.color}),
    );
    changes.insert(
        "position_json".to_string(),
        serde_json::json!({"new_value": row.position_json}),
    );
    changes.insert(
        "page_number".to_string(),
        serde_json::json!({"new_value": row.page_number}),
    );
    changes.insert(
        "source_file".to_string(),
        serde_json::json!({"new_value": row.source_file}),
    );
    changes.insert(
        "created_date".to_string(),
        serde_json::json!({"new_value": row.created_date}),
    );
    changes.insert(
        "modified_date".to_string(),
        serde_json::json!({"new_value": row.modified_date}),
    );
    if let Some(ref c) = row.comment {
        changes.insert("comment".to_string(), serde_json::json!({"new_value": c}));
    }
    if let Some(ref st) = row.selected_text {
        changes.insert(
            "selected_text".to_string(),
            serde_json::json!({"new_value": st}),
        );
    }
    let _ = db.track_change("annotation", &row.id, "create", Some(&changes), None);

    Ok(row_to_response(&row))
}

#[tauri::command]
pub async fn list_annotations(
    state: State<'_, AppState>,
    paper_id: String,
    source_file: Option<String>,
) -> Result<Vec<AnnotationResponse>, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let rows = annotations::list_annotations(&db.conn, &paper_id, source_file.as_deref())
        .map_err(|e| format!("{}", e))?;
    Ok(rows.iter().map(row_to_response).collect())
}

#[tauri::command]
pub async fn update_annotation(
    state: State<'_, AppState>,
    id: String,
    color: Option<String>,
    comment: Option<Option<String>>,
) -> Result<AnnotationResponse, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let comment_ref = comment.as_ref().map(|c| c.as_deref());
    let row = annotations::update_annotation(&db.conn, &id, color.as_deref(), comment_ref)
        .map_err(|e| format!("{}", e))?;

    // Track change for sync
    let mut changes = HashMap::new();
    if let Some(ref c) = color {
        changes.insert("color".to_string(), serde_json::json!({"new_value": c}));
    }
    if let Some(ref c) = comment {
        changes.insert("comment".to_string(), serde_json::json!({"new_value": c}));
    }
    changes.insert(
        "modified_date".to_string(),
        serde_json::json!({"new_value": row.modified_date}),
    );
    let _ = db.track_change("annotation", &id, "update", Some(&changes), None);

    Ok(row_to_response(&row))
}

#[tauri::command]
pub async fn delete_annotation(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    annotations::delete_annotation(&db.conn, &id).map_err(|e| format!("{}", e))?;

    // Track change for sync
    let _ = db.track_change("annotation", &id, "delete", None, None);

    Ok(())
}

#[tauri::command]
pub async fn update_annotation_type(
    state: State<'_, AppState>,
    id: String,
    annotation_type: String,
) -> Result<AnnotationResponse, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let row = annotations::update_annotation_type(&db.conn, &id, &annotation_type)
        .map_err(|e| format!("{}", e))?;

    // Track change for sync
    let mut changes = HashMap::new();
    changes.insert(
        "type".to_string(),
        serde_json::json!({"new_value": annotation_type}),
    );
    changes.insert(
        "modified_date".to_string(),
        serde_json::json!({"new_value": row.modified_date}),
    );
    let _ = db.track_change("annotation", &id, "update", Some(&changes), None);

    Ok(row_to_response(&row))
}
