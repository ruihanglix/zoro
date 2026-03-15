// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::DbError;
use rusqlite::{params, Connection};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TagRow {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
}

pub fn create_tag(conn: &Connection, name: &str, color: Option<&str>) -> Result<TagRow, DbError> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO tags (id, name, color) VALUES (?1, ?2, ?3)",
        params![id, name, color],
    )?;
    Ok(TagRow {
        id,
        name: name.to_string(),
        color: color.map(String::from),
    })
}

pub fn list_tags(conn: &Connection) -> Result<Vec<TagRow>, DbError> {
    let mut stmt = conn.prepare("SELECT id, name, color FROM tags ORDER BY name")?;
    let rows = stmt.query_map([], |row| {
        Ok(TagRow {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
        })
    })?;
    let mut tags = Vec::new();
    for row in rows {
        tags.push(row?);
    }
    Ok(tags)
}

pub fn delete_tag(conn: &Connection, id: &str) -> Result<(), DbError> {
    let deleted = conn.execute("DELETE FROM tags WHERE id = ?1", params![id])?;
    if deleted == 0 {
        return Err(DbError::NotFound(format!("Tag not found: {}", id)));
    }
    Ok(())
}

pub fn update_tag(
    conn: &Connection,
    id: &str,
    name: Option<&str>,
    color: Option<Option<&str>>,
) -> Result<(), DbError> {
    let mut sets: Vec<String> = Vec::new();
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    if let Some(name) = name {
        sets.push(format!("name = ?{}", idx));
        param_values.push(Box::new(name.to_string()));
        idx += 1;
    }
    if let Some(color) = color {
        sets.push(format!("color = ?{}", idx));
        param_values.push(Box::new(color.map(String::from)));
        idx += 1;
    }

    if sets.is_empty() {
        return Ok(());
    }

    let sql = format!("UPDATE tags SET {} WHERE id = ?{}", sets.join(", "), idx);
    param_values.push(Box::new(id.to_string()));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|b| b.as_ref()).collect();
    let updated = conn.execute(&sql, param_refs.as_slice())?;
    if updated == 0 {
        return Err(DbError::NotFound(format!("Tag not found: {}", id)));
    }
    Ok(())
}

pub fn add_tag_to_paper(
    conn: &Connection,
    paper_id: &str,
    tag_name: &str,
    source: &str,
) -> Result<(), DbError> {
    // Get or create tag
    let tag_id = match conn.query_row(
        "SELECT id FROM tags WHERE name = ?1",
        params![tag_name],
        |row| row.get::<_, String>(0),
    ) {
        Ok(id) => id,
        Err(_) => {
            let tag = create_tag(conn, tag_name, None)?;
            tag.id
        }
    };
    conn.execute(
        "INSERT OR IGNORE INTO paper_tags (paper_id, tag_id, source) VALUES (?1, ?2, ?3)",
        params![paper_id, tag_id, source],
    )?;
    Ok(())
}

pub fn remove_tag_from_paper(
    conn: &Connection,
    paper_id: &str,
    tag_name: &str,
) -> Result<(), DbError> {
    conn.execute(
        "DELETE FROM paper_tags WHERE paper_id = ?1 AND tag_id = (SELECT id FROM tags WHERE name = ?2)",
        params![paper_id, tag_name],
    )?;
    Ok(())
}

pub fn get_paper_tags(conn: &Connection, paper_id: &str) -> Result<Vec<TagRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, t.color FROM tags t JOIN paper_tags pt ON t.id = pt.tag_id WHERE pt.paper_id = ?1 ORDER BY t.name"
    )?;
    let rows = stmt.query_map(params![paper_id], |row| {
        Ok(TagRow {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
        })
    })?;
    let mut tags = Vec::new();
    for row in rows {
        tags.push(row?);
    }
    Ok(tags)
}

pub fn get_tag_paper_count(conn: &Connection, tag_id: &str) -> Result<i64, DbError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM paper_tags WHERE tag_id = ?1",
        params![tag_id],
        |row| row.get(0),
    )?;
    Ok(count)
}

pub fn search_tags(conn: &Connection, prefix: &str, limit: i64) -> Result<Vec<TagRow>, DbError> {
    let pattern = format!("{}%", prefix);
    let mut stmt =
        conn.prepare("SELECT id, name, color FROM tags WHERE name LIKE ?1 ORDER BY name LIMIT ?2")?;
    let rows = stmt.query_map(params![pattern, limit], |row| {
        Ok(TagRow {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
        })
    })?;
    let mut tags = Vec::new();
    for row in rows {
        tags.push(row?);
    }
    Ok(tags)
}
