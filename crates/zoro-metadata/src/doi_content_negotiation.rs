// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::MetadataError;
use std::collections::BTreeMap;

#[derive(Debug, Clone, serde::Serialize)]
pub struct HttpDebugInfo {
    pub method: String,
    pub request_url: String,
    pub request_headers: BTreeMap<String, String>,
    pub status_code: u16,
    pub final_url: String,
    pub response_headers: BTreeMap<String, String>,
    pub body: String,
}

/// Fetch BibTeX for a DOI via content negotiation.
///
/// Sends `Accept: application/x-bibtex` to `https://doi.org/{doi}`.
pub async fn fetch_bibtex(doi: &str) -> Result<String, MetadataError> {
    let (text, _) = fetch_doi_content(doi, "application/x-bibtex").await?;
    Ok(text)
}

/// Like `fetch_bibtex` but also returns raw HTTP debug info.
pub async fn fetch_bibtex_debug(doi: &str) -> Result<(String, HttpDebugInfo), MetadataError> {
    fetch_doi_content(doi, "application/x-bibtex").await
}

/// Fetch a formatted citation for a DOI via content negotiation.
///
/// Sends `Accept: text/x-bibliography; style={style}` to `https://doi.org/{doi}`.
///
/// Supported styles: `apa`, `ieee`, `modern-language-association`,
/// `chicago-author-date`, `chicago-fullnote-bibliography`, `vancouver`,
/// `harvard-cite-them-right`, `nature`, `science`, and any CSL style name.
pub async fn fetch_formatted_citation(doi: &str, style: &str) -> Result<String, MetadataError> {
    let accept = format!("text/x-bibliography; style={}", style);
    let (text, _) = fetch_doi_content(doi, &accept).await?;
    Ok(text)
}

/// Like `fetch_formatted_citation` but also returns raw HTTP debug info.
pub async fn fetch_formatted_citation_debug(
    doi: &str,
    style: &str,
) -> Result<(String, HttpDebugInfo), MetadataError> {
    let accept = format!("text/x-bibliography; style={}", style);
    fetch_doi_content(doi, &accept).await
}

async fn fetch_doi_content(
    doi: &str,
    accept: &str,
) -> Result<(String, HttpDebugInfo), MetadataError> {
    let url = format!("https://doi.org/{}", doi);
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?;

    let mut request_headers = BTreeMap::new();
    request_headers.insert("Accept".into(), accept.to_string());
    request_headers.insert("User-Agent".into(), "Zoro/0.1".into());

    let resp = client
        .get(&url)
        .header("Accept", accept)
        .header("User-Agent", "Zoro/0.1")
        .send()
        .await?;

    let status_code = resp.status().as_u16();
    let final_url = resp.url().to_string();
    let response_headers: BTreeMap<String, String> = resp
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("<binary>").to_string()))
        .collect();

    if resp.status() == 404 {
        return Err(MetadataError::NotFound(format!("DOI not found: {}", doi)));
    }
    if !resp.status().is_success() {
        return Err(MetadataError::ApiError {
            status: status_code,
            message: resp.text().await.unwrap_or_default(),
        });
    }

    let body = resp.text().await?.trim().to_string();

    let debug = HttpDebugInfo {
        method: "GET".into(),
        request_url: url,
        request_headers,
        status_code,
        final_url,
        response_headers,
        body: body.clone(),
    };

    Ok((body, debug))
}

/// Map user-facing style names to CSL style identifiers for DOI content negotiation.
pub fn normalize_style_name(style: &str) -> &str {
    match style {
        "apa" => "apa",
        "ieee" => "ieee",
        "mla" => "modern-language-association",
        "chicago" | "chicago-author-date" => "chicago-author-date",
        "chicago-note" | "chicago-fullnote" => "chicago-fullnote-bibliography",
        "vancouver" => "vancouver",
        "harvard" => "harvard-cite-them-right",
        "nature" => "nature",
        "science" => "science",
        other => other,
    }
}
