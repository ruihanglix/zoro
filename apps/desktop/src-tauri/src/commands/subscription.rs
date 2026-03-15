// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::storage;
use crate::AppState;
use tauri::State;
use zoro_db::queries::{attachments, papers, subscriptions as sub_queries};
use zoro_subscriptions::{build_item_data_json, extract_source_date, HuggingFaceDailyPapers};

#[derive(Debug, serde::Serialize)]
pub struct SubscriptionResponse {
    pub id: String,
    pub source_type: String,
    pub name: String,
    pub enabled: bool,
    pub poll_interval_minutes: i32,
    pub last_polled: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct FeedItemResponse {
    pub id: String,
    pub external_id: String,
    pub title: String,
    pub authors: Vec<FeedAuthorResponse>,
    pub abstract_text: Option<String>,
    pub url: Option<String>,
    pub pdf_url: Option<String>,
    pub html_url: Option<String>,
    pub upvotes: Option<i32>,
    pub published_at: Option<String>,
    pub fetched_date: String,
    pub added_to_library: bool,
    // New metadata fields from HF API
    pub thumbnail_url: Option<String>,
    pub ai_summary: Option<String>,
    pub ai_keywords: Option<Vec<String>>,
    pub project_page: Option<String>,
    pub github_repo: Option<String>,
    pub github_stars: Option<i32>,
    pub num_comments: Option<i32>,
    /// Author-uploaded media (images, videos, gifs)
    pub media_urls: Vec<String>,
    /// Local path to a cached thumbnail image (if available)
    pub cached_thumbnail_path: Option<String>,
    /// Organization that claimed this paper on HuggingFace
    pub organization: Option<FeedOrganizationResponse>,
}

#[derive(Debug, serde::Serialize)]
pub struct FeedOrganizationResponse {
    pub name: Option<String>,
    pub fullname: Option<String>,
    pub avatar: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct FeedAuthorResponse {
    pub name: String,
    pub affiliation: Option<String>,
}

/// Helper to extract an optional string from a JSON value.
fn json_str(data: &serde_json::Value, key: &str) -> Option<String> {
    data.get(key).and_then(|v| v.as_str()).map(String::from)
}

/// Return the on-disk cache path for a thumbnail URL. The filename is a
/// deterministic hash of the URL so repeated requests hit disk cache.
fn thumbnail_cache_path(data_dir: &std::path::Path, url: &str) -> std::path::PathBuf {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let hash = hasher.finish();
    // Preserve file extension from URL for MIME sniffing
    let ext = url
        .rsplit('.')
        .next()
        .filter(|e| e.len() <= 4 && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or("jpg");
    data_dir
        .join("cache")
        .join("thumbnails")
        .join(format!("{:016x}.{}", hash, ext))
}

/// Check if a cached thumbnail exists and return its path.
fn get_cached_thumbnail(data_dir: &std::path::Path, url: &str) -> Option<String> {
    let path = thumbnail_cache_path(data_dir, url);
    if path.exists()
        && std::fs::metadata(&path)
            .map(|m| m.len() > 0)
            .unwrap_or(false)
    {
        Some(path.to_string_lossy().to_string())
    } else {
        None
    }
}

/// Download a thumbnail from a remote URL and cache it to disk.
/// Returns the local path on success. Runs synchronously (intended for
/// `tokio::spawn` background tasks).
async fn download_and_cache_thumbnail(
    data_dir: std::path::PathBuf,
    url: String,
) -> Result<String, String> {
    let cache_path = thumbnail_cache_path(&data_dir, &url);

    // Skip if already cached
    if cache_path.exists()
        && std::fs::metadata(&cache_path)
            .map(|m| m.len() > 0)
            .unwrap_or(false)
    {
        return Ok(cache_path.to_string_lossy().to_string());
    }

    // Ensure parent directory exists
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create thumbnail cache dir: {}", e))?;
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let response = client
        .get(&url)
        .header("User-Agent", "Zoro/0.1.0")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch thumbnail: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {} for thumbnail: {}", response.status(), url));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read thumbnail bytes: {}", e))?;

