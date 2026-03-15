// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

pub mod error;
pub mod queries;
pub mod schema;

use rusqlite::Connection;
use std::path::Path;

pub use error::DbError;

pub struct Database {
    pub conn: Connection,
    /// When set, CRUD operations will record changes to the sync_changelog table.
    /// This is the local device ID used for sync.
    pub sync_device_id: Option<String>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, DbError> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self {
            conn,
            sync_device_id: None,
        };
        db.initialize()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self, DbError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        let db = Self {
            conn,
            sync_device_id: None,
        };
        db.initialize()?;
        Ok(db)
    }

    /// Enable sync change tracking with the given device ID.
    pub fn enable_sync_tracking(&mut self, device_id: String) {
        self.sync_device_id = Some(device_id);
    }

    /// Record a change to the sync changelog if sync tracking is enabled.
    /// Silently does nothing if sync is not enabled.
    pub fn track_change(
        &self,
        entity_type: &str,
        entity_id: &str,
        operation: &str,
        field_changes: Option<&std::collections::HashMap<String, serde_json::Value>>,
        file_info: Option<&serde_json::Value>,
    ) -> Result<(), DbError> {
        if let Some(ref device_id) = self.sync_device_id {
            queries::sync::record_change(
                &self.conn,
                device_id,
                entity_type,
                entity_id,
                operation,
                field_changes,
                file_info,
            )?;
        }
        Ok(())
    }

    fn initialize(&self) -> Result<(), DbError> {
        schema::create_tables(&self.conn)?;
        Ok(())
    }
}
