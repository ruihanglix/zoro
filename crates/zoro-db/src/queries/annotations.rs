// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::DbError;
use rusqlite::{params, Connection};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnnotationRow {
    pub id: String,
    pub paper_id: String,
    #[serde(rename = "type")]
    pub annotation_type: String,
    pub color: String,
    pub comment: Option<String>,
    pub selected_text: Option<String>,
    pub image_data: Option<String>,
    pub position_json: String,
    pub page_number: i64,
    pub created_date: String,
    pub modified_date: String,
    pub source_file: String,
}

#[allow(clippy::too_many_arguments)]
pub fn insert_annotation(
    conn: &Connection,
    paper_id: &str,
    annotation_type: &str,
    color: &str,
    comment: Option<&str>,
    selected_text: Option<&str>,
    image_data: Option<&str>,
    position_json: &str,
    page_number: i64,
    source_file: Option<&str>,
) -> Result<AnnotationRow, DbError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let sf = source_file.unwrap_or("paper.pdf");

    conn.execute(
        "INSERT INTO annotations
         (id, paper_id, type, color, comment, selected_text, image_data,
          position_json, page_number, created_date, modified_date, source_file)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            id,
            paper_id,
            annotation_type,
            color,
            comment,
            selected_text,
            image_data,
            position_json,
            page_number,
            now,
            now,
            sf,
        ],
    )?;

    Ok(AnnotationRow {
        id,
        paper_id: paper_id.to_string(),
        annotation_type: annotation_type.to_string(),
        color: color.to_string(),
        comment: comment.map(String::from),
        selected_text: selected_text.map(String::from),
        image_data: image_data.map(String::from),
        position_json: position_json.to_string(),
        page_number,
        created_date: now.clone(),
        modified_date: now,
        source_file: sf.to_string(),
    })
}

pub fn list_annotations(
    conn: &Connection,
    paper_id: &str,
    source_file: Option<&str>,
) -> Result<Vec<AnnotationRow>, DbError> {
    let sf = source_file.unwrap_or("paper.pdf");
    let mut stmt = conn.prepare(
        "SELECT id, paper_id, type, color, comment, selected_text, image_data,
                position_json, page_number, created_date, modified_date,
                COALESCE(source_file, 'paper.pdf')
         FROM annotations WHERE paper_id = ?1 AND COALESCE(source_file, 'paper.pdf') = ?2
         ORDER BY page_number ASC, created_date ASC",
    )?;
    let rows = stmt.query_map(params![paper_id, sf], |row| {
        Ok(AnnotationRow {
            id: row.get(0)?,
            paper_id: row.get(1)?,
            annotation_type: row.get(2)?,
            color: row.get(3)?,
            comment: row.get(4)?,
            selected_text: row.get(5)?,
            image_data: row.get(6)?,
            position_json: row.get(7)?,
            page_number: row.get(8)?,
            created_date: row.get(9)?,
            modified_date: row.get(10)?,
            source_file: row.get(11)?,
        })
    })?;
    let mut annotations = Vec::new();
    for row in rows {
        annotations.push(row?);
    }
    Ok(annotations)
}

pub fn update_annotation(
    conn: &Connection,
    id: &str,
    color: Option<&str>,
    comment: Option<Option<&str>>,
) -> Result<AnnotationRow, DbError> {
    let now = chrono::Utc::now().to_rfc3339();

    let updated = conn.execute(
        "UPDATE annotations SET color = COALESCE(?1, color), comment = CASE WHEN ?2 = 1 \
         THEN ?3 ELSE comment END, modified_date = ?4 WHERE id = ?5",
        params![color, comment.is_some(), comment.unwrap_or(None), now, id,],
    )?;

    if updated == 0 {
        return Err(DbError::NotFound(format!("Annotation not found: {}", id)));
    }

    conn.query_row(
        "SELECT id, paper_id, type, color, comment, selected_text, image_data,
                position_json, page_number, created_date, modified_date,
                COALESCE(source_file, 'paper.pdf')
         FROM annotations WHERE id = ?1",
        params![id],
        |row| {
            Ok(AnnotationRow {
                id: row.get(0)?,
                paper_id: row.get(1)?,
                annotation_type: row.get(2)?,
                color: row.get(3)?,
                comment: row.get(4)?,
                selected_text: row.get(5)?,
                image_data: row.get(6)?,
                position_json: row.get(7)?,
                page_number: row.get(8)?,
                created_date: row.get(9)?,
                modified_date: row.get(10)?,
                source_file: row.get(11)?,
            })
        },
    )
    .map_err(DbError::Sqlite)
}

