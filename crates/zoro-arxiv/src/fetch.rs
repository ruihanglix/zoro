// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::ArxivError;
use base64::Engine;
use scraper::{Html, Selector};
use std::collections::HashSet;
use std::path::Path;
use tracing;

/// Fetch arXiv HTML for a given arXiv ID and return self-contained HTML
/// with all images and CSS inlined as base64/data URIs.
/// Tries the official arxiv.org/html/ first, falls back to ar5iv.labs.arxiv.org.
pub async fn fetch_arxiv_html(arxiv_id: &str) -> Result<String, ArxivError> {
    let client = build_client();

    let urls = [
        crate::arxiv_id::build_html_url(arxiv_id),
        crate::arxiv_id::build_ar5iv_url(arxiv_id),
    ];

    let (html, html_url) = fetch_html_with_fallback(&client, arxiv_id, &urls).await?;

    tracing::info!(arxiv_id = %arxiv_id, raw_len = html.len(), "Raw HTML fetched");
    let base_url = ensure_directory_url(&html_url);
    tracing::info!(base_url = %base_url, "Base URL for resource resolution");

    let html = inline_images(&client, &html, &base_url).await;
    tracing::info!(arxiv_id = %arxiv_id, len = html.len(), "Images inlined");

    let html = inline_stylesheets(&client, &html, &base_url).await;
    tracing::info!(arxiv_id = %arxiv_id, len = html.len(), "Stylesheets inlined");

    let html = inline_scripts(&html, &base_url);
    tracing::info!(
        arxiv_id = %arxiv_id,
        html_len = html.len(),
        "arXiv HTML fetched and resources inlined"
    );

    Ok(html)
}

/// Try each URL in order, returning the first successful response body and the URL used.
async fn fetch_html_with_fallback(
    client: &reqwest::Client,
    arxiv_id: &str,
    urls: &[String],
) -> Result<(String, String), ArxivError> {
    let mut last_err = None;
    for url in urls {
        tracing::info!(arxiv_id = %arxiv_id, url = %url, "Trying arXiv HTML source");
        match client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let html = resp.text().await?;
                tracing::info!(arxiv_id = %arxiv_id, url = %url, "HTML source succeeded");
                return Ok((html, url.clone()));
            }
            Ok(resp) => {
                tracing::warn!(
                    arxiv_id = %arxiv_id, url = %url, status = %resp.status(),
                    "HTML source returned non-success status"
                );
                last_err = Some(ArxivError::Fetch(format!(
                    "HTTP {} from {}",
                    resp.status(),
                    url
                )));
            }
            Err(e) => {
                tracing::warn!(
                    arxiv_id = %arxiv_id, url = %url, error = %e,
                    "HTML source request failed"
                );
                last_err = Some(ArxivError::Http(e));
            }
        }
    }
    Err(last_err.unwrap_or_else(|| ArxivError::Fetch("No HTML URLs to try".into())))
}

/// Save fetched HTML to a file path.
pub async fn fetch_and_save(arxiv_id: &str, dest: &Path) -> Result<(), ArxivError> {
    let html = fetch_arxiv_html(arxiv_id).await?;
    tokio::fs::write(dest, &html).await?;
    Ok(())
}

fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; Zoro/1.0)")
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .unwrap_or_default()
}

fn ensure_directory_url(url: &str) -> String {
    if url.ends_with('/') {
        return url.to_string();
    }
    if let Some(pos) = url.rfind('/') {
        format!("{}/", &url[..pos])
    } else {
        format!("{}/", url)
    }
}

fn resolve_url(href: &str, base_url: &str) -> Option<String> {
    if href.starts_with("data:") || href.starts_with('#') || href.starts_with("javascript:") {
        return None;
    }
    if href.starts_with("http://") || href.starts_with("https://") || href.starts_with("//") {
        let full = if href.starts_with("//") {
            format!("https:{}", href)
        } else {
            href.to_string()
        };
        return Some(full);
    }
    if let Ok(base) = url::Url::parse(base_url) {
        if let Ok(resolved) = base.join(href) {
            return Some(resolved.to_string());
        }
    }
    None
}

fn guess_mime(url: &str) -> &str {
    let lower = url.to_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".svg") {
        "image/svg+xml"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".ico") {
        "image/x-icon"
    } else {
        "image/png"
    }
}