    std::fs::write(&cache_path, &bytes)
        .map_err(|e| format!("Failed to write cached thumbnail: {}", e))?;

    tracing::debug!("Cached thumbnail: {} -> {}", url, cache_path.display());
    Ok(cache_path.to_string_lossy().to_string())
}

/// Public wrapper so the background poller can cache thumbnails.
pub async fn download_and_cache_thumbnail_bg(
    data_dir: std::path::PathBuf,
    url: String,
) -> Result<String, String> {
    download_and_cache_thumbnail(data_dir, url).await
}

/// Helper to extract an optional i32 from a JSON value.
fn json_i32(data: &serde_json::Value, key: &str) -> Option<i32> {
    data.get(key).and_then(|v| v.as_i64()).map(|v| v as i32)
}

#[tauri::command]
pub async fn list_subscriptions(
    state: State<'_, AppState>,
) -> Result<Vec<SubscriptionResponse>, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let subs = sub_queries::list_subscriptions(&db.conn).map_err(|e| format!("{}", e))?;
    Ok(subs
        .into_iter()
        .map(|s| SubscriptionResponse {
            id: s.id,
            source_type: s.source_type,
            name: s.name,
            enabled: s.enabled,
            poll_interval_minutes: s.poll_interval_minutes,
            last_polled: s.last_polled,
        })
        .collect())
}

#[tauri::command]
pub async fn list_feed_items(
    state: State<'_, AppState>,
    subscription_id: String,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<FeedItemResponse>, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let items = sub_queries::list_subscription_items(
        &db.conn,
        &subscription_id,
        limit.unwrap_or(50),
        offset.unwrap_or(0),
    )
    .map_err(|e| format!("{}", e))?;
    let data_dir = state.data_dir.clone();
    Ok(items
        .into_iter()
        .map(|i| {
            let data: serde_json::Value = i
                .data_json
                .as_deref()
                .and_then(|d| serde_json::from_str(d).ok())
                .unwrap_or(serde_json::json!({}));
            map_item_row_to_response(i, data, &data_dir)
        })
        .collect())
}

