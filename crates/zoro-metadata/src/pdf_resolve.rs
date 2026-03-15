// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::{openalex, semantic_scholar, unpaywall};

/// Try to resolve an open-access PDF URL using multiple strategies.
///
/// Resolution order (stops at first success):
/// 1. ArXiv — derive URL directly from arXiv ID (no API call)
/// 2. Semantic Scholar — `openAccessPdf` field
/// 3. Unpaywall — `best_oa_location.url_for_pdf` (DOI required)
/// 4. OpenAlex — `best_oa_location.pdf_url` (DOI required)
pub async fn resolve_pdf_url(doi: Option<&str>, arxiv_id: Option<&str>) -> Option<String> {
    // 1. ArXiv: trivial derivation, no network needed
    if let Some(id) = arxiv_id {
        let id = id.trim();
        if !id.is_empty() {
            tracing::info!(arxiv_id = %id, "PDF resolved via arXiv ID");
            return Some(format!("https://arxiv.org/pdf/{}", id));
        }
    }

    // 2. Semantic Scholar
    let s2_id = doi
        .map(|d| format!("DOI:{}", d))
        .or_else(|| arxiv_id.map(|a| format!("ArXiv:{}", a)));

    if let Some(ref id) = s2_id {
        match semantic_scholar::fetch_semantic_scholar(id).await {
            Ok(paper) => {
                if let Some(ref oa) = paper.open_access_pdf {
                    if let Some(ref url) = oa.url {
                        tracing::info!(url = %url, "PDF resolved via Semantic Scholar");
                        return Some(url.clone());
                    }
                }
            }
            Err(e) => {
                tracing::debug!("Semantic Scholar PDF lookup failed: {}", e);
            }
        }
    }

    // Remaining strategies require a DOI
    let doi = doi?;

    // 3. Unpaywall
    match unpaywall::fetch_unpaywall(doi).await {
        Ok(resp) => {
            if let Some(url) = resp.pdf_url() {
                tracing::info!(url = %url, "PDF resolved via Unpaywall");
                return Some(url.to_string());
            }
        }
        Err(e) => {
            tracing::debug!("Unpaywall PDF lookup failed: {}", e);
        }
    }

    // 4. OpenAlex
    match openalex::fetch_openalex(doi).await {
        Ok(work) => {
            if let Some(url) = work.oa_pdf_url() {
                tracing::info!(url = %url, "PDF resolved via OpenAlex");
                return Some(url.to_string());
            }
        }
        Err(e) => {
            tracing::debug!("OpenAlex PDF lookup failed: {}", e);
        }
    }

    tracing::debug!(doi = %doi, "No OA PDF found from any source");
    None
}
