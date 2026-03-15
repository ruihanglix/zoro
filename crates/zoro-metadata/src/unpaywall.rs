// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::MetadataError;
use serde::Deserialize;

const UNPAYWALL_API: &str = "https://api.unpaywall.org/v2";
const EMAIL: &str = "zoro@gmail.com";

#[derive(Debug, Clone, Deserialize)]
pub struct UnpaywallResponse {
    pub doi: Option<String>,
    pub is_oa: Option<bool>,
    pub best_oa_location: Option<UnpaywallLocation>,
    pub oa_locations: Option<Vec<UnpaywallLocation>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UnpaywallLocation {
    pub url_for_pdf: Option<String>,
    pub url: Option<String>,
    pub host_type: Option<String>,
    pub version: Option<String>,
}

impl UnpaywallResponse {
    /// Return the best available PDF URL from Unpaywall results.
    pub fn pdf_url(&self) -> Option<&str> {
        if let Some(ref best) = self.best_oa_location {
            if let Some(ref url) = best.url_for_pdf {
                return Some(url);
            }
        }
        // Fallback: scan all OA locations for a PDF
        if let Some(ref locations) = self.oa_locations {
            for loc in locations {
                if let Some(ref url) = loc.url_for_pdf {
                    return Some(url);
                }
            }
        }
        None
    }
}

/// Look up open-access PDF availability for a DOI via the Unpaywall API.
pub async fn fetch_unpaywall(doi: &str) -> Result<UnpaywallResponse, MetadataError> {
    let url = format!("{}/{}?email={}", UNPAYWALL_API, doi, EMAIL);
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

    let data: UnpaywallResponse = resp
        .json()
        .await
        .map_err(|e| MetadataError::Json(e.to_string()))?;
    Ok(data)
}