#[tauri::command]
pub async fn add_feed_item_to_library(
    state: State<'_, AppState>,
    item_id: String,
) -> Result<String, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;

    // Get the subscription item
    let item =
        sub_queries::get_subscription_item(&db.conn, &item_id).map_err(|e| format!("{}", e))?;

    if item.added_to_library {
        return Err("Item already added to library".to_string());
    }

    // Parse the stored data to extract full metadata
    let data: serde_json::Value = item
        .data_json
        .as_deref()
        .and_then(|d| serde_json::from_str(d).ok())
        .unwrap_or(serde_json::json!({}));

    let arxiv_id = item.external_id.clone();
    let title = item.title.clone();
    let url =
        json_str(&data, "url").unwrap_or_else(|| format!("https://arxiv.org/abs/{}", arxiv_id));
    let pdf_url =
        json_str(&data, "pdf_url").unwrap_or_else(|| format!("https://arxiv.org/pdf/{}", arxiv_id));
    let html_url = json_str(&data, "html_url")
        .unwrap_or_else(|| format!("https://arxiv.org/html/{}", arxiv_id));
    let abstract_text = json_str(&data, "abstract_text");
    let thumbnail_url = json_str(&data, "thumbnail");

    // Build extra JSON with HF-specific metadata
    let mut extra = serde_json::json!({});
    if let Some(val) = data.get("upvotes") {
        extra["hf_upvotes"] = val.clone();
    }
    if let Some(val) = data.get("project_page") {
        extra["project_page"] = val.clone();
    }
    if let Some(val) = data.get("github_repo") {
        extra["github_repo"] = val.clone();
    }
    if let Some(val) = data.get("github_stars") {
        extra["github_stars"] = val.clone();
    }
    if let Some(val) = data.get("ai_summary") {
        extra["ai_summary"] = val.clone();
    }
    if let Some(val) = data.get("num_comments") {
        extra["hf_num_comments"] = val.clone();
    }

    // Extract authors
    let authors_data: Vec<(String, Option<String>, Option<String>)> = data
        .get("authors")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| {
                    a.get("name").and_then(|n| n.as_str()).map(|name| {
                        let aff = a
                            .get("affiliation")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        (name.to_string(), aff, None)
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let slug = zoro_core::slug_utils::generate_paper_slug(
        &title,
        &arxiv_id,
        data.get("published_at").and_then(|v| v.as_str()),
    );
    let papers_dir = state.data_dir.join("library/papers");
    let paper_dir = storage::paper_dir::create_paper_dir(&papers_dir, &slug)
        .map_err(|e| format!("Failed to create paper dir: {}", e))?;

    let db_input = papers::CreatePaperInput {
        slug: slug.clone(),
        title: title.clone(),
        short_title: None,
        abstract_text: abstract_text.clone(),
        doi: None,
        arxiv_id: Some(arxiv_id.clone()),
        url: Some(url),
        pdf_url: Some(pdf_url.clone()),
        html_url: Some(html_url.clone()),
        thumbnail_url: thumbnail_url.clone(),
        published_date: json_str(&data, "published_at"),
        source: Some("subscription".to_string()),
        dir_path: format!("papers/{}", slug),
        extra_json: if extra == serde_json::json!({}) {
            None
        } else {
            serde_json::to_string(&extra).ok()
        },
        entry_type: None,
        journal: None,
        volume: None,
        issue: None,
        pages: None,
        publisher: None,
        issn: None,
        isbn: None,
        added_date: None,
    };

    let row = papers::insert_paper(&db.conn, &db_input)
        .map_err(|e| format!("Failed to insert paper: {}", e))?;

    // FIX: Set authors in paper_authors table
    papers::set_paper_authors(&db.conn, &row.id, &authors_data)
        .map_err(|e| format!("Failed to set authors: {}", e))?;

    // FIX: Store extra JSON
    if extra != serde_json::json!({}) {
        let extra_str = serde_json::to_string(&extra).ok();
        let _ = db.conn.execute(
            "UPDATE papers SET extra_json = ?1 WHERE id = ?2",
            rusqlite::params![extra_str, row.id],
        );
    }

    // FIX: Write metadata.json (tags are empty — only user can add tags manually)
    let metadata = zoro_core::models::PaperMetadata {
        id: row.id.clone(),
        slug: slug.clone(),
        title: title.clone(),
        short_title: None,
        authors: authors_data
            .iter()
            .map(|(name, aff, _)| zoro_core::models::Author {
                name: name.clone(),
                affiliation: aff.clone(),
                orcid: None,
            })
            .collect(),
        abstract_text,
        doi: None,
        arxiv_id: Some(arxiv_id.clone()),
        url: Some(pdf_url.replace("/pdf/", "/abs/")),
        pdf_url: Some(pdf_url.clone()),
        html_url: Some(html_url.clone()),
        thumbnail_url,
        published_date: json_str(&data, "published_at"),
        added_date: row.added_date.clone(),
        source: Some("subscription".to_string()),
        tags: Vec::new(),
        collections: Vec::new(),
        attachments: Vec::new(),
        notes: Vec::new(),
        read_status: zoro_core::models::ReadStatus::Unread,
        rating: None,
        extra,
        entry_type: None,
        journal: None,
        volume: None,
        issue: None,
        pages: None,
        publisher: None,
        issn: None,
        isbn: None,
        annotations: Vec::new(),
    };
    let _ = storage::paper_dir::write_metadata(&paper_dir, &metadata);

    sub_queries::mark_item_added_to_library(&db.conn, &item_id, &row.id)
        .map_err(|e| format!("{}", e))?;

    // Sync library index
    storage::sync::rebuild_library_index(&db, &state.data_dir);

    // FIX: Download PDF and HTML in background with proper attachment records
    let pdf_path = paper_dir.join("paper.pdf");
    let html_path = paper_dir.join("paper.html");
    let pdf_url_clone = pdf_url.clone();
    let html_url_clone = html_url.clone();
    let paper_id_for_pdf = row.id.clone();
    let paper_id_for_html = row.id.clone();
    let db_path = state.data_dir.join("library.db");
    let db_path_html = db_path.clone();

    tokio::spawn(async move {
        if let Ok(()) = storage::attachments::download_file(&pdf_url_clone, &pdf_path).await {
            let file_size = storage::attachments::get_file_size(&pdf_path);
            if let Ok(dl_db) = zoro_db::Database::open(&db_path) {
                let _ = attachments::insert_attachment(
                    &dl_db.conn,
                    &paper_id_for_pdf,
                    "paper.pdf",
                    "pdf",
                    Some("application/pdf"),
                    file_size,
                    "paper.pdf",
                    "subscription",
                );
            }
        }
    });

    let arxiv_id_for_html = arxiv_id.clone();
    tokio::spawn(async move {
        // Use zoro_arxiv to fetch self-contained HTML (images/CSS inlined as base64)
        match zoro_arxiv::fetch::fetch_and_save(&arxiv_id_for_html, &html_path).await {
            Ok(()) => {
                // Run cleanup on the downloaded HTML
                let _ = zoro_arxiv::clean::clean_html_file(&html_path, &[]).await;
                let file_size = storage::attachments::get_file_size(&html_path);
                if let Ok(dl_db) = zoro_db::Database::open(&db_path_html) {
                    let _ = attachments::insert_attachment(
                        &dl_db.conn,
                        &paper_id_for_html,
                        "paper.html",
                        "html",
                        Some("text/html"),
                        file_size,
                        "paper.html",
                        "subscription",
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    arxiv_id = %arxiv_id_for_html,
                    error = %e,
                    "Failed to fetch self-contained arXiv HTML, falling back to direct download"
                );
                // Fallback to simple download
                if let Ok(()) =
                    storage::attachments::download_file(&html_url_clone, &html_path).await
                {
                    let file_size = storage::attachments::get_file_size(&html_path);
                    if let Ok(dl_db) = zoro_db::Database::open(&db_path_html) {
                        let _ = attachments::insert_attachment(
                            &dl_db.conn,
                            &paper_id_for_html,
                            "paper.html",
                            "html",
                            Some("text/html"),
                            file_size,
                            "paper.html",
                            "subscription",
                        );
                    }
                }
            }
        }
    });

    // Background metadata enrichment (reuse the same pattern as library.rs add_paper)
    {
        let enrich_paper_id = row.id.clone();
        let enrich_arxiv = Some(arxiv_id);
        let enrich_db_path = state.data_dir.join("library.db");
        tokio::spawn(async move {
            match zoro_metadata::enrich_paper(None, enrich_arxiv.as_deref()).await {
                Ok(enrichment) => {
                    if let Ok(enrich_db) = zoro_db::Database::open(&enrich_db_path) {
                        if let Ok(current) = papers::get_paper(&enrich_db.conn, &enrich_paper_id) {
                            let update = papers::UpdatePaperInput {
                                title: None,
                                short_title: None,
                                abstract_text: if current.abstract_text.is_none() {
                                    enrichment.abstract_text.map(Some)
                                } else {
                                    None
                                },
                                doi: if current.doi.is_none() {
                                    enrichment.doi.map(Some)
                                } else {
                                    None
                                },
                                arxiv_id: None,
                                url: None,
                                pdf_url: None,
                                html_url: None,
                                thumbnail_url: None,
                                published_date: if current.published_date.is_none() {
                                    enrichment.published_date.map(Some)
                                } else {
                                    None
                                },
                                source: None,
                                entry_type: if current.entry_type.is_none() {
                                    enrichment.entry_type.map(Some)
                                } else {
                                    None
                                },
                                journal: if current.journal.is_none() {
                                    enrichment.journal.map(Some)
                                } else {
                                    None
                                },
                                volume: if current.volume.is_none() {
                                    enrichment.volume.map(Some)
                                } else {
                                    None
                                },
                                issue: if current.issue.is_none() {
                                    enrichment.issue.map(Some)
                                } else {
                                    None
                                },
                                pages: if current.pages.is_none() {
                                    enrichment.pages.map(Some)
                                } else {
                                    None
                                },
                                publisher: if current.publisher.is_none() {
                                    enrichment.publisher.map(Some)
                                } else {
                                    None
                                },
                                issn: if current.issn.is_none() {
                                    enrichment.issn.map(Some)
                                } else {
                                    None
                                },
                                isbn: if current.isbn.is_none() {
                                    enrichment.isbn.map(Some)
                                } else {
                                    None
                                },
                                extra_json: None,
                            };
                            let _ =
                                papers::update_paper(&enrich_db.conn, &enrich_paper_id, &update);
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!("Background enrichment failed for subscription item: {}", e);
                }
            }
        });
    }

    Ok(row.id)
}

#[tauri::command]
pub async fn refresh_subscription(
    state: State<'_, AppState>,
    subscription_id: String,
) -> Result<i32, String> {
    let source = HuggingFaceDailyPapers::new();

    // First, resolve the latest date from the HF API so we fetch the same
    // set of papers that fetch_feed_items_by_date would return.
    let latest_date = zoro_subscriptions::fetch_latest_date()
        .await
        .map_err(|e| format!("Failed to fetch latest date: {}", e))?
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());

    let items = source
        .fetch_by_date(&latest_date)
        .await
        .map_err(|e| format!("{}", e))?;

    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let mut new_count = 0;
    for item in &items {
        let data_json = build_item_data_json(item);
        let source_date = extract_source_date(item);
        match sub_queries::insert_subscription_item(
            &db.conn,
            &subscription_id,
            &item.external_id,
            &item.title,
            data_json.as_deref(),
            source_date.as_deref(),
        ) {
            Ok(_) => new_count += 1,
            Err(zoro_db::DbError::Duplicate(_)) => {}
            Err(e) => tracing::warn!("Failed to insert subscription item: {}", e),
        }
    }

    sub_queries::update_last_polled(&db.conn, &subscription_id).map_err(|e| format!("{}", e))?;

    // Background-cache thumbnail images
    let thumbnail_urls: Vec<String> = items
        .iter()
        .filter_map(|item| {
            item.data
                .as_ref()
                .and_then(|d| d.get("thumbnail"))
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .collect();

    if !thumbnail_urls.is_empty() {
        let data_dir = state.data_dir.clone();
        tokio::spawn(async move {
            for url in thumbnail_urls {
                let _ = download_and_cache_thumbnail(data_dir.clone(), url).await;
            }
        });
    }

    Ok(new_count)
}

#[tauri::command]
pub async fn toggle_subscription(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    sub_queries::toggle_subscription(&db.conn, &id, enabled).map_err(|e| format!("{}", e))
}

/// Fetch the latest available date from the HuggingFace Daily Papers API.
/// Returns the date as YYYY-MM-DD, or null if unavailable.
#[tauri::command]
pub async fn get_latest_feed_date() -> Result<Option<String>, String> {
    let result = zoro_subscriptions::fetch_latest_date()
        .await
        .map_err(|e| format!("{}", e))?;
    tracing::info!("get_latest_feed_date command returning: {:?}", result);
    Ok(result)
}

/// Get feed items for a specific date, using local cache when available.
///
/// When `force_refresh` is false (default), returns cached items from the DB
/// if any exist for this date. Only hits the HuggingFace API on a cache miss.
/// When `force_refresh` is true, always fetches from the API.
#[tauri::command]
pub async fn fetch_feed_items_by_date(
    state: State<'_, AppState>,
    subscription_id: String,
    date: String,
    force_refresh: Option<bool>,
) -> Result<Vec<FeedItemResponse>, String> {
    let force = force_refresh.unwrap_or(false);
    let data_dir = state.data_dir.clone();

    // Try cache first (unless forcing refresh)
    if !force {
        let db = state
            .db
            .lock()
            .map_err(|e| format!("DB lock error: {}", e))?;
        let cached =
            sub_queries::list_subscription_items_by_source_date(&db.conn, &subscription_id, &date)
                .map_err(|e| format!("{}", e))?;

        if !cached.is_empty() {
            let mut items: Vec<FeedItemResponse> = cached
                .into_iter()
                .map(|i| {
                    let data: serde_json::Value = i
                        .data_json
                        .as_deref()
                        .and_then(|d| serde_json::from_str(d).ok())
                        .unwrap_or(serde_json::json!({}));
                    map_item_row_to_response(i, data, &data_dir)
                })
                .collect();

            items.sort_by(|a, b| b.upvotes.unwrap_or(0).cmp(&a.upvotes.unwrap_or(0)));
            spawn_thumbnail_downloads(&items, &data_dir);
            return Ok(items);
        }
    }

    // Cache miss or force refresh — fetch from API
    let source = HuggingFaceDailyPapers::new();
    let items = source
        .fetch_by_date(&date)
        .await
        .map_err(|e| format!("{}", e))?;

    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;

    let date_external_ids: std::collections::HashSet<String> =
        items.iter().map(|item| item.external_id.clone()).collect();

    for item in &items {
        let data_json = build_item_data_json(item);
        match sub_queries::insert_subscription_item(
            &db.conn,
            &subscription_id,
            &item.external_id,
            &item.title,
            data_json.as_deref(),
            Some(&date),
        ) {
            Ok(_) => {}
            Err(zoro_db::DbError::Duplicate(_)) => {}
            Err(e) => tracing::warn!("Failed to insert subscription item: {}", e),
        }
    }

    let all_items = sub_queries::list_subscription_items(&db.conn, &subscription_id, 500, 0)
        .map_err(|e| format!("{}", e))?;

    let mut filtered: Vec<FeedItemResponse> = all_items
        .into_iter()
        .filter_map(|i| {
            if date_external_ids.contains(&i.external_id) {
                let data: serde_json::Value = i
                    .data_json
                    .as_deref()
                    .and_then(|d| serde_json::from_str(d).ok())
                    .unwrap_or(serde_json::json!({}));
                Some(map_item_row_to_response(i, data, &data_dir))
            } else {
                None
            }
        })
        .collect();

    filtered.sort_by(|a, b| b.upvotes.unwrap_or(0).cmp(&a.upvotes.unwrap_or(0)));
    spawn_thumbnail_downloads(&filtered, &data_dir);
    Ok(filtered)
}

fn spawn_thumbnail_downloads(items: &[FeedItemResponse], data_dir: &std::path::Path) {
    let urls_to_cache: Vec<String> = items
        .iter()
        .filter(|f| f.cached_thumbnail_path.is_none())
        .filter_map(|f| f.thumbnail_url.clone())
        .collect();

    if !urls_to_cache.is_empty() {
        let bg_data_dir = data_dir.to_path_buf();
        tokio::spawn(async move {
            for url in urls_to_cache {
                let _ = download_and_cache_thumbnail(bg_data_dir.clone(), url).await;
            }
        });
    }
}

/// Map a SubscriptionItemRow + parsed data JSON to a FeedItemResponse.
fn map_item_row_to_response(
    i: sub_queries::SubscriptionItemRow,
    data: serde_json::Value,
    data_dir: &std::path::Path,
) -> FeedItemResponse {
    let authors = data
        .get("authors")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| {
                    a.get("name")
                        .and_then(|n| n.as_str())
                        .map(|name| FeedAuthorResponse {
                            name: name.to_string(),
                            affiliation: a
                                .get("affiliation")
                                .and_then(|v| v.as_str())
                                .map(String::from),
                        })
                })
                .collect()
        })
        .unwrap_or_default();

    let ai_keywords = data
        .get("ai_keywords")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });

    let media_urls = data
        .get("media_urls")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    FeedItemResponse {
        id: i.id,
        external_id: i.external_id,
        title: i.title,
        authors,
        abstract_text: json_str(&data, "abstract_text"),
        url: json_str(&data, "url"),
        pdf_url: json_str(&data, "pdf_url"),
        html_url: json_str(&data, "html_url"),
        upvotes: json_i32(&data, "upvotes"),
        published_at: json_str(&data, "published_at"),
        fetched_date: i.fetched_date,
        added_to_library: i.added_to_library,
        thumbnail_url: json_str(&data, "thumbnail"),
        ai_summary: json_str(&data, "ai_summary"),
        ai_keywords,
        project_page: json_str(&data, "project_page"),
        github_repo: json_str(&data, "github_repo"),
        github_stars: json_i32(&data, "github_stars"),
        num_comments: json_i32(&data, "num_comments"),
        media_urls,
        cached_thumbnail_path: json_str(&data, "thumbnail")
            .and_then(|url| get_cached_thumbnail(data_dir, &url)),
        organization: data.get("organization").and_then(|org| {
            if org.is_null() {
                None
            } else {
                Some(FeedOrganizationResponse {
                    name: org.get("name").and_then(|v| v.as_str()).map(String::from),
                    fullname: org
                        .get("fullname")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    avatar: org.get("avatar").and_then(|v| v.as_str()).map(String::from),
                })
            }
        }),
    }
}

#[derive(Debug, serde::Serialize)]
pub struct StorageInfoResponse {
    pub data_dir: String,
    pub total_papers: i64,
    pub feed_cache_items: i64,
    pub feed_total_items: i64,
    pub feed_cache_retention_days: i32,
}

#[tauri::command]
pub async fn get_storage_info(state: State<'_, AppState>) -> Result<StorageInfoResponse, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;

    let total_papers: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM papers", [], |row| row.get(0))
        .unwrap_or(0);

    let (feed_total, feed_cached) =
        sub_queries::count_subscription_items(&db.conn, None).unwrap_or((0, 0));

    let config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?;

    Ok(StorageInfoResponse {
        data_dir: state.data_dir.to_string_lossy().to_string(),
        total_papers,
        feed_cache_items: feed_cached,
        feed_total_items: feed_total,
        feed_cache_retention_days: config.subscriptions.feed_cache_retention_days,
    })
}

#[tauri::command]
pub async fn clear_feed_cache(
    state: State<'_, AppState>,
    subscription_id: Option<String>,
) -> Result<i64, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let deleted = sub_queries::clear_subscription_cache(&db.conn, subscription_id.as_deref())
        .map_err(|e| format!("{}", e))?;
    Ok(deleted as i64)
}

