// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::DbError;
use rusqlite::{params, Connection};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CollectionRow {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub parent_id: Option<String>,
    pub position: i32,
    pub created_date: String,
    pub description: Option<String>,
}

pub fn create_collection(
    conn: &Connection,
    name: &str,
    parent_id: Option<&str>,
    description: Option<&str>,
) -> Result<CollectionRow, DbError> {
    let id = Uuid::new_v4().to_string();
    let base_slug = slug::slugify(name);
    let slug = unique_slug_in_parent(conn, &base_slug, parent_id)?;
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO collections (id, name, slug, parent_id, position, created_date, description) VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6)",
        params![id, name, slug, parent_id, now, description],
    )?;

    Ok(CollectionRow {
        id,
        name: name.to_string(),
        slug,
        parent_id: parent_id.map(String::from),
        position: 0,
        created_date: now,
        description: description.map(String::from),
    })
}

/// Generate a unique slug within the same parent scope.
/// If `base_slug` already exists under `parent_id`, appends `-2`, `-3`, etc.
fn unique_slug_in_parent(
    conn: &Connection,
    base_slug: &str,
    parent_id: Option<&str>,
) -> Result<String, DbError> {
    let parent_val = parent_id.unwrap_or("");
    let exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM collections WHERE slug = ?1 AND COALESCE(parent_id, '') = ?2",
        params![base_slug, parent_val],
        |row| row.get(0),
    )?;
    if !exists {
        return Ok(base_slug.to_string());
    }

    // Find the next available suffix
    for i in 2..1000 {
        let candidate = format!("{}-{}", base_slug, i);
        let taken: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM collections WHERE slug = ?1 AND COALESCE(parent_id, '') = ?2",
            params![candidate, parent_val],
            |row| row.get(0),
        )?;
        if !taken {
            return Ok(candidate);
        }
    }

    // Fallback: append a UUID fragment
    Ok(format!(
        "{}-{}",
        base_slug,
        &Uuid::new_v4().to_string()[..8]
    ))
}

/// Same as `unique_slug_in_parent`, but excludes the given collection ID from the check
/// (used when updating a collection — its own current record should not count as a conflict).
fn unique_slug_in_parent_excluding(
    conn: &Connection,
    base_slug: &str,
    parent_id: Option<&str>,
    exclude_id: &str,
) -> Result<String, DbError> {
    let parent_val = parent_id.unwrap_or("");
    let exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM collections WHERE slug = ?1 AND COALESCE(parent_id, '') = ?2 AND id != ?3",
        params![base_slug, parent_val, exclude_id],
        |row| row.get(0),
    )?;
    if !exists {
        return Ok(base_slug.to_string());
    }

    for i in 2..1000 {
        let candidate = format!("{}-{}", base_slug, i);
        let taken: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM collections WHERE slug = ?1 AND COALESCE(parent_id, '') = ?2 AND id != ?3",
            params![candidate, parent_val, exclude_id],
            |row| row.get(0),
        )?;
        if !taken {
            return Ok(candidate);
        }
    }

    Ok(format!(
        "{}-{}",
        base_slug,
        &Uuid::new_v4().to_string()[..8]
    ))
}

pub fn list_collections(conn: &Connection) -> Result<Vec<CollectionRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, slug, parent_id, position, created_date, description FROM collections ORDER BY position, name"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(CollectionRow {
            id: row.get(0)?,
            name: row.get(1)?,
            slug: row.get(2)?,
            parent_id: row.get(3)?,
            position: row.get(4)?,
            created_date: row.get(5)?,
            description: row.get(6)?,
        })
    })?;
    let mut collections = Vec::new();
    for row in rows {
        collections.push(row?);
    }
    Ok(collections)
}

pub fn delete_collection(conn: &Connection, id: &str) -> Result<(), DbError> {
    let deleted = conn.execute("DELETE FROM collections WHERE id = ?1", params![id])?;
    if deleted == 0 {
        return Err(DbError::NotFound(format!("Collection not found: {}", id)));
    }
    Ok(())
}

pub fn add_paper_to_collection(
    conn: &Connection,
    paper_id: &str,
    collection_id: &str,
) -> Result<(), DbError> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR IGNORE INTO paper_collections (paper_id, collection_id, added_date) VALUES (?1, ?2, ?3)",
        params![paper_id, collection_id, now],
    )?;
    Ok(())
}

