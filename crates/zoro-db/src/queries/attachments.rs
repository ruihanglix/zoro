// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::DbError;
use rusqlite::{params, Connection};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AttachmentRow {
    pub id: String,
    pub paper_id: String,
    pub filename: String,
    pub file_type: String,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
    pub relative_path: String,
    pub created_date: String,
    pub modified_date: String,
    pub source: String,
    pub metadata_json: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub fn insert_attachment(
    conn: &Connection,
    paper_id: &str,
    filename: &str,
    file_type: &str,
    mime_type: Option<&str>,
    file_size: Option<i64>,
    relative_path: &str,
    source: &str,
) -> Result<AttachmentRow, DbError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO attachments (id, paper_id, filename, file_type, mime_type, file_size, relative_path, created_date, modified_date, source)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![id, paper_id, filename, file_type, mime_type, file_size, relative_path, now, now, source],
    )?;
    Ok(AttachmentRow {
        id,
        paper_id: paper_id.to_string(),
        filename: filename.to_string(),
        file_type: file_type.to_string(),
        mime_type: mime_type.map(String::from),
        file_size,
        relative_path: relative_path.to_string(),
        created_date: now.clone(),
        modified_date: now,
        source: source.to_string(),
        metadata_json: None,
    })
}

pub fn get_paper_attachments(
    conn: &Connection,
    paper_id: &str,
) -> Result<Vec<AttachmentRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, paper_id, filename, file_type, mime_type, file_size, relative_path, created_date, modified_date, source, metadata_json
         FROM attachments WHERE paper_id = ?1 ORDER BY created_date"
    )?;
    let rows = stmt.query_map(params![paper_id], |row| {
        Ok(AttachmentRow {
            id: row.get(0)?,
            paper_id: row.get(1)?,
            filename: row.get(2)?,
            file_type: row.get(3)?,
            mime_type: row.get(4)?,
            file_size: row.get(5)?,
            relative_path: row.get(6)?,
            created_date: row.get(7)?,
            modified_date: row.get(8)?,
            source: row.get(9)?,
            metadata_json: row.get(10)?,
        })
    })?;
    let mut attachments = Vec::new();
    for row in rows {
        attachments.push(row?);
    }
    Ok(attachments)
}

pub fn delete_attachment(conn: &Connection, id: &str) -> Result<(), DbError> {
    let deleted = conn.execute("DELETE FROM attachments WHERE id = ?1", params![id])?;
    if deleted == 0 {
        return Err(DbError::NotFound(format!("Attachment not found: {}", id)));
    }
    Ok(())
}
