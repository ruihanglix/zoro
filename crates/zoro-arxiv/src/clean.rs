// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::ArxivError;
use scraper::{Html, Selector};

const DEFAULT_BLOCKLIST: &[&str] = &[
    "header.desktop_header",
    "button#openForm",
    "div.html-header-nav",
    "div.html-header-logo",
    "nav.ltx_page_navbar",
    "footer.ltx_page_footer",
    "footer.arxiv-html-footer",
    "header.arxiv-html-header",
];

/// Collect elements to hide: returns (original_html_snippet, tag_name, existing_style) tuples.
fn collect_elements_to_hide(html: &str, selectors: &[String]) -> Vec<(String, String)> {
    let doc = Html::parse_document(html);
    let mut results = Vec::new();

    for selector_str in selectors {
        let sel = match Selector::parse(selector_str) {
            Ok(s) => s,
            Err(_) => {
                tracing::warn!(selector = %selector_str, "Invalid CSS selector, skipping");
                continue;
            }
        };
        for el in doc.select(&sel) {
            let original_html = el.html();
            // Build the modified opening tag
            let tag_name = el.value().name().to_string();
            let existing_style = el.value().attr("style").unwrap_or("").to_string();
            let separator = if existing_style.is_empty() || existing_style.trim().ends_with(';') {
                ""
            } else {
                ";"
            };
            let new_style = format!("{}{}display: none !important;", existing_style, separator);

            let mut new_tag = format!("<{}", tag_name);
            for attr in el.value().attrs() {
                if attr.0 == "style" {
                    continue;
                }
                new_tag.push_str(&format!(" {}=\"{}\"", attr.0, attr.1));
            }
            new_tag.push_str(&format!(
                " style=\"{}\" hidden data-zotero-hidden=\"true\"",
                new_style
            ));
            new_tag.push('>');

            if let Some(close_bracket) = original_html.find('>') {
                let old_opening = original_html[..=close_bracket].to_string();
                results.push((old_opening, new_tag));
            }
        }
    }

    results
}

/// Clean an HTML string by hiding elements matching the given CSS selectors.
/// Returns the modified HTML and the count of hidden elements.
pub fn clean_html(html: &str, extra_selectors: &[String]) -> Result<(String, usize), ArxivError> {
    let mut selectors: Vec<String> = DEFAULT_BLOCKLIST.iter().map(|s| s.to_string()).collect();
    for s in extra_selectors {
        let trimmed = s.trim();
        if !trimmed.is_empty() && !selectors.contains(&trimmed.to_string()) {
            selectors.push(trimmed.to_string());
        }
    }

    let replacements = collect_elements_to_hide(html, &selectors);
    let mut result = html.to_string();
    let mut hidden = 0;

    for (old_opening, new_tag) in &replacements {
        if result.contains(old_opening.as_str()) {
            result = result.replacen(old_opening.as_str(), new_tag, 1);
            hidden += 1;
        }
    }

    Ok((result, hidden))
}

/// Clean an HTML file in place.
pub async fn clean_html_file(
    path: &std::path::Path,
    extra_selectors: &[String],
) -> Result<usize, ArxivError> {
    let html = tokio::fs::read_to_string(path).await?;
    let (cleaned, count) = clean_html(&html, extra_selectors)?;
    if count > 0 {
        tokio::fs::write(path, &cleaned).await?;
    }
    Ok(count)
}
