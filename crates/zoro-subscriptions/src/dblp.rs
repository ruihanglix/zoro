// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

//! DBLP API client for author publication tracking.
//!
//! DBLP provides stable author IDs and excellent CS coverage.
//! API docs: <https://dblp.org/faq/How+to+use+the+dblp+search+API.html>

use crate::error::SubscriptionError;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct DblpSearchResult {
    result: Option<DblpResult>,
}

#[derive(Debug, Deserialize)]
struct DblpResult {
    hits: Option<DblpHits>,
}

#[derive(Debug, Deserialize)]
struct DblpHits {
    hit: Option<Vec<DblpHit>>,
}

#[derive(Debug, Deserialize)]
struct DblpHit {
    info: Option<DblpPubInfo>,
}

#[derive(Debug, Deserialize)]
struct DblpPubInfo {
    title: Option<String>,
    authors: Option<DblpAuthors>,
    venue: Option<String>,
    year: Option<String>,
    doi: Option<String>,
    url: Option<String>,
    #[serde(rename = "type")]
    pub_type: Option<String>,
    key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DblpAuthors {
    author: Option<DblpAuthorList>,
}

/// DBLP returns either a single author object or an array.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum DblpAuthorList {
    Single(DblpAuthorEntry),
    Multiple(Vec<DblpAuthorEntry>),
}

#[derive(Debug, Deserialize)]
struct DblpAuthorEntry {
    text: Option<String>,
}

/// A paper discovered from DBLP.
#[derive(Debug, Clone)]
pub struct DblpPaper {
    pub title: String,
    pub authors: Vec<String>,
    pub venue: Option<String>,
    pub year: Option<String>,
    pub doi: Option<String>,
    pub dblp_url: Option<String>,
    pub dblp_key: Option<String>,
    pub pub_type: Option<String>,
}

/// Search DBLP for an author and return their publications.
///
/// `author_url_id` is the DBLP author URL suffix, e.g. "y/YannLeCun".
/// Alternatively, pass a search query like "Yann LeCun".
pub async fn search_author_publications(
    query: &str,
    max_results: u32,
) -> Result<Vec<DblpPaper>, SubscriptionError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let url = format!(
        "https://dblp.org/search/publ/api?q={}&format=json&h={}",
        urlencoding::encode(query),
        max_results
    );

    let resp: DblpSearchResult = client
        .get(&url)
        .header("User-Agent", "Zoro/0.1.0")
        .send()
        .await?
        .json()
        .await?;

    let hits = resp
        .result
        .and_then(|r| r.hits)
        .and_then(|h| h.hit)
        .unwrap_or_default();

    let papers: Vec<DblpPaper> = hits
        .into_iter()
        .filter_map(|hit| {
            let info = hit.info?;
            let title = info.title?.trim_end_matches('.').to_string();
            let authors = match info.authors.and_then(|a| a.author) {
                Some(DblpAuthorList::Single(a)) => a.text.map(|t| vec![t]).unwrap_or_default(),
                Some(DblpAuthorList::Multiple(arr)) => {
                    arr.into_iter().filter_map(|a| a.text).collect()
                }
                None => Vec::new(),
            };
            Some(DblpPaper {
                title,
                authors,
                venue: info.venue,
                year: info.year,
                doi: info.doi,
                dblp_url: info.url,
                dblp_key: info.key,
                pub_type: info.pub_type,
            })
        })
        .collect();

    Ok(papers)
}

/// Search DBLP for author suggestions (for the "add author" UI).
#[derive(Debug, Clone)]
pub struct DblpAuthorSuggestion {
    pub name: String,
    pub url: String,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DblpAuthorSearchResult {
    result: Option<DblpAuthorResult>,
}

#[derive(Debug, Deserialize)]
struct DblpAuthorResult {
    hits: Option<DblpAuthorHits>,
}

#[derive(Debug, Deserialize)]
struct DblpAuthorHits {
    hit: Option<Vec<DblpAuthorHit>>,
}

#[derive(Debug, Deserialize)]
struct DblpAuthorHit {
    info: Option<DblpAuthorInfo>,
}

#[derive(Debug, Deserialize)]
struct DblpAuthorInfo {
    author: Option<String>,
    url: Option<String>,
    notes: Option<DblpAuthorNotes>,
}

#[derive(Debug, Deserialize)]
struct DblpAuthorNotes {
    note: Option<DblpNoteList>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum DblpNoteList {
    Single(DblpNoteEntry),
    Multiple(Vec<DblpNoteEntry>),
}

#[derive(Debug, Deserialize)]
struct DblpNoteEntry {
    text: Option<String>,
}

pub async fn search_authors(
    query: &str,
    max_results: u32,
) -> Result<Vec<DblpAuthorSuggestion>, SubscriptionError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    let url = format!(
        "https://dblp.org/search/author/api?q={}&format=json&h={}",
        urlencoding::encode(query),
        max_results
    );

    let resp: DblpAuthorSearchResult = client
        .get(&url)
        .header("User-Agent", "Zoro/0.1.0")
        .send()
        .await?
        .json()
        .await?;

    let hits = resp
        .result
        .and_then(|r| r.hits)
        .and_then(|h| h.hit)
        .unwrap_or_default();

    let suggestions: Vec<DblpAuthorSuggestion> = hits
        .into_iter()
        .filter_map(|hit| {
            let info = hit.info?;
            let name = info.author?;
            let url = info.url.unwrap_or_default();
            let notes = info.notes.and_then(|n| match n.note {
                Some(DblpNoteList::Single(e)) => e.text,
                Some(DblpNoteList::Multiple(arr)) => arr.into_iter().filter_map(|e| e.text).next(),
                None => None,
            });
            Some(DblpAuthorSuggestion { name, url, notes })
        })
        .collect();

    Ok(suggestions)
}
