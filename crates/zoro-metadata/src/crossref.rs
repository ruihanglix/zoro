// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::MetadataError;
use serde::Deserialize;

const CROSSREF_API: &str = "https://api.crossref.org/works";
const USER_AGENT: &str = "Zoro/0.1 (https://github.com/ruihanglix/zoro; mailto:zoro@gmail.com)";

#[derive(Debug, Clone, Deserialize)]
pub struct CrossRefResponse {
    pub message: CrossRefWork,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CrossRefWork {
    #[serde(rename = "type")]
    pub work_type: Option<String>,
    pub title: Option<Vec<String>>,
    pub author: Option<Vec<CrossRefAuthor>>,
    #[serde(rename = "container-title")]
    pub container_title: Option<Vec<String>>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub page: Option<String>,
    pub publisher: Option<String>,
    #[serde(rename = "ISSN")]
    pub issn: Option<Vec<String>>,
    #[serde(rename = "ISBN")]
    pub isbn: Option<Vec<String>>,
    #[serde(rename = "DOI")]
    pub doi: Option<String>,
    pub published: Option<CrossRefDate>,
    #[serde(rename = "published-print")]
    pub published_print: Option<CrossRefDate>,
    #[serde(rename = "published-online")]
    pub published_online: Option<CrossRefDate>,
    #[serde(rename = "abstract")]
    pub abstract_text: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CrossRefAuthor {
    pub given: Option<String>,
    pub family: Option<String>,
    pub name: Option<String>,
    pub affiliation: Option<Vec<CrossRefAffiliation>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CrossRefAffiliation {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CrossRefDate {
    #[serde(rename = "date-parts")]
    pub date_parts: Option<Vec<Vec<Option<i32>>>>,
}

impl CrossRefWork {
    /// Extract a published date string (YYYY-MM-DD or YYYY-01-01)
    pub fn published_date(&self) -> Option<String> {
        let date = self
            .published
            .as_ref()
            .or(self.published_print.as_ref())
            .or(self.published_online.as_ref())?;
        let parts = date.date_parts.as_ref()?.first()?;
        let year = (*parts.first()?)?;
        let month = parts.get(1).and_then(|m| *m).unwrap_or(1);
        let day = parts.get(2).and_then(|d| *d).unwrap_or(1);
        Some(format!("{:04}-{:02}-{:02}", year, month, day))
    }
}

/// Fetch metadata from CrossRef API for a DOI.
pub async fn fetch_crossref_metadata(doi: &str) -> Result<CrossRefWork, MetadataError> {
    let url = format!("{}/{}", CROSSREF_API, doi);
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("User-Agent", USER_AGENT)
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

    let body: CrossRefResponse = resp
        .json()
        .await
        .map_err(|e| MetadataError::Json(e.to_string()))?;
    Ok(body.message)
}
