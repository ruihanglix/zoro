// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::DbError;
use rusqlite::{params, Connection};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SubscriptionRow {
    pub id: String,
    pub source_type: String,
    pub name: String,
    pub config_json: Option<String>,
    pub enabled: bool,
    pub poll_interval_minutes: i32,
    pub last_polled: Option<String>,
    pub created_date: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SubscriptionItemRow {
    pub id: String,
    pub subscription_id: String,
    pub paper_id: Option<String>,
    pub external_id: String,
    pub title: String,
    pub data_json: Option<String>,
    pub fetched_date: String,
    pub added_to_library: bool,
    pub source_date: Option<String>,
}

pub fn create_subscription(
    conn: &Connection,
    source_type: &str,
    name: &str,
    config_json: Option<&str>,
    poll_interval: i32,
) -> Result<SubscriptionRow, DbError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO subscriptions (id, source_type, name, config_json, enabled, poll_interval_minutes, created_date)
         VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6)",
        params![id, source_type, name, config_json, poll_interval, now],
    )?;
    Ok(SubscriptionRow {
        id,
        source_type: source_type.to_string(),
        name: name.to_string(),
        config_json: config_json.map(String::from),
        enabled: true,
        poll_interval_minutes: poll_interval,
        last_polled: None,
        created_date: now,
    })
}

pub fn list_subscriptions(conn: &Connection) -> Result<Vec<SubscriptionRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, source_type, name, config_json, enabled, poll_interval_minutes, last_polled, created_date FROM subscriptions ORDER BY name"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(SubscriptionRow {
            id: row.get(0)?,
            source_type: row.get(1)?,
            name: row.get(2)?,
            config_json: row.get(3)?,
            enabled: row.get(4)?,
            poll_interval_minutes: row.get(5)?,
            last_polled: row.get(6)?,
            created_date: row.get(7)?,
        })
    })?;
    let mut subs = Vec::new();
    for row in rows {
        subs.push(row?);
    }
    Ok(subs)
}

pub fn update_last_polled(conn: &Connection, id: &str) -> Result<(), DbError> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE subscriptions SET last_polled = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

pub fn toggle_subscription(conn: &Connection, id: &str, enabled: bool) -> Result<(), DbError> {
    conn.execute(
        "UPDATE subscriptions SET enabled = ?1 WHERE id = ?2",
        params![enabled, id],
    )?;
    Ok(())
}

pub fn insert_subscription_item(
    conn: &Connection,
    subscription_id: &str,
    external_id: &str,
    title: &str,
    data_json: Option<&str>,
    source_date: Option<&str>,
) -> Result<SubscriptionItemRow, DbError> {
    // Check if item already exists
    let exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM subscription_items WHERE subscription_id = ?1 AND external_id = ?2",
        params![subscription_id, external_id],
        |row| row.get(0),
    )?;
    if exists {
        // Update existing row: refresh data_json (upvotes, comments, etc.
        // change over time) and backfill source_date / title.
        conn.execute(
            "UPDATE subscription_items
                SET data_json    = COALESCE(?1, data_json),
                    title        = COALESCE(?2, title),
                    source_date  = COALESCE(?3, source_date)
              WHERE subscription_id = ?4 AND external_id = ?5",
            params![data_json, title, source_date, subscription_id, external_id],
        )?;
        return Err(DbError::Duplicate(format!(
            "Subscription item already exists: {}",
            external_id
        )));
    }

    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO subscription_items (id, subscription_id, external_id, title, data_json, fetched_date, added_to_library, source_date)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
        params![id, subscription_id, external_id, title, data_json, now, source_date],
    )?;
    Ok(SubscriptionItemRow {
        id,
        subscription_id: subscription_id.to_string(),
        paper_id: None,
        external_id: external_id.to_string(),
        title: title.to_string(),
        data_json: data_json.map(String::from),
        fetched_date: now,
        added_to_library: false,
        source_date: source_date.map(String::from),
    })
}

pub fn list_subscription_items(
    conn: &Connection,
    subscription_id: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<SubscriptionItemRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, subscription_id, paper_id, external_id, title, data_json, fetched_date, added_to_library, source_date
         FROM subscription_items WHERE subscription_id = ?1 ORDER BY fetched_date DESC LIMIT ?2 OFFSET ?3"
    )?;
    let rows = stmt.query_map(params![subscription_id, limit, offset], |row| {
        Ok(SubscriptionItemRow {
            id: row.get(0)?,
            subscription_id: row.get(1)?,
            paper_id: row.get(2)?,
            external_id: row.get(3)?,
            title: row.get(4)?,
            data_json: row.get(5)?,
            fetched_date: row.get(6)?,
            added_to_library: row.get(7)?,
            source_date: row.get(8)?,
        })
    })?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