/// Download a resource and return it as a base64 data URI.
async fn fetch_as_data_uri(
    client: &reqwest::Client,
    url: &str,
    fallback_mime: &str,
) -> Option<String> {
    let resp = client.get(url).send().await.ok()?;
    if !resp.status().is_success() {
        tracing::debug!(url = %url, status = %resp.status(), "Failed to fetch resource");
        return None;
    }
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(';').next().unwrap_or(s).trim().to_string())
        .unwrap_or_else(|| fallback_mime.to_string());
    let bytes = resp.bytes().await.ok()?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Some(format!("data:{};base64,{}", content_type, b64))
}

/// Collect image URLs to inline from the HTML (synchronous parse).
fn collect_image_urls(html: &str, base_url: &str) -> Vec<(String, String, String)> {
    let doc = Html::parse_document(html);
    let img_sel = Selector::parse("img[src]").unwrap();
    let source_sel = Selector::parse("source[srcset]").unwrap();

    let mut urls: Vec<(String, String, String)> = Vec::new(); // (attr_pattern, abs_url, mime)
    let mut seen = HashSet::new();

    for el in doc.select(&img_sel) {
        if let Some(src) = el.value().attr("src") {
            if src.starts_with("data:") || seen.contains(src) {
                continue;
            }
            seen.insert(src.to_string());
            if let Some(abs_url) = resolve_url(src, base_url) {
                let mime = guess_mime(&abs_url).to_string();
                urls.push((src.to_string(), abs_url, mime));
            }
        }
    }

    for el in doc.select(&source_sel) {
        if let Some(srcset) = el.value().attr("srcset") {
            if srcset.starts_with("data:") || seen.contains(srcset) {
                continue;
            }
            let trimmed = srcset.trim();
            let first_url = trimmed.split_whitespace().next().unwrap_or(trimmed);
            let first_url_comma = first_url.trim_end_matches(',');
            if !first_url_comma.contains(',') {
                seen.insert(srcset.to_string());
                if let Some(abs_url) = resolve_url(first_url_comma, base_url) {
                    let mime = guess_mime(&abs_url).to_string();
                    urls.push((srcset.to_string(), abs_url, mime));
                }
            }
        }
    }

    urls
}

/// Inline all `<img>` src and `<source>` srcset as base64 data URIs.
async fn inline_images(client: &reqwest::Client, html: &str, base_url: &str) -> String {
    let urls = collect_image_urls(html, base_url);

    let mut replacements: Vec<(String, String)> = Vec::new();

    for (original_attr, abs_url, mime) in &urls {
        if let Some(data_uri) = fetch_as_data_uri(client, abs_url, mime).await {
            replacements.push((
                format!("=\"{}\"", original_attr),
                format!("=\"{}\"", data_uri),
            ));
            // Handle single-quoted attributes
            replacements.push((
                format!("='{}'", original_attr),
                format!("=\"{}\"", data_uri),
            ));
        }
    }

    let mut result = html.to_string();
    for (old, new) in &replacements {
        result = result.replace(old, new);
    }
    result
}

