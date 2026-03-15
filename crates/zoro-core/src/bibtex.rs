// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::CoreError;
use crate::models::{Author, Paper, ReadStatus};
use std::collections::HashMap;
use uuid::Uuid;

/// Parse a BibTeX string into a list of Papers
pub fn parse_bibtex(input: &str) -> Result<Vec<Paper>, CoreError> {
    let mut papers = Vec::new();
    let mut remaining = input.trim();

    while let Some(start) = remaining.find('@') {
        remaining = &remaining[start..];

        // Find the entry type
        let type_end = remaining
            .find('{')
            .ok_or_else(|| CoreError::ParseError("Missing '{' in BibTeX entry".to_string()))?;
        let entry_type = remaining[1..type_end].trim().to_lowercase();

        // Find matching closing brace (handling nesting)
        let entry_content = extract_braced_content(&remaining[type_end..])?;

        // Parse the entry
        if let Ok(paper) = parse_single_entry(&entry_type, entry_content) {
            papers.push(paper);
        }

        remaining = &remaining[type_end + entry_content.len() + 2..]; // +2 for { and }
    }

    Ok(papers)
}

fn extract_braced_content(s: &str) -> Result<&str, CoreError> {
    if !s.starts_with('{') {
        return Err(CoreError::ParseError("Expected '{'".to_string()));
    }
    let mut depth = 0;
    for (i, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(&s[1..i]);
                }
            }
            _ => {}
        }
    }
    Err(CoreError::ParseError(
        "Unmatched braces in BibTeX".to_string(),
    ))
}

fn parse_single_entry(entry_type: &str, content: &str) -> Result<Paper, CoreError> {
    let mut fields: HashMap<String, String> = HashMap::new();

    // Extract cite key (first thing before comma)
    let first_comma = content.find(',').unwrap_or(content.len());
    let cite_key = content[..first_comma].trim();
    let field_str = if first_comma < content.len() {
        &content[first_comma + 1..]
    } else {
        ""
    };

    // Parse fields: key = {value} or key = "value"
    let mut remaining = field_str.trim();
    while !remaining.is_empty() {
        // Skip whitespace and commas
        remaining = remaining.trim_start_matches(|c: char| c.is_whitespace() || c == ',');
        if remaining.is_empty() {
            break;
        }

        // Find key
        let eq_pos = match remaining.find('=') {
            Some(p) => p,
            None => break,
        };
        let key = remaining[..eq_pos].trim().to_lowercase();
        remaining = remaining[eq_pos + 1..].trim();

        // Find value
        let (value, rest) = if remaining.starts_with('{') {
            match extract_braced_content(remaining) {
                Ok(v) => (v.to_string(), &remaining[v.len() + 2..]),
                Err(_) => break,
            }
        } else if let Some(stripped) = remaining.strip_prefix('"') {
            let end = stripped.find('"').unwrap_or(stripped.len());
            (stripped[..end].to_string(), &remaining[end + 2..])
        } else {
            let end = remaining.find(',').unwrap_or(remaining.len());
            (remaining[..end].trim().to_string(), &remaining[end..])
        };

        fields.insert(key, value);
        remaining = rest;
    }

    let title = fields.get("title").cloned().unwrap_or_default();
    let now = chrono::Utc::now().to_rfc3339();
    let id = Uuid::new_v4().to_string();

    let authors = fields
        .get("author")
        .map(|a| parse_bibtex_authors(a))
        .unwrap_or_default();

    let year = fields.get("year").cloned();
    let doi = fields.get("doi").cloned();
    let arxiv_id = fields
        .get("eprint")
        .cloned()
        .or_else(|| fields.get("arxivid").cloned());
    let url = fields.get("url").cloned();

    // Citation metadata fields
    let journal = fields
        .get("journal")
        .or_else(|| fields.get("booktitle"))
        .cloned();
    let volume = fields.get("volume").cloned();
    let issue = fields.get("number").cloned();
    let pages = fields.get("pages").cloned();
    let publisher = fields.get("publisher").cloned();
    let issn = fields.get("issn").cloned();
    let isbn = fields.get("isbn").cloned();

    let identifier = doi.as_deref().or(arxiv_id.as_deref()).unwrap_or(&id);

    let slug = crate::slug_utils::generate_paper_slug(&title, identifier, year.as_deref());

    Ok(Paper {
        id,
        slug,
        title,
        short_title: None,
        authors,
        abstract_text: fields.get("abstract").cloned(),
        doi,
        arxiv_id,
        url,
        pdf_url: None,
        html_url: None,
        thumbnail_url: None,
        published_date: year.map(|y| format!("{}-01-01", y)),
        added_date: now.clone(),
        modified_date: now,
        source: Some("import".to_string()),
        tags: Vec::new(),
        collections: Vec::new(),
        attachments: Vec::new(),
        notes: Vec::new(),
        read_status: ReadStatus::Unread,
        rating: None,
        extra: if cite_key.is_empty() {
            serde_json::json!({})
        } else {
            serde_json::json!({ "cite_key": cite_key })
        },
        entry_type: Some(entry_type.to_string()),
        journal,
        volume,
        issue,
        pages,
        publisher,
        issn,
        isbn,
    })
}