pub fn delete_annotation(conn: &Connection, id: &str) -> Result<(), DbError> {
    let deleted = conn.execute("DELETE FROM annotations WHERE id = ?1", params![id])?;
    if deleted == 0 {
        return Err(DbError::NotFound(format!("Annotation not found: {}", id)));
    }
    Ok(())
}

/// Get a single annotation by ID.
pub fn get_annotation(conn: &Connection, id: &str) -> Result<AnnotationRow, DbError> {
    conn.query_row(
        "SELECT id, paper_id, type, color, comment, selected_text, image_data,
                position_json, page_number, created_date, modified_date,
                COALESCE(source_file, 'paper.pdf')
         FROM annotations WHERE id = ?1",
        params![id],
        |row| {
            Ok(AnnotationRow {
                id: row.get(0)?,
                paper_id: row.get(1)?,
                annotation_type: row.get(2)?,
                color: row.get(3)?,
                comment: row.get(4)?,
                selected_text: row.get(5)?,
                image_data: row.get(6)?,
                position_json: row.get(7)?,
                page_number: row.get(8)?,
                created_date: row.get(9)?,
                modified_date: row.get(10)?,
                source_file: row.get(11)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            DbError::NotFound(format!("Annotation not found: {}", id))
        }
        _ => DbError::Sqlite(e),
    })
}

/// List ALL annotations for a paper across all source files (used by sync).
pub fn list_all_annotations(
    conn: &Connection,
    paper_id: &str,
) -> Result<Vec<AnnotationRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, paper_id, type, color, comment, selected_text, image_data,
                position_json, page_number, created_date, modified_date,
                COALESCE(source_file, 'paper.pdf')
         FROM annotations WHERE paper_id = ?1
         ORDER BY page_number ASC, created_date ASC",
    )?;
    let rows = stmt.query_map(params![paper_id], |row| {
        Ok(AnnotationRow {
            id: row.get(0)?,
            paper_id: row.get(1)?,
            annotation_type: row.get(2)?,
            color: row.get(3)?,
            comment: row.get(4)?,
            selected_text: row.get(5)?,
            image_data: row.get(6)?,
            position_json: row.get(7)?,
            page_number: row.get(8)?,
            created_date: row.get(9)?,
            modified_date: row.get(10)?,
            source_file: row.get(11)?,
        })
    })?;
    let mut annotations = Vec::new();
    for row in rows {
        annotations.push(row?);
    }
    Ok(annotations)
}

/// Insert an annotation with a specific ID (used during sync import).
#[allow(clippy::too_many_arguments)]
pub fn insert_annotation_with_id(
    conn: &Connection,
    id: &str,
    paper_id: &str,
    annotation_type: &str,
    color: &str,
    comment: Option<&str>,
    selected_text: Option<&str>,
    image_data: Option<&str>,
    position_json: &str,
    page_number: i64,
    source_file: Option<&str>,
    created_date: &str,
    modified_date: &str,
) -> Result<AnnotationRow, DbError> {
    let sf = source_file.unwrap_or("paper.pdf");

    conn.execute(
        "INSERT OR IGNORE INTO annotations
         (id, paper_id, type, color, comment, selected_text, image_data,
          position_json, page_number, created_date, modified_date, source_file)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            id,
            paper_id,
            annotation_type,
            color,
            comment,
            selected_text,
            image_data,
            position_json,
            page_number,
            created_date,
            modified_date,
            sf,
        ],
    )?;

    Ok(AnnotationRow {
        id: id.to_string(),
        paper_id: paper_id.to_string(),
        annotation_type: annotation_type.to_string(),
        color: color.to_string(),
        comment: comment.map(String::from),
        selected_text: selected_text.map(String::from),
        image_data: image_data.map(String::from),
        position_json: position_json.to_string(),
        page_number,
        created_date: created_date.to_string(),
        modified_date: modified_date.to_string(),
        source_file: sf.to_string(),
    })
}

