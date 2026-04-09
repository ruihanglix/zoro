// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

//! OpenAlex API client for metadata enrichment.
//!
//! OpenAlex is used to fetch paper metadata (title, abstract, authors, etc.)
//! given a DOI or OpenAlex work ID. It also supports author and citation queries.
//!
//! API docs: <https://docs.openalex.org/>
//! Free tier: $1/day ≈ 10k list requests with API key.

use crate::error::SubscriptionError;
use serde::Deserialize;

/// A work (paper) from OpenAlex.
#[derive(Debug, Clone)]
pub struct OpenAlexWork {
    pub openalex_id: String,
    pub doi: Option<String>,
    pub title: Option<String>,
    pub abstract_text: Option<String>,
    pub authors: Vec<OpenAlexAuthor>,
    pub publication_date: Option<String>,
    pub pdf_url: Option<String>,
    pub landing_page_url: Option<String>,
    pub cited_by_count: i32,
}

#[derive(Debug, Clone)]
pub struct OpenAlexAuthor {
    pub name: String,
    pub openalex_id: Option<String>,
    pub orcid: Option<String>,
}

// ── Internal deserialization types ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct OaWorksResponse {
    results: Option<Vec<OaWork>>,
}

#[derive(Debug, Deserialize)]
struct OaWork {
    id: Option<String>,
    doi: Option<String>,
    title: Option<String>,
    #[serde(rename = "abstract_inverted_index")]
    abstract_inverted_index: Option<serde_json::Value>,
    authorships: Option<Vec<OaAuthorship>>,
    publication_date: Option<String>,
    best_oa_location: Option<OaLocation>,
    primary_location: Option<OaLocation>,
    cited_by_count: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct OaAuthorship {
    author: Option<OaAuthor>,
}

#[derive(Debug, Deserialize)]
struct OaAuthor {
    id: Option<String>,
    display_name: Option<String>,
    orcid: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OaLocation {
    pdf_url: Option<String>,
    landing_page_url: Option<String>,
}

/// Reconstruct abstract text from OpenAlex's inverted index format.
fn reconstruct_abstract(inverted_index: &serde_json::Value) -> Option<String> {
    let obj = inverted_index.as_object()?;
    let mut positions: Vec<(usize, &str)> = Vec::new();
    for (word, indices) in obj {
        if let Some(arr) = indices.as_array() {
            for idx in arr {
                if let Some(pos) = idx.as_u64() {
                    positions.push((pos as usize, word.as_str()));
                }
            }
        }
    }
    positions.sort_by_key(|(pos, _)| *pos);
    let words: Vec<&str> = positions.iter().map(|(_, w)| *w).collect();
    if words.is_empty() {
        None
    } else {
        Some(words.join(" "))
    }
}

fn map_oa_work(w: OaWork) -> OpenAlexWork {
    let abstract_text = w
        .abstract_inverted_index
        .as_ref()
        .and_then(reconstruct_abstract);

    let authors: Vec<OpenAlexAuthor> = w
        .authorships
        .unwrap_or_default()
        .into_iter()
        .filter_map(|a| {
            let author = a.author?;
            Some(OpenAlexAuthor {
                name: author.display_name.unwrap_or_default(),
                openalex_id: author.id,
                orcid: author.orcid,
            })
        })
        .collect();

    let pdf_url = w
        .best_oa_location
        .as_ref()
        .and_then(|l| l.pdf_url.clone())
        .or_else(|| w.primary_location.as_ref().and_then(|l| l.pdf_url.clone()));

    let landing_page_url = w
        .primary_location
        .as_ref()
        .and_then(|l| l.landing_page_url.clone())
        .or_else(|| {
            w.best_oa_location
                .as_ref()
                .and_then(|l| l.landing_page_url.clone())
        });

    OpenAlexWork {
        openalex_id: w.id.unwrap_or_default(),
        doi: w.doi,
        title: w.title,
        abstract_text,
        authors,
        publication_date: w.publication_date,
        pdf_url,
        landing_page_url,
        cited_by_count: w.cited_by_count.unwrap_or(0),
    }
}

/// Fetch metadata for papers by their DOIs (batch, up to 50).
pub async fn fetch_works_by_dois(
    dois: &[&str],
    api_key: Option<&str>,
) -> Result<Vec<OpenAlexWork>, SubscriptionError> {
    if dois.is_empty() {
        return Ok(Vec::new());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    // OpenAlex supports OR filter: doi:doi1|doi2|doi3
    let doi_filter = dois.join("|");
    let mut url = format!(
        "https://api.openalex.org/works?filter=doi:{}&per_page=50&select=id,doi,title,abstract_inverted_index,authorships,publication_date,best_oa_location,primary_location,cited_by_count",
        urlencoding::encode(&doi_filter)
    );
    if let Some(key) = api_key {
        url.push_str(&format!("&api_key={}", key));
    }

    let resp = client
        .get(&url)
        .header("User-Agent", "Zoro/0.1.0 (mailto:zoro@example.com)")
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(SubscriptionError::Other(format!(
            "OpenAlex API returned HTTP {}",
            resp.status()
        )));
    }

    let body: OaWorksResponse = resp.json().await?;
    let works: Vec<OpenAlexWork> = body
        .results
        .unwrap_or_default()
        .into_iter()
        .map(map_oa_work)
        .collect();

    Ok(works)
}

/// Fetch a single work by DOI.
pub async fn fetch_work_by_doi(
    doi: &str,
    api_key: Option<&str>,
) -> Result<Option<OpenAlexWork>, SubscriptionError> {
    let works = fetch_works_by_dois(&[doi], api_key).await?;
    Ok(works.into_iter().next())
}