fn parse_bibtex_authors(authors_str: &str) -> Vec<Author> {
    authors_str
        .split(" and ")
        .map(|name| Author {
            name: name.trim().to_string(),
            affiliation: None,
            orcid: None,
        })
        .collect()
}

/// Generate a BibTeX string from papers
pub fn generate_bibtex(papers: &[Paper]) -> String {
    let mut output = String::new();
    for paper in papers {
        let cite_key = paper
            .extra
            .get("cite_key")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| paper.slug.replace('-', "_"));
        let entry_type = paper.entry_type.as_deref().unwrap_or("article");
        output.push_str(&format!("@{}{{{},\n", entry_type, cite_key));
        output.push_str(&format!("  title = {{{}}},\n", paper.title));

        if !paper.authors.is_empty() {
            let authors: Vec<&str> = paper.authors.iter().map(|a| a.name.as_str()).collect();
            output.push_str(&format!("  author = {{{}}},\n", authors.join(" and ")));
        }

        if let Some(ref year) = paper.published_date {
            if year.len() >= 4 {
                output.push_str(&format!("  year = {{{}}},\n", &year[..4]));
            }
        }

        if let Some(ref journal) = paper.journal {
            output.push_str(&format!("  journal = {{{}}},\n", journal));
        }

        if let Some(ref volume) = paper.volume {
            output.push_str(&format!("  volume = {{{}}},\n", volume));
        }

        if let Some(ref issue) = paper.issue {
            output.push_str(&format!("  number = {{{}}},\n", issue));
        }

        if let Some(ref pages) = paper.pages {
            output.push_str(&format!("  pages = {{{}}},\n", pages));
        }

        if let Some(ref publisher) = paper.publisher {
            output.push_str(&format!("  publisher = {{{}}},\n", publisher));
        }

        if let Some(ref doi) = paper.doi {
            output.push_str(&format!("  doi = {{{}}},\n", doi));
        }

        if let Some(ref arxiv_id) = paper.arxiv_id {
            output.push_str(&format!("  eprint = {{{}}},\n", arxiv_id));
            output.push_str("  archivePrefix = {arXiv},\n");
        }

        if let Some(ref url) = paper.url {
            output.push_str(&format!("  url = {{{}}},\n", url));
        }

        if let Some(ref abs) = paper.abstract_text {
            output.push_str(&format!("  abstract = {{{}}},\n", abs));
        }

        if let Some(ref issn) = paper.issn {
            output.push_str(&format!("  issn = {{{}}},\n", issn));
        }

        if let Some(ref isbn) = paper.isbn {
            output.push_str(&format!("  isbn = {{{}}},\n", isbn));
        }

        output.push_str("}\n\n");
    }
    output
}
