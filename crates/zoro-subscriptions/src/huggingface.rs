// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::SubscriptionError;
use crate::source::SubscriptionSource;
use async_trait::async_trait;
use serde::Deserialize;
use tracing::info;
use zoro_core::models::{Author, SubscriptionItem};

pub struct HuggingFaceDailyPapers {
    client: reqwest::Client,
}

impl HuggingFaceDailyPapers {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Fetch papers for a specific date (YYYY-MM-DD format).
    /// Uses the HuggingFace API `?date=` query parameter.
    pub async fn fetch_by_date(
        &self,
        date: &str,
    ) -> Result<Vec<SubscriptionItem>, SubscriptionError> {
        info!("Fetching HuggingFace Daily Papers for date: {}", date);

        let url = format!("https://huggingface.co/api/daily_papers?date={}", date);
        let response = self
            .client
            .get(&url)
            .header("User-Agent", "Zoro/0.1.0")
            .send()
            .await?;

        let papers: Vec<DailyPaperResponse> = response.json().await?;
        info!(
            "Fetched {} papers from HuggingFace for date {}",
            papers.len(),
            date
        );

        Ok(papers.into_iter().map(Self::map_paper_response).collect())
    }

    /// Map a single HF API response to a SubscriptionItem.
    fn map_paper_response(dp: DailyPaperResponse) -> SubscriptionItem {
        let arxiv_id = dp.paper.id.clone();
        let title = dp
            .paper
            .title
            .unwrap_or_else(|| dp.title.unwrap_or_default());
        let abstract_text = dp.paper.summary;
        let authors = dp
            .paper
            .authors
            .unwrap_or_default()
            .into_iter()
            .filter_map(|a| {
                a.name.map(|name| Author {
                    name,
                    affiliation: None,
                    orcid: None,
                })
            })
            .collect();

        let url = format!("https://arxiv.org/abs/{}", arxiv_id);
        let pdf_url = format!("https://arxiv.org/pdf/{}", arxiv_id);
        let html_url = format!("https://arxiv.org/html/{}", arxiv_id);

        let now = chrono::Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();

        SubscriptionItem {
            id,
            subscription_id: String::new(),
            paper_id: None,
            external_id: arxiv_id,
            title,
            authors,
            abstract_text,
            url: Some(url),
            pdf_url: Some(pdf_url),
            html_url: Some(html_url),
            upvotes: dp.paper.upvotes,
            data: Some(serde_json::json!({
                "published_at": dp.published_at,
                "submitted_on_daily_at": dp.submitted_on_daily_at,
                "upvotes": dp.paper.upvotes,
                "thumbnail": dp.thumbnail,
                "ai_summary": dp.paper.ai_summary,
                "ai_keywords": dp.paper.ai_keywords,
                "project_page": dp.paper.project_page,
                "github_repo": dp.paper.github_repo,
                "github_stars": dp.paper.github_stars,
                "num_comments": dp.num_comments,
                "media_urls": dp.media_urls,
                "organization": dp.organization.as_ref().map(|org| serde_json::json!({
                    "name": org.name,
                    "fullname": org.fullname,
                    "avatar": org.avatar,
                })),
            })),
            fetched_date: now,
            added_to_library: false,
        }
    }
}

impl Default for HuggingFaceDailyPapers {
    fn default() -> Self {
        Self::new()
    }
}

// API response types
#[derive(Debug, Deserialize)]
struct DailyPaperResponse {
    paper: HfPaper,
    title: Option<String>,
    #[serde(rename = "publishedAt")]
    published_at: Option<String>,
    /// The date this paper was submitted/added to HF Daily Papers.
    /// This is the authoritative "daily" date shown on the HF website.
    #[serde(rename = "submittedOnDailyAt")]
    submitted_on_daily_at: Option<String>,
    thumbnail: Option<String>,
    #[serde(rename = "numComments")]
    num_comments: Option<i32>,
    #[serde(rename = "mediaUrls", default)]
    media_urls: Vec<String>,
    /// Organization that claimed this paper (may be absent)
    organization: Option<HfOrganization>,
}

