// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::MetadataError;
use serde::Deserialize;

const S2_API: &str = "https://api.semanticscholar.org/graph/v1/paper";

#[derive(Debug, Clone, Deserialize)]
pub struct S2Paper {
    #[serde(rename = "paperId")]
    pub paper_id: Option<String>,
    pub title: Option<String>,
    #[serde(rename = "abstract")]
    pub abstract_text: Option<String>,
    pub year: Option<i32>,
    pub venue: Option<String>,
    #[serde(rename = "publicationDate")]
    pub publication_date: Option<String>,
    #[serde(rename = "externalIds")]
    pub external_ids: Option<serde_json::Value>,
    pub journal: Option<S2Journal>,
    pub authors: Option<Vec<S2Author>>,
    #[serde(rename = "s2FieldsOfStudy")]
    pub s2_fields_of_study: Option<Vec<S2FieldOfStudy>>,
    #[serde(rename = "openAccessPdf")]
    pub open_access_pdf: Option<S2OpenAccessPdf>,
    #[serde(rename = "publicationTypes")]
    pub publication_types: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct S2OpenAccessPdf {
    pub url: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct S2Journal {
    pub name: Option<String>,
    pub volume: Option<String>,
    pub pages: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct S2Author {
    pub name: Option<String>,
    #[serde(rename = "authorId")]
    pub author_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct S2FieldOfStudy {
    pub category: Option<String>,
    pub source: Option<String>,
}

const S2_FIELDS: &str = "title,abstract,year,venue,publicationDate,externalIds,journal,authors,s2FieldsOfStudy,openAccessPdf,publicationTypes";

/// Fetch paper metadata from Semantic Scholar.
///
/// `paper_id` can be: `DOI:10.xxx`, `ArXiv:2301.00001`, `CorpusId:xxx`, or an S2 paper ID.
pub async fn fetch_semantic_scholar(paper_id: &str) -> Result<S2Paper, MetadataError> {
    let url = format!("{}/{}?fields={}", S2_API, paper_id, S2_FIELDS);
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("User-Agent", "Zoro/0.1")
        .send()
        .await?;

    if resp.status() == 404 {
        return Err(MetadataError::NotFound(format!(
            "Paper not found: {}",
            paper_id
        )));
    }
    if !resp.status().is_success() {
        return Err(MetadataError::ApiError {
            status: resp.status().as_u16(),
            message: resp.text().await.unwrap_or_default(),
        });
    }

    let paper: S2Paper = resp
        .json()
        .await
        .map_err(|e| MetadataError::Json(e.to_string()))?;
    Ok(paper)
}

#[derive(Debug, Clone, Deserialize)]
struct S2SearchResponse {
    data: Option<Vec<S2Paper>>,
}

/// Search Semantic Scholar by title and return the best match.
/// Uses relevance search; only returns a result if the top hit title
/// closely matches the query to avoid false positives.
pub async fn search_by_title(title: &str) -> Result<Option<S2Paper>, MetadataError> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/search", S2_API))
        .query(&[("query", title), ("fields", S2_FIELDS), ("limit", "3")])
        .header("User-Agent", "Zoro/0.1")
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(MetadataError::ApiError {
            status: resp.status().as_u16(),
            message: resp.text().await.unwrap_or_default(),
        });
    }

    let body: S2SearchResponse = resp
        .json()
        .await
        .map_err(|e| MetadataError::Json(e.to_string()))?;

    let papers = match body.data {
        Some(d) if !d.is_empty() => d,
        _ => return Ok(None),
    };

    // Only accept the top hit if the title is a close match
    let best = &papers[0];
    if let Some(ref found_title) = best.title {
        if titles_match(title, found_title) {
            return Ok(Some(best.clone()));
        }
    }

    Ok(None)
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
