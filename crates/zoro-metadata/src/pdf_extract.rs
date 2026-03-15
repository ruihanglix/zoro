// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::path::Path;

use crate::MetadataError;

/// Metadata extracted from a local PDF file.
#[derive(Debug, Clone, Default)]
pub struct PdfMetadata {
    pub title: Option<String>,
    pub authors: Option<Vec<String>>,
    pub doi: Option<String>,
    pub subject: Option<String>,
    pub arxiv_id: Option<String>,
}

/// Extract metadata from a PDF file on disk.
///
/// Strategy:
/// 1. Read the PDF Info dictionary (`/Title`, `/Author`, `/Subject`)
/// 2. Scan the first few pages for a DOI using a regex
/// 3. Attempt to find an arXiv ID in the text as well
pub fn extract_pdf_metadata(path: &Path) -> Result<PdfMetadata, MetadataError> {
    let doc = lopdf::Document::load(path)
        .map_err(|e| MetadataError::PdfParse(format!("Failed to load PDF: {}", e)))?;

    let mut meta = PdfMetadata::default();

    // 1. Read Info dictionary metadata
    extract_info_dict(&doc, &mut meta);

    // 2. Extract text from first pages and scan for DOI / arXiv ID
    let text = extract_first_pages_text(&doc, 3);
    if !text.is_empty() {
        if meta.doi.is_none() {
            meta.doi = find_doi(&text);
        }
        if meta.arxiv_id.is_none() {
            meta.arxiv_id = find_arxiv_id(&text);
        }
    }

    Ok(meta)
}

/// Read metadata from the PDF's Info dictionary.
fn extract_info_dict(doc: &lopdf::Document, meta: &mut PdfMetadata) {
    if let Ok(info_ref) = doc.trailer.get(b"Info") {
        let info_obj = if let Ok(reference) = info_ref.as_reference() {
            doc.get_object(reference).ok()
        } else {
            Some(info_ref)
        };

        if let Some(lopdf::Object::Dictionary(ref dict)) = info_obj {
            // Title
            if let Some(title) = get_string_from_dict(dict, b"Title") {
                let trimmed = title.trim().to_string();
                if !trimmed.is_empty() && !is_generic_title(&trimmed) {
                    meta.title = Some(trimmed);
                }
            }

            // Author
            if let Some(author) = get_string_from_dict(dict, b"Author") {
                let trimmed = author.trim().to_string();
                if !trimmed.is_empty() {
                    // Authors may be separated by commas, semicolons, or "and"
                    let authors: Vec<String> = trimmed
                        .split([';', ','])
                        .flat_map(|part| part.split(" and "))
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    if !authors.is_empty() {
                        meta.authors = Some(authors);
                    }
                }
            }

            // Subject
            if let Some(subject) = get_string_from_dict(dict, b"Subject") {
                let trimmed = subject.trim().to_string();
                if !trimmed.is_empty() {
                    meta.subject = Some(trimmed);
                }
            }

            // Some PDFs store DOI in the Info dict under custom keys
            for key in &[b"doi".as_slice(), b"DOI".as_slice()] {
                if meta.doi.is_none() {
                    if let Some(val) = get_string_from_dict(dict, key) {
                        let trimmed = val.trim().to_string();
                        if is_valid_doi(&trimmed) {
                            meta.doi = Some(trimmed);
                        }
                    }
                }
            }
        }
    }
}

/// Check if a title is a generic/auto-generated one (e.g. from LaTeX or word processors).
fn is_generic_title(title: &str) -> bool {
    let lower = title.to_lowercase();
    lower == "untitled"
        || lower == "microsoft word"
        || lower.starts_with("microsoft word -")
        || lower.starts_with("untitled-")
        || lower == "document"
}

/// Extract a UTF-8 string from a PDF dictionary entry.
fn get_string_from_dict(dict: &lopdf::Dictionary, key: &[u8]) -> Option<String> {
    let obj = dict.get(key).ok()?;
    match obj {
        lopdf::Object::String(bytes, _) => {
            // Try UTF-16 BE first (starts with BOM 0xFE 0xFF)
            if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
                let chars: Vec<u16> = bytes[2..]
                    .chunks(2)
                    .filter_map(|chunk| {
                        if chunk.len() == 2 {
                            Some(u16::from_be_bytes([chunk[0], chunk[1]]))
                        } else {
                            None
                        }
                    })
                    .collect();
                String::from_utf16(&chars).ok()
            } else {
                // Try UTF-8, fall back to lossy Latin-1
                Some(String::from_utf8_lossy(bytes).to_string())
            }
        }
        lopdf::Object::Name(name) => Some(String::from_utf8_lossy(name).to_string()),
        _ => None,
    }
}