/// Change the data directory. Optionally moves all existing data to the new location.
/// After success the app should be restarted for the change to take full effect.
#[tauri::command]
pub async fn change_data_dir(
    state: State<'_, AppState>,
    new_path: String,
    move_data: bool,
) -> Result<(), String> {
    let new_dir = std::path::PathBuf::from(&new_path);
    let old_dir = state.data_dir.clone();

    // Reject if the new path is the same as the current one
    if new_dir == old_dir {
        return Err("New path is the same as the current data directory".into());
    }

    if move_data {
        // Move all files from old_dir to new_dir
        copy_dir_recursive(&old_dir, &new_dir)
            .map_err(|e| format!("Failed to copy data to new directory: {}", e))?;
    } else {
        // Just create the directory structure at the new location
        storage::init_data_dir(&new_dir)
            .map_err(|e| format!("Failed to initialize new data directory: {}", e))?;
    }

    // Update config.toml in the NEW directory so it records the new data_dir
    let mut config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?
        .clone();
    config.general.data_dir = new_path.clone();
    storage::config::save_config(&new_dir, &config)
        .map_err(|e| format!("Failed to save config: {}", e))?;

    // Also update the OLD config.toml to point to the new location,
    // so that the next launch can detect the redirect.
    let mut redirect_config = storage::config::load_config(&old_dir);
    redirect_config.general.data_dir = new_path;
    let _ = storage::config::save_config(&old_dir, &redirect_config);

    Ok(())
}