#[derive(Debug, Deserialize)]
struct HfPaper {
    id: String, // ArXiv ID like "2603.05706"
    title: Option<String>,
    summary: Option<String>,
    authors: Option<Vec<HfAuthor>>,
    upvotes: Option<i32>,
    #[serde(rename = "projectPage")]
    project_page: Option<String>,
    #[serde(rename = "githubRepo")]
    github_repo: Option<String>,
    #[serde(rename = "githubStars")]
    github_stars: Option<i32>,
    ai_summary: Option<String>,
    ai_keywords: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
struct HfAuthor {
    name: Option<String>,
    #[serde(rename = "_id")]
    #[allow(dead_code)]
    id: Option<String>,
}

/// Organization that claimed the paper on HuggingFace.
#[derive(Debug, Clone, Deserialize)]
struct HfOrganization {
    #[allow(dead_code)]
    #[serde(rename = "_id")]
    id: Option<String>,
    /// Short slug, e.g. "MIT"
    name: Option<String>,
    /// Full display name, e.g. "Massachusetts Institute of Technology"
    fullname: Option<String>,
    /// Avatar image URL
    avatar: Option<String>,
}

/// Serialize a `SubscriptionItem` from HuggingFace into its `data_json` blob.
///
/// This is the single source of truth for the JSON schema stored in the DB.
/// Both `refresh_subscription` and the background poller must use this function.
pub fn build_item_data_json(item: &SubscriptionItem) -> Option<String> {
    serde_json::to_string(&item_to_data_value(item)).ok()
}

/// Extract the source date (YYYY-MM-DD) from a SubscriptionItem's data blob.
pub fn extract_source_date(item: &SubscriptionItem) -> Option<String> {
    item.data
        .as_ref()
        .and_then(|d| {
            // Prefer submittedOnDailyAt (HF daily date); fall back to publishedAt (arXiv date)
            d.get("submitted_on_daily_at")
                .or_else(|| d.get("published_at"))
        })
        .and_then(|v| v.as_str())
        .map(|s| s.chars().take(10).collect())
}

/// Fetch the latest available date from the HuggingFace Daily Papers API.
///
/// Probes from today backwards (up to 7 days) using `?date=YYYY-MM-DD` and
/// returns the first date that has at least one paper. This matches the date
/// shown on the HuggingFace Daily Papers website.
pub async fn fetch_latest_date() -> Result<Option<String>, SubscriptionError> {
    let client = reqwest::Client::new();
    let today = chrono::Utc::now().date_naive();

    // Look back up to 7 days (covers weekends and holidays)
    for offset in 0..7 {
        let date = today - chrono::Duration::days(offset);
        let date_str = date.format("%Y-%m-%d").to_string();
        let url = format!("https://huggingface.co/api/daily_papers?date={}", date_str);
        info!("fetch_latest_date: probing {} ...", url);

        let response = client
            .get(&url)
            .header("User-Agent", "Zoro/0.1.0")
            .send()
            .await?;

        // Quick check: deserialize just enough to count papers
        let papers: Vec<serde_json::Value> = response.json().await?;
        info!("fetch_latest_date: {} => {} papers", date_str, papers.len());

        if !papers.is_empty() {
            info!("fetch_latest_date: resolved latest date = {}", date_str);
            return Ok(Some(date_str));
        }
    }

    info!("fetch_latest_date: no papers found in the last 7 days");
    Ok(None)
}

/// Build the `serde_json::Value` for a subscription item's `data` field.
fn item_to_data_value(item: &SubscriptionItem) -> serde_json::Value {
    let authors_json: Vec<serde_json::Value> = item
        .authors
        .iter()
        .map(|a| serde_json::json!({ "name": &a.name, "affiliation": &a.affiliation }))
        .collect();

    let mut data = serde_json::json!({
        "published_at": item.data.as_ref().and_then(|d| d.get("published_at")).cloned(),
        "upvotes": item.upvotes,
        "abstract_text": item.abstract_text,
        "authors": authors_json,
        "url": item.url,
        "pdf_url": item.pdf_url,
        "html_url": item.html_url,
    });

    // Merge extra fields from the item's own `data` blob (thumbnail, ai_*, github, etc.)
    if let Some(ref orig) = item.data {
        let extra_keys = [
            "thumbnail",
            "ai_summary",
            "ai_keywords",
            "project_page",
            "github_repo",
            "github_stars",
            "num_comments",
            "media_urls",
            "organization",
        ];
        for key in extra_keys {
            if let Some(val) = orig.get(key) {
                data[key] = val.clone();
            }
        }
    }

    data
}

#[async_trait]
impl SubscriptionSource for HuggingFaceDailyPapers {
    fn source_type(&self) -> &str {
        "huggingface-daily"
    }

