// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::CoreError;
use crate::models::{Author, Paper, ReadStatus};
use uuid::Uuid;

pub fn parse_ris(input: &str) -> Result<Vec<Paper>, CoreError> {
    let mut papers = Vec::new();
    let mut current_fields: Vec<(String, String)> = Vec::new();

    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with("ER  -") {
            if !current_fields.is_empty() {
                if let Ok(paper) = fields_to_paper(&current_fields) {
                    papers.push(paper);
                }
                current_fields.clear();
            }
            continue;
        }

        if line.len() >= 6 && &line[2..6] == "  - " {
            let tag = line[..2].to_string();
            let value = line[6..].to_string();
            current_fields.push((tag, value));
        }
    }

    // Handle case where file doesn't end with ER
    if !current_fields.is_empty() {
        if let Ok(paper) = fields_to_paper(&current_fields) {
            papers.push(paper);
        }
    }

    Ok(papers)
}

/// Map RIS type code to BibTeX-style entry type
fn ris_type_to_entry_type(ris_type: &str) -> String {
    match ris_type {
        "JOUR" | "JFULL" => "article".to_string(),
        "CONF" | "CPAPER" => "inproceedings".to_string(),
        "BOOK" | "WHOLE" => "book".to_string(),
        "CHAP" => "incollection".to_string(),
        "THES" => "phdthesis".to_string(),
        "RPRT" | "REPORT" => "techreport".to_string(),
        "UNPB" => "unpublished".to_string(),
        "GEN" | "ICOMM" => "misc".to_string(),
        other => other.to_lowercase(),
    }
}

/// Map BibTeX-style entry type to RIS type code
fn entry_type_to_ris_type(entry_type: &str) -> &'static str {
    match entry_type {
        "article" => "JOUR",
        "inproceedings" | "conference" => "CONF",
        "book" => "BOOK",
        "incollection" => "CHAP",
        "phdthesis" | "mastersthesis" => "THES",
        "techreport" => "RPRT",
        "unpublished" => "UNPB",
        "misc" => "GEN",
        _ => "GEN",
    }
}

fn fields_to_paper(fields: &[(String, String)]) -> Result<Paper, CoreError> {
    let mut title = String::new();
    let mut authors = Vec::new();
    let mut abstract_text = None;
    let mut doi = None;
    let mut url = None;
    let mut year = None;
    let mut entry_type = None;
    let mut journal = None;
    let mut volume = None;
    let mut issue = None;
    let mut start_page = None;
    let mut end_page = None;
    let mut publisher = None;
    let mut issn_or_isbn = None;

    for (tag, value) in fields {
        match tag.as_str() {
            "TY" => entry_type = Some(ris_type_to_entry_type(value)),
            "TI" | "T1" => title = value.clone(),
            "AU" | "A1" => authors.push(Author {
                name: value.clone(),
                affiliation: None,
                orcid: None,
            }),
            "AB" | "N2" => abstract_text = Some(value.clone()),
            "DO" => doi = Some(value.clone()),
            "UR" => url = Some(value.clone()),
            "PY" | "Y1" => year = Some(value.clone()),
            "JO" | "JF" | "JA" | "T2" => journal = Some(value.clone()),
            "VL" => volume = Some(value.clone()),
            "IS" => issue = Some(value.clone()),
            "SP" => start_page = Some(value.clone()),
            "EP" => end_page = Some(value.clone()),
            "PB" => publisher = Some(value.clone()),
            "SN" => issn_or_isbn = Some(value.clone()),
            _ => {}
        }
    }

    // Combine start and end pages
    let pages = match (start_page, end_page) {
        (Some(sp), Some(ep)) => Some(format!("{}-{}", sp, ep)),
        (Some(sp), None) => Some(sp),
        _ => None,
    };

    // Guess ISSN vs ISBN: if it looks like an ISBN (13 or 10 digits), use isbn
    let (issn, isbn) = match issn_or_isbn {
        Some(ref sn) => {
            let digits: String = sn.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.len() == 13 || digits.len() == 10 {
                (None, Some(sn.clone()))
            } else {
                (Some(sn.clone()), None)
            }
        }
        None => (None, None),
    };

    let now = chrono::Utc::now().to_rfc3339();
    let id = Uuid::new_v4().to_string();
    let identifier = doi.as_deref().unwrap_or(&id);
    let slug = crate::slug_utils::generate_paper_slug(&title, identifier, year.as_deref());

    Ok(Paper {
        id,
        slug,
        title,
        short_title: None,
        authors,
        abstract_text,
        doi,
        arxiv_id: None,
        url,
        pdf_url: None,
        html_url: None,
        thumbnail_url: None,
        published_date: year.map(|y| {
            if y.len() >= 4 {
                format!("{}-01-01", &y[..4])
            } else {
                y
            }
        }),
        added_date: now.clone(),
        modified_date: now,
        source: Some("import".to_string()),
        tags: Vec::new(),
        collections: Vec::new(),
        attachments: Vec::new(),
        notes: Vec::new(),
        read_status: ReadStatus::Unread,
        rating: None,
        extra: serde_json::json!({}),
        entry_type,
        journal,
        volume,
        issue,
        pages,
        publisher,
        issn,
        isbn,
    })
}

pub fn generate_ris(papers: &[Paper]) -> String {
    let mut output = String::new();
    for paper in papers {
        let ris_type = paper
            .entry_type
            .as_deref()
            .map(entry_type_to_ris_type)
            .unwrap_or("JOUR");
        output.push_str(&format!("TY  - {}\n", ris_type));
        output.push_str(&format!("TI  - {}\n", paper.title));

        for author in &paper.authors {
            output.push_str(&format!("AU  - {}\n", author.name));
        }

        if let Some(ref abs) = paper.abstract_text {
            output.push_str(&format!("AB  - {}\n", abs));
        }
        if let Some(ref journal) = paper.journal {
            output.push_str(&format!("JO  - {}\n", journal));
        }
        if let Some(ref volume) = paper.volume {
            output.push_str(&format!("VL  - {}\n", volume));
        }
        if let Some(ref issue) = paper.issue {
            output.push_str(&format!("IS  - {}\n", issue));
        }
        if let Some(ref pages) = paper.pages {
            // Split "start-end" into SP/EP
            if let Some(dash) = pages.find('-') {
                output.push_str(&format!("SP  - {}\n", &pages[..dash]));
                output.push_str(&format!("EP  - {}\n", &pages[dash + 1..]));
            } else {
                output.push_str(&format!("SP  - {}\n", pages));
            }
        }
        if let Some(ref publisher) = paper.publisher {
            output.push_str(&format!("PB  - {}\n", publisher));
        }
        if let Some(ref doi) = paper.doi {
            output.push_str(&format!("DO  - {}\n", doi));
        }
        if let Some(ref url) = paper.url {
            output.push_str(&format!("UR  - {}\n", url));
        }
        if let Some(ref date) = paper.published_date {
            if date.len() >= 4 {
                output.push_str(&format!("PY  - {}\n", &date[..4]));
            }
        }
        if let Some(ref issn) = paper.issn {
            output.push_str(&format!("SN  - {}\n", issn));
        }
        if let Some(ref isbn) = paper.isbn {
            output.push_str(&format!("SN  - {}\n", isbn));
        }
        output.push_str("ER  - \n\n");
    }
    output
}
