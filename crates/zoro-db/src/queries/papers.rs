// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::DbError;
use rusqlite::{params, Connection};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PaperRow {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub short_title: Option<String>,
    pub abstract_text: Option<String>,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    pub url: Option<String>,
    pub pdf_url: Option<String>,
    pub html_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub published_date: Option<String>,
    pub added_date: String,
    pub modified_date: String,
    pub source: Option<String>,
    pub read_status: String,
    pub rating: Option<i32>,
    pub extra_json: Option<String>,
    pub dir_path: String,
    pub entry_type: Option<String>,
    pub journal: Option<String>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub pages: Option<String>,
    pub publisher: Option<String>,
    pub issn: Option<String>,
    pub isbn: Option<String>,
    #[serde(default = "default_downloaded")]
    pub pdf_downloaded: bool,
    #[serde(default = "default_downloaded")]
    pub html_downloaded: bool,
}

fn default_downloaded() -> bool {
    true
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreatePaperInput {
    pub slug: String,
    pub title: String,
    pub short_title: Option<String>,
    pub abstract_text: Option<String>,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    pub url: Option<String>,
    pub pdf_url: Option<String>,
    pub html_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub published_date: Option<String>,
    pub added_date: Option<String>,
    pub source: Option<String>,
    pub dir_path: String,
    pub extra_json: Option<String>,
    pub entry_type: Option<String>,
    pub journal: Option<String>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub pages: Option<String>,
    pub publisher: Option<String>,
    pub issn: Option<String>,
    pub isbn: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PaperFilter {
    pub collection_id: Option<String>,
    pub tag_name: Option<String>,
    pub read_status: Option<String>,
    pub search_query: Option<String>,
    pub uncategorized: Option<bool>,
    pub sort_by: Option<String>, // "added_date", "title", "published_date"
    pub sort_order: Option<String>, // "asc", "desc"
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub fn insert_paper(conn: &Connection, input: &CreatePaperInput) -> Result<PaperRow, DbError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let added = input.added_date.as_deref().unwrap_or(&now);

    // Generate a unique slug — if the base slug already exists, append -2, -3, etc.
    let slug = unique_paper_slug(conn, &input.slug);
    // Update dir_path to use the actual (possibly suffixed) slug
    let dir_path = if slug != input.slug {
        input.dir_path.replace(&input.slug, &slug)
    } else {
        input.dir_path.clone()
    };

    conn.execute(
        "INSERT INTO papers (id, slug, title, short_title, abstract_text, doi, arxiv_id, url, pdf_url, html_url, thumbnail_url, published_date, added_date, modified_date, source, read_status, extra_json, dir_path, entry_type, journal, volume, issue, pages, publisher, issn, isbn)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 'unread', ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)",
        params![
            id, slug, input.title, input.short_title, input.abstract_text, input.doi, input.arxiv_id,
            input.url, input.pdf_url, input.html_url, input.thumbnail_url, input.published_date,
            added, now, input.source, input.extra_json, dir_path,
            input.entry_type, input.journal, input.volume, input.issue,
            input.pages, input.publisher, input.issn, input.isbn
        ],
    )?;

    get_paper(conn, &id)
}

/// Generate a unique slug for a paper. If `base_slug` already exists, try
/// `base_slug-2`, `base_slug-3`, etc. until a free one is found.
fn unique_paper_slug(conn: &Connection, base_slug: &str) -> String {
    let exists = |s: &str| -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM papers WHERE slug = ?1",
            params![s],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
            > 0
    };

    if !exists(base_slug) {
        return base_slug.to_string();
    }

    let mut counter = 2u32;
    loop {
        let candidate = format!("{}-{}", base_slug, counter);
        if !exists(&candidate) {
            return candidate;
        }
        counter += 1;
    }
}

pub fn get_paper(conn: &Connection, id: &str) -> Result<PaperRow, DbError> {
    conn.query_row(
        "SELECT id, slug, title, short_title, abstract_text, doi, arxiv_id, url, pdf_url, html_url,
                thumbnail_url, published_date, added_date, modified_date, source, read_status,
                rating, extra_json, dir_path,
                entry_type, journal, volume, issue, pages, publisher, issn, isbn,
                COALESCE(pdf_downloaded, 1), COALESCE(html_downloaded, 1)
         FROM papers WHERE id = ?1",
        params![id],
        |row| {
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
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            DbError::NotFound(format!("Paper not found: {}", id))
        }
        other => DbError::Sqlite(other),
    })
}

