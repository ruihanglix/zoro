// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::AppState;
use std::collections::HashMap;
use tauri::State;
use zoro_db::queries::reader_state;

#[derive(Debug, serde::Serialize)]
pub struct ReaderStateResponse {
    pub paper_id: String,
    pub scroll_position: Option<f64>,
    pub scale: Option<f64>,
    pub modified_date: String,
}

fn row_to_response(row: &reader_state::ReaderStateRow) -> ReaderStateResponse {
    ReaderStateResponse {
        paper_id: row.paper_id.clone(),
        scroll_position: row.scroll_position,
        scale: row.scale,
        modified_date: row.modified_date.clone(),
    }
}

#[tauri::command]
pub async fn get_reader_state(
    state: State<'_, AppState>,
    paper_id: String,
) -> Result<Option<ReaderStateResponse>, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let result =
        reader_state::get_reader_state(&db.conn, &paper_id).map_err(|e| format!("{}", e))?;
    Ok(result.as_ref().map(row_to_response))
}

#[tauri::command]
pub async fn save_reader_state(
    state: State<'_, AppState>,
    paper_id: String,
    scroll_position: Option<f64>,
    scale: Option<f64>,
) -> Result<ReaderStateResponse, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let row = reader_state::save_reader_state(&db.conn, &paper_id, scroll_position, scale)
        .map_err(|e| format!("{}", e))?;

    // Track change for sync
    {
        let mut changes = HashMap::new();
        if let Some(sp) = scroll_position {
            changes.insert(
                "scroll_position".to_string(),
                serde_json::json!({"new_value": sp}),
            );
        }
        if let Some(sc) = scale {
            changes.insert("scale".to_string(), serde_json::json!({"new_value": sc}));
        }
        changes.insert(
            "modified_date".to_string(),
            serde_json::json!({"new_value": row.modified_date}),
        );
        let _ = db.track_change("reader_state", &paper_id, "update", Some(&changes), None);
    }

    Ok(row_to_response(&row))
}
