// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::ArxivError;
use std::sync::OnceLock;
use tokio::sync::Mutex;

const AR5IV_CSS_URL: &str = "https://raw.githubusercontent.com/dginev/ar5iv-css/main/css/ar5iv.css";
const AR5IV_STYLE_ID: &str = "zotero-ar5iv-css";

static CSS_CACHE: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn cache() -> &'static Mutex<Option<String>> {
    CSS_CACHE.get_or_init(|| Mutex::new(None))
}

async fn fetch_ar5iv_css() -> Result<String, ArxivError> {
    {
        let guard = cache().lock().await;
        if let Some(ref cached) = *guard {
            return Ok(cached.clone());
        }
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_default();

    let resp = client.get(AR5IV_CSS_URL).send().await?;
    if !resp.status().is_success() {
        return Err(ArxivError::CssDownloadFailed(resp.status().as_u16()));
    }
    let text = resp.text().await?;
    if text.trim().is_empty() {
        return Err(ArxivError::CssDownloadFailed(0));
    }

    let mut guard = cache().lock().await;
    *guard = Some(text.clone());
    Ok(text)
}

/// Inject ar5iv CSS into an HTML string for better rendering.
/// Replaces any existing ar5iv style block.
pub async fn fix_html_style(html: &str) -> Result<String, ArxivError> {
    let css_text = fetch_ar5iv_css().await?;

    let mut result = html.to_string();

    // Remove existing ar5iv style if present
    let marker_start = format!("<style id=\"{}\">", AR5IV_STYLE_ID);
    if let Some(start) = result.find(&marker_start) {
        if let Some(end) = result[start..].find("</style>") {
            let remove_end = start + end + "</style>".len();
            result = format!("{}{}", &result[..start], &result[remove_end..]);
        }
    }

    // Also remove by data attribute pattern
    let data_marker = "data-zotero-ar5iv-css=\"true\"";
    if let Some(start) = result.find(data_marker) {
        // Walk back to find <style
        if let Some(style_start) = result[..start].rfind("<style") {
            if let Some(end) = result[style_start..].find("</style>") {
                let remove_end = style_start + end + "</style>".len();
                result = format!("{}{}", &result[..style_start], &result[remove_end..]);
            }
        }
    }

    let style_tag = format!(
        "<style id=\"{}\" data-zotero-ar5iv-css=\"true\">\n{}\n</style>",
        AR5IV_STYLE_ID, css_text
    );

    // Insert before </head> or at the start of the document
    if let Some(pos) = result.find("</head>") {
        result.insert_str(pos, &style_tag);
    } else if let Some(pos) = result.find("<body") {
        result.insert_str(pos, &format!("<head>{}</head>", style_tag));
    } else {
        result = format!("{}{}", style_tag, result);
    }

    Ok(result)
}

/// Fix the style of an HTML file in place.
pub async fn fix_html_file_style(path: &std::path::Path) -> Result<(), ArxivError> {
    let html = tokio::fs::read_to_string(path).await?;
    let fixed = fix_html_style(&html).await?;
    tokio::fs::write(path, &fixed).await?;
    Ok(())
}
