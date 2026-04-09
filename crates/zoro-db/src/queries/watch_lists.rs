// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::DbError;
use rusqlite::{params, Connection};
use uuid::Uuid;

// ── Row types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WatchListRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub poll_interval_minutes: i32,
    pub last_polled: Option<String>,
    pub created_date: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WatchListItemRow {
    pub id: String,
    pub list_id: String,
    pub item_type: String,
    pub external_id: String,
    pub source: String,
    pub display_name: String,
    pub config_json: Option<String>,
    pub last_checked: Option<String>,
    pub created_date: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WatchListResultRow {
    pub id: String,
    pub list_id: String,
    pub item_id: String,
    pub item_type: String,
    pub external_id: String,
    pub title: String,
    pub data_json: Option<String>,
    pub fetched_date: String,
    pub added_to_library: bool,
    pub paper_id: Option<String>,
    pub published_date: Option<String>,
}

// ── Watch List CRUD ─────────────────────────────────────────────────────────

pub fn create_watch_list(
    conn: &Connection,
    name: &str,
    description: Option<&str>,
    poll_interval_minutes: i32,
) -> Result<WatchListRow, DbError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO watch_lists (id, name, description, poll_interval_minutes, created_date)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, name, description, poll_interval_minutes, now],
    )?;
    Ok(WatchListRow {
        id,
        name: name.to_string(),
        description: description.map(String::from),
        poll_interval_minutes,
        last_polled: None,
        created_date: now,
    })
}

pub fn list_watch_lists(conn: &Connection) -> Result<Vec<WatchListRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, poll_interval_minutes, last_polled, created_date
         FROM watch_lists ORDER BY name",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(WatchListRow {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            poll_interval_minutes: row.get(3)?,
            last_polled: row.get(4)?,
            created_date: row.get(5)?,
        })
    })?;
    let mut lists = Vec::new();
    for row in rows {
        lists.push(row?);
    }
    Ok(lists)
}

pub fn get_watch_list(conn: &Connection, id: &str) -> Result<WatchListRow, DbError> {
    conn.query_row(
        "SELECT id, name, description, poll_interval_minutes, last_polled, created_date
         FROM watch_lists WHERE id = ?1",
        params![id],
        |row| {
            Ok(WatchListRow {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                poll_interval_minutes: row.get(3)?,
                last_polled: row.get(4)?,
                created_date: row.get(5)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            DbError::NotFound(format!("Watch list not found: {}", id))
        }
        _ => DbError::from(e),
    })
}

pub fn update_watch_list(
    conn: &Connection,
    id: &str,
    name: Option<&str>,
    description: Option<Option<&str>>,
    poll_interval_minutes: Option<i32>,
) -> Result<(), DbError> {
    if let Some(n) = name {
        conn.execute(
            "UPDATE watch_lists SET name = ?1 WHERE id = ?2",
            params![n, id],
        )?;
    }
    if let Some(d) = description {
        conn.execute(
            "UPDATE watch_lists SET description = ?1 WHERE id = ?2",
            params![d, id],
        )?;
    }
    if let Some(p) = poll_interval_minutes {
        conn.execute(
            "UPDATE watch_lists SET poll_interval_minutes = ?1 WHERE id = ?2",
            params![p, id],
        )?;
    }
    Ok(())
}

pub fn update_watch_list_last_polled(conn: &Connection, id: &str) -> Result<(), DbError> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE watch_lists SET last_polled = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

pub fn delete_watch_list(conn: &Connection, id: &str) -> Result<(), DbError> {
    let deleted = conn.execute("DELETE FROM watch_lists WHERE id = ?1", params![id])?;
    if deleted == 0 {
        return Err(DbError::NotFound(format!("Watch list not found: {}", id)));
    }
    Ok(())
}

// ── Watch List Items CRUD ───────────────────────────────────────────────────

pub fn add_watch_list_item(
    conn: &Connection,
    list_id: &str,
    item_type: &str,
    external_id: &str,
    source: &str,
    display_name: &str,
    config_json: Option<&str>,
) -> Result<WatchListItemRow, DbError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO watch_list_items (id, list_id, item_type, external_id, source, display_name, config_json, created_date)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![id, list_id, item_type, external_id, source, display_name, config_json, now],
    )?;
    Ok(WatchListItemRow {
        id,
        list_id: list_id.to_string(),
        item_type: item_type.to_string(),
        external_id: external_id.to_string(),
        source: source.to_string(),
        display_name: display_name.to_string(),
        config_json: config_json.map(String::from),
        last_checked: None,
        created_date: now,
    })
}

