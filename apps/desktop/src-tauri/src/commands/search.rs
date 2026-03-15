// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::commands::library::{paper_row_to_response, PaperResponse};
use crate::AppState;
use tauri::State;
use zoro_db::queries::{attachments, notes, papers, search, tags};

#[tauri::command]
pub async fn search_papers(
    state: State<'_, AppState>,
    query: String,
    limit: Option<i64>,
    whole_word: Option<bool>,
) -> Result<Vec<PaperResponse>, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let rows = search::search_papers_opts(
        &db.conn,
        &query,
        limit.unwrap_or(50),
        whole_word.unwrap_or(false),
    )
    .map_err(|e| format!("{}", e))?;

    let mut result = Vec::new();
    for row in &rows {
        let author_list = papers::get_paper_authors(&db.conn, &row.id).unwrap_or_default();
        let tag_list = tags::get_paper_tags(&db.conn, &row.id).unwrap_or_default();
        let attachment_list =
            attachments::get_paper_attachments(&db.conn, &row.id).unwrap_or_default();
        let note_list = notes::list_notes(&db.conn, &row.id).unwrap_or_default();

        result.push(paper_row_to_response(
            row,
            author_list,
            tag_list,
            attachment_list,
            note_list,
            &state.data_dir,
        ));
    }
    Ok(result)
}