pub fn list_papers(conn: &Connection, filter: &PaperFilter) -> Result<Vec<PaperRow>, DbError> {
    let mut sql = String::from(
        "SELECT p.id, p.slug, p.title, p.short_title, p.abstract_text, p.doi, p.arxiv_id, p.url, p.pdf_url, p.html_url,
                p.thumbnail_url, p.published_date, p.added_date, p.modified_date, p.source, p.read_status,
                p.rating, p.extra_json, p.dir_path,
                p.entry_type, p.journal, p.volume, p.issue, p.pages, p.publisher, p.issn, p.isbn,
                COALESCE(p.pdf_downloaded, 1), COALESCE(p.html_downloaded, 1)
         FROM papers p"
    );
    let mut conditions: Vec<String> = Vec::new();
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut param_idx = 1;

    if let Some(ref collection_id) = filter.collection_id {
        sql.push_str(" JOIN paper_collections pc ON p.id = pc.paper_id");
        conditions.push(format!("pc.collection_id = ?{}", param_idx));
        param_values.push(Box::new(collection_id.clone()));
        param_idx += 1;
    }

    if let Some(ref tag_name) = filter.tag_name {
        sql.push_str(" JOIN paper_tags pt ON p.id = pt.paper_id JOIN tags t ON pt.tag_id = t.id");
        conditions.push(format!("t.name = ?{}", param_idx));
        param_values.push(Box::new(tag_name.clone()));
        param_idx += 1;
    }

    if let Some(ref read_status) = filter.read_status {
        conditions.push(format!("p.read_status = ?{}", param_idx));
        param_values.push(Box::new(read_status.clone()));
        param_idx += 1;
    }

    if filter.uncategorized == Some(true) {
        conditions.push(
            "NOT EXISTS (SELECT 1 FROM paper_collections pc2 WHERE pc2.paper_id = p.id)"
                .to_string(),
        );
    }

    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }

    let sort_by = filter.sort_by.as_deref().unwrap_or("added_date");
    let sort_order = filter.sort_order.as_deref().unwrap_or("desc");
    let sort_col = match sort_by {
        "title" => "p.title",
        "published_date" => "p.published_date",
        "modified_date" => "p.modified_date",
        _ => "p.added_date",
    };
    let order = if sort_order == "asc" { "ASC" } else { "DESC" };
    sql.push_str(&format!(" ORDER BY {} {}", sort_col, order));

    let limit = filter.limit.unwrap_or(50);
    let offset = filter.offset.unwrap_or(0);
    sql.push_str(&format!(" LIMIT ?{} OFFSET ?{}", param_idx, param_idx + 1));
    param_values.push(Box::new(limit));
    param_values.push(Box::new(offset));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|b| b.as_ref()).collect();

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
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
    })?;

    let mut papers = Vec::new();
    for row in rows {
        papers.push(row?);
    }
    Ok(papers)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UpdatePaperInput {
    pub title: Option<String>,
    pub short_title: Option<Option<String>>,
    pub abstract_text: Option<Option<String>>,
    pub doi: Option<Option<String>>,
    pub arxiv_id: Option<Option<String>>,
    pub url: Option<Option<String>>,
    pub pdf_url: Option<Option<String>>,
    pub html_url: Option<Option<String>>,
    pub thumbnail_url: Option<Option<String>>,
    pub published_date: Option<Option<String>>,
    pub source: Option<Option<String>>,
    pub extra_json: Option<Option<String>>,
    pub entry_type: Option<Option<String>>,
    pub journal: Option<Option<String>>,
    pub volume: Option<Option<String>>,
    pub issue: Option<Option<String>>,
    pub pages: Option<Option<String>>,
    pub publisher: Option<Option<String>>,
    pub issn: Option<Option<String>>,
    pub isbn: Option<Option<String>>,
}

