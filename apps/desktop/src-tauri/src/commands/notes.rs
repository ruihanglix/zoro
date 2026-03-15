// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::AppState;
use tauri::State;
use zoro_db::queries::notes;

#[derive(Debug, serde::Serialize)]
pub struct NoteResponse {
    pub id: String,
    pub paper_id: String,
    pub content: String,
    pub created_date: String,
    pub modified_date: String,
}

fn note_row_to_response(row: &notes::NoteRow) -> NoteResponse {
    NoteResponse {
        id: row.id.clone(),
        paper_id: row.paper_id.clone(),
        content: row.content.clone(),
        created_date: row.created_date.clone(),
        modified_date: row.modified_date.clone(),
    }
}

#[tauri::command]
pub async fn add_note(
    state: State<'_, AppState>,
    paper_id: String,
    content: String,
) -> Result<NoteResponse, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let row = notes::insert_note(&db.conn, &paper_id, &content).map_err(|e| format!("{}", e))?;
    Ok(note_row_to_response(&row))
}

#[tauri::command]
pub async fn list_notes(
    state: State<'_, AppState>,
    paper_id: String,
) -> Result<Vec<NoteResponse>, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let rows = notes::list_notes(&db.conn, &paper_id).map_err(|e| format!("{}", e))?;
    Ok(rows.iter().map(note_row_to_response).collect())
}

#[tauri::command]
pub async fn update_note(
    state: State<'_, AppState>,
    id: String,
    content: String,
) -> Result<NoteResponse, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let row = notes::update_note(&db.conn, &id, &content).map_err(|e| format!("{}", e))?;
    Ok(note_row_to_response(&row))
}

#[tauri::command]
pub async fn delete_note(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    notes::delete_note(&db.conn, &id).map_err(|e| format!("{}", e))?;
    Ok(())
}
