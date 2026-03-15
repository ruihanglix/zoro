// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use super::types::{ZoteroCreator, ZoteroItem};
use crate::commands::library::{AddPaperInput, AuthorInput};

/// Convert a Zotero item to a Zoro AddPaperInput.
pub fn zotero_item_to_paper_input(item: &ZoteroItem) -> AddPaperInput {
    let title = item.title.clone().unwrap_or_else(|| "Untitled".to_string());

    // Extract authors from creators
    let authors = extract_authors(item.creators.as_deref().unwrap_or(&[]));

    // Extract tags as labels (stored in extra_json, NOT as sidebar tags).
    // Sidebar tags are user-curated only — external labels from Zotero/arXiv
    // go into extra_json.labels as read-only metadata.
    let labels: Vec<String> = item
        .tags
        .as_ref()
        .map(|t| t.iter().map(|t| t.tag.clone()).collect())
        .unwrap_or_default();

    // Try to extract arxiv_id from URL or extra fields
    let arxiv_id = extract_arxiv_id(item);

    // Build extra_json containing Zotero-specific metadata that doesn't
    // map directly to AddPaperInput fields.
    let mut extra_json = build_extra_json(item);
    if !labels.is_empty() {
        if let serde_json::Value::Object(ref mut map) = extra_json {
            map.insert("labels".to_string(), serde_json::json!(labels));
        }
    }
    let extra_json_str = serde_json::to_string(&extra_json)
        .ok()
        .filter(|s| s != "{}");

    AddPaperInput {
        title,
        short_title: item.short_title.clone(),
        authors,
        abstract_text: item.abstract_note.clone(),
        doi: item.doi.clone(),
        arxiv_id,
        url: item.url.clone(),
        pdf_url: None, // PDFs come via saveAttachment, not as URLs
        html_url: None,
        published_date: item.date.clone(),
        source: Some("zotero-connector".to_string()),
        tags: None, // Tags are user-curated only; Zotero tags stored as labels in extra_json
        extra_json: extra_json_str,
        entry_type: item.item_type.clone(),
        journal: item.publication_title.clone(),
        volume: item.volume.clone(),
        issue: item.issue.clone(),
        pages: item.pages.clone(),
        publisher: item.publisher.clone(),
        issn: item.issn.clone(),
        isbn: item.isbn.clone(),
    }
}

/// Extract author names from Zotero creators.
/// Only includes creators with creatorType "author" (or unspecified).
/// Other types (editor, translator, etc.) are preserved in extra_json.
fn extract_authors(creators: &[ZoteroCreator]) -> Vec<AuthorInput> {
    creators
        .iter()
        .filter(|c| c.creator_type.is_none() || c.creator_type.as_deref() == Some("author"))
        .map(|c| {
            let name = format_creator_name(c);
            AuthorInput {
                name,
                affiliation: None,
            }
        })
        .collect()
}

/// Format a Zotero creator into a display name.
fn format_creator_name(creator: &ZoteroCreator) -> String {
    // If single-field name is provided, use it directly
    if let Some(ref name) = creator.name {
        return name.clone();
    }
    // Otherwise combine firstName + lastName
    match (&creator.first_name, &creator.last_name) {
        (Some(first), Some(last)) => format!("{} {}", first, last),
        (None, Some(last)) => last.clone(),
        (Some(first), None) => first.clone(),
        (None, None) => "Unknown".to_string(),
    }
}

/// Try to extract an arXiv ID from the item's URL or extra fields.
fn extract_arxiv_id(item: &ZoteroItem) -> Option<String> {
    // Check URL for arxiv pattern
    if let Some(ref url) = item.url {
        if let Some(id) = parse_arxiv_url(url) {
            return Some(id);
        }
    }
    // Check extra_fields for "arXiv" or "arxiv" key
    if let Some(val) = item.extra_fields.get("arXiv") {
        if let Some(s) = val.as_str() {
            return Some(s.to_string());
        }
    }
    None
}

