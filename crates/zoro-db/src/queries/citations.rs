// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::DbError;
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, Clone)]
pub struct CitationCacheRow {
    pub paper_id: String,
    pub style: String,
    pub text: String,
    pub provider: String,
    pub doi: Option<String>,
    pub request_url: Option<String>,
    pub accept_header: Option<String>,
    pub fetched_date: String,
}

pub fn get_cached_citation(
    conn: &Connection,
    paper_id: &str,
    style: &str,
) -> Result<Option<CitationCacheRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT paper_id, style, text, provider, doi, request_url, accept_header, fetched_date
         FROM citation_cache
         WHERE paper_id = ?1 AND style = ?2",
    )?;

    let row = stmt
        .query_row(params![paper_id, style], |row| {
            Ok(CitationCacheRow {
                paper_id: row.get(0)?,
                style: row.get(1)?,
                text: row.get(2)?,
                provider: row.get(3)?,
                doi: row.get(4)?,
                request_url: row.get(5)?,
                accept_header: row.get(6)?,
                fetched_date: row.get(7)?,
            })
        })
        .optional()?;

    Ok(row)
}

pub struct CitationCacheInput<'a> {
    pub paper_id: &'a str,
    pub style: &'a str,
    pub text: &'a str,
    pub provider: &'a str,
    pub doi: Option<&'a str>,
    pub request_url: Option<&'a str>,
    pub accept_header: Option<&'a str>,
}

pub fn upsert_citation_cache(conn: &Connection, input: &CitationCacheInput) -> Result<(), DbError> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO citation_cache (paper_id, style, text, provider, doi, request_url, accept_header, fetched_date)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(paper_id, style)
         DO UPDATE SET text = excluded.text,
                       provider = excluded.provider,
                       doi = excluded.doi,
                       request_url = excluded.request_url,
                       accept_header = excluded.accept_header,
                       fetched_date = excluded.fetched_date",
        params![
            input.paper_id,
            input.style,
            input.text,
            input.provider,
            input.doi,
            input.request_url,
            input.accept_header,
            now
        ],
    )?;
    Ok(())
}

pub fn delete_paper_citation_cache(conn: &Connection, paper_id: &str) -> Result<usize, DbError> {
    let deleted = conn.execute(
        "DELETE FROM citation_cache WHERE paper_id = ?1",
        params![paper_id],
    )?;
    Ok(deleted)
}