/// Extract plain text from the first N pages of a PDF.
fn extract_first_pages_text(doc: &lopdf::Document, max_pages: usize) -> String {
    let mut text = String::new();
    let page_count = doc.get_pages().len().min(max_pages);

    for page_num in 1..=page_count {
        if let Ok(page_text) = doc.extract_text(&[page_num as u32]) {
            text.push_str(&page_text);
            text.push('\n');
        }
    }

    text
}

/// Find a DOI in text using a regex pattern.
fn find_doi(text: &str) -> Option<String> {
    let pattern = r#"(?i)(?:doi[:\s]*|https?://(?:dx\.)?doi\.org/)?(10\.\d{4,9}/[^\s,;"'<>\])}]+)"#;
    let re = regex::Regex::new(pattern).ok()?;

    for cap in re.captures_iter(text) {
        if let Some(m) = cap.get(1) {
            let doi = m.as_str().trim_end_matches(['.', ')']);
            if is_valid_doi(doi) {
                return Some(doi.to_string());
            }
        }
    }

    None
}

/// Find an arXiv ID in text.
fn find_arxiv_id(text: &str) -> Option<String> {
    // New-style: 1234.56789 (optionally with vN version)
    let re_new = regex::Regex::new(r"arXiv[:\s]*(\d{4}\.\d{4,5}(?:v\d+)?)").ok()?;
    if let Some(cap) = re_new.captures(text) {
        if let Some(m) = cap.get(1) {
            return Some(m.as_str().to_string());
        }
    }

    // Old-style: category/YYMMNNN
    let re_old = regex::Regex::new(r"arXiv[:\s]*([a-z\-]+/\d{7}(?:v\d+)?)").ok()?;
    if let Some(cap) = re_old.captures(text) {
        if let Some(m) = cap.get(1) {
            return Some(m.as_str().to_string());
        }
    }

    None
}

/// Basic validation that a string looks like a DOI.
fn is_valid_doi(s: &str) -> bool {
    s.starts_with("10.") && s.len() >= 7 && s.contains('/')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_doi_basic() {
        let text = "This paper has DOI: 10.1234/example.2024.001 in it.";
        let doi = find_doi(text);
        assert_eq!(doi, Some("10.1234/example.2024.001".to_string()));
    }

    #[test]
    fn test_find_doi_url() {
        let text = "Available at https://doi.org/10.1038/s41586-023-06747-5";
        let doi = find_doi(text);
        assert_eq!(doi, Some("10.1038/s41586-023-06747-5".to_string()));
    }

    #[test]
    fn test_find_doi_none() {
        let text = "This text has no DOI in it.";
        assert_eq!(find_doi(text), None);
    }

    #[test]
    fn test_find_arxiv_id_new() {
        let text = "Published as arXiv:2301.07041v2";
        let id = find_arxiv_id(text);
        assert_eq!(id, Some("2301.07041v2".to_string()));
    }

    #[test]
    fn test_find_arxiv_id_old() {
        let text = "Available at arXiv: hep-ph/0601234v1";
        let id = find_arxiv_id(text);
        assert_eq!(id, Some("hep-ph/0601234v1".to_string()));
    }

    #[test]
    fn test_find_arxiv_id_none() {
        let text = "No arXiv reference here.";
        assert_eq!(find_arxiv_id(text), None);
    }

    #[test]
    fn test_is_generic_title() {
        assert!(is_generic_title("Untitled"));
        assert!(is_generic_title("Microsoft Word - draft.docx"));
        assert!(!is_generic_title("Attention Is All You Need"));
    }

    #[test]
    fn test_is_valid_doi() {
        assert!(is_valid_doi("10.1234/test"));
        assert!(!is_valid_doi("11.1234/test"));
        assert!(!is_valid_doi("10.12"));
    }
}
