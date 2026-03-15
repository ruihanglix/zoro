// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::DbError;
use rusqlite::{params, Connection};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GlossaryRow {
    pub id: String,
    pub source_term: String,
    pub translated_term: String,
    pub target_lang: String,
    pub source: String,
    pub occurrence_count: i64,
    pub created_date: String,
    pub updated_date: String,
}

pub fn list_glossary(conn: &Connection, target_lang: &str) -> Result<Vec<GlossaryRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, source_term, translated_term, target_lang, source,
                occurrence_count, created_date, updated_date
         FROM glossary
         WHERE target_lang = ?1
         ORDER BY occurrence_count DESC, source_term ASC",
    )?;
    let rows = stmt
        .query_map(params![target_lang], |row| {
            Ok(GlossaryRow {
                id: row.get(0)?,
                source_term: row.get(1)?,
                translated_term: row.get(2)?,
                target_lang: row.get(3)?,
                source: row.get(4)?,
                occurrence_count: row.get(5)?,
                created_date: row.get(6)?,
                updated_date: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Get all active glossary terms (above threshold or manual) for a language.
pub fn list_active_glossary(
    conn: &Connection,
    target_lang: &str,
    threshold: i64,
) -> Result<Vec<GlossaryRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, source_term, translated_term, target_lang, source,
                occurrence_count, created_date, updated_date
         FROM glossary
         WHERE target_lang = ?1
           AND (source = 'manual' OR occurrence_count >= ?2)
         ORDER BY source_term ASC",
    )?;
    let rows = stmt
        .query_map(params![target_lang, threshold], |row| {
            Ok(GlossaryRow {
                id: row.get(0)?,
                source_term: row.get(1)?,
                translated_term: row.get(2)?,
                target_lang: row.get(3)?,
                source: row.get(4)?,
                occurrence_count: row.get(5)?,
                created_date: row.get(6)?,
                updated_date: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn add_glossary_term(
    conn: &Connection,
    source_term: &str,
    translated_term: &str,
    target_lang: &str,
    source: &str,
) -> Result<GlossaryRow, DbError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO glossary (id, source_term, translated_term, target_lang, source,
                               occurrence_count, created_date, updated_date)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?6)
         ON CONFLICT(source_term COLLATE NOCASE, target_lang) DO UPDATE SET
             translated_term = CASE
                 WHEN glossary.source = 'manual' THEN glossary.translated_term
                 ELSE excluded.translated_term
             END,
             occurrence_count = glossary.occurrence_count + 1,
             updated_date = excluded.updated_date",
        params![id, source_term, translated_term, target_lang, source, now],
    )?;

    let row = conn.query_row(
        "SELECT id, source_term, translated_term, target_lang, source,
                occurrence_count, created_date, updated_date
         FROM glossary WHERE source_term = ?1 COLLATE NOCASE AND target_lang = ?2",
        params![source_term, target_lang],
        |row| {
            Ok(GlossaryRow {
                id: row.get(0)?,
                source_term: row.get(1)?,
                translated_term: row.get(2)?,
                target_lang: row.get(3)?,
                source: row.get(4)?,
                occurrence_count: row.get(5)?,
                created_date: row.get(6)?,
                updated_date: row.get(7)?,
            })
        },
    )?;
    Ok(row)
}

pub fn update_glossary_term(
    conn: &Connection,
    id: &str,
    translated_term: &str,
) -> Result<(), DbError> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE glossary SET translated_term = ?1, source = 'manual', updated_date = ?2
         WHERE id = ?3",
        params![translated_term, now, id],
    )?;
    Ok(())
}

pub fn promote_glossary_term(conn: &Connection, id: &str) -> Result<(), DbError> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE glossary SET source = 'manual', updated_date = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

pub fn delete_glossary_term(conn: &Connection, id: &str) -> Result<(), DbError> {
    conn.execute("DELETE FROM glossary WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn clear_glossary(conn: &Connection, target_lang: &str) -> Result<usize, DbError> {
    let count = conn.execute(
        "DELETE FROM glossary WHERE target_lang = ?1",
        params![target_lang],
    )?;
    Ok(count)
}

/// Upsert terms extracted by the LLM, respecting per-entity deduplication.
/// Returns the number of newly incremented terms.
pub fn upsert_extracted_terms(
    conn: &Connection,
    terms: &[(String, String)],
    target_lang: &str,
    entity_id: &str,
) -> Result<usize, DbError> {
    let mut count = 0;
    let now = chrono::Utc::now().to_rfc3339();

    for (source_term, translated_term) in terms {
        let trimmed = source_term.trim();
        if trimmed.is_empty() {
            continue;
        }

        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO glossary (id, source_term, translated_term, target_lang, source,
                                   occurrence_count, created_date, updated_date)
             VALUES (?1, ?2, ?3, ?4, 'auto', 1, ?5, ?5)
             ON CONFLICT(source_term COLLATE NOCASE, target_lang) DO UPDATE SET
                 translated_term = CASE
                     WHEN glossary.source = 'manual' THEN glossary.translated_term
                     ELSE excluded.translated_term
                 END,
                 updated_date = excluded.updated_date",
            params![id, trimmed, translated_term.trim(), target_lang, now],
        )?;

        let glossary_id: String = conn.query_row(
            "SELECT id FROM glossary WHERE source_term = ?1 COLLATE NOCASE AND target_lang = ?2",
            params![trimmed, target_lang],
            |row| row.get(0),
        )?;

        let already_counted: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM glossary_occurrences
             WHERE glossary_id = ?1 AND entity_id = ?2",
            params![glossary_id, entity_id],
            |row| row.get(0),
        )?;

        if !already_counted {
            conn.execute(
                "INSERT INTO glossary_occurrences (glossary_id, entity_id) VALUES (?1, ?2)",
                params![glossary_id, entity_id],
            )?;
            conn.execute(
                "UPDATE glossary SET occurrence_count = (
                     SELECT COUNT(*) FROM glossary_occurrences WHERE glossary_id = ?1
                 ), updated_date = ?2
                 WHERE id = ?1",
                params![glossary_id, now],
            )?;
            count += 1;
        }
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::schema::create_tables(&conn).unwrap();
        conn
    }

    #[test]
    fn test_add_and_list() {
        let conn = setup_db();
        add_glossary_term(&conn, "Transformer", "Transformer", "zh", "manual").unwrap();
        add_glossary_term(&conn, "attention", "注意力", "zh", "manual").unwrap();

        let terms = list_glossary(&conn, "zh").unwrap();
        assert_eq!(terms.len(), 2);
    }

    #[test]
    fn test_upsert_extracted_dedup() {
        let conn = setup_db();
        let terms = vec![
            ("RLHF".to_string(), "人类反馈强化学习".to_string()),
            ("fine-tuning".to_string(), "微调".to_string()),
        ];

        let count1 = upsert_extracted_terms(&conn, &terms, "zh", "paper-1").unwrap();
        assert_eq!(count1, 2);

        // Same paper again — should not increment
        let count2 = upsert_extracted_terms(&conn, &terms, "zh", "paper-1").unwrap();
        assert_eq!(count2, 0);

        // Different paper — should increment
        let count3 = upsert_extracted_terms(&conn, &terms, "zh", "paper-2").unwrap();
        assert_eq!(count3, 2);

        let all = list_glossary(&conn, "zh").unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].occurrence_count, 2);
    }

    #[test]
    fn test_manual_not_overwritten() {
        let conn = setup_db();
        add_glossary_term(&conn, "attention", "注意力机制", "zh", "manual").unwrap();

        let terms = vec![("attention".to_string(), "注意力".to_string())];
        upsert_extracted_terms(&conn, &terms, "zh", "paper-1").unwrap();

        let all = list_glossary(&conn, "zh").unwrap();
        assert_eq!(all[0].translated_term, "注意力机制");
    }

    #[test]
    fn test_promote_and_active() {
        let conn = setup_db();
        let terms = vec![("RLHF".to_string(), "人类反馈强化学习".to_string())];
        upsert_extracted_terms(&conn, &terms, "zh", "paper-1").unwrap();

        let active = list_active_glossary(&conn, "zh", 5).unwrap();
        assert!(active.is_empty());

        let all = list_glossary(&conn, "zh").unwrap();
        promote_glossary_term(&conn, &all[0].id).unwrap();

        let active = list_active_glossary(&conn, "zh", 5).unwrap();
        assert_eq!(active.len(), 1);
    }

    #[test]
    fn test_case_insensitive_upsert() {
        let conn = setup_db();
        // Insert with different casing — should merge into a single row
        let terms1 = vec![(
            "Diffusion Transformers".to_string(),
            "扩散Transformer".to_string(),
        )];
        upsert_extracted_terms(&conn, &terms1, "zh", "paper-1").unwrap();

        let terms2 = vec![(
            "Diffusion transformers".to_string(),
            "扩散变换器".to_string(),
        )];
        upsert_extracted_terms(&conn, &terms2, "zh", "paper-2").unwrap();

        let all = list_glossary(&conn, "zh").unwrap();
        assert_eq!(all.len(), 1, "Terms differing only in case should merge");
        assert_eq!(all[0].occurrence_count, 2);
    }

    #[test]
    fn test_case_insensitive_manual_add() {
        let conn = setup_db();
        add_glossary_term(&conn, "Transformer", "Transformer模型", "zh", "manual").unwrap();
        // Adding same term with different case should hit the existing row
        let row = add_glossary_term(&conn, "transformer", "变换器", "zh", "manual").unwrap();

        let all = list_glossary(&conn, "zh").unwrap();
        assert_eq!(
            all.len(),
            1,
            "Manual terms differing only in case should merge"
        );
        // The id returned should match the existing row
        assert_eq!(row.id, all[0].id);
    }
}
