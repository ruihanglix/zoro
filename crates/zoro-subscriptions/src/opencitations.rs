// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

//! OpenCitations API client for citation tracking.
//!
//! API docs: <https://opencitations.net/index/api/v2>
//! Rate limit: 180 req/min per IP.

use crate::error::SubscriptionError;
use serde::Deserialize;

/// A citation record from OpenCitations.
#[derive(Debug, Clone)]
pub struct CitationRecord {
    /// DOI(s) of the citing paper.
    pub citing_dois: Vec<String>,
    /// DOI(s) of the cited paper.
    pub cited_dois: Vec<String>,
    /// Publication date of the citing paper (YYYY-MM-DD).
    pub creation: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OcCitationRow {
    citing: Option<String>,
    cited: Option<String>,
    creation: Option<String>,
}

/// Extract DOIs from an OpenCitations PID string like
/// "omid:br/06101801781 doi:10.7717/peerj-cs.421 pmid:33817056".
fn extract_dois(pid_str: &str) -> Vec<String> {
    pid_str
        .split_whitespace()
        .filter_map(|part| part.strip_prefix("doi:").map(String::from))
        .collect()
}

/// Fetch all papers that cite the given DOI.
///
/// Returns a list of citation records. Each record contains the DOIs of the
/// citing paper and the creation date.
pub async fn fetch_citations(
    doi: &str,
    access_token: Option<&str>,
) -> Result<Vec<CitationRecord>, SubscriptionError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let url = format!(
        "https://api.opencitations.net/index/v2/citations/doi:{}",
        doi
    );

    let mut req = client.get(&url).header("User-Agent", "Zoro/0.1.0");
    if let Some(token) = access_token {
        req = req.header("authorization", token);
    }

    let resp = req.send().await?;
    if !resp.status().is_success() {
        return Err(SubscriptionError::Other(format!(
            "OpenCitations API returned HTTP {}",
            resp.status()
        )));
    }

    let rows: Vec<OcCitationRow> = resp.json().await?;

    let records: Vec<CitationRecord> = rows
        .into_iter()
        .map(|row| {
            let citing_dois = row.citing.as_deref().map(extract_dois).unwrap_or_default();
            let cited_dois = row.cited.as_deref().map(extract_dois).unwrap_or_default();
            CitationRecord {
                citing_dois,
                cited_dois,
                creation: row.creation,
            }
        })
        .collect();

    Ok(records)
}

/// Get the citation count for a DOI (quick check without fetching all records).
pub async fn citation_count(doi: &str) -> Result<i64, SubscriptionError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    let url = format!(
        "https://api.opencitations.net/index/v2/citation-count/doi:{}",
        doi
    );

    let resp = client
        .get(&url)
        .header("User-Agent", "Zoro/0.1.0")
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(SubscriptionError::Other(format!(
            "OpenCitations API returned HTTP {}",
            resp.status()
        )));
    }

    #[derive(Deserialize)]
    struct CountRow {
        count: Option<String>,
    }

    let rows: Vec<CountRow> = resp.json().await?;
    let count = rows
        .first()
        .and_then(|r| r.count.as_deref())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    Ok(count)
}
