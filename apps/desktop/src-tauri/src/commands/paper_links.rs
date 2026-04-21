// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::AppState;
use tauri::State;
use zoro_db::queries::paper_links;

#[derive(Debug, serde::Serialize)]
pub struct PaperLinkResponse {
    pub id: String,
    pub paper_id: String,
    pub url: String,
    pub title: Option<String>,
    pub favicon: Option<String>,
    pub created_date: String,
}

fn row_to_response(row: &paper_links::PaperLinkRow) -> PaperLinkResponse {
    PaperLinkResponse {
        id: row.id.clone(),
        paper_id: row.paper_id.clone(),
        url: row.url.clone(),
        title: row.title.clone(),
        favicon: row.favicon.clone(),
        created_date: row.created_date.clone(),
    }
}

#[tauri::command]
pub async fn add_paper_link(
    state: State<'_, AppState>,
    paper_id: String,
    url: String,
    title: Option<String>,
    favicon: Option<String>,
) -> Result<PaperLinkResponse, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let row = paper_links::insert_paper_link(
        &db.conn,
        &paper_id,
        &url,
        title.as_deref(),
        favicon.as_deref(),
    )
    .map_err(|e| format!("{}", e))?;
    Ok(row_to_response(&row))
}

#[tauri::command]
pub async fn list_paper_links(
    state: State<'_, AppState>,
    paper_id: String,
) -> Result<Vec<PaperLinkResponse>, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let rows = paper_links::list_paper_links(&db.conn, &paper_id).map_err(|e| format!("{}", e))?;
    Ok(rows.iter().map(row_to_response).collect())
}

#[tauri::command]
pub async fn update_paper_link(
    state: State<'_, AppState>,
    id: String,
    url: Option<String>,
    title: Option<String>,
    favicon: Option<String>,
) -> Result<PaperLinkResponse, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let row = paper_links::update_paper_link(
        &db.conn,
        &id,
        url.as_deref(),
        title.as_deref(),
        favicon.as_deref(),
    )
    .map_err(|e| format!("{}", e))?;
    Ok(row_to_response(&row))
}

#[tauri::command]
pub async fn delete_paper_link(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    paper_links::delete_paper_link(&db.conn, &id).map_err(|e| format!("{}", e))?;
    Ok(())
}
