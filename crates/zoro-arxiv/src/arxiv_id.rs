// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use regex::Regex;
use std::sync::LazyLock;

static ARXIV_URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"https?://(?:www\.)?arxiv\.org/(abs|pdf|format)/([^?#\s]+?)(?:\.pdf)?(?:[?#]|$)")
        .unwrap()
});

static ARXIV_ABS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"https?://(?:www\.)?arxiv\.org/abs/([^?#\s]+)").unwrap());

static ARXIV_ID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:arxiv:)?([a-z-]+/\d{7}|\d{4}\.\d{4,5})(v\d+)?\b").unwrap()
});

static ARXIV_DOI_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)10\.48550/arXiv\.(\d{4}\.\d{4,5})(v\d+)?").unwrap());

/// Extract a normalised arXiv abs URL from a raw URL field.
pub fn normalize_arxiv_url(url: &str) -> Option<String> {
    let caps = ARXIV_URL_RE.captures(url)?;
    let raw_id = caps.get(2)?.as_str().trim_end_matches(".pdf");
    Some(format!("https://arxiv.org/abs/{}", raw_id))
}

/// Extract an arXiv ID from free text (e.g. "arXiv:2301.12345v2").
pub fn extract_arxiv_id(text: &str) -> Option<String> {
    let caps = ARXIV_ID_RE.captures(text)?;
    let id = caps.get(1)?.as_str();
    let version = caps.get(2).map(|m| m.as_str()).unwrap_or("");
    Some(format!("{}{}", id, version))
}

/// Extract an arXiv ID from a DOI string.
pub fn extract_arxiv_id_from_doi(doi: &str) -> Option<String> {
    let caps = ARXIV_DOI_RE.captures(doi)?;
    let id = caps.get(1)?.as_str();
    let version = caps.get(2).map(|m| m.as_str()).unwrap_or("");
    Some(format!("{}{}", id, version))
}

pub fn build_abs_url(id: &str) -> String {
    format!("https://arxiv.org/abs/{}", id)
}

pub fn build_html_url(id: &str) -> String {
    format!("https://arxiv.org/html/{}", id)
}

pub fn build_ar5iv_url(id: &str) -> String {
    format!("https://ar5iv.labs.arxiv.org/html/{}", id)
}

/// Get the arXiv ID from an abs URL.
pub fn id_from_abs_url(abs_url: &str) -> Option<String> {
    let caps = ARXIV_ABS_RE.captures(abs_url)?;
    Some(caps.get(1)?.as_str().to_string())
}

/// Try to find an arXiv ID from various paper metadata fields.
pub fn find_arxiv_id(
    arxiv_id: Option<&str>,
    url: Option<&str>,
    doi: Option<&str>,
    extra: Option<&str>,
) -> Option<String> {
    // Direct arxiv_id field
    if let Some(id) = arxiv_id {
        let id = id.trim();
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }

    // URL field
    if let Some(url) = url {
        if let Some(abs_url) = normalize_arxiv_url(url) {
            return id_from_abs_url(&abs_url);
        }
    }

    // DOI field
    if let Some(doi) = doi {
        if let Some(id) = extract_arxiv_id_from_doi(doi) {
            return Some(id);
        }
        if let Some(id) = extract_arxiv_id(doi) {
            return Some(id);
        }
    }

    // Extra / archiveLocation field
    if let Some(extra) = extra {
        if let Some(id) = extract_arxiv_id(extra) {
            return Some(id);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_arxiv_url() {
        assert_eq!(
            normalize_arxiv_url("https://arxiv.org/abs/2301.12345"),
            Some("https://arxiv.org/abs/2301.12345".to_string())
        );
        assert_eq!(
            normalize_arxiv_url("https://arxiv.org/pdf/2301.12345.pdf"),
            Some("https://arxiv.org/abs/2301.12345".to_string())
        );
        assert_eq!(normalize_arxiv_url("https://example.com"), None);
    }

    #[test]
    fn test_extract_arxiv_id() {
        assert_eq!(
            extract_arxiv_id("arXiv:2301.12345v2"),
            Some("2301.12345v2".to_string())
        );
        assert_eq!(
            extract_arxiv_id("some text 2301.12345 more"),
            Some("2301.12345".to_string())
        );
    }

    #[test]
    fn test_extract_from_doi() {
        assert_eq!(
            extract_arxiv_id_from_doi("10.48550/arXiv.2301.12345"),
            Some("2301.12345".to_string())
        );
    }

    #[test]
    fn test_find_arxiv_id() {
        assert_eq!(
            find_arxiv_id(Some("2301.12345"), None, None, None),
            Some("2301.12345".to_string())
        );
        assert_eq!(
            find_arxiv_id(None, Some("https://arxiv.org/abs/2301.12345"), None, None),
            Some("2301.12345".to_string())
        );
        assert_eq!(
            find_arxiv_id(None, None, Some("10.48550/arXiv.2301.12345"), None),
            Some("2301.12345".to_string())
        );
    }
}