pub fn update_annotation_type(
    conn: &Connection,
    id: &str,
    annotation_type: &str,
) -> Result<AnnotationRow, DbError> {
    let now = chrono::Utc::now().to_rfc3339();

    let updated = conn.execute(
        "UPDATE annotations SET type = ?1, modified_date = ?2 WHERE id = ?3",
        params![annotation_type, now, id],
    )?;

    if updated == 0 {
        return Err(DbError::NotFound(format!("Annotation not found: {}", id)));
    }

    conn.query_row(
        "SELECT id, paper_id, type, color, comment, selected_text, image_data,
                position_json, page_number, created_date, modified_date,
                COALESCE(source_file, 'paper.pdf')
         FROM annotations WHERE id = ?1",
        params![id],
        |row| {
            Ok(AnnotationRow {
                id: row.get(0)?,
                paper_id: row.get(1)?,
                annotation_type: row.get(2)?,
                color: row.get(3)?,
                comment: row.get(4)?,
                selected_text: row.get(5)?,
                image_data: row.get(6)?,
                position_json: row.get(7)?,
                page_number: row.get(8)?,
                created_date: row.get(9)?,
                modified_date: row.get(10)?,
                source_file: row.get(11)?,
            })
        },
    )
    .map_err(DbError::Sqlite)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::schema::create_tables(&conn).unwrap();
        // Insert a test paper
        conn.execute(
            "INSERT INTO papers (id, slug, title, added_date, modified_date, dir_path)
             VALUES ('p1', 'test-paper', 'Test Paper', '2026-01-01', '2026-01-01', '/tmp/test')",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_insert_and_list_annotations() {
        let conn = setup_db();
        let position = r#"{"boundingRect":{"x1":0.1,"y1":0.2,"x2":0.9,"y2":0.3,"width":612,"height":792,"pageNumber":1},"rects":[],"pageNumber":1}"#;

        let ann = insert_annotation(
            &conn,
            "p1",
            "highlight",
            "#ffe28f",
            Some("important finding"),
            Some("selected text here"),
            None,
            position,
            1,
            None,
        )
        .unwrap();

        assert_eq!(ann.paper_id, "p1");
        assert_eq!(ann.annotation_type, "highlight");
        assert_eq!(ann.color, "#ffe28f");
        assert_eq!(ann.comment, Some("important finding".to_string()));
        assert_eq!(ann.page_number, 1);
        assert_eq!(ann.source_file, "paper.pdf");

        let annotations = list_annotations(&conn, "p1", None).unwrap();
        assert_eq!(annotations.len(), 1);
        assert_eq!(annotations[0].id, ann.id);
    }

    #[test]
    fn test_annotations_scoped_by_source_file() {
        let conn = setup_db();
        let pos = r#"{"pageNumber":1,"boundingRect":{},"rects":[]}"#;

        insert_annotation(
            &conn,
            "p1",
            "highlight",
            "#ffe28f",
            None,
            None,
            None,
            pos,
            1,
            None,
        )
        .unwrap();

        insert_annotation(
            &conn,
            "p1",
            "highlight",
            "#a8e6a3",
            None,
            None,
            None,
            pos,
            1,
            Some("paper.zh.pdf"),
        )
        .unwrap();

        let default_anns = list_annotations(&conn, "p1", None).unwrap();
        assert_eq!(default_anns.len(), 1);
        assert_eq!(default_anns[0].color, "#ffe28f");

        let zh_anns = list_annotations(&conn, "p1", Some("paper.zh.pdf")).unwrap();
        assert_eq!(zh_anns.len(), 1);
        assert_eq!(zh_anns[0].color, "#a8e6a3");
    }

    #[test]
    fn test_update_annotation_color() {
        let conn = setup_db();
        let position = r#"{"pageNumber":1,"boundingRect":{},"rects":[]}"#;

        let ann = insert_annotation(
            &conn,
            "p1",
            "highlight",
            "#ffe28f",
            None,
            None,
            None,
            position,
            1,
            None,
        )
        .unwrap();

        let updated = update_annotation(&conn, &ann.id, Some("#a8e6a3"), None).unwrap();
        assert_eq!(updated.color, "#a8e6a3");
        assert_eq!(updated.comment, None);
    }

    #[test]
    fn test_update_annotation_comment() {
        let conn = setup_db();
        let position = r#"{"pageNumber":1,"boundingRect":{},"rects":[]}"#;

        let ann = insert_annotation(
            &conn,
            "p1",
            "highlight",
            "#ffe28f",
            None,
            None,
            None,
            position,
            1,
            None,
        )
        .unwrap();

        let updated = update_annotation(&conn, &ann.id, None, Some(Some("new comment"))).unwrap();
        assert_eq!(updated.comment, Some("new comment".to_string()));
    }

    #[test]
    fn test_delete_annotation() {
        let conn = setup_db();
        let position = r#"{"pageNumber":1,"boundingRect":{},"rects":[]}"#;

        let ann = insert_annotation(
            &conn,
            "p1",
            "highlight",
            "#ffe28f",
            None,
            None,
            None,
            position,
            1,
            None,
        )
        .unwrap();

        delete_annotation(&conn, &ann.id).unwrap();
        let annotations = list_annotations(&conn, "p1", None).unwrap();
        assert!(annotations.is_empty());
    }

    #[test]
    fn test_delete_nonexistent_annotation() {
        let conn = setup_db();
        let result = delete_annotation(&conn, "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_annotations_ordered_by_page() {
        let conn = setup_db();
        let pos = r#"{"pageNumber":1,"boundingRect":{},"rects":[]}"#;

        insert_annotation(
            &conn,
            "p1",
            "highlight",
            "#ffe28f",
            None,
            None,
            None,
            pos,
            3,
            None,
        )
        .unwrap();
        insert_annotation(
            &conn,
            "p1",
            "highlight",
            "#a8e6a3",
            None,
            None,
            None,
            pos,
            1,
            None,
        )
        .unwrap();
        insert_annotation(
            &conn,
            "p1",
            "underline",
            "#f5a3b5",
            None,
            None,
            None,
            pos,
            2,
            None,
        )
        .unwrap();

        let annotations = list_annotations(&conn, "p1", None).unwrap();
        assert_eq!(annotations.len(), 3);
        assert_eq!(annotations[0].page_number, 1);
        assert_eq!(annotations[1].page_number, 2);
        assert_eq!(annotations[2].page_number, 3);
    }

    #[test]
    fn test_cascade_delete_on_paper() {
        let conn = setup_db();
        let pos = r#"{"pageNumber":1,"boundingRect":{},"rects":[]}"#;

        insert_annotation(
            &conn,
            "p1",
            "highlight",
            "#ffe28f",
            None,
            None,
            None,
            pos,
            1,
            None,
        )
        .unwrap();

        conn.execute("DELETE FROM papers WHERE id = 'p1'", [])
            .unwrap();

        let annotations = list_annotations(&conn, "p1", None).unwrap();
        assert!(annotations.is_empty());
    }
}