/// Parse an arXiv URL to extract the ID.
fn parse_arxiv_url(url: &str) -> Option<String> {
    // Patterns: https://arxiv.org/abs/2301.12345, https://arxiv.org/pdf/2301.12345
    let patterns = ["arxiv.org/abs/", "arxiv.org/pdf/"];
    for pattern in &patterns {
        if let Some(pos) = url.find(pattern) {
            let id_start = pos + pattern.len();
            let id = url[id_start..].split(&['?', '#', '/'][..]).next()?;
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    None
}

/// Build extra_json containing Zotero-specific metadata that doesn't map
/// directly to Zoro fields.
fn build_extra_json(item: &ZoteroItem) -> serde_json::Value {
    let mut extra = serde_json::Map::new();

    if let Some(ref item_type) = item.item_type {
        extra.insert(
            "zotero_item_type".to_string(),
            serde_json::Value::String(item_type.clone()),
        );
    }

    // Collect non-author creators
    if let Some(ref creators) = item.creators {
        let non_authors: Vec<&ZoteroCreator> = creators
            .iter()
            .filter(|c| c.creator_type.is_some() && c.creator_type.as_deref() != Some("author"))
            .collect();
        if !non_authors.is_empty() {
            extra.insert(
                "zotero_creators".to_string(),
                serde_json::to_value(&non_authors).unwrap_or_default(),
            );
        }
    }

    // Journal metadata
    let mut journal = serde_json::Map::new();
    if let Some(ref v) = item.publication_title {
        journal.insert(
            "publicationTitle".to_string(),
            serde_json::Value::String(v.clone()),
        );
    }
    if let Some(ref v) = item.volume {
        journal.insert("volume".to_string(), serde_json::Value::String(v.clone()));
    }
    if let Some(ref v) = item.issue {
        journal.insert("issue".to_string(), serde_json::Value::String(v.clone()));
    }
    if let Some(ref v) = item.pages {
        journal.insert("pages".to_string(), serde_json::Value::String(v.clone()));
    }
    if let Some(ref v) = item.issn {
        journal.insert("ISSN".to_string(), serde_json::Value::String(v.clone()));
    }
    if let Some(ref v) = item.isbn {
        journal.insert("ISBN".to_string(), serde_json::Value::String(v.clone()));
    }
    if let Some(ref v) = item.publisher {
        journal.insert(
            "publisher".to_string(),
            serde_json::Value::String(v.clone()),
        );
    }
    if let Some(ref v) = item.place {
        journal.insert("place".to_string(), serde_json::Value::String(v.clone()));
    }
    if let Some(ref v) = item.language {
        journal.insert("language".to_string(), serde_json::Value::String(v.clone()));
    }
    if let Some(ref v) = item.journal_abbreviation {
        journal.insert(
            "journalAbbreviation".to_string(),
            serde_json::Value::String(v.clone()),
        );
    }
    if !journal.is_empty() {
        extra.insert(
            "zotero_metadata".to_string(),
            serde_json::Value::Object(journal),
        );
    }

    serde_json::Value::Object(extra)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_creator_name_full() {
        let creator = ZoteroCreator {
            first_name: Some("John".to_string()),
            last_name: Some("Doe".to_string()),
            name: None,
            creator_type: Some("author".to_string()),
        };
        assert_eq!(format_creator_name(&creator), "John Doe");
    }

    #[test]
    fn test_format_creator_name_single() {
        let creator = ZoteroCreator {
            first_name: None,
            last_name: None,
            name: Some("OpenAI Research".to_string()),
            creator_type: Some("author".to_string()),
        };
        assert_eq!(format_creator_name(&creator), "OpenAI Research");
    }

    #[test]
    fn test_parse_arxiv_url() {
        assert_eq!(
            parse_arxiv_url("https://arxiv.org/abs/2301.12345"),
            Some("2301.12345".to_string())
        );
        assert_eq!(
            parse_arxiv_url("https://arxiv.org/pdf/2301.12345"),
            Some("2301.12345".to_string())
        );
        assert_eq!(parse_arxiv_url("https://example.com"), None);
    }

    #[test]
    fn test_zotero_item_to_paper_input() {
        let item = ZoteroItem {
            id: Some("abc123".to_string()),
            item_type: Some("journalArticle".to_string()),
            title: Some("Test Paper".to_string()),
            creators: Some(vec![
                ZoteroCreator {
                    first_name: Some("Alice".to_string()),
                    last_name: Some("Smith".to_string()),
                    name: None,
                    creator_type: Some("author".to_string()),
                },
                ZoteroCreator {
                    first_name: Some("Bob".to_string()),
                    last_name: Some("Editor".to_string()),
                    name: None,
                    creator_type: Some("editor".to_string()),
                },
            ]),
            date: Some("2024-01-15".to_string()),
            url: Some("https://arxiv.org/abs/2401.12345".to_string()),
            doi: Some("10.1234/test".to_string()),
            abstract_note: Some("Test abstract".to_string()),
            tags: Some(vec![super::super::types::ZoteroTag {
                tag: "ML".to_string(),
                tag_type: None,
            }]),
            attachments: None,
            notes: None,
            access_date: None,
            publication_title: Some("Nature".to_string()),
            volume: Some("42".to_string()),
            issue: Some("3".to_string()),
            pages: Some("100-115".to_string()),
            issn: None,
            isbn: None,
            publisher: None,
            place: None,
            language: None,
            short_title: None,
            journal_abbreviation: None,
            rights: None,
            series: None,
            series_title: None,
            series_text: None,
            number_of_volumes: None,
            edition: None,
            num_pages: None,
            extra_fields: std::collections::HashMap::new(),
        };

        let input = zotero_item_to_paper_input(&item);
        assert_eq!(input.title, "Test Paper");
        assert_eq!(input.authors.len(), 1); // Only "author" type
        assert_eq!(input.authors[0].name, "Alice Smith");
        assert_eq!(input.doi, Some("10.1234/test".to_string()));
        assert_eq!(input.arxiv_id, Some("2401.12345".to_string()));
        assert_eq!(input.source, Some("zotero-connector".to_string()));
        // Tags should NOT be set — Zotero tags become labels in extra_json
        assert_eq!(input.tags, None);
        // Verify labels are in extra_json
        let extra: serde_json::Value =
            serde_json::from_str(input.extra_json.as_deref().unwrap()).unwrap();
        assert_eq!(extra["labels"], serde_json::json!(["ML"]));
    }
}