/// List subscription items filtered by source_date (YYYY-MM-DD).
pub fn list_subscription_items_by_source_date(
    conn: &Connection,
    subscription_id: &str,
    source_date: &str,
) -> Result<Vec<SubscriptionItemRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, subscription_id, paper_id, external_id, title, data_json, fetched_date, added_to_library, source_date
         FROM subscription_items
         WHERE subscription_id = ?1 AND source_date = ?2
         ORDER BY fetched_date DESC"
    )?;
    let rows = stmt.query_map(params![subscription_id, source_date], |row| {
        Ok(SubscriptionItemRow {
            id: row.get(0)?,
            subscription_id: row.get(1)?,
            paper_id: row.get(2)?,
            external_id: row.get(3)?,
            title: row.get(4)?,
            data_json: row.get(5)?,
            fetched_date: row.get(6)?,
            added_to_library: row.get(7)?,
            source_date: row.get(8)?,
        })
    })?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

pub fn mark_item_added_to_library(
    conn: &Connection,
    item_id: &str,
    paper_id: &str,
) -> Result<(), DbError> {
    conn.execute(
        "UPDATE subscription_items SET added_to_library = 1, paper_id = ?1 WHERE id = ?2",
        params![paper_id, item_id],
    )?;
    Ok(())
}

pub fn get_subscription_item(
    conn: &Connection,
    item_id: &str,
) -> Result<SubscriptionItemRow, DbError> {
    conn.query_row(
        "SELECT id, subscription_id, paper_id, external_id, title, data_json, fetched_date, added_to_library, source_date
         FROM subscription_items WHERE id = ?1",
        params![item_id],
        |row| {
            Ok(SubscriptionItemRow {
                id: row.get(0)?,
                subscription_id: row.get(1)?,
                paper_id: row.get(2)?,
                external_id: row.get(3)?,
                title: row.get(4)?,
                data_json: row.get(5)?,
                fetched_date: row.get(6)?,
                added_to_library: row.get(7)?,
                source_date: row.get(8)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            DbError::NotFound(format!("Subscription item not found: {}", item_id))
        }
        _ => DbError::from(e),
    })
}

pub fn delete_subscription(conn: &Connection, id: &str) -> Result<(), DbError> {
    let deleted = conn.execute("DELETE FROM subscriptions WHERE id = ?1", params![id])?;
    if deleted == 0 {
        return Err(DbError::NotFound(format!("Subscription not found: {}", id)));
    }
    Ok(())
}

/// Delete old subscription items that have not been added to the library.
/// Items older than `max_age_days` (based on `fetched_date`) are removed.
/// Returns the number of deleted rows.
pub fn delete_old_subscription_items(
    conn: &Connection,
    subscription_id: &str,
    max_age_days: i32,
) -> Result<usize, DbError> {
    let cutoff = chrono::Utc::now() - chrono::Duration::days(max_age_days as i64);
    let cutoff_str = cutoff.to_rfc3339();
    let deleted = conn.execute(
        "DELETE FROM subscription_items
         WHERE subscription_id = ?1
           AND added_to_library = 0
           AND fetched_date < ?2",
        params![subscription_id, cutoff_str],
    )?;
    Ok(deleted)
}

/// Clear all subscription items that have NOT been added to the library.
/// If subscription_id is provided, only clear items for that subscription.
/// Returns the number of deleted rows.
pub fn clear_subscription_cache(
    conn: &Connection,
    subscription_id: Option<&str>,
) -> Result<usize, DbError> {
    let deleted = if let Some(sub_id) = subscription_id {
        conn.execute(
            "DELETE FROM subscription_items
             WHERE subscription_id = ?1 AND added_to_library = 0",
            params![sub_id],
        )?
    } else {
        conn.execute(
            "DELETE FROM subscription_items WHERE added_to_library = 0",
            [],
        )?
    };
    Ok(deleted)
}

/// Count subscription items, returning (total, not_added_to_library).
pub fn count_subscription_items(
    conn: &Connection,
    subscription_id: Option<&str>,
) -> Result<(i64, i64), DbError> {
    let (total, cached) = if let Some(sub_id) = subscription_id {
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM subscription_items WHERE subscription_id = ?1",
            params![sub_id],
            |row| row.get(0),
        )?;
        let cached: i64 = conn.query_row(
            "SELECT COUNT(*) FROM subscription_items
             WHERE subscription_id = ?1 AND added_to_library = 0",
            params![sub_id],
            |row| row.get(0),
        )?;
        (total, cached)
    } else {
        let total: i64 = conn.query_row("SELECT COUNT(*) FROM subscription_items", [], |row| {
            row.get(0)
        })?;
        let cached: i64 = conn.query_row(
            "SELECT COUNT(*) FROM subscription_items WHERE added_to_library = 0",
            [],
            |row| row.get(0),
        )?;
        (total, cached)
    };
    Ok((total, cached))
}