pub fn list_watch_list_items(
    conn: &Connection,
    list_id: &str,
) -> Result<Vec<WatchListItemRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, list_id, item_type, external_id, source, display_name, config_json, last_checked, created_date
         FROM watch_list_items WHERE list_id = ?1 ORDER BY created_date",
    )?;
    let rows = stmt.query_map(params![list_id], |row| {
        Ok(WatchListItemRow {
            id: row.get(0)?,
            list_id: row.get(1)?,
            item_type: row.get(2)?,
            external_id: row.get(3)?,
            source: row.get(4)?,
            display_name: row.get(5)?,
            config_json: row.get(6)?,
            last_checked: row.get(7)?,
            created_date: row.get(8)?,
        })
    })?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

pub fn delete_watch_list_item(conn: &Connection, item_id: &str) -> Result<(), DbError> {
    let deleted = conn.execute(
        "DELETE FROM watch_list_items WHERE id = ?1",
        params![item_id],
    )?;
    if deleted == 0 {
        return Err(DbError::NotFound(format!(
            "Watch list item not found: {}",
            item_id
        )));
    }
    Ok(())
}

pub fn update_watch_list_item_last_checked(
    conn: &Connection,
    item_id: &str,
) -> Result<(), DbError> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE watch_list_items SET last_checked = ?1 WHERE id = ?2",
        params![now, item_id],
    )?;
    Ok(())
}

// ── Watch List Results ──────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn insert_watch_list_result(
    conn: &Connection,
    list_id: &str,
    item_id: &str,
    item_type: &str,
    external_id: &str,
    title: &str,
    data_json: Option<&str>,
    published_date: Option<&str>,
) -> Result<WatchListResultRow, DbError> {
    // Check for duplicate (same list + external_id)
    let exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM watch_list_results WHERE list_id = ?1 AND external_id = ?2",
        params![list_id, external_id],
        |row| row.get(0),
    )?;
    if exists {
        // Update data_json if newer data is available
        conn.execute(
            "UPDATE watch_list_results
                SET data_json = COALESCE(?1, data_json),
                    title     = COALESCE(?2, title)
              WHERE list_id = ?3 AND external_id = ?4",
            params![data_json, title, list_id, external_id],
        )?;
        return Err(DbError::Duplicate(format!(
            "Watch list result already exists: {}",
            external_id
        )));
    }

    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO watch_list_results (id, list_id, item_id, item_type, external_id, title, data_json, fetched_date, added_to_library, published_date)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9)",
        params![id, list_id, item_id, item_type, external_id, title, data_json, now, published_date],
    )?;
    Ok(WatchListResultRow {
        id,
        list_id: list_id.to_string(),
        item_id: item_id.to_string(),
        item_type: item_type.to_string(),
        external_id: external_id.to_string(),
        title: title.to_string(),
        data_json: data_json.map(String::from),
        fetched_date: now,
        added_to_library: false,
        paper_id: None,
        published_date: published_date.map(String::from),
    })
}

pub fn list_watch_list_results(
    conn: &Connection,
    list_id: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<WatchListResultRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, list_id, item_id, item_type, external_id, title, data_json, fetched_date, added_to_library, paper_id, published_date
         FROM watch_list_results WHERE list_id = ?1 ORDER BY fetched_date DESC LIMIT ?2 OFFSET ?3",
    )?;
    let rows = stmt.query_map(params![list_id, limit, offset], |row| {
        Ok(WatchListResultRow {
            id: row.get(0)?,
            list_id: row.get(1)?,
            item_id: row.get(2)?,
            item_type: row.get(3)?,
            external_id: row.get(4)?,
            title: row.get(5)?,
            data_json: row.get(6)?,
            fetched_date: row.get(7)?,
            added_to_library: row.get(8)?,
            paper_id: row.get(9)?,
            published_date: row.get(10)?,
        })
    })?;
    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

