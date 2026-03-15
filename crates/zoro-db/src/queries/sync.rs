// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::DbError;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use uuid::Uuid;

/// A changelog row stored in the local SQLite database.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChangelogRow {
    pub id: String,
    pub sequence: i64,
    pub device_id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub operation: String,
    pub field_changes_json: Option<String>,
    pub file_info_json: Option<String>,
    pub timestamp: String,
}

/// Local sync state row.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncStateRow {
    pub device_id: String,
    pub last_sync_time: Option<String>,
    pub last_local_sequence: i64,
    pub last_remote_sequences_json: String,
}

// ---------------------------------------------------------------------------
// Sync changelog operations
// ---------------------------------------------------------------------------

/// Insert a new changelog entry. The sequence number is auto-assigned
/// by reading the current max and incrementing.
pub fn insert_changelog(
    conn: &Connection,
    device_id: &str,
    entity_type: &str,
    entity_id: &str,
    operation: &str,
    field_changes_json: Option<&str>,
    file_info_json: Option<&str>,
) -> Result<ChangelogRow, DbError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    // Get next sequence number for this device
    let next_seq: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM sync_changelog WHERE device_id = ?1",
            params![device_id],
            |row| row.get(0),
        )
        .unwrap_or(1);

    conn.execute(
        "INSERT INTO sync_changelog (id, sequence, device_id, entity_type, entity_id, operation, field_changes_json, file_info_json, timestamp)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            id,
            next_seq,
            device_id,
            entity_type,
            entity_id,
            operation,
            field_changes_json,
            file_info_json,
            now
        ],
    )?;

    Ok(ChangelogRow {
        id,
        sequence: next_seq,
        device_id: device_id.to_string(),
        entity_type: entity_type.to_string(),
        entity_id: entity_id.to_string(),
        operation: operation.to_string(),
        field_changes_json: field_changes_json.map(String::from),
        file_info_json: file_info_json.map(String::from),
        timestamp: now,
    })
}

/// Get changelog entries for a device starting from a given sequence number
/// (exclusive), up to `limit` entries.
pub fn get_changelog_since(
    conn: &Connection,
    device_id: &str,
    after_sequence: i64,
    limit: i64,
) -> Result<Vec<ChangelogRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, sequence, device_id, entity_type, entity_id, operation,
                field_changes_json, file_info_json, timestamp
         FROM sync_changelog
         WHERE device_id = ?1 AND sequence > ?2
         ORDER BY sequence ASC
         LIMIT ?3",
    )?;

    let rows = stmt.query_map(params![device_id, after_sequence, limit], |row| {
        Ok(ChangelogRow {
            id: row.get(0)?,
            sequence: row.get(1)?,
            device_id: row.get(2)?,
            entity_type: row.get(3)?,
            entity_id: row.get(4)?,
            operation: row.get(5)?,
            field_changes_json: row.get(6)?,
            file_info_json: row.get(7)?,
            timestamp: row.get(8)?,
        })
    })?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row?);
    }
    Ok(entries)
}

/// Get the latest sequence number for a device.
pub fn get_latest_sequence(conn: &Connection, device_id: &str) -> Result<i64, DbError> {
    let seq: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sequence), 0) FROM sync_changelog WHERE device_id = ?1",
        params![device_id],
        |row| row.get(0),
    )?;
    Ok(seq)
}

/// Delete changelog entries older than the given timestamp.
pub fn cleanup_old_changelog(conn: &Connection, before_timestamp: &str) -> Result<u64, DbError> {
    let deleted = conn.execute(
        "DELETE FROM sync_changelog WHERE timestamp < ?1",
        params![before_timestamp],
    )?;
    Ok(deleted as u64)
}

/// Import a remote changelog entry (from another device) into the local table.
/// This preserves the original id, sequence, device_id, and timestamp.
pub fn import_remote_changelog(conn: &Connection, entry: &ChangelogRow) -> Result<(), DbError> {
    conn.execute(
        "INSERT OR IGNORE INTO sync_changelog (id, sequence, device_id, entity_type, entity_id, operation, field_changes_json, file_info_json, timestamp)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            entry.id,
            entry.sequence,
            entry.device_id,
            entry.entity_type,
            entry.entity_id,
            entry.operation,
            entry.field_changes_json,
            entry.file_info_json,
            entry.timestamp,
        ],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Sync state operations
// ---------------------------------------------------------------------------

/// Get or create the local sync state for this device.
pub fn get_or_create_sync_state(
    conn: &Connection,
    device_id: &str,
) -> Result<SyncStateRow, DbError> {
    match conn.query_row(
        "SELECT device_id, last_sync_time, last_local_sequence, last_remote_sequences_json
         FROM sync_state WHERE device_id = ?1",
        params![device_id],
        |row| {
            Ok(SyncStateRow {
                device_id: row.get(0)?,
                last_sync_time: row.get(1)?,
                last_local_sequence: row.get(2)?,
                last_remote_sequences_json: row.get(3)?,
            })
        },
    ) {
        Ok(state) => Ok(state),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            let empty_sequences = serde_json::to_string(&HashMap::<String, i64>::new())
                .unwrap_or_else(|_| "{}".to_string());
            conn.execute(
                "INSERT INTO sync_state (device_id, last_local_sequence, last_remote_sequences_json)
                 VALUES (?1, 0, ?2)",
                params![device_id, empty_sequences],
            )?;
            Ok(SyncStateRow {
                device_id: device_id.to_string(),
                last_sync_time: None,
                last_local_sequence: 0,
                last_remote_sequences_json: empty_sequences,
            })
        }
        Err(e) => Err(DbError::Sqlite(e)),
    }
}

/// Update the local sync state after a successful sync.
pub fn update_sync_state(
    conn: &Connection,
    device_id: &str,
    last_sync_time: &str,
    last_local_sequence: i64,
    last_remote_sequences_json: &str,
) -> Result<(), DbError> {
    conn.execute(
        "UPDATE sync_state SET last_sync_time = ?1, last_local_sequence = ?2, last_remote_sequences_json = ?3
         WHERE device_id = ?4",
        params![
            last_sync_time,
            last_local_sequence,
            last_remote_sequences_json,
            device_id
        ],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Change tracking helper — called by CRUD operations
// ---------------------------------------------------------------------------

/// Record a change in the sync changelog. This is meant to be called from
/// within existing CRUD functions when sync is enabled.
///
/// The `device_id` must be set in the connection's application data or passed
/// explicitly. We use a thread-local to avoid changing every CRUD function
/// signature.
///
/// Returns `Ok(())` silently if the sync tables don't exist yet (graceful
/// degradation for databases that haven't been migrated).
pub fn record_change(
    conn: &Connection,
    device_id: &str,
    entity_type: &str,
    entity_id: &str,
    operation: &str,
    field_changes: Option<&HashMap<String, serde_json::Value>>,
    file_info: Option<&serde_json::Value>,
) -> Result<(), DbError> {
    // Check if sync tables exist (graceful degradation)
    let table_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='sync_changelog'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if !table_exists {
        return Ok(());
    }

    let fc_json = field_changes
        .map(serde_json::to_string)
        .transpose()
        .map_err(DbError::Serde)?;

    let fi_json = file_info
        .map(serde_json::to_string)
        .transpose()
        .map_err(DbError::Serde)?;

    insert_changelog(
        conn,
        device_id,
        entity_type,
        entity_id,
        operation,
        fc_json.as_deref(),
        fi_json.as_deref(),
    )?;

    Ok(())
}
