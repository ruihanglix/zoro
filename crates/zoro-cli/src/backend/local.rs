// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::path::PathBuf;
use std::sync::Mutex;

use zoro_core::{bibtex, models, ris};
use zoro_db::queries::{collections, notes, papers, search, tags};
use zoro_db::Database;

use super::{
    Backend, BackendError, CollectionInfo, ExportResult, NoteInfo, PaperInfo, StatusInfo, TagInfo,
};

pub struct LocalBackend {
    db: Mutex<Database>,
    data_dir: PathBuf,
}

impl LocalBackend {
    pub fn new(db: Database, data_dir: PathBuf) -> Self {
        Self {
            db: Mutex::new(db),
            data_dir,
        }
    }

    /// Resolve a paper ID or slug to the actual paper row.
    fn resolve_paper(&self, id_or_slug: &str) -> Result<papers::PaperRow, BackendError> {
        let db = self
            .db
            .lock()
            .map_err(|e| BackendError(format!("DB lock: {}", e)))?;

        // Try by ID first
        match papers::get_paper(&db.conn, id_or_slug) {
            Ok(row) => return Ok(row),
            Err(zoro_db::DbError::NotFound(_)) => {}
            Err(e) => return Err(BackendError(e.to_string())),
        }

        // Try by slug
        papers::get_paper_by_slug(&db.conn, id_or_slug).map_err(|e| BackendError(e.to_string()))
    }

    /// Resolve a collection by name or ID.
    fn resolve_collection(
        &self,
        name_or_id: &str,
    ) -> Result<collections::CollectionRow, BackendError> {
        let db = self
            .db
            .lock()
            .map_err(|e| BackendError(format!("DB lock: {}", e)))?;
        let all = collections::list_collections(&db.conn)?;

        // Single pass: prefer ID match, then name, then slug
        let mut by_name = None;
        let mut by_slug = None;
        for c in &all {
            if c.id == name_or_id {
                return Ok(c.clone());
            }
            if by_name.is_none() && c.name.eq_ignore_ascii_case(name_or_id) {
                by_name = Some(c);
            }
            if by_slug.is_none() && c.slug == name_or_id {
                by_slug = Some(c);
            }
        }

        by_name
            .or(by_slug)
            .cloned()
            .ok_or_else(|| BackendError(format!("Collection not found: {}", name_or_id)))
    }

    fn paper_row_to_info(&self, row: &papers::PaperRow) -> PaperInfo {
        let db = self
            .db
            .lock()
            .map_err(|e| BackendError(format!("DB lock: {}", e)))
            .expect("DB lock poisoned");
        let authors = papers::get_paper_authors(&db.conn, &row.id).unwrap_or_default();
        let authors_display = format_authors(&authors);

        let paper_tags = tags::get_paper_tags(&db.conn, &row.id).unwrap_or_default();
        let paper_collections =
            collections::get_collections_for_paper(&db.conn, &row.id).unwrap_or_default();

        PaperInfo {
            id: row.id.clone(),
            slug: row.slug.clone(),
            title: row.title.clone(),
            authors_display,
            abstract_text: row.abstract_text.clone(),
            doi: row.doi.clone(),
            arxiv_id: row.arxiv_id.clone(),
            url: row.url.clone(),
            pdf_url: row.pdf_url.clone(),
            published_date: row.published_date.clone(),
            added_date: row.added_date.clone(),
            read_status: row.read_status.clone(),
            rating: row.rating,
            tags: paper_tags.iter().map(|t| t.name.clone()).collect(),
            collections: paper_collections.iter().map(|c| c.name.clone()).collect(),
            entry_type: row.entry_type.clone(),
            journal: row.journal.clone(),
            volume: row.volume.clone(),
            issue: row.issue.clone(),
            pages: row.pages.clone(),
            publisher: row.publisher.clone(),
        }
    }