/// Recursively copy a directory tree from `src` to `dst`.
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Proxy-fetch a PDF from a remote URL via the Rust backend (reqwest),
/// bypassing WebKit's network stack which fails on cross-origin ArXiv PDFs.
/// Saves to a temp file under the data dir and returns its path so the
/// frontend can read it via the Tauri FS plugin (binary IPC, no JSON bloat).
///
/// Uses a URL-keyed on-disk cache so the same PDF is not downloaded twice,
/// validates that the response is actually a PDF (Content-Type + magic bytes),
/// and sets a 60 s timeout to avoid indefinite hangs.
#[tauri::command]
pub async fn fetch_remote_pdf(state: State<'_, AppState>, url: String) -> Result<String, String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let cache_dir = state.data_dir.join("cache");
    std::fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("Failed to create cache dir: {}", e))?;

    // Deterministic filename based on URL so repeated requests hit disk cache
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let url_hash = hasher.finish();
    let filename = format!("{:016x}.pdf", url_hash);
    let file_path = cache_dir.join(&filename);

    // Return cached file if it already exists and looks like a valid PDF
    if file_path.exists() {
        let cached_bytes =
            std::fs::read(&file_path).map_err(|e| format!("Failed to read cached file: {}", e))?;
        if cached_bytes.len() >= 4 && cached_bytes.starts_with(b"%PDF") {
            return Ok(file_path.to_string_lossy().to_string());
        }
        let _ = std::fs::remove_file(&file_path);
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let response = client
        .get(&url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
             AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/131.0.0.0 Safari/537.36",
        )
        .header("Accept", "application/pdf,*/*;q=0.8")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch PDF: {}", e))?;

    let status = response.status();

    if !status.is_success() {
        return Err(format!("HTTP {}: {}", status, url));
    }

    // Check Content-Type — ArXiv may return HTML error/captcha pages with 200
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    if content_type.contains("text/html") {
        // Read body to log what ArXiv actually returned
        return Err(format!(
            "Remote server returned HTML instead of PDF (Content-Type: {})",
            content_type,
        ));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read PDF bytes: {}", e))?;

    // Validate PDF magic bytes (%PDF at offset 0)
    if bytes.len() < 4 || &bytes[..4] != b"%PDF" {
        let preview: String = bytes
            .iter()
            .take(500)
            .map(|&b| {
                if b.is_ascii_graphic() || b == b' ' {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        return Err(format!(
            "Downloaded content is not a valid PDF. Content-Type was '{}'. First bytes: {}",
            content_type,
            &preview[..preview.len().min(200)]
        ));
    }

    std::fs::write(&file_path, &bytes).map_err(|e| format!("Failed to write temp PDF: {}", e))?;

    Ok(file_path.to_string_lossy().to_string())
}

#[derive(Debug, serde::Serialize)]
pub struct SubscriptionsConfigResponse {
    pub feed_cache_retention_days: i32,
    pub poll_interval_minutes: i32,
}

#[tauri::command]
pub async fn get_subscriptions_config(
    state: State<'_, AppState>,
) -> Result<SubscriptionsConfigResponse, String> {
    let config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?;
    Ok(SubscriptionsConfigResponse {
        feed_cache_retention_days: config.subscriptions.feed_cache_retention_days,
        poll_interval_minutes: config.subscriptions.poll_interval_minutes,
    })
}

#[tauri::command]
pub async fn update_subscriptions_config(
    state: State<'_, AppState>,
    feed_cache_retention_days: Option<i32>,
) -> Result<(), String> {
    let mut config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?;

    if let Some(days) = feed_cache_retention_days {
        config.subscriptions.feed_cache_retention_days = days;
    }

    crate::storage::config::save_config(&state.data_dir, &config)
        .map_err(|e| format!("Failed to save config: {}", e))
}