pub fn update_paper(conn: &Connection, id: &str, input: &UpdatePaperInput) -> Result<(), DbError> {
    let mut sets: Vec<String> = Vec::new();
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    if let Some(ref title) = input.title {
        sets.push(format!("title = ?{}", idx));
        param_values.push(Box::new(title.clone()));
        idx += 1;
    }
    if let Some(ref short_title) = input.short_title {
        sets.push(format!("short_title = ?{}", idx));
        param_values.push(Box::new(short_title.clone()));
        idx += 1;
    }
    if let Some(ref abstract_text) = input.abstract_text {
        sets.push(format!("abstract_text = ?{}", idx));
        param_values.push(Box::new(abstract_text.clone()));
        idx += 1;
    }
    if let Some(ref doi) = input.doi {
        sets.push(format!("doi = ?{}", idx));
        param_values.push(Box::new(doi.clone()));
        idx += 1;
    }
    if let Some(ref arxiv_id) = input.arxiv_id {
        sets.push(format!("arxiv_id = ?{}", idx));
        param_values.push(Box::new(arxiv_id.clone()));
        idx += 1;
    }
    if let Some(ref url) = input.url {
        sets.push(format!("url = ?{}", idx));
        param_values.push(Box::new(url.clone()));
        idx += 1;
    }
    if let Some(ref pdf_url) = input.pdf_url {
        sets.push(format!("pdf_url = ?{}", idx));
        param_values.push(Box::new(pdf_url.clone()));
        idx += 1;
    }
    if let Some(ref html_url) = input.html_url {
        sets.push(format!("html_url = ?{}", idx));
        param_values.push(Box::new(html_url.clone()));
        idx += 1;
    }
    if let Some(ref thumbnail_url) = input.thumbnail_url {
        sets.push(format!("thumbnail_url = ?{}", idx));
        param_values.push(Box::new(thumbnail_url.clone()));
        idx += 1;
    }
    if let Some(ref published_date) = input.published_date {
        sets.push(format!("published_date = ?{}", idx));
        param_values.push(Box::new(published_date.clone()));
        idx += 1;
    }
    if let Some(ref source) = input.source {
        sets.push(format!("source = ?{}", idx));
        param_values.push(Box::new(source.clone()));
        idx += 1;
    }
    if let Some(ref extra_json) = input.extra_json {
        sets.push(format!("extra_json = ?{}", idx));
        param_values.push(Box::new(extra_json.clone()));
        idx += 1;
    }
    if let Some(ref entry_type) = input.entry_type {
        sets.push(format!("entry_type = ?{}", idx));
        param_values.push(Box::new(entry_type.clone()));
        idx += 1;
    }
    if let Some(ref journal) = input.journal {
        sets.push(format!("journal = ?{}", idx));
        param_values.push(Box::new(journal.clone()));
        idx += 1;
    }
    if let Some(ref volume) = input.volume {
        sets.push(format!("volume = ?{}", idx));
        param_values.push(Box::new(volume.clone()));
        idx += 1;
    }
    if let Some(ref issue) = input.issue {
        sets.push(format!("issue = ?{}", idx));
        param_values.push(Box::new(issue.clone()));
        idx += 1;
    }
    if let Some(ref pages) = input.pages {
        sets.push(format!("pages = ?{}", idx));
        param_values.push(Box::new(pages.clone()));
        idx += 1;
    }
    if let Some(ref publisher) = input.publisher {
        sets.push(format!("publisher = ?{}", idx));
        param_values.push(Box::new(publisher.clone()));
        idx += 1;
    }
    if let Some(ref issn) = input.issn {
        sets.push(format!("issn = ?{}", idx));
        param_values.push(Box::new(issn.clone()));
        idx += 1;
    }
    if let Some(ref isbn) = input.isbn {
        sets.push(format!("isbn = ?{}", idx));
        param_values.push(Box::new(isbn.clone()));
        idx += 1;
    }

    if sets.is_empty() {
        return Ok(());
    }

    // Always update modified_date
    let now = chrono::Utc::now().to_rfc3339();
    sets.push(format!("modified_date = ?{}", idx));
    param_values.push(Box::new(now));
    idx += 1;

    let sql = format!("UPDATE papers SET {} WHERE id = ?{}", sets.join(", "), idx);
    param_values.push(Box::new(id.to_string()));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|b| b.as_ref()).collect();
    let updated = conn.execute(&sql, param_refs.as_slice())?;
    if updated == 0 {
        return Err(DbError::NotFound(format!("Paper not found: {}", id)));
    }
    Ok(())
}

pub fn update_paper_status(conn: &Connection, id: &str, read_status: &str) -> Result<(), DbError> {
    let now = chrono::Utc::now().to_rfc3339();
    let updated = conn.execute(
        "UPDATE papers SET read_status = ?1, modified_date = ?2 WHERE id = ?3",
        params![read_status, now, id],
    )?;
    if updated == 0 {
        return Err(DbError::NotFound(format!("Paper not found: {}", id)));
    }
    Ok(())
}