    fn row_to_core_paper(
        &self,
        row: &papers::PaperRow,
        authors: Vec<(String, Option<String>, Option<String>)>,
    ) -> models::Paper {
        models::Paper {
            id: row.id.clone(),
            slug: row.slug.clone(),
            title: row.title.clone(),
            short_title: row.short_title.clone(),
            authors: authors
                .into_iter()
                .map(|(name, aff, orcid)| models::Author {
                    name,
                    affiliation: aff,
                    orcid,
                })
                .collect(),
            abstract_text: row.abstract_text.clone(),
            doi: row.doi.clone(),
            arxiv_id: row.arxiv_id.clone(),
            url: row.url.clone(),
            pdf_url: row.pdf_url.clone(),
            html_url: row.html_url.clone(),
            thumbnail_url: row.thumbnail_url.clone(),
            published_date: row.published_date.clone(),
            added_date: row.added_date.clone(),
            modified_date: row.modified_date.clone(),
            source: row.source.clone(),
            tags: Vec::new(),
            collections: Vec::new(),
            attachments: Vec::new(),
            notes: Vec::new(),
            read_status: models::ReadStatus::Unread,
            rating: row.rating.map(|r| r as u8),
            extra: row
                .extra_json
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_else(|| serde_json::json!({})),
            entry_type: row.entry_type.clone(),
            journal: row.journal.clone(),
            volume: row.volume.clone(),
            issue: row.issue.clone(),
            pages: row.pages.clone(),
            publisher: row.publisher.clone(),
            issn: row.issn.clone(),
            isbn: row.isbn.clone(),
        }
    }
}

impl Backend for LocalBackend {
    fn mode_name(&self) -> &str {
        "Local (direct DB)"
    }

    fn data_dir_display(&self) -> String {
        self.data_dir.display().to_string()
    }

    fn search_papers(&self, query: &str, limit: i64) -> Result<Vec<PaperInfo>, BackendError> {
        let db = self
            .db
            .lock()
            .map_err(|e| BackendError(format!("DB lock: {}", e)))?;
        let rows = search::search_papers(&db.conn, query, limit)?;
        drop(db); // Release before paper_row_to_info re-acquires

        Ok(rows.iter().map(|r| self.paper_row_to_info(r)).collect())
    }

    fn list_papers(
        &self,
        collection: Option<&str>,
        tag: Option<&str>,
        status: Option<&str>,
        limit: i64,
    ) -> Result<Vec<PaperInfo>, BackendError> {
        // Resolve collection name to ID if provided
        let collection_id = if let Some(coll) = collection {
            Some(self.resolve_collection(coll)?.id)
        } else {
            None
        };

        let filter = papers::PaperFilter {
            collection_id,
            tag_name: tag.map(String::from),
            read_status: status.map(String::from),
            search_query: None,
            uncategorized: None,
            sort_by: None,
            sort_order: None,
            limit: Some(limit),
            offset: Some(0),
        };

        let db = self
            .db
            .lock()
            .map_err(|e| BackendError(format!("DB lock: {}", e)))?;
        let rows = papers::list_papers(&db.conn, &filter)?;
        drop(db); // Release before paper_row_to_info re-acquires

        Ok(rows.iter().map(|r| self.paper_row_to_info(r)).collect())
    }

    fn get_paper(&self, id_or_slug: &str) -> Result<PaperInfo, BackendError> {
        let row = self.resolve_paper(id_or_slug)?;
        Ok(self.paper_row_to_info(&row))
    }

