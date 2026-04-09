// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

//! Semantic Scholar API client (optional, requires user-provided API key).
//!
//! API docs: <https://api.semanticscholar.org/api-docs/graph>
//! Rate limits: 100 req/5min without key, higher with key.

use crate::error::SubscriptionError;
use serde::Deserialize;

/// A paper from Semantic Scholar.
#[derive(Debug, Clone)]
pub struct S2Paper {
    pub paper_id: String,
    pub title: String,
    pub authors: Vec<S2Author>,
    pub abstract_text: Option<String>,
    pub year: Option<i32>,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    pub url: Option<String>,
    pub pdf_url: Option<String>,
    pub citation_count: i32,
    pub publication_date: Option<String>,
}

#[derive(Debug, Clone)]
pub struct S2Author {
    pub author_id: Option<String>,
    pub name: String,
}

/// An author suggestion from Semantic Scholar.
#[derive(Debug, Clone)]
pub struct S2AuthorSuggestion {
    pub author_id: String,
    pub name: String,
    pub paper_count: i32,
    pub citation_count: i32,
    pub affiliations: Vec<String>,
}

// ── Internal deserialization types ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct S2PaperResp {
    #[serde(rename = "paperId")]
    paper_id: Option<String>,
    title: Option<String>,
    authors: Option<Vec<S2AuthorResp>>,
    #[serde(rename = "abstract")]
    abstract_text: Option<String>,
    year: Option<i32>,
    #[serde(rename = "externalIds")]
    external_ids: Option<S2ExternalIds>,
    url: Option<String>,
    #[serde(rename = "openAccessPdf")]
    open_access_pdf: Option<S2OpenAccessPdf>,
    #[serde(rename = "citationCount")]
    citation_count: Option<i32>,
    #[serde(rename = "publicationDate")]
    publication_date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct S2AuthorResp {
    #[serde(rename = "authorId")]
    author_id: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct S2ExternalIds {
    #[serde(rename = "DOI")]
    doi: Option<String>,
    #[serde(rename = "ArXiv")]
    arxiv: Option<String>,
}

#[derive(Debug, Deserialize)]
struct S2OpenAccessPdf {
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct S2PapersResponse {
    data: Option<Vec<S2PaperResp>>,
}

#[derive(Debug, Deserialize)]
struct S2AuthorSearchResponse {
    data: Option<Vec<S2AuthorSearchResp>>,
}

#[derive(Debug, Deserialize)]
struct S2AuthorSearchResp {
    #[serde(rename = "authorId")]
    author_id: Option<String>,
    name: Option<String>,
    #[serde(rename = "paperCount")]
    paper_count: Option<i32>,
    #[serde(rename = "citationCount")]
    citation_count: Option<i32>,
    affiliations: Option<Vec<String>>,
}

fn map_s2_paper(p: S2PaperResp) -> S2Paper {
    let authors: Vec<S2Author> = p
        .authors
        .unwrap_or_default()
        .into_iter()
        .map(|a| S2Author {
            author_id: a.author_id,
            name: a.name.unwrap_or_default(),
        })
        .collect();

    S2Paper {
        paper_id: p.paper_id.unwrap_or_default(),
        title: p.title.unwrap_or_default(),
        authors,
        abstract_text: p.abstract_text,
        year: p.year,
        doi: p.external_ids.as_ref().and_then(|e| e.doi.clone()),
        arxiv_id: p.external_ids.as_ref().and_then(|e| e.arxiv.clone()),
        url: p.url,
        pdf_url: p.open_access_pdf.and_then(|o| o.url),
        citation_count: p.citation_count.unwrap_or(0),
        publication_date: p.publication_date,
    }
}

const PAPER_FIELDS: &str = "paperId,title,authors,abstract,year,externalIds,url,openAccessPdf,citationCount,publicationDate";

fn build_client() -> Result<reqwest::Client, SubscriptionError> {
    Ok(reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?)
}

/// Fetch papers by a specific author (by S2 author ID).
pub async fn fetch_author_papers(
    author_id: &str,
    api_key: &str,
    limit: u32,
) -> Result<Vec<S2Paper>, SubscriptionError> {
    let client = build_client()?;

    let url = format!(
        "https://api.semanticscholar.org/graph/v1/author/{}/papers?fields={}&limit={}",
        author_id, PAPER_FIELDS, limit
    );

    let resp = client
        .get(&url)
        .header("User-Agent", "Zoro/0.1.0")
        .header("x-api-key", api_key)
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(SubscriptionError::Other(format!(
            "Semantic Scholar API returned HTTP {}",
            resp.status()
        )));
    }

    let body: S2PapersResponse = resp.json().await?;
    let papers: Vec<S2Paper> = body
        .data
        .unwrap_or_default()
        .into_iter()
        .map(map_s2_paper)
        .collect();

    Ok(papers)
}

/// Fetch papers that cite a given paper (by S2 paper ID or DOI).
pub async fn fetch_citations(
    paper_id: &str,
    api_key: &str,
    limit: u32,
) -> Result<Vec<S2Paper>, SubscriptionError> {
    let client = build_client()?;

    let url = format!(
        "https://api.semanticscholar.org/graph/v1/paper/{}/citations?fields={}&limit={}",
        paper_id, PAPER_FIELDS, limit
    );

    let resp = client
        .get(&url)
        .header("User-Agent", "Zoro/0.1.0")
        .header("x-api-key", api_key)
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(SubscriptionError::Other(format!(
            "Semantic Scholar API returned HTTP {}",
            resp.status()
        )));
    }

    // Citations endpoint wraps each paper in { "citingPaper": { ... } }
    #[derive(Deserialize)]
    struct CitationWrapper {
        #[serde(rename = "citingPaper")]
        citing_paper: Option<S2PaperResp>,
    }
    #[derive(Deserialize)]
    struct CitationsResponse {
        data: Option<Vec<CitationWrapper>>,
    }

    let body: CitationsResponse = resp.json().await?;
    let papers: Vec<S2Paper> = body
        .data
        .unwrap_or_default()
        .into_iter()
        .filter_map(|w| w.citing_paper.map(map_s2_paper))
        .collect();

    Ok(papers)
}

/// Search for authors by name.
pub async fn search_authors(
    query: &str,
    api_key: &str,
    limit: u32,
) -> Result<Vec<S2AuthorSuggestion>, SubscriptionError> {
    let client = build_client()?;

    let url = format!(
        "https://api.semanticscholar.org/graph/v1/author/search?query={}&fields=authorId,name,paperCount,citationCount,affiliations&limit={}",
        urlencoding::encode(query),
        limit
    );

    let resp = client
        .get(&url)
        .header("User-Agent", "Zoro/0.1.0")
        .header("x-api-key", api_key)
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(SubscriptionError::Other(format!(
            "Semantic Scholar API returned HTTP {}",
            resp.status()
        )));
    }

    let body: S2AuthorSearchResponse = resp.json().await?;
    let suggestions: Vec<S2AuthorSuggestion> = body
        .data
        .unwrap_or_default()
        .into_iter()
        .filter_map(|a| {
            Some(S2AuthorSuggestion {
                author_id: a.author_id?,
                name: a.name.unwrap_or_default(),
                paper_count: a.paper_count.unwrap_or(0),
                citation_count: a.citation_count.unwrap_or(0),
                affiliations: a.affiliations.unwrap_or_default(),
            })
        })
        .collect();

    Ok(suggestions)
}