pub fn update_paper_rating(
    conn: &Connection,
    id: &str,
    rating: Option<i32>,
) -> Result<(), DbError> {
    let now = chrono::Utc::now().to_rfc3339();
    let updated = conn.execute(
        "UPDATE papers SET rating = ?1, modified_date = ?2 WHERE id = ?3",
        params![rating, now, id],
    )?;
    if updated == 0 {
        return Err(DbError::NotFound(format!("Paper not found: {}", id)));
    }
    Ok(())
}

pub fn delete_paper(conn: &Connection, id: &str) -> Result<(), DbError> {
    let deleted = conn.execute("DELETE FROM papers WHERE id = ?1", params![id])?;
    if deleted == 0 {
        return Err(DbError::NotFound(format!("Paper not found: {}", id)));
    }
    Ok(())
}

pub fn count_papers(conn: &Connection) -> Result<i64, DbError> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM papers", [], |row| row.get(0))?;
    Ok(count)
}

// Author operations for a paper
pub fn set_paper_authors(
    conn: &Connection,
    paper_id: &str,
    authors: &[(String, Option<String>, Option<String>)],
) -> Result<(), DbError> {
    // Delete existing
    conn.execute(
        "DELETE FROM paper_authors WHERE paper_id = ?1",
        params![paper_id],
    )?;

    for (position, (name, affiliation, orcid)) in authors.iter().enumerate() {
        // Find or create author
        let author_id = match conn.query_row(
            "SELECT id FROM authors WHERE name = ?1",
            params![name],
            |row| row.get::<_, String>(0),
        ) {
            Ok(id) => id,
            Err(_) => {
                let id = Uuid::new_v4().to_string();
                conn.execute(
                    "INSERT INTO authors (id, name, affiliation, orcid) VALUES (?1, ?2, ?3, ?4)",
                    params![id, name, affiliation, orcid],
                )?;
                id
            }
        };

        conn.execute(
            "INSERT INTO paper_authors (paper_id, author_id, position) VALUES (?1, ?2, ?3)",
            params![paper_id, author_id, position as i32],
        )?;
    }

    Ok(())
}

#[allow(clippy::type_complexity)]
pub fn get_paper_authors(
    conn: &Connection,
    paper_id: &str,
) -> Result<Vec<(String, Option<String>, Option<String>)>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT a.name, a.affiliation, a.orcid
         FROM authors a
         JOIN paper_authors pa ON a.id = pa.author_id
         WHERE pa.paper_id = ?1
         ORDER BY pa.position",
    )?;

    let rows = stmt.query_map(params![paper_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<String>>(2)?,
        ))
    })?;

    let mut authors = Vec::new();
    for row in rows {
        authors.push(row?);
    }
    Ok(authors)
}

/// Update the pdf_downloaded flag for a paper.
pub fn set_pdf_downloaded(conn: &Connection, id: &str, downloaded: bool) -> Result<(), DbError> {
    let val: i32 = if downloaded { 1 } else { 0 };
    let updated = conn.execute(
        "UPDATE papers SET pdf_downloaded = ?1 WHERE id = ?2",
        params![val, id],
    )?;
    if updated == 0 {
        return Err(DbError::NotFound(format!("Paper not found: {}", id)));
    }
    Ok(())
}

/// Update the html_downloaded flag for a paper.
pub fn set_html_downloaded(conn: &Connection, id: &str, downloaded: bool) -> Result<(), DbError> {
    let val: i32 = if downloaded { 1 } else { 0 };
    let updated = conn.execute(
        "UPDATE papers SET html_downloaded = ?1 WHERE id = ?2",
        params![val, id],
    )?;
    if updated == 0 {
        return Err(DbError::NotFound(format!("Paper not found: {}", id)));
    }
    Ok(())
}

/// Get a paper by its slug.
pub fn get_paper_by_slug(conn: &Connection, slug: &str) -> Result<PaperRow, DbError> {
    conn.query_row(
        "SELECT id, slug, title, short_title, abstract_text, doi, arxiv_id, url, pdf_url, html_url,
                thumbnail_url, published_date, added_date, modified_date, source, read_status,
                rating, extra_json, dir_path,
                entry_type, journal, volume, issue, pages, publisher, issn, isbn,
                COALESCE(pdf_downloaded, 1), COALESCE(html_downloaded, 1)
         FROM papers WHERE slug = ?1",
        params![slug],
        |row| {
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
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            DbError::NotFound(format!("Paper not found: {}", slug))
        }
        other => DbError::Sqlite(other),
    })
}
