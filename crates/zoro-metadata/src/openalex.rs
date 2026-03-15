// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::MetadataError;
use serde::Deserialize;

const OPENALEX_API: &str = "https://api.openalex.org/works";

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAlexWork {
    pub id: Option<String>,
    pub title: Option<String>,
    #[serde(rename = "type")]
    pub work_type: Option<String>,
    pub publication_date: Option<String>,
    pub biblio: Option<OpenAlexBiblio>,
    pub primary_location: Option<OpenAlexLocation>,
    pub best_oa_location: Option<OpenAlexOALocation>,
    pub open_access: Option<OpenAlexOpenAccess>,
    pub ids: Option<OpenAlexIds>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAlexBiblio {
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub first_page: Option<String>,
    pub last_page: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAlexLocation {
    pub source: Option<OpenAlexSource>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAlexSource {
    pub display_name: Option<String>,
    pub issn_l: Option<String>,
    #[serde(rename = "type")]
    pub source_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAlexOALocation {
    pub pdf_url: Option<String>,
    pub landing_page_url: Option<String>,
    pub is_oa: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAlexOpenAccess {
    pub is_oa: Option<bool>,
    pub oa_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAlexIds {
    pub openalex: Option<String>,
    pub doi: Option<String>,
}

impl OpenAlexWork {
    /// Return the best available OA PDF URL.
    pub fn oa_pdf_url(&self) -> Option<&str> {
        if let Some(ref loc) = self.best_oa_location {
            if let Some(ref url) = loc.pdf_url {
                return Some(url);
            }
        }
        if let Some(ref oa) = self.open_access {
            if let Some(ref url) = oa.oa_url {
                if url.ends_with(".pdf") {
                    return Some(url);
                }
            }
        }
        None
    }
}

/// Fetch metadata from OpenAlex API for a DOI.
pub async fn fetch_openalex(doi: &str) -> Result<OpenAlexWork, MetadataError> {
    let url = format!("{}/doi:{}?mailto=zoro@gmail.com", OPENALEX_API, doi);
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("User-Agent", "Zoro/0.1")
        .send()
        .await?;

    if resp.status() == 404 {
        return Err(MetadataError::NotFound(format!("DOI not found: {}", doi)));
    }
    if !resp.status().is_success() {
        return Err(MetadataError::ApiError {
            status: resp.status().as_u16(),
            message: resp.text().await.unwrap_or_default(),
        });
    }

    let work: OpenAlexWork = resp
        .json()
        .await
        .map_err(|e| MetadataError::Json(e.to_string()))?;
    Ok(work)
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAlexSearchResponse {
    results: Option<Vec<OpenAlexWork>>,
}

/// Search OpenAlex by title and return DOI/metadata if a close match is found.
///
/// OpenAlex has no strict rate limit (only requires `mailto`), making it a
/// good fallback when Semantic Scholar returns 429 errors.
pub async fn search_by_title(title: &str) -> Result<Option<OpenAlexWork>, MetadataError> {
    let client = reqwest::Client::new();
    let resp = client
        .get(OPENALEX_API)
        .query(&[
            ("search", title),
            ("mailto", "zoro@gmail.com"),
            ("per_page", "3"),
        ])
        .header("User-Agent", "Zoro/0.1")
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(MetadataError::ApiError {
            status: resp.status().as_u16(),
            message: resp.text().await.unwrap_or_default(),
        });
    }

    let body: OpenAlexSearchResponse = resp
        .json()
        .await
        .map_err(|e| MetadataError::Json(e.to_string()))?;

    let works = match body.results {
        Some(w) if !w.is_empty() => w,
        _ => return Ok(None),
    };

    // Only accept the top hit if the title is a close match
    let best = &works[0];
    if let Some(ref found_title) = best.title {
        if titles_match(title, found_title) {
            return Ok(Some(best.clone()));
        }
    }

    Ok(None)
}

/// Extract DOI from an OpenAlexWork (strip the "https://doi.org/" prefix if present).
impl OpenAlexWork {
    pub fn extracted_doi(&self) -> Option<String> {
        self.ids
            .as_ref()
            .and_then(|ids| ids.doi.as_ref())
            .map(|d| d.trim_start_matches("https://doi.org/").to_string())
    }
}

/// Fuzzy title comparison: normalize and check containment / high overlap.
fn titles_match(query: &str, candidate: &str) -> bool {
    let norm = |s: &str| {
        s.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let q = norm(query);
    let c = norm(candidate);
    if q.is_empty() || c.is_empty() {
        return false;
    }
    q == c || c.contains(&q) || q.contains(&c)
}
