// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::SubscriptionError;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use tracing::info;

const BASE_URL: &str = "https://papers.cool";
const USER_AGENT: &str = "Zoro/0.1.0";

// ── Public types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PapersCoolPaper {
    pub external_id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub abstract_text: Option<String>,
    pub categories: Vec<PapersCoolCategory>,
    pub published_date: Option<String>,
    pub pdf_url: Option<String>,
    pub abs_url: Option<String>,
    pub papers_cool_url: String,
    pub pdf_opens: i32,
    pub kimi_opens: i32,
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PapersCoolCategory {
    pub code: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PapersCoolPage {
    pub title: String,
    pub total: i32,
    pub papers: Vec<PapersCoolPaper>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PapersCoolIndex {
    pub arxiv_groups: Vec<ArxivGroup>,
    pub venues: Vec<VenueConference>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArxivGroup {
    pub name: String,
    pub categories: Vec<ArxivCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArxivCategory {
    pub code: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VenueConference {
    pub name: String,
    pub editions: Vec<VenueEdition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VenueEdition {
    pub key: String,
    pub year: String,
    pub groups: Vec<VenueGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VenueGroup {
    pub name: String,
    pub query: String,
}

// ── Client ──────────────────────────────────────────────────────────────────

pub struct PapersCool {
    client: reqwest::Client,
}

impl PapersCool {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub async fn fetch_index(&self) -> Result<PapersCoolIndex, SubscriptionError> {
        info!("Fetching papers.cool index");
        let html = self.fetch_html(BASE_URL).await?;
        parse_index(&html)
    }

    pub async fn browse_arxiv(
        &self,
        category: &str,
        date: Option<&str>,
    ) -> Result<PapersCoolPage, SubscriptionError> {
        let mut url = format!("{}/arxiv/{}?show=200", BASE_URL, category);
        if let Some(d) = date {
            url = format!("{}/arxiv/{}?date={}&show=200", BASE_URL, category, d);
        }
        info!("Browsing papers.cool arXiv: {}", url);
        let html = self.fetch_html(&url).await?;
        parse_papers_page(&html)
    }

    pub async fn browse_venue(
        &self,
        venue_key: &str,
        group: Option<&str>,
    ) -> Result<PapersCoolPage, SubscriptionError> {
        let mut url = format!("{}/venue/{}", BASE_URL, venue_key);
        if let Some(g) = group {
            url = format!("{}?group={}", url, urlencoding::encode(g));
        }
        info!("Browsing papers.cool venue: {}", url);
        let html = self.fetch_html(&url).await?;
        parse_papers_page(&html)
    }

    pub async fn search(&self, query: &str) -> Result<PapersCoolPage, SubscriptionError> {
        let url = format!(
            "{}/arxiv/search?query={}&branch=arxiv",
            BASE_URL,
            urlencoding::encode(query)
        );
        info!("Searching papers.cool: {}", url);
        let html = self.fetch_html(&url).await?;
        parse_papers_page(&html)
    }

    async fn fetch_html(&self, url: &str) -> Result<String, SubscriptionError> {
        let response = self
            .client
            .get(url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await?;
        let text = response.text().await?;
        Ok(text)
    }
}

impl Default for PapersCool {
    fn default() -> Self {
        Self::new()
    }
}

// ── HTML parsing — paper list pages ─────────────────────────────────────────

fn parse_papers_page(html: &str) -> Result<PapersCoolPage, SubscriptionError> {
    let document = Html::parse_document(html);

    let title = document
        .select(&sel("h1"))
        .next()
        .map(|el| el.text().collect::<String>().trim().to_string())
        .unwrap_or_default();

    let total = parse_total(&document);

    let paper_sel = sel("div.panel.paper");
    let title_link_sel = sel("a.title-link");
    let title_pdf_sel = sel("a.title-pdf");
    let title_kimi_sel = sel("a.title-kimi");
    let authors_sel = sel("p.authors");
    let author_sel = sel("a.author");
    let summary_sel = sel("p.summary");
    let subjects_sel = sel("p.subjects");
    let subject_link_sel = sel("a[class^='subject-']");
    let date_sel = sel("p.date");
    let date_data_sel = sel("span.date-data");
    let sup_sel = sel("sup");

    let mut papers = Vec::new();

    for paper_el in document.select(&paper_sel) {
        let external_id = paper_el.value().id().unwrap_or_default().to_string();

        if external_id.is_empty() {
            continue;
        }

        let paper_title = paper_el
            .select(&title_link_sel)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        let papers_cool_href = paper_el
            .select(&title_link_sel)
            .next()
            .and_then(|el| el.value().attr("href"))
            .unwrap_or_default();

        let pdf_url = paper_el
            .select(&title_pdf_sel)
            .next()
            .and_then(|el| el.value().attr("data"))
            .map(String::from);

        let pdf_opens = paper_el
            .select(&title_pdf_sel)
            .next()
            .and_then(|el| el.select(&sup_sel).next())
            .map(|el| {
                el.text()
                    .collect::<String>()
                    .trim()
                    .parse::<i32>()
                    .unwrap_or(0)
            })
            .unwrap_or(0);

        let kimi_opens = paper_el
            .select(&title_kimi_sel)
            .next()
            .and_then(|el| el.select(&sup_sel).next())
            .map(|el| {
                el.text()
                    .collect::<String>()
                    .trim()
                    .parse::<i32>()
                    .unwrap_or(0)
            })
            .unwrap_or(0);

        let authors: Vec<String> = paper_el
            .select(&authors_sel)
            .next()
            .map(|p| {
                p.select(&author_sel)
                    .map(|a| a.text().collect::<String>().trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        let abstract_text = paper_el
            .select(&summary_sel)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty());

        let categories: Vec<PapersCoolCategory> = paper_el
            .select(&subjects_sel)
            .next()
            .map(|p| {
                p.select(&subject_link_sel)
                    .filter_map(|a| {
                        let name = a.text().collect::<String>().trim().to_string();
                        let href = a.value().attr("href").unwrap_or_default();
                        extract_category_code(href).map(|code| PapersCoolCategory { code, name })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let published_date = paper_el
            .select(&date_sel)
            .next()
            .and_then(|p| p.select(&date_data_sel).next())
            .map(|el| el.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty());

        let abs_url = derive_abs_url(&external_id, &pdf_url);

        let keywords: Vec<String> = paper_el
            .value()
            .attr("keywords")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let papers_cool_url = format!("{}{}", BASE_URL, papers_cool_href);

        papers.push(PapersCoolPaper {
            external_id,
            title: paper_title,
            authors,
            abstract_text,
            categories,
            published_date,
            pdf_url,
            abs_url,
            papers_cool_url,
            pdf_opens,
            kimi_opens,
            keywords,
        });
    }

    Ok(PapersCoolPage {
        title,
        total,
        papers,
    })
}

// ── HTML parsing — home page index ──────────────────────────────────────────

fn parse_index(html: &str) -> Result<PapersCoolIndex, SubscriptionError> {
    let document = Html::parse_document(html);

    let h2_sel = sel("h2");
    let link_sel = sel("a");

    let mut arxiv_groups: Vec<ArxivGroup> = Vec::new();
    let mut venues: Vec<VenueConference> = Vec::new();
    let mut in_venue_section = false;
    let mut current_arxiv_group: Option<ArxivGroup> = None;
    let mut current_venue: Option<VenueConference> = None;

    // The home page has a flat list of <h2> headings for arXiv groups,
    // followed by a "Venue" section with conference <h2>s.
    // Each section heading is followed by links.
    // We iterate through the body's children to build the tree.

    // Simpler approach: iterate over all <a> links, categorize by href pattern.
    for element in document.select(&link_sel) {
        let href = element.value().attr("href").unwrap_or_default();
        let text = element.text().collect::<String>().trim().to_string();

        if text.is_empty() || href.is_empty() {
            continue;
        }

        if href.starts_with("/arxiv/") && !href.contains("search") {
            let code = href.trim_start_matches("/arxiv/");
            if code.is_empty() || code.contains('?') {
                continue;
            }

            let group_name = detect_arxiv_group(code);

            let needs_new_group = current_arxiv_group
                .as_ref()
                .is_none_or(|g| g.name != group_name);

            if needs_new_group {
                if let Some(g) = current_arxiv_group.take() {
                    arxiv_groups.push(g);
                }
                current_arxiv_group = Some(ArxivGroup {
                    name: group_name.to_string(),
                    categories: Vec::new(),
                });
            }

            if let Some(ref mut g) = current_arxiv_group {
                // Avoid duplicates (main page lists each category twice)
                if !g.categories.iter().any(|c| c.code == code) {
                    g.categories.push(ArxivCategory {
                        code: code.to_string(),
                        name: text,
                    });
                }
            }
        } else if href.starts_with("/venue/") {
            in_venue_section = true;
            let path = href.trim_start_matches("/venue/");
            if path.is_empty() {
                continue;
            }

            // Check if this is a group link (has ?group=) or an edition link
            if let Some(q_pos) = path.find('?') {
                // Group link like "/venue/AAAI.2025?group=Computer%20Vision"
                let edition_key = &path[..q_pos];
                let group_query = &path[q_pos + 1..];
                let group_name = group_query.strip_prefix("group=").unwrap_or(group_query);
                let group_name_decoded = urlencoding::decode(group_name)
                    .unwrap_or_default()
                    .to_string();

                if let Some(ref mut venue) = current_venue {
                    if let Some(edition) = venue.editions.iter_mut().find(|e| e.key == edition_key)
                    {
                        edition.groups.push(VenueGroup {
                            name: group_name_decoded,
                            query: group_name.to_string(),
                        });
                    }
                }
            } else {
                // Edition link like "/venue/AAAI.2025"
                let conf_name = extract_venue_conference_name(path);

                let needs_new_venue = current_venue.as_ref().is_none_or(|v| v.name != conf_name);

                if needs_new_venue {
                    if let Some(v) = current_venue.take() {
                        venues.push(v);
                    }
                    current_venue = Some(VenueConference {
                        name: conf_name.to_string(),
                        editions: Vec::new(),
                    });
                }

                let year = extract_venue_year(path);
                if let Some(ref mut venue) = current_venue {
                    if !venue.editions.iter().any(|e| e.key == path) {
                        venue.editions.push(VenueEdition {
                            key: path.to_string(),
                            year: year.to_string(),
                            groups: Vec::new(),
                        });
                    }
                }
            }
        }
    }

    // Flush remaining groups
    if let Some(g) = current_arxiv_group {
        arxiv_groups.push(g);
    }
    if let Some(v) = current_venue {
        venues.push(v);
    }

    // If we didn't parse venues from link walking, check for <h2> headings
    // that appear in the venue section (this is a fallback).
    if in_venue_section && venues.is_empty() {
        for h2 in document.select(&h2_sel) {
            let text = h2.text().collect::<String>().trim().to_string();
            if !text.is_empty() {
                // Just log — the main parsing above should catch venues
                tracing::debug!("Venue heading found: {}", text);
            }
        }
    }

    info!(
        "Parsed papers.cool index: {} arXiv groups, {} venues",
        arxiv_groups.len(),
        venues.len()
    );

    Ok(PapersCoolIndex {
        arxiv_groups,
        venues,
    })
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn sel(selector: &str) -> Selector {
    Selector::parse(selector).expect("invalid CSS selector")
}

fn parse_total(document: &Html) -> i32 {
    let info_sel = sel("p.info");
    document
        .select(&info_sel)
        .next()
        .map(|el| {
            let text = el.text().collect::<String>();
            // Look for "Total: 189" pattern
            text.split("Total:")
                .nth(1)
                .and_then(|s| s.split_whitespace().next())
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(0)
        })
        .unwrap_or(0)
}

/// Extract category code from href like "/arxiv/cs.AI" → "cs.AI"
/// or "/venue/ICLR.2025?group=Oral" → None (not an arXiv category)
fn extract_category_code(href: &str) -> Option<String> {
    if href.starts_with("/arxiv/") {
        let code = href.trim_start_matches("/arxiv/");
        if !code.is_empty() && !code.contains('?') {
            return Some(code.to_string());
        }
    }
    if href.starts_with("/venue/") {
        let path = href.trim_start_matches("/venue/");
        if let Some(q) = path.find('?') {
            let group_part = &path[q + 1..];
            if group_part.starts_with("group=") {
                let name = path[..q].to_string();
                return Some(name);
            }
        }
        return Some(path.to_string());
    }
    None
}

/// Determine the major arXiv group from a category code.
fn detect_arxiv_group(code: &str) -> &str {
    let prefix = code.split('.').next().unwrap_or(code);
    match prefix {
        "astro-ph" => "Astrophysics",
        "cond-mat" => "Condensed Matter",
        "gr-qc" => "General Relativity and Quantum Cosmology",
        "hep-ex" | "hep-lat" | "hep-ph" | "hep-th" => "High Energy Physics",
        "math-ph" => "Mathematical Physics",
        "nlin" => "Nonlinear Sciences",
        "nucl-ex" | "nucl-th" => "Nuclear Physics",
        "physics" => "Physics",
        "quant-ph" => "Quantum Physics",
        "math" => "Mathematics",
        "cs" => "Computer Science",
        "q-bio" => "Quantitative Biology",
        "q-fin" => "Quantitative Finance",
        "stat" => "Statistics",
        "eess" => "Electrical Engineering and Systems Science",
        "econ" => "Economics",
        _ => "Other",
    }
}

/// Derive the abstract/forum URL from external_id.
fn derive_abs_url(external_id: &str, pdf_url: &Option<String>) -> Option<String> {
    if external_id.contains("@OpenReview") {
        let review_id = external_id.split('@').next().unwrap_or_default();
        Some(format!("https://openreview.net/forum?id={}", review_id))
    } else if external_id.chars().any(|c| c.is_ascii_digit()) {
        Some(format!("https://arxiv.org/abs/{}", external_id))
    } else {
        pdf_url.as_ref().map(|u| u.replace("/pdf", "/abs"))
    }
}

/// Extract conference name from venue key like "AAAI.2025" → "AAAI"
fn extract_venue_conference_name(key: &str) -> &str {
    key.rsplit_once('.').map_or(key, |(name, _)| name)
}

/// Extract year from venue key like "AAAI.2025" → "2025"
fn extract_venue_year(key: &str) -> &str {
    key.rsplit_once('.').map_or("", |(_, year)| year)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_arxiv_group() {
        assert_eq!(detect_arxiv_group("cs.AI"), "Computer Science");
        assert_eq!(detect_arxiv_group("math.AG"), "Mathematics");
        assert_eq!(detect_arxiv_group("hep-th"), "High Energy Physics");
        assert_eq!(detect_arxiv_group("quant-ph"), "Quantum Physics");
    }

    #[test]
    fn test_extract_venue_parts() {
        assert_eq!(extract_venue_conference_name("AAAI.2025"), "AAAI");
        assert_eq!(extract_venue_year("AAAI.2025"), "2025");
        assert_eq!(extract_venue_conference_name("ICLR.2025"), "ICLR");
    }

    #[test]
    fn test_derive_abs_url() {
        assert_eq!(
            derive_abs_url("2603.09957", &None),
            Some("https://arxiv.org/abs/2603.09957".to_string())
        );
        assert_eq!(
            derive_abs_url("GMwRl2e9Y1@OpenReview", &None),
            Some("https://openreview.net/forum?id=GMwRl2e9Y1".to_string())
        );
    }

    #[tokio::test]
    #[ignore]
    async fn test_browse_arxiv() {
        let client = PapersCool::new();
        let page = client.browse_arxiv("cs.AI", None).await.unwrap();
        assert!(!page.papers.is_empty());
        assert!(!page.title.is_empty());
        let p = &page.papers[0];
        assert!(!p.external_id.is_empty());
        assert!(!p.title.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn test_browse_venue() {
        let client = PapersCool::new();
        let page = client.browse_venue("ICLR.2025", None).await.unwrap();
        assert!(!page.papers.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn test_search() {
        let client = PapersCool::new();
        let page = client.search("attention mechanism").await.unwrap();
        assert!(!page.papers.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn test_fetch_index() {
        let client = PapersCool::new();
        let index = client.fetch_index().await.unwrap();
        assert!(!index.arxiv_groups.is_empty());
        assert!(!index.venues.is_empty());
        let cs = index
            .arxiv_groups
            .iter()
            .find(|g| g.name == "Computer Science");
        assert!(cs.is_some());
        assert!(cs.unwrap().categories.iter().any(|c| c.code == "cs.AI"));
    }
}
