// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use super::papers::PaperRow;
use crate::error::DbError;
use rusqlite::{params, Connection};
use std::collections::HashSet;

const PAPER_SELECT_COLS: &str =
    "p.id, p.slug, p.title, p.short_title, p.abstract_text, p.doi, p.arxiv_id, p.url, \
     p.pdf_url, p.html_url, p.thumbnail_url, p.published_date, p.added_date, p.modified_date, \
     p.source, p.read_status, p.rating, p.extra_json, p.dir_path, \
     p.entry_type, p.journal, p.volume, p.issue, p.pages, p.publisher, p.issn, p.isbn, \
     COALESCE(p.pdf_downloaded, 1), COALESCE(p.html_downloaded, 1)";

fn row_to_paper(row: &rusqlite::Row) -> rusqlite::Result<PaperRow> {
    Ok(PaperRow {
        id: row.get(0)?,
        slug: row.get(1)?,
        title: row.get(2)?,
        short_title: row.get(3)?,
        abstract_text: row.get(4)?,
        doi: row.get(5)?,
        arxiv_id: row.get(6)?,
        url: row.get(7)?,
        pdf_url: row.get(8)?,
        html_url: row.get(9)?,
        thumbnail_url: row.get(10)?,
        published_date: row.get(11)?,
        added_date: row.get(12)?,
        modified_date: row.get(13)?,
        source: row.get(14)?,
        read_status: row.get(15)?,
        rating: row.get(16)?,
        extra_json: row.get(17)?,
        dir_path: row.get(18)?,
        entry_type: row.get(19)?,
        journal: row.get(20)?,
        volume: row.get(21)?,
        issue: row.get(22)?,
        pages: row.get(23)?,
        publisher: row.get(24)?,
        issn: row.get(25)?,
        isbn: row.get(26)?,
        pdf_downloaded: row.get::<_, i32>(27)? != 0,
        html_downloaded: row.get::<_, i32>(28)? != 0,
    })
}

pub fn search_papers(conn: &Connection, query: &str, limit: i64) -> Result<Vec<PaperRow>, DbError> {
    search_papers_opts(conn, query, limit, false)
}

pub fn search_papers_opts(
    conn: &Connection,
    query: &str,
    limit: i64,
    whole_word: bool,
) -> Result<Vec<PaperRow>, DbError> {
    let fts_query = query
        .split_whitespace()
        .map(|w| {
            if whole_word {
                format!("\"{}\"", w)
            } else {
                format!("{}*", w)
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    let sql = format!(
        "SELECT {} FROM papers p
         JOIN papers_fts fts ON p.rowid = fts.rowid
         WHERE papers_fts MATCH ?1
         ORDER BY rank
         LIMIT ?2",
        PAPER_SELECT_COLS
    );
    let mut stmt = conn.prepare(&sql)?;
    let fts_rows = stmt.query_map(params![fts_query, limit], row_to_paper)?;

    let mut seen = HashSet::new();
    let mut papers = Vec::new();
    for row in fts_rows {
        let p = row?;
        seen.insert(p.id.clone());
        papers.push(p);
    }

    // Also search by author name (LIKE-based, since authors live in a separate table)
    let words: Vec<&str> = query.split_whitespace().collect();
    if !words.is_empty() {
        let like_clauses: Vec<String> = words.iter().map(|_| "a.name LIKE ?".to_string()).collect();
        let author_sql = format!(
            "SELECT DISTINCT {} FROM papers p
             JOIN paper_authors pa ON pa.paper_id = p.id
             JOIN authors a ON a.id = pa.author_id
             WHERE {}
             LIMIT ?",
            PAPER_SELECT_COLS,
            like_clauses.join(" AND ")
        );
        let mut author_stmt = conn.prepare(&author_sql)?;
        let like_params: Vec<String> = words.iter().map(|w| format!("%{}%", w)).collect();
        let mut param_values: Vec<&dyn rusqlite::types::ToSql> = like_params
            .iter()
            .map(|s| s as &dyn rusqlite::types::ToSql)
            .collect();
        param_values.push(&limit);

        let author_rows = author_stmt
            .query_map(rusqlite::params_from_iter(param_values), |row| {
                row_to_paper(row)
            })?;

        for row in author_rows {
            let p = row?;
            if !seen.contains(&p.id) {
                seen.insert(p.id.clone());
                papers.push(p);
            }
        }
    }

    Ok(papers)
}