    fn add_paper(&self, source: &str) -> Result<PaperInfo, BackendError> {
        // Determine what kind of source this is:
        // - DOI (starts with 10. or contains doi.org)
        // - arXiv ID (e.g. 2301.12345)
        // - URL (starts with http)
        // - File path (everything else, check if file exists)
        let title = source.to_string();
        let mut doi = None;
        let mut arxiv_id = None;
        let mut url = None;

        if source.starts_with("10.") || source.contains("doi.org/") {
            doi = Some(
                source
                    .trim_start_matches("https://doi.org/")
                    .trim_start_matches("http://doi.org/")
                    .to_string(),
            );
        } else if source.contains("arxiv.org/") || is_arxiv_id(source) {
            arxiv_id = Some(extract_arxiv_id(source));
        } else if source.starts_with("http://") || source.starts_with("https://") {
            url = Some(source.to_string());
        } else {
            // Treat as a file path — for now just add with title as filename
            let path = std::path::Path::new(source);
            if !path.exists() {
                return Err(BackendError(format!(
                    "File not found and not recognized as DOI/arXiv/URL: {}",
                    source
                )));
            }
            // Use filename as title placeholder
            let filename = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| source.to_string());
            return self.add_paper_with_metadata(&filename, None, None, Some(source));
        }

