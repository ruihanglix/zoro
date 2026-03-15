// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::DbError;
use rusqlite::{params, Connection};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncConflictRow {
    pub id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub field: Option<String>,
    pub local_value: Option<String>,
    pub remote_value: Option<String>,
    pub remote_device_id: Option<String>,
    pub resolved: bool,
    pub created_date: String,
}

/// Insert a new sync conflict record.
pub fn insert_conflict(
    conn: &Connection,
    entity_type: &str,
    entity_id: &str,
    field: Option<&str>,
    local_value: Option<&str>,
    remote_value: Option<&str>,
    remote_device_id: Option<&str>,
) -> Result<SyncConflictRow, DbError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO sync_conflicts (id, entity_type, entity_id, field, local_value, remote_value, remote_device_id, resolved, created_date)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8)",
        params![id, entity_type, entity_id, field, local_value, remote_value, remote_device_id, now],
    )?;

    Ok(SyncConflictRow {
        id,
        entity_type: entity_type.to_string(),
        entity_id: entity_id.to_string(),
        field: field.map(String::from),
        local_value: local_value.map(String::from),
        remote_value: remote_value.map(String::from),
        remote_device_id: remote_device_id.map(String::from),
        resolved: false,
        created_date: now,
    })
}

/// List all unresolved sync conflicts.
pub fn list_unresolved_conflicts(conn: &Connection) -> Result<Vec<SyncConflictRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, entity_type, entity_id, field, local_value, remote_value,
                remote_device_id, resolved, created_date
         FROM sync_conflicts WHERE resolved = 0
         ORDER BY created_date DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(SyncConflictRow {
            id: row.get(0)?,
            entity_type: row.get(1)?,
            entity_id: row.get(2)?,
            field: row.get(3)?,
            local_value: row.get(4)?,
            remote_value: row.get(5)?,
            remote_device_id: row.get(6)?,
            resolved: row.get::<_, i32>(7)? != 0,
            created_date: row.get(8)?,
        })
    })?;
    let mut conflicts = Vec::new();
    for row in rows {
        conflicts.push(row?);
    }
    Ok(conflicts)
}

/// Resolve a sync conflict by marking it as resolved.
pub fn resolve_conflict(conn: &Connection, id: &str) -> Result<(), DbError> {
    let updated = conn.execute(
        "UPDATE sync_conflicts SET resolved = 1 WHERE id = ?1",
        params![id],
    )?;
    if updated == 0 {
        return Err(DbError::NotFound(format!("Conflict not found: {}", id)));
    }
    Ok(())
}

/// Count the number of unresolved conflicts.
pub fn count_unresolved_conflicts(conn: &Connection) -> Result<i64, DbError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sync_conflicts WHERE resolved = 0",
        [],
        |row| row.get(0),
    )?;
    Ok(count)
}
