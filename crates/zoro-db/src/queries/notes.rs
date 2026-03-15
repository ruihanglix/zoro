// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::DbError;
use rusqlite::{params, Connection};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NoteRow {
    pub id: String,
    pub paper_id: String,
    pub content: String,
    pub created_date: String,
    pub modified_date: String,
}

pub fn insert_note(conn: &Connection, paper_id: &str, content: &str) -> Result<NoteRow, DbError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO notes (id, paper_id, content, created_date, modified_date)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, paper_id, content, now, now],
    )?;

    Ok(NoteRow {
        id,
        paper_id: paper_id.to_string(),
        content: content.to_string(),
        created_date: now.clone(),
        modified_date: now,
    })
}

pub fn get_note(conn: &Connection, id: &str) -> Result<NoteRow, DbError> {
    conn.query_row(
        "SELECT id, paper_id, content, created_date, modified_date FROM notes WHERE id = ?1",
        params![id],
        |row| {
            Ok(NoteRow {
                id: row.get(0)?,
                paper_id: row.get(1)?,
                content: row.get(2)?,
                created_date: row.get(3)?,
                modified_date: row.get(4)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            DbError::NotFound(format!("Note not found: {}", id))
        }
        other => DbError::Sqlite(other),
    })
}

pub fn list_notes(conn: &Connection, paper_id: &str) -> Result<Vec<NoteRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, paper_id, content, created_date, modified_date
         FROM notes WHERE paper_id = ?1 ORDER BY created_date DESC",
    )?;
    let rows = stmt.query_map(params![paper_id], |row| {
        Ok(NoteRow {
            id: row.get(0)?,
            paper_id: row.get(1)?,
            content: row.get(2)?,
            created_date: row.get(3)?,
            modified_date: row.get(4)?,
        })
    })?;
    let mut notes = Vec::new();
    for row in rows {
        notes.push(row?);
    }
    Ok(notes)
}

pub fn update_note(conn: &Connection, id: &str, content: &str) -> Result<NoteRow, DbError> {
    let now = chrono::Utc::now().to_rfc3339();
    let updated = conn.execute(
        "UPDATE notes SET content = ?1, modified_date = ?2 WHERE id = ?3",
        params![content, now, id],
    )?;
    if updated == 0 {
        return Err(DbError::NotFound(format!("Note not found: {}", id)));
    }
    conn.query_row(
        "SELECT id, paper_id, content, created_date, modified_date FROM notes WHERE id = ?1",
        params![id],
        |row| {
            Ok(NoteRow {
                id: row.get(0)?,
                paper_id: row.get(1)?,
                content: row.get(2)?,
                created_date: row.get(3)?,
                modified_date: row.get(4)?,
            })
        },
    )
    .map_err(DbError::Sqlite)
}

pub fn delete_note(conn: &Connection, id: &str) -> Result<(), DbError> {
    let deleted = conn.execute("DELETE FROM notes WHERE id = ?1", params![id])?;
    if deleted == 0 {
        return Err(DbError::NotFound(format!("Note not found: {}", id)));
    }
    Ok(())
}