        self.add_paper_with_metadata(&title, doi.as_deref(), arxiv_id.as_deref(), url.as_deref())
    }

    fn delete_paper(&self, id_or_slug: &str) -> Result<(), BackendError> {
        let row = self.resolve_paper(id_or_slug)?;
        let db = self
            .db
            .lock()
            .map_err(|e| BackendError(format!("DB lock: {}", e)))?;
        papers::delete_paper(&db.conn, &row.id)?;
        drop(db); // Release before filesystem operations

        // Clean up paper directory
        let papers_dir = self.data_dir.join("library/papers");
        let _ = zoro_storage::paper_dir::delete_paper_dir(&papers_dir, &row.slug);

        Ok(())
    }

    fn open_paper(&self, id_or_slug: &str) -> Result<(), BackendError> {
        let _row = self.resolve_paper(id_or_slug)?;
        // Try to open via deep link or just print info
        // For now, print a message; deep link support requires platform-specific code
        Err(BackendError(
            "Opening papers in the desktop app requires the Zoro app to be running. \
             Use --local flag and the desktop app's connector instead."
                .to_string(),
        ))
    }

    fn list_collections(&self) -> Result<Vec<CollectionInfo>, BackendError> {
        let db = self
            .db
            .lock()
            .map_err(|e| BackendError(format!("DB lock: {}", e)))?;
        let rows = collections::list_collections(&db.conn)?;

        let mut result = Vec::new();
        for c in &rows {
            let count = collections::get_collection_paper_count(&db.conn, &c.id)?;
            result.push(CollectionInfo {
                id: c.id.clone(),
                name: c.name.clone(),
                slug: c.slug.clone(),
                description: c.description.clone(),
                paper_count: count,
            });
        }
        Ok(result)
    }

    fn create_collection(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<CollectionInfo, BackendError> {
        let db = self
            .db
            .lock()
            .map_err(|e| BackendError(format!("DB lock: {}", e)))?;
        let row = collections::create_collection(&db.conn, name, None, description)?;
        Ok(CollectionInfo {
            id: row.id,
            name: row.name,
            slug: row.slug,
            description: row.description,
            paper_count: 0,
        })
    }

    fn add_paper_to_collection(
        &self,
        paper_id_or_slug: &str,
        collection_name_or_id: &str,
    ) -> Result<(), BackendError> {
        let paper = self.resolve_paper(paper_id_or_slug)?;
        let coll = self.resolve_collection(collection_name_or_id)?;
        let db = self
            .db
            .lock()
            .map_err(|e| BackendError(format!("DB lock: {}", e)))?;
        collections::add_paper_to_collection(&db.conn, &paper.id, &coll.id)?;
        Ok(())
    }

    fn remove_paper_from_collection(
        &self,
        paper_id_or_slug: &str,
        collection_name_or_id: &str,
    ) -> Result<(), BackendError> {
        let paper = self.resolve_paper(paper_id_or_slug)?;
        let coll = self.resolve_collection(collection_name_or_id)?;
        let db = self
            .db
            .lock()
            .map_err(|e| BackendError(format!("DB lock: {}", e)))?;
        collections::remove_paper_from_collection(&db.conn, &paper.id, &coll.id)?;
        Ok(())
    }

    fn list_tags(&self) -> Result<Vec<TagInfo>, BackendError> {
        let db = self
            .db
            .lock()
            .map_err(|e| BackendError(format!("DB lock: {}", e)))?;
        let rows = tags::list_tags(&db.conn)?;

        let mut result = Vec::new();
        for t in &rows {
            let count = tags::get_tag_paper_count(&db.conn, &t.id)?;
            result.push(TagInfo {
                id: t.id.clone(),
                name: t.name.clone(),
                color: t.color.clone(),
                paper_count: count,
            });
        }
        Ok(result)
    }

    fn add_tag_to_paper(&self, paper_id_or_slug: &str, tag_name: &str) -> Result<(), BackendError> {
        let paper = self.resolve_paper(paper_id_or_slug)?;
        let db = self
            .db
            .lock()
            .map_err(|e| BackendError(format!("DB lock: {}", e)))?;
        tags::add_tag_to_paper(&db.conn, &paper.id, tag_name, "cli")?;
        Ok(())
    }

    fn remove_tag_from_paper(
        &self,
        paper_id_or_slug: &str,
        tag_name: &str,
    ) -> Result<(), BackendError> {
        let paper = self.resolve_paper(paper_id_or_slug)?;
        let db = self
            .db
            .lock()
            .map_err(|e| BackendError(format!("DB lock: {}", e)))?;
        tags::remove_tag_from_paper(&db.conn, &paper.id, tag_name)?;
        Ok(())
    }

    fn list_notes(&self, paper_id_or_slug: &str) -> Result<Vec<NoteInfo>, BackendError> {
        let paper = self.resolve_paper(paper_id_or_slug)?;
        let db = self
            .db
            .lock()
            .map_err(|e| BackendError(format!("DB lock: {}", e)))?;
        let rows = notes::list_notes(&db.conn, &paper.id)?;
        Ok(rows
            .iter()
            .map(|n| NoteInfo {
                id: n.id.clone(),
                paper_id: n.paper_id.clone(),
                content: n.content.clone(),
                created_date: n.created_date.clone(),
                modified_date: n.modified_date.clone(),
            })
            .collect())
    }

    fn add_note(&self, paper_id_or_slug: &str, content: &str) -> Result<NoteInfo, BackendError> {
        let paper = self.resolve_paper(paper_id_or_slug)?;
        let db = self
            .db
            .lock()
            .map_err(|e| BackendError(format!("DB lock: {}", e)))?;
        let row = notes::insert_note(&db.conn, &paper.id, content)?;
        Ok(NoteInfo {
            id: row.id,
            paper_id: row.paper_id,
            content: row.content,
            created_date: row.created_date,
            modified_date: row.modified_date,
        })
    }

    fn delete_note(&self, note_id: &str) -> Result<(), BackendError> {
        let db = self
            .db
            .lock()
            .map_err(|e| BackendError(format!("DB lock: {}", e)))?;
        notes::delete_note(&db.conn, note_id)?;
        Ok(())
    }

    fn export_paper(
        &self,
        paper_id_or_slug: &str,
        format: &str,
    ) -> Result<ExportResult, BackendError> {
        let row = self.resolve_paper(paper_id_or_slug)?;
        let db = self
            .db
            .lock()
            .map_err(|e| BackendError(format!("DB lock: {}", e)))?;
        let authors = papers::get_paper_authors(&db.conn, &row.id).unwrap_or_default();
        drop(db);

        let core_paper = self.row_to_core_paper(&row, authors);

        let content = match format {
            "bibtex" | "bib" => bibtex::generate_bibtex(&[core_paper]),
            "ris" => ris::generate_ris(&[core_paper]),
            "json" => {
                serde_json::to_string_pretty(&self.paper_row_to_info(&row)).unwrap_or_default()
            }
            _ => {
                return Err(BackendError(format!(
                    "Unknown export format: {}. Supported: bibtex, ris, json",
                    format
                )));
            }
        };

        Ok(ExportResult {
            format: format.to_string(),
            content,
        })
    }

    fn status(&self) -> Result<StatusInfo, BackendError> {
        let db = self
            .db
            .lock()
            .map_err(|e| BackendError(format!("DB lock: {}", e)))?;
        let paper_count = papers::count_papers(&db.conn)?;
        let collection_count = collections::list_collections(&db.conn)?.len() as i64;
        let tag_count = tags::list_tags(&db.conn)?.len() as i64;

        Ok(StatusInfo {
            mode: self.mode_name().to_string(),
            data_dir: self.data_dir_display(),
            paper_count,
            collection_count,
            tag_count,
        })
    }
}