/// List results across ALL watch lists (for the "all" view).
pub fn list_all_watch_list_results(
    conn: &Connection,
    limit: i64,
    offset: i64,
) -> Result<Vec<WatchListResultRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, list_id, item_id, item_type, external_id, title, data_json, fetched_date, added_to_library, paper_id, published_date
         FROM watch_list_results ORDER BY fetched_date DESC LIMIT ?1 OFFSET ?2",
    )?;
    let rows = stmt.query_map(params![limit, offset], |row| {
        Ok(WatchListResultRow {
            id: row.get(0)?,
            list_id: row.get(1)?,
            item_id: row.get(2)?,
            item_type: row.get(3)?,
            external_id: row.get(4)?,
            title: row.get(5)?,
            data_json: row.get(6)?,
            fetched_date: row.get(7)?,
            added_to_library: row.get(8)?,
            paper_id: row.get(9)?,
            published_date: row.get(10)?,
        })
    })?;
    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

pub fn get_watch_list_result(
    conn: &Connection,
    result_id: &str,
) -> Result<WatchListResultRow, DbError> {
    conn.query_row(
        "SELECT id, list_id, item_id, item_type, external_id, title, data_json, fetched_date, added_to_library, paper_id, published_date
         FROM watch_list_results WHERE id = ?1",
        params![result_id],
        |row| {
            Ok(WatchListResultRow {
                id: row.get(0)?,
                list_id: row.get(1)?,
                item_id: row.get(2)?,
                item_type: row.get(3)?,
                external_id: row.get(4)?,
                title: row.get(5)?,
                data_json: row.get(6)?,
                fetched_date: row.get(7)?,
                added_to_library: row.get(8)?,
                paper_id: row.get(9)?,
                published_date: row.get(10)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            DbError::NotFound(format!("Watch list result not found: {}", result_id))
        }
        _ => DbError::from(e),
    })
}

pub fn mark_watch_list_result_added(
    conn: &Connection,
    result_id: &str,
    paper_id: &str,
) -> Result<(), DbError> {
    conn.execute(
        "UPDATE watch_list_results SET added_to_library = 1, paper_id = ?1 WHERE id = ?2",
        params![paper_id, result_id],
    )?;
    Ok(())
}

/// Count results for a watch list: (total, not_added_to_library).
pub fn count_watch_list_results(
    conn: &Connection,
    list_id: Option<&str>,
) -> Result<(i64, i64), DbError> {
    let (total, new) = if let Some(lid) = list_id {
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM watch_list_results WHERE list_id = ?1",
            params![lid],
            |row| row.get(0),
        )?;
        let new: i64 = conn.query_row(
            "SELECT COUNT(*) FROM watch_list_results WHERE list_id = ?1 AND added_to_library = 0",
            params![lid],
            |row| row.get(0),
        )?;
        (total, new)
    } else {
        let total: i64 = conn.query_row("SELECT COUNT(*) FROM watch_list_results", [], |row| {
            row.get(0)
        })?;
        let new: i64 = conn.query_row(
            "SELECT COUNT(*) FROM watch_list_results WHERE added_to_library = 0",
            [],
            |row| row.get(0),
        )?;
        (total, new)
    };
    Ok((total, new))
}

/// Delete old results that have not been added to the library.
pub fn delete_old_watch_list_results(
    conn: &Connection,
    list_id: &str,
    max_age_days: i32,
) -> Result<usize, DbError> {
    let cutoff = chrono::Utc::now() - chrono::Duration::days(max_age_days as i64);
    let cutoff_str = cutoff.to_rfc3339();
    let deleted = conn.execute(
        "DELETE FROM watch_list_results
         WHERE list_id = ?1
           AND added_to_library = 0
           AND fetched_date < ?2",
        params![list_id, cutoff_str],
    )?;
    Ok(deleted)
}
