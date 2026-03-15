// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::DbError;
use rusqlite::{params, Connection};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReaderStateRow {
    pub paper_id: String,
    pub scroll_position: Option<f64>,
    pub scale: Option<f64>,
    pub modified_date: String,
}

pub fn get_reader_state(
    conn: &Connection,
    paper_id: &str,
) -> Result<Option<ReaderStateRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT paper_id, scroll_position, scale, modified_date
         FROM reader_state WHERE paper_id = ?1",
    )?;
    let result = stmt.query_row(params![paper_id], |row| {
        Ok(ReaderStateRow {
            paper_id: row.get(0)?,
            scroll_position: row.get(1)?,
            scale: row.get(2)?,
            modified_date: row.get(3)?,
        })
    });
    match result {
        Ok(state) => Ok(Some(state)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(DbError::Sqlite(e)),
    }
}

pub fn save_reader_state(
    conn: &Connection,
    paper_id: &str,
    scroll_position: Option<f64>,
    scale: Option<f64>,
) -> Result<ReaderStateRow, DbError> {
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO reader_state (paper_id, scroll_position, scale, modified_date)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(paper_id) DO UPDATE SET
            scroll_position = COALESCE(?2, scroll_position),
            scale = COALESCE(?3, scale),
            modified_date = ?4",
        params![paper_id, scroll_position, scale, now],
    )?;

    Ok(ReaderStateRow {
        paper_id: paper_id.to_string(),
        scroll_position,
        scale,
        modified_date: now,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::schema::create_tables(&conn).unwrap();
        conn.execute(
            "INSERT INTO papers (id, slug, title, added_date, modified_date, dir_path)
             VALUES ('p1', 'test-paper', 'Test Paper', '2026-01-01', '2026-01-01', '/tmp/test')",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_save_and_get_reader_state() {
        let conn = setup_db();

        let state = save_reader_state(&conn, "p1", Some(42.5), Some(1.5)).unwrap();
        assert_eq!(state.paper_id, "p1");
        assert_eq!(state.scroll_position, Some(42.5));
        assert_eq!(state.scale, Some(1.5));

        let loaded = get_reader_state(&conn, "p1").unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.scroll_position, Some(42.5));
        assert_eq!(loaded.scale, Some(1.5));
    }

    #[test]
    fn test_get_nonexistent_reader_state() {
        let conn = setup_db();
        let result = get_reader_state(&conn, "p1").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_upsert_reader_state() {
        let conn = setup_db();

        save_reader_state(&conn, "p1", Some(10.0), Some(1.0)).unwrap();
        save_reader_state(&conn, "p1", Some(50.0), Some(2.0)).unwrap();

        let loaded = get_reader_state(&conn, "p1").unwrap().unwrap();
        assert_eq!(loaded.scroll_position, Some(50.0));
        assert_eq!(loaded.scale, Some(2.0));
    }

    #[test]
    fn test_cascade_delete_reader_state() {
        let conn = setup_db();

        save_reader_state(&conn, "p1", Some(10.0), Some(1.0)).unwrap();
        conn.execute("DELETE FROM papers WHERE id = 'p1'", [])
            .unwrap();

        let result = get_reader_state(&conn, "p1").unwrap();
        assert!(result.is_none());
    }
}