impl LocalBackend {
    fn add_paper_with_metadata(
        &self,
        title: &str,
        doi: Option<&str>,
        arxiv_id: Option<&str>,
        url: Option<&str>,
    ) -> Result<PaperInfo, BackendError> {
        let slug_base = slug::slugify(title);
        let dir_path = format!("papers/{}", slug_base);

        let input = papers::CreatePaperInput {
            slug: slug_base.clone(),
            title: title.to_string(),
            short_title: None,
            abstract_text: None,
            doi: doi.map(String::from),
            arxiv_id: arxiv_id.map(String::from),
            url: url.map(String::from),
            pdf_url: arxiv_id.map(|id| format!("https://arxiv.org/pdf/{}", id)),
            html_url: None,
            thumbnail_url: None,
            published_date: None,
            source: Some("cli".to_string()),
            dir_path,
            extra_json: None,
            entry_type: None,
            journal: None,
            volume: None,
            issue: None,
            pages: None,
            publisher: None,
            issn: None,
            isbn: None,
            added_date: None,
        };

        let db = self
            .db
            .lock()
            .map_err(|e| BackendError(format!("DB lock: {}", e)))?;
        let row = papers::insert_paper(&db.conn, &input)?;

        // Create paper directory
        let papers_dir = self.data_dir.join("library/papers");
        let _ = zoro_storage::paper_dir::create_paper_dir(&papers_dir, &row.slug);
        drop(db);

        Ok(self.paper_row_to_info(&row))
    }
}

fn format_authors(authors: &[(String, Option<String>, Option<String>)]) -> String {
    if authors.is_empty() {
        return String::new();
    }
    if authors.len() == 1 {
        return authors[0].0.clone();
    }
    if authors.len() == 2 {
        return format!("{} and {}", authors[0].0, authors[1].0);
    }
    format!("{} et al.", authors[0].0)
}

fn is_arxiv_id(s: &str) -> bool {
    // Match patterns like 2301.12345 or 2301.12345v2
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 2 {
        return false;
    }
    let yymm = parts[0];
    if yymm.len() != 4 || !yymm.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    // Strip optional version suffix (e.g. "v2") then check remaining is all digits
    let num = parts[1];
    let num_part = if let Some(vi) = num.find('v') {
        let (digits, ver) = num.split_at(vi);
        // version part must be "v" followed by digits
        if !ver[1..].bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
        digits
    } else {
        num
    };
    !num_part.is_empty() && num_part.bytes().all(|b| b.is_ascii_digit())
}

fn extract_arxiv_id(s: &str) -> String {
    // Handle URLs like https://arxiv.org/abs/2301.12345
    if let Some(pos) = s.find("arxiv.org/abs/") {
        return s[pos + 14..].to_string();
    }
    if let Some(pos) = s.find("arxiv.org/pdf/") {
        return s[pos + 14..].trim_end_matches(".pdf").to_string();
    }
    s.to_string()
}
