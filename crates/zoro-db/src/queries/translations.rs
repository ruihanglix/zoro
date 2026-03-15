// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::DbError;
use rusqlite::{params, Connection};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TranslationRow {
    pub id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub field: String,
    pub target_lang: String,
    pub translated_text: String,
    pub model: Option<String>,
    pub created_date: String,
    pub modified_date: String,
}

/// Get all translations for a given entity and target language.
pub fn get_translations(
    conn: &Connection,
    entity_type: &str,
    entity_id: &str,
    target_lang: &str,
) -> Result<Vec<TranslationRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, entity_type, entity_id, field, target_lang,
                translated_text, model, created_date, modified_date
         FROM translations
         WHERE entity_type = ?1 AND entity_id = ?2 AND target_lang = ?3",
    )?;

    let rows = stmt.query_map(params![entity_type, entity_id, target_lang], |row| {
        Ok(TranslationRow {
            id: row.get(0)?,
            entity_type: row.get(1)?,
            entity_id: row.get(2)?,
            field: row.get(3)?,
            target_lang: row.get(4)?,
            translated_text: row.get(5)?,
            model: row.get(6)?,
            created_date: row.get(7)?,
            modified_date: row.get(8)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// Batch-get translations for multiple entities of the same type and language.
pub fn get_translations_batch(
    conn: &Connection,
    entity_type: &str,
    entity_ids: &[String],
    target_lang: &str,
) -> Result<Vec<TranslationRow>, DbError> {
    if entity_ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders: Vec<String> = entity_ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 3))
        .collect();
    let sql = format!(
        "SELECT id, entity_type, entity_id, field, target_lang,
                translated_text, model, created_date, modified_date
         FROM translations
         WHERE entity_type = ?1 AND target_lang = ?2 AND entity_id IN ({})",
        placeholders.join(", ")
    );

    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    param_values.push(Box::new(entity_type.to_string()));
    param_values.push(Box::new(target_lang.to_string()));
    for id in entity_ids {
        param_values.push(Box::new(id.clone()));
    }

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|b| b.as_ref()).collect();

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(TranslationRow {
            id: row.get(0)?,
            entity_type: row.get(1)?,
            entity_id: row.get(2)?,
            field: row.get(3)?,
            target_lang: row.get(4)?,
            translated_text: row.get(5)?,
            model: row.get(6)?,
            created_date: row.get(7)?,
            modified_date: row.get(8)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// Upsert a translation (insert or replace on conflict).
pub fn upsert_translation(
    conn: &Connection,
    entity_type: &str,
    entity_id: &str,
    field: &str,
    target_lang: &str,
    translated_text: &str,
    model: Option<&str>,
) -> Result<TranslationRow, DbError> {
    let now = chrono::Utc::now().to_rfc3339();
    let id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO translations (id, entity_type, entity_id, field, target_lang,
                                    translated_text, model, created_date, modified_date)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(entity_type, entity_id, field, target_lang)
         DO UPDATE SET translated_text = excluded.translated_text,
                       model = excluded.model,
                       modified_date = excluded.modified_date",
        params![
            id,
            entity_type,
            entity_id,
            field,
            target_lang,
            translated_text,
            model,
            now,
            now
        ],
    )?;

    // Fetch the actual row (may have a different id if it was an update)
    conn.query_row(
        "SELECT id, entity_type, entity_id, field, target_lang,
                translated_text, model, created_date, modified_date
         FROM translations
         WHERE entity_type = ?1 AND entity_id = ?2 AND field = ?3 AND target_lang = ?4",
        params![entity_type, entity_id, field, target_lang],
        |row| {
            Ok(TranslationRow {
                id: row.get(0)?,
                entity_type: row.get(1)?,
                entity_id: row.get(2)?,
                field: row.get(3)?,
                target_lang: row.get(4)?,
                translated_text: row.get(5)?,
                model: row.get(6)?,
                created_date: row.get(7)?,
                modified_date: row.get(8)?,
            })
        },
    )
    .map_err(DbError::Sqlite)
}

/// Delete all translations for a given entity.
pub fn delete_translations(
    conn: &Connection,
    entity_type: &str,
    entity_id: &str,
) -> Result<usize, DbError> {
    let deleted = conn.execute(
        "DELETE FROM translations WHERE entity_type = ?1 AND entity_id = ?2",
        params![entity_type, entity_id],
    )?;
    Ok(deleted)
}

/// Delete translations for a specific entity and language.
pub fn delete_translations_for_lang(
    conn: &Connection,
    entity_type: &str,
    entity_id: &str,
    target_lang: &str,
) -> Result<usize, DbError> {
    let deleted = conn.execute(
        "DELETE FROM translations WHERE entity_type = ?1 AND entity_id = ?2 AND target_lang = ?3",
        params![entity_type, entity_id, target_lang],
    )?;
    Ok(deleted)
}

/// Search translations via FTS5 and return matching paper IDs.
/// This is used to augment the main paper search with translated content.
pub fn search_translations_fts(
    conn: &Connection,
    query: &str,
    entity_type: &str,
    limit: i64,
) -> Result<Vec<String>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT t.entity_id
         FROM translations t
         JOIN translations_fts fts ON t.rowid = fts.rowid
         WHERE translations_fts MATCH ?1 AND t.entity_type = ?2
         LIMIT ?3",
    )?;

    let rows = stmt.query_map(params![query, entity_type, limit], |row| {
        row.get::<_, String>(0)
    })?;

    let mut ids = Vec::new();
    for row in rows {
        ids.push(row?);
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        schema::create_tables(&conn).unwrap();
        conn
    }

    #[test]
    fn test_upsert_and_get_translation() {
        let conn = setup_db();

        // Insert
        let row = upsert_translation(
            &conn,
            "paper",
            "paper-1",
            "title",
            "zh",
            "测试标题",
            Some("gpt-4o-mini"),
        )
        .unwrap();
        assert_eq!(row.translated_text, "测试标题");
        assert_eq!(row.model, Some("gpt-4o-mini".to_string()));

        // Get
        let rows = get_translations(&conn, "paper", "paper-1", "zh").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].field, "title");

        // Upsert (update)
        let row2 = upsert_translation(
            &conn,
            "paper",
            "paper-1",
            "title",
            "zh",
            "更新后的标题",
            Some("gpt-4o"),
        )
        .unwrap();
        assert_eq!(row2.translated_text, "更新后的标题");

        // Should still be only 1 row
        let rows = get_translations(&conn, "paper", "paper-1", "zh").unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn test_delete_translations() {
        let conn = setup_db();

        upsert_translation(&conn, "paper", "p1", "title", "zh", "标题", None).unwrap();
        upsert_translation(&conn, "paper", "p1", "abstract_text", "zh", "摘要", None).unwrap();

        let deleted = delete_translations(&conn, "paper", "p1").unwrap();
        assert_eq!(deleted, 2);

        let rows = get_translations(&conn, "paper", "p1", "zh").unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn test_batch_get() {
        let conn = setup_db();

        upsert_translation(&conn, "paper", "p1", "title", "zh", "标题1", None).unwrap();
        upsert_translation(&conn, "paper", "p2", "title", "zh", "标题2", None).unwrap();
        upsert_translation(&conn, "paper", "p3", "title", "zh", "标题3", None).unwrap();

        let ids = vec!["p1".to_string(), "p3".to_string()];
        let rows = get_translations_batch(&conn, "paper", &ids, "zh").unwrap();
        assert_eq!(rows.len(), 2);
    }
}
