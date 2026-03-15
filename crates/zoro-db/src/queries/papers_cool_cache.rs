// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::DbError;
use rusqlite::{params, Connection};

pub fn get_cached(conn: &Connection, cache_key: &str) -> Result<Option<String>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT response_json, fetched_at, ttl_seconds FROM papers_cool_cache WHERE cache_key = ?1",
    )?;

    let result = stmt.query_row(params![cache_key], |row| {
        let json: String = row.get(0)?;
        let fetched_at: String = row.get(1)?;
        let ttl: i64 = row.get(2)?;
        Ok((json, fetched_at, ttl))
    });

    match result {
        Ok((json, fetched_at, ttl)) => {
            if let Ok(fetched) = chrono::DateTime::parse_from_rfc3339(&fetched_at) {
                let now = chrono::Utc::now();
                let elapsed = now.signed_duration_since(fetched);
                if elapsed.num_seconds() < ttl {
                    return Ok(Some(json));
                }
            }
            Ok(None)
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(DbError::from(e)),
    }
}

pub fn set_cached(
    conn: &Connection,
    cache_key: &str,
    response_json: &str,
    ttl_seconds: i64,
) -> Result<(), DbError> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO papers_cool_cache (cache_key, response_json, fetched_at, ttl_seconds)
         VALUES (?1, ?2, ?3, ?4)",
        params![cache_key, response_json, now, ttl_seconds],
    )?;
    Ok(())
}

pub fn clear_cache(conn: &Connection) -> Result<usize, DbError> {
    let deleted = conn.execute("DELETE FROM papers_cool_cache", [])?;
    Ok(deleted)
}

pub fn clear_expired(conn: &Connection) -> Result<usize, DbError> {
    let now = chrono::Utc::now().to_rfc3339();
    let deleted = conn.execute(
        "DELETE FROM papers_cool_cache
         WHERE datetime(fetched_at, '+' || ttl_seconds || ' seconds') < datetime(?1)",
        params![now],
    )?;
    Ok(deleted)
}

// ── Papers.cool paper texts (for translation system) ────────────────────────

pub fn upsert_paper_text(
    conn: &Connection,
    external_id: &str,
    title: &str,
    abstract_text: Option<&str>,
) -> Result<(), DbError> {
    conn.execute(
        "INSERT OR REPLACE INTO papers_cool_texts (external_id, title, abstract_text)
         VALUES (?1, ?2, ?3)",
        params![external_id, title, abstract_text],
    )?;
    Ok(())
}

pub fn upsert_paper_texts_batch(
    conn: &Connection,
    papers: &[(String, String, Option<String>)],
) -> Result<(), DbError> {
    let mut stmt = conn.prepare(
        "INSERT OR REPLACE INTO papers_cool_texts (external_id, title, abstract_text)
         VALUES (?1, ?2, ?3)",
    )?;
    for (eid, title, abs) in papers {
        stmt.execute(params![eid, title, abs])?;
    }
    Ok(())
}

pub fn get_paper_text(
    conn: &Connection,
    external_id: &str,
) -> Result<Option<(String, Option<String>)>, DbError> {
    let mut stmt =
        conn.prepare("SELECT title, abstract_text FROM papers_cool_texts WHERE external_id = ?1")?;
    let result = stmt.query_row(params![external_id], |row| {
        let title: String = row.get(0)?;
        let abs: Option<String> = row.get(1)?;
        Ok((title, abs))
    });
    match result {
        Ok(pair) => Ok(Some(pair)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(DbError::from(e)),
    }
}