/// Resolve relative URLs inside CSS text (url(...) and @import).
fn resolve_css_urls(css: &str, css_base_url: &str) -> String {
    let url_re = regex::Regex::new(r#"url\(\s*['"]?([^'")]+?)['"]?\s*\)"#).unwrap();
    url_re
        .replace_all(css, |caps: &regex::Captures| {
            let raw = caps[1].trim();
            if raw.starts_with("data:")
                || raw.starts_with("http://")
                || raw.starts_with("https://")
                || raw.starts_with("//")
                || raw.starts_with('#')
            {
                return caps[0].to_string();
            }
            match resolve_url(raw, css_base_url) {
                Some(resolved) => format!("url(\"{}\")", resolved),
                None => caps[0].to_string(),
            }
        })
        .to_string()
}

/// Fetch a CSS file and recursively inline its @import statements.
async fn fetch_and_inline_css(
    client: &reqwest::Client,
    css_url: &str,
    visited: &mut HashSet<String>,
) -> Option<String> {
    if visited.contains(css_url) {
        tracing::debug!(css_url = %css_url, "CSS already visited, skipping");
        return None;
    }
    visited.insert(css_url.to_string());

    tracing::info!(css_url = %css_url, "Fetching CSS");
    let resp = client.get(css_url).send().await.ok()?;
    if !resp.status().is_success() {
        tracing::warn!(css_url = %css_url, status = %resp.status(), "CSS fetch failed");
        return None;
    }
    let css_text = resp.text().await.ok()?;
    tracing::info!(css_url = %css_url, css_len = css_text.len(), "CSS fetched OK");
    let css_base = ensure_directory_url(css_url);
    let resolved = resolve_css_urls(&css_text, &css_base);

    let import_re =
        regex::Regex::new(r#"@import\s+(?:url\()?\s*['"]?([^'")\s]+)['"]?\s*\)?\s*[^;]*;"#)
            .unwrap();

    let mut result = String::new();
    let mut last = 0;
    for caps in import_re.captures_iter(&resolved) {
        let full_match = caps.get(0).unwrap();
        result.push_str(&resolved[last..full_match.start()]);
        last = full_match.end();

        let import_path = &caps[1];
        if let Some(import_url) = resolve_url(import_path, &css_base) {
            if let Some(imported) =
                Box::pin(fetch_and_inline_css(client, &import_url, visited)).await
            {
                result.push_str(&format!("\n/* inlined: {} */\n{}\n", import_url, imported));
                continue;
            }
        }
        result.push_str(full_match.as_str());
    }
    result.push_str(&resolved[last..]);
    Some(result)
}

/// Collect stylesheet hrefs from <link> tags (synchronous parse).
fn collect_stylesheet_hrefs(html: &str, base_url: &str) -> Vec<(String, String)> {
    let doc = Html::parse_document(html);
    let link_sel = Selector::parse("link[rel]").unwrap();

    let mut results: Vec<(String, String)> = Vec::new(); // (href, css_url)
    let mut seen = HashSet::new();

    for el in doc.select(&link_sel) {
        let rel = el.value().attr("rel").unwrap_or("").to_lowercase();
        let is_stylesheet = rel.contains("stylesheet")
            || (rel.contains("preload")
                && el.value().attr("as").map(|a| a.to_lowercase()) == Some("style".to_string()));
        if !is_stylesheet {
            continue;
        }
        let href = match el.value().attr("href") {
            Some(h) => h,
            None => continue,
        };
        if seen.contains(href) {
            continue;
        }
        seen.insert(href.to_string());
        if let Some(css_url) = resolve_url(href, base_url) {
            tracing::debug!(href = %href, css_url = %css_url, "Found stylesheet link");
            results.push((href.to_string(), css_url));
        }
    }

    results
}

/// Replace `<link rel="stylesheet">` tags with inline `<style>` blocks.
/// Uses regex to find the exact `<link ...>` tag in the raw HTML by its href.
async fn inline_stylesheets(client: &reqwest::Client, html: &str, base_url: &str) -> String {
    let hrefs = collect_stylesheet_hrefs(html, base_url);
    tracing::info!(count = hrefs.len(), "Stylesheet links to inline");

    let mut visited = HashSet::new();
    let mut result = html.to_string();

    for (href, css_url) in &hrefs {
        let css_text = match fetch_and_inline_css(client, css_url, &mut visited).await {
            Some(text) => text,
            None => {
                tracing::warn!(css_url = %css_url, "Failed to fetch CSS");
                continue;
            }
        };
        tracing::info!(href = %href, css_len = css_text.len(), "CSS fetched, inlining");

        let escaped_href = regex::escape(href);
        let link_re = regex::Regex::new(&format!(
            r#"<link\b[^>]*\bhref\s*=\s*["']{}["'][^>]*/?\s*>"#,
            escaped_href
        ))
        .unwrap();

        let style_tag = format!("<style data-inlined-from=\"{}\">{}</style>", href, css_text);
        let new_result = link_re.replace(&result, style_tag.as_str()).to_string();
        if new_result.len() != result.len() {
            tracing::info!(href = %href, "Successfully replaced <link> with <style>");
        } else {
            tracing::warn!(href = %href, "Regex did not match any <link> tag in HTML");
        }
        result = new_result;
    }

    result
}

/// Remove external `<script src="...">` tags (they won't work offline).
fn inline_scripts(html: &str, _base_url: &str) -> String {
    let script_re =
        regex::Regex::new(r#"<script\b[^>]*\bsrc\s*=\s*["'][^"']+["'][^>]*>\s*</script>"#).unwrap();
    script_re
        .replace_all(html, "<!-- script removed -->")
        .to_string()
}