pub fn remove_paper_from_collection(
    conn: &Connection,
    paper_id: &str,
    collection_id: &str,
) -> Result<(), DbError> {
    conn.execute(
        "DELETE FROM paper_collections WHERE paper_id = ?1 AND collection_id = ?2",
        params![paper_id, collection_id],
    )?;
    Ok(())
}

pub fn get_collection_paper_count(conn: &Connection, collection_id: &str) -> Result<i64, DbError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM paper_collections WHERE collection_id = ?1",
        params![collection_id],
        |row| row.get(0),
    )?;
    Ok(count)
}

pub fn update_collection(
    conn: &Connection,
    id: &str,
    name: Option<&str>,
    parent_id: Option<Option<&str>>,
    description: Option<Option<&str>>,
    position: Option<i32>,
) -> Result<(), DbError> {
    let mut sets: Vec<String> = Vec::new();
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    // When name or parent_id changes, we need to regenerate the slug to ensure
    // uniqueness within the new parent scope.
    let needs_slug_update = name.is_some() || parent_id.is_some();
    if needs_slug_update {
        // Determine the target parent_id (new if provided, else current)
        let target_parent: Option<String> = if let Some(ref pid) = parent_id {
            pid.map(String::from)
        } else {
            conn.query_row(
                "SELECT parent_id FROM collections WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap_or(None)
        };

        // Determine the slug base (from new name if provided, else current name)
        let base_slug = if let Some(n) = name {
            slug::slugify(n)
        } else {
            let current_slug: String = conn
                .query_row(
                    "SELECT slug FROM collections WHERE id = ?1",
                    params![id],
                    |row| row.get(0),
                )
                .unwrap_or_default();
            current_slug
        };

        let new_slug =
            unique_slug_in_parent_excluding(conn, &base_slug, target_parent.as_deref(), id)?;

        if let Some(n) = name {
            sets.push(format!("name = ?{}, slug = ?{}", idx, idx + 1));
            param_values.push(Box::new(n.to_string()));
            param_values.push(Box::new(new_slug));
            idx += 2;
        } else {
            sets.push(format!("slug = ?{}", idx));
            param_values.push(Box::new(new_slug));
            idx += 1;
        }
    }

    if let Some(pid) = parent_id {
        sets.push(format!("parent_id = ?{}", idx));
        param_values.push(Box::new(pid.map(String::from)));
        idx += 1;
    }
    if let Some(desc) = description {
        sets.push(format!("description = ?{}", idx));
        param_values.push(Box::new(desc.map(String::from)));
        idx += 1;
    }
    if let Some(pos) = position {
        sets.push(format!("position = ?{}", idx));
        param_values.push(Box::new(pos));
        idx += 1;
    }

    if sets.is_empty() {
        return Ok(());
    }

    let sql = format!(
        "UPDATE collections SET {} WHERE id = ?{}",
        sets.join(", "),
        idx
    );
    param_values.push(Box::new(id.to_string()));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|b| b.as_ref()).collect();
    let updated = conn.execute(&sql, param_refs.as_slice())?;
    if updated == 0 {
        return Err(DbError::NotFound(format!("Collection not found: {}", id)));
    }
    Ok(())
}

pub fn get_collections_for_paper(
    conn: &Connection,
    paper_id: &str,
) -> Result<Vec<CollectionRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.name, c.slug, c.parent_id, c.position, c.created_date, c.description
         FROM collections c
         JOIN paper_collections pc ON c.id = pc.collection_id
         WHERE pc.paper_id = ?1
         ORDER BY c.position, c.name",
    )?;
    let rows = stmt.query_map(params![paper_id], |row| {
        Ok(CollectionRow {
            id: row.get(0)?,
            name: row.get(1)?,
            slug: row.get(2)?,
            parent_id: row.get(3)?,
            position: row.get(4)?,
            created_date: row.get(5)?,
            description: row.get(6)?,
        })
    })?;
    let mut collections = Vec::new();
    for row in rows {
        collections.push(row?);
    }
    Ok(collections)
}

pub fn reorder_collections(
    conn: &Connection,
    id_position_pairs: &[(String, i32)],
) -> Result<(), DbError> {
    for (id, position) in id_position_pairs {
        conn.execute(
            "UPDATE collections SET position = ?1 WHERE id = ?2",
            params![position, id],
        )?;
    }
    Ok(())
}

pub fn count_uncategorized_papers(conn: &Connection) -> Result<i64, DbError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM papers p
         WHERE NOT EXISTS (SELECT 1 FROM paper_collections pc WHERE pc.paper_id = p.id)",
        [],
        |row| row.get(0),
    )?;
    Ok(count)
}
