// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use serde::Deserialize;

use crate::error::MetadataError;

const DBLP_SEARCH_API: &str = "https://dblp.org/search/publ/api";

/// A single DBLP search hit.
#[derive(Debug, Clone)]
pub struct DblpHit {
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub year: Option<String>,
    pub venue: Option<String>,
    pub doi: Option<String>,
    pub url: Option<String>,
}

// ── Raw JSON structures from DBLP API ───────────────────────────────────────

#[derive(Debug, Deserialize)]
struct DblpResponse {
    result: Option<DblpResult>,
}

#[derive(Debug, Deserialize)]
struct DblpResult {
    hits: Option<DblpHits>,
}

#[derive(Debug, Deserialize)]
struct DblpHits {
    hit: Option<Vec<DblpRawHit>>,
}

#[derive(Debug, Deserialize)]
struct DblpRawHit {
    info: Option<DblpInfo>,
}

#[derive(Debug, Deserialize)]
struct DblpInfo {
    title: Option<String>,
    authors: Option<DblpAuthors>,
    year: Option<String>,
    venue: Option<String>,
    doi: Option<String>,
    url: Option<String>,
}

/// DBLP returns `authors` in two forms:
/// - a single object `{ "author": { "text": "..." } }`
/// - an array `{ "author": [ { "text": "..." }, ... ] }`
#[derive(Debug, Deserialize)]
struct DblpAuthors {
    author: Option<DblpAuthorField>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum DblpAuthorField {
    Single(DblpAuthor),
    Multiple(Vec<DblpAuthor>),
}

#[derive(Debug, Deserialize)]
struct DblpAuthor {
    text: Option<String>,
}

/// Search DBLP by query and return up to `limit` hits.
pub async fn search_by_query(query: &str, limit: usize) -> Result<Vec<DblpHit>, MetadataError> {
    let client = reqwest::Client::new();
    let resp = client
        .get(DBLP_SEARCH_API)
        .query(&[("q", query), ("format", "json"), ("h", &limit.to_string())])
        .header("User-Agent", "Zoro/0.1")
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(MetadataError::ApiError {
            status: resp.status().as_u16(),
            message: resp.text().await.unwrap_or_default(),
        });
    }

    let body: DblpResponse = resp
        .json()
        .await
        .map_err(|e| MetadataError::Json(e.to_string()))?;

    let raw_hits = match body.result.and_then(|r| r.hits).and_then(|h| h.hit) {
        Some(h) => h,
        None => return Ok(Vec::new()),
    };

    let mut results = Vec::with_capacity(raw_hits.len());
    for raw in raw_hits {
        let info = match raw.info {
            Some(i) => i,
            None => continue,
        };

        let authors = match info.authors.and_then(|a| a.author) {
            Some(DblpAuthorField::Single(a)) => a.text.into_iter().collect(),
            Some(DblpAuthorField::Multiple(list)) => {
                list.into_iter().filter_map(|a| a.text).collect()
            }
            None => Vec::new(),
        };

        // Clean trailing period from DBLP titles (e.g. "Attention Is All You Need.")
        let title = info.title.map(|t| {
            let t = t.trim().to_string();
            if t.ends_with('.') {
                t[..t.len() - 1].to_string()
            } else {
                t
            }
        });

        results.push(DblpHit {
            title,
            authors,
            year: info.year,
            venue: info.venue,
            doi: info.doi,
            url: info.url,
        });
    }

    Ok(results)
}
