// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::DbError;
use rusqlite::Connection;

/// Get a value from plugin storage.
pub fn plugin_storage_get(
    conn: &Connection,
    plugin_id: &str,
    key: &str,
) -> Result<Option<String>, DbError> {
    let mut stmt =
        conn.prepare("SELECT value FROM plugin_storage WHERE plugin_id = ?1 AND key = ?2")?;
    let result = stmt
        .query_row(rusqlite::params![plugin_id, key], |row| row.get(0))
        .ok();
    Ok(result)
}

/// Set a value in plugin storage.
pub fn plugin_storage_set(
    conn: &Connection,
    plugin_id: &str,
    key: &str,
    value: &str,
) -> Result<(), DbError> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO plugin_storage (plugin_id, key, value, updated_at)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![plugin_id, key, value, now],
    )?;
    Ok(())
}

/// Delete a value from plugin storage.
pub fn plugin_storage_delete(conn: &Connection, plugin_id: &str, key: &str) -> Result<(), DbError> {
    conn.execute(
        "DELETE FROM plugin_storage WHERE plugin_id = ?1 AND key = ?2",
        rusqlite::params![plugin_id, key],
    )?;
    Ok(())
}

/// Delete all storage for a plugin (used when uninstalling).
pub fn plugin_storage_clear(conn: &Connection, plugin_id: &str) -> Result<(), DbError> {
    conn.execute(
        "DELETE FROM plugin_storage WHERE plugin_id = ?1",
        rusqlite::params![plugin_id],
    )?;
    Ok(())
}