    fn display_name(&self) -> &str {
        "HuggingFace Daily Papers"
    }

    fn description(&self) -> &str {
        "Daily curated ML/AI papers from Hugging Face community"
    }

    fn default_config(&self) -> serde_json::Value {
        serde_json::json!({
            "auto_download_pdf": false,
            "auto_download_html": false
        })
    }

    async fn fetch(
        &self,
        _config: &serde_json::Value,
        _since: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Vec<SubscriptionItem>, SubscriptionError> {
        info!("Fetching HuggingFace Daily Papers...");

        let url = "https://huggingface.co/api/daily_papers";
        let response = self
            .client
            .get(url)
            .header("User-Agent", "Zoro/0.1.0")
            .send()
            .await?;

        let papers: Vec<DailyPaperResponse> = response.json().await?;
        info!("Fetched {} papers from HuggingFace", papers.len());

        Ok(papers.into_iter().map(Self::map_paper_response).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires network access
    async fn test_fetch_daily_papers() {
        let source = HuggingFaceDailyPapers::new();
        let config = source.default_config();
        let items = source.fetch(&config, None).await.unwrap();
        assert!(!items.is_empty(), "Should fetch at least one paper");
        for item in &items {
            assert!(!item.title.is_empty(), "Paper should have a title");
            assert!(
                !item.external_id.is_empty(),
                "Paper should have an ArXiv ID"
            );
            // Verify new metadata fields are present in data
            if let Some(ref data) = item.data {
                assert!(
                    data.get("thumbnail").is_some(),
                    "Should have thumbnail field"
                );
            }
        }
    }

    #[test]
    fn test_build_item_data_json() {
        let item = SubscriptionItem {
            id: "test".to_string(),
            subscription_id: "sub1".to_string(),
            paper_id: None,
            external_id: "2603.05706".to_string(),
            title: "Test Paper".to_string(),
            authors: vec![Author {
                name: "Alice".to_string(),
                affiliation: None,
                orcid: None,
            }],
            abstract_text: Some("Abstract".to_string()),
            url: Some("https://arxiv.org/abs/2603.05706".to_string()),
            pdf_url: Some("https://arxiv.org/pdf/2603.05706".to_string()),
            html_url: Some("https://arxiv.org/html/2603.05706".to_string()),
            upvotes: Some(42),
            data: Some(serde_json::json!({
                "published_at": "2026-03-05",
                "upvotes": 42,
                "thumbnail": "https://cdn-thumbnails.huggingface.co/social-thumbnails/papers/2603.05706.png",
                "ai_keywords": ["machine-learning", "transformers"],
                "media_urls": ["https://cdn-uploads.huggingface.co/demo.mp4"],
            })),
            fetched_date: "2026-03-05T00:00:00Z".to_string(),
            added_to_library: false,
        };

        let json_str = build_item_data_json(&item).unwrap();
        let data: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(
            data["thumbnail"],
            "https://cdn-thumbnails.huggingface.co/social-thumbnails/papers/2603.05706.png"
        );
        assert_eq!(data["ai_keywords"][0], "machine-learning");
        assert_eq!(data["authors"][0]["name"], "Alice");
        assert_eq!(
            data["media_urls"][0],
            "https://cdn-uploads.huggingface.co/demo.mp4"
        );
    }

    #[test]
    fn test_parse_daily_papers_response() {
        let json = r#"[
            {
                "paper": {
                    "id": "2603.05706",
                    "title": "Test Paper Title",
                    "summary": "This is a test abstract",
                    "authors": [
                        {"name": "Alice Smith", "_id": "asmith"},
                        {"name": "Bob Jones", "_id": "bjones"}
                    ],
                    "upvotes": 42,
                    "projectPage": "https://example.com/project",
                    "githubRepo": "https://github.com/example/repo",
                    "githubStars": 100,
                    "ai_summary": "An AI-generated summary",
                    "ai_keywords": ["ml", "transformers"]
                },
                "title": "Test Paper Title",
                "publishedAt": "2026-03-05T00:00:00.000Z",
                "thumbnail": "https://example.com/thumb.png",
                "numComments": 5,
                "mediaUrls": [
                    "https://cdn-uploads.huggingface.co/demo.mp4",
                    "https://cdn-uploads.huggingface.co/fig.png"
                ]
            },
            {
                "paper": {
                    "id": "2603.09999",
                    "title": null,
                    "summary": null,
                    "authors": null,
                    "upvotes": 0,
                    "projectPage": null,
                    "githubRepo": null,
                    "githubStars": null,
                    "ai_summary": null,
                    "ai_keywords": null
                },
                "title": "Fallback Title",
                "publishedAt": null,
                "thumbnail": null,
                "numComments": null
            }
        ]"#;

        let papers: Vec<DailyPaperResponse> = serde_json::from_str(json).unwrap();
        assert_eq!(papers.len(), 2);

        // First paper: all fields populated (including mediaUrls)
        assert_eq!(papers[0].paper.id, "2603.05706");
        assert_eq!(papers[0].paper.title.as_deref(), Some("Test Paper Title"));
        assert_eq!(
            papers[0].paper.summary.as_deref(),
            Some("This is a test abstract")
        );
        assert_eq!(papers[0].paper.authors.as_ref().unwrap().len(), 2);
        assert_eq!(
            papers[0].paper.authors.as_ref().unwrap()[0].name.as_deref(),
            Some("Alice Smith")
        );
        assert_eq!(papers[0].paper.upvotes, Some(42));
        assert_eq!(papers[0].num_comments, Some(5));
        assert_eq!(
            papers[0].thumbnail.as_deref(),
            Some("https://example.com/thumb.png")
        );
        assert_eq!(papers[0].media_urls.len(), 2);
        assert!(papers[0].media_urls[0].ends_with(".mp4"));

        // Second paper: sparse data with nulls (mediaUrls defaults to empty)
        assert_eq!(papers[1].paper.id, "2603.09999");
        assert!(papers[1].paper.title.is_none());
        assert_eq!(papers[1].title.as_deref(), Some("Fallback Title"));
        assert!(papers[1].paper.authors.is_none());
        assert!(papers[1].thumbnail.is_none());
        assert!(papers[1].media_urls.is_empty());
    }

    #[test]
    fn test_parse_empty_response() {
        let json = "[]";
        let papers: Vec<DailyPaperResponse> = serde_json::from_str(json).unwrap();
        assert!(papers.is_empty());
    }

    #[test]
    fn test_parse_malformed_response() {
        let json = r#"{"error": "not an array"}"#;
        let result: Result<Vec<DailyPaperResponse>, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_item_conversion_with_all_fields() {
        let json = r#"[
            {
                "paper": {
                    "id": "2603.05706",
                    "title": "Attention Is All You Need",
                    "summary": "The dominant sequence transduction models...",
                    "authors": [
                        {"name": "Ashish Vaswani", "_id": "avaswani"},
                        {"name": null, "_id": "unknown"}
                    ],
                    "upvotes": 1500,
                    "projectPage": null,
                    "githubRepo": null,
                    "githubStars": null,
                    "ai_summary": null,
                    "ai_keywords": null
                },
                "title": "Attention Is All You Need",
                "publishedAt": "2026-03-10T00:00:00.000Z",
                "thumbnail": null,
                "numComments": 0
            }
        ]"#;

        let papers: Vec<DailyPaperResponse> = serde_json::from_str(json).unwrap();
        let dp = &papers[0];

        // Simulate the conversion logic
        let arxiv_id = dp.paper.id.clone();
        let title = dp
            .paper
            .title
            .clone()
            .unwrap_or_else(|| dp.title.clone().unwrap_or_default());
        let authors: Vec<Author> = dp
            .paper
            .authors
            .clone()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|a| {
                a.name.map(|name| Author {
                    name,
                    affiliation: None,
                    orcid: None,
                })
            })
            .collect();

        assert_eq!(arxiv_id, "2603.05706");
        assert_eq!(title, "Attention Is All You Need");
        // Author with null name should be filtered out
        assert_eq!(authors.len(), 1);
        assert_eq!(authors[0].name, "Ashish Vaswani");
    }
}
