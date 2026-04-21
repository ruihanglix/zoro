// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::DbError;
use rusqlite::{params, Connection};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PaperLinkRow {
    pub id: String,
    pub paper_id: String,
    pub url: String,
    pub title: Option<String>,
    pub favicon: Option<String>,
    pub created_date: String,
}

pub fn insert_paper_link(
    conn: &Connection,
    paper_id: &str,
    url: &str,
    title: Option<&str>,
    favicon: Option<&str>,
) -> Result<PaperLinkRow, DbError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO paper_links (id, paper_id, url, title, favicon, created_date)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, paper_id, url, title, favicon, now],
    )?;

    Ok(PaperLinkRow {
        id,
        paper_id: paper_id.to_string(),
        url: url.to_string(),
        title: title.map(|s| s.to_string()),
        favicon: favicon.map(|s| s.to_string()),
        created_date: now,
    })
}

pub fn list_paper_links(
    conn: &Connection,
    paper_id: &str,
) -> Result<Vec<PaperLinkRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, paper_id, url, title, favicon, created_date
         FROM paper_links WHERE paper_id = ?1 ORDER BY created_date DESC",
    )?;
    let rows = stmt.query_map(params![paper_id], |row| {
        Ok(PaperLinkRow {
            id: row.get(0)?,
            paper_id: row.get(1)?,
            url: row.get(2)?,
            title: row.get(3)?,
            favicon: row.get(4)?,
            created_date: row.get(5)?,
        })
    })?;
    let mut links = Vec::new();
    for row in rows {
        links.push(row?);
    }
    Ok(links)
}

pub fn update_paper_link(
    conn: &Connection,
    id: &str,
    url: Option<&str>,
    title: Option<&str>,
    favicon: Option<&str>,
) -> Result<PaperLinkRow, DbError> {
    // Build dynamic UPDATE
    let mut sets = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    if let Some(v) = url {
        sets.push(format!("url = ?{}", idx));
        values.push(Box::new(v.to_string()));
        idx += 1;
    }
    if let Some(v) = title {
        sets.push(format!("title = ?{}", idx));
        values.push(Box::new(v.to_string()));
        idx += 1;
    }
    if let Some(v) = favicon {
        sets.push(format!("favicon = ?{}", idx));
        values.push(Box::new(v.to_string()));
        idx += 1;
    }

    if sets.is_empty() {
        // Nothing to update, just return current row
        return get_paper_link(conn, id);
    }

    let sql = format!(
        "UPDATE paper_links SET {} WHERE id = ?{}",
        sets.join(", "),
        idx
    );
    values.push(Box::new(id.to_string()));

    let params: Vec<&dyn rusqlite::types::ToSql> = values.iter().map(|v| v.as_ref()).collect();
    let updated = conn.execute(&sql, params.as_slice())?;
    if updated == 0 {
        return Err(DbError::NotFound(format!("Paper link not found: {}", id)));
    }

    get_paper_link(conn, id)
}

fn get_paper_link(conn: &Connection, id: &str) -> Result<PaperLinkRow, DbError> {
    conn.query_row(
        "SELECT id, paper_id, url, title, favicon, created_date FROM paper_links WHERE id = ?1",
        params![id],
        |row| {
            Ok(PaperLinkRow {
                id: row.get(0)?,
                paper_id: row.get(1)?,
                url: row.get(2)?,
                title: row.get(3)?,
                favicon: row.get(4)?,
                created_date: row.get(5)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            DbError::NotFound(format!("Paper link not found: {}", id))
        }
        other => DbError::Sqlite(other),
    })
}

pub fn delete_paper_link(conn: &Connection, id: &str) -> Result<(), DbError> {
    let deleted = conn.execute("DELETE FROM paper_links WHERE id = ?1", params![id])?;
    if deleted == 0 {
        return Err(DbError::NotFound(format!("Paper link not found: {}", id)));
    }
    Ok(())
}
