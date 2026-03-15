// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::AppState;
use serde::Serialize;
use tauri::State;
use zoro_subscriptions::papers_cool;

// ── TTL constants (seconds) ─────────────────────────────────────────────────

const TTL_INDEX: i64 = 86400; // 1 day
const TTL_ARXIV_TODAY: i64 = 1800; // 30 min
const TTL_ARXIV_PAST: i64 = 604800; // 7 days
const TTL_VENUE: i64 = 604800; // 7 days
const TTL_SEARCH: i64 = 3600; // 1 hour

// ── Response types ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct PapersCoolPageResponse {
    pub title: String,
    pub total: i32,
    pub papers: Vec<PapersCoolPaperResponse>,
}

#[derive(Debug, Serialize)]
pub struct PapersCoolPaperResponse {
    pub external_id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub abstract_text: Option<String>,
    pub categories: Vec<PapersCoolCategoryResponse>,
    pub published_date: Option<String>,
    pub pdf_url: Option<String>,
    pub abs_url: Option<String>,
    pub papers_cool_url: String,
    pub pdf_opens: i32,
    pub kimi_opens: i32,
    pub keywords: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PapersCoolCategoryResponse {
    pub code: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct PapersCoolIndexResponse {
    pub arxiv_groups: Vec<ArxivGroupResponse>,
    pub venues: Vec<VenueConferenceResponse>,
}

#[derive(Debug, Serialize)]
pub struct ArxivGroupResponse {
    pub name: String,
    pub categories: Vec<ArxivCategoryResponse>,
}

#[derive(Debug, Serialize)]
pub struct ArxivCategoryResponse {
    pub code: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct VenueConferenceResponse {
    pub name: String,
    pub editions: Vec<VenueEditionResponse>,
}

#[derive(Debug, Serialize)]
pub struct VenueEditionResponse {
    pub key: String,
    pub year: String,
    pub groups: Vec<VenueGroupResponse>,
}

#[derive(Debug, Serialize)]
pub struct VenueGroupResponse {
    pub name: String,
    pub query: String,
}

// ── Commands ────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn papers_cool_index(
    state: State<'_, AppState>,
    force_refresh: Option<bool>,
) -> Result<PapersCoolIndexResponse, String> {
    let cache_key = "index".to_string();
    let force = force_refresh.unwrap_or(false);

    if !force {
        if let Some(cached) = read_cache(&state, &cache_key) {
            if let Ok(index) = serde_json::from_str::<papers_cool::PapersCoolIndex>(&cached) {
                return Ok(map_index(index));
            }
        }
    }

    let client = papers_cool::PapersCool::new();
    let index = client
        .fetch_index()
        .await
        .map_err(|e| format!("Failed to fetch index: {}", e))?;

    write_cache(&state, &cache_key, &index, TTL_INDEX);

    Ok(map_index(index))
}

#[tauri::command]
pub async fn papers_cool_browse_arxiv(
    state: State<'_, AppState>,
    category: String,
    date: Option<String>,
    force_refresh: Option<bool>,
) -> Result<PapersCoolPageResponse, String> {
    let cache_key = match &date {
        Some(d) => format!("arxiv:{}:{}", category, d),
        None => format!("arxiv:{}:latest", category),
    };
    let force = force_refresh.unwrap_or(false);

    if !force {
        if let Some(cached) = read_cache(&state, &cache_key) {
            if let Ok(page) = serde_json::from_str::<papers_cool::PapersCoolPage>(&cached) {
                store_paper_texts(&state, &page);
                return Ok(map_page(page));
            }
        }
    }

    let client = papers_cool::PapersCool::new();
    let page = client
        .browse_arxiv(&category, date.as_deref())
        .await
        .map_err(|e| format!("Failed to browse arXiv: {}", e))?;

    let ttl = if is_today(date.as_deref()) {
        TTL_ARXIV_TODAY
    } else {
        TTL_ARXIV_PAST
    };
    write_cache(&state, &cache_key, &page, ttl);
    store_paper_texts(&state, &page);

    Ok(map_page(page))
}

#[tauri::command]
pub async fn papers_cool_browse_venue(
    state: State<'_, AppState>,
    venue_key: String,
    group: Option<String>,
    force_refresh: Option<bool>,
) -> Result<PapersCoolPageResponse, String> {
    let cache_key = match &group {
        Some(g) => format!("venue:{}:{}", venue_key, g),
        None => format!("venue:{}", venue_key),
    };
    let force = force_refresh.unwrap_or(false);

    if !force {
        if let Some(cached) = read_cache(&state, &cache_key) {
            if let Ok(page) = serde_json::from_str::<papers_cool::PapersCoolPage>(&cached) {
                store_paper_texts(&state, &page);
                return Ok(map_page(page));
            }
        }
    }

    let client = papers_cool::PapersCool::new();
    let page = client
        .browse_venue(&venue_key, group.as_deref())
        .await
        .map_err(|e| format!("Failed to browse venue: {}", e))?;

    write_cache(&state, &cache_key, &page, TTL_VENUE);
    store_paper_texts(&state, &page);

    Ok(map_page(page))
}

#[tauri::command]
pub async fn papers_cool_search(
    state: State<'_, AppState>,
    query: String,
    force_refresh: Option<bool>,
) -> Result<PapersCoolPageResponse, String> {
    let cache_key = format!("search:{}", query);
    let force = force_refresh.unwrap_or(false);

    if !force {
        if let Some(cached) = read_cache(&state, &cache_key) {
            if let Ok(page) = serde_json::from_str::<papers_cool::PapersCoolPage>(&cached) {
                store_paper_texts(&state, &page);
                return Ok(map_page(page));
            }
        }
    }

    let client = papers_cool::PapersCool::new();
    let page = client
        .search(&query)
        .await
        .map_err(|e| format!("Failed to search: {}", e))?;

    write_cache(&state, &cache_key, &page, TTL_SEARCH);
    store_paper_texts(&state, &page);

    Ok(map_page(page))
}

// ── Cache helpers ───────────────────────────────────────────────────────────

fn read_cache(state: &State<'_, AppState>, key: &str) -> Option<String> {
    let db = state.db.lock().ok()?;
    zoro_db::queries::papers_cool_cache::get_cached(&db.conn, key).ok()?
}

fn write_cache<T: Serialize>(state: &State<'_, AppState>, key: &str, value: &T, ttl: i64) {
    if let Ok(json) = serde_json::to_string(value) {
        if let Ok(db) = state.db.lock() {
            let _ = zoro_db::queries::papers_cool_cache::set_cached(&db.conn, key, &json, ttl);
        }
    }
}

fn store_paper_texts(state: &State<'_, AppState>, page: &papers_cool::PapersCoolPage) {
    let texts: Vec<(String, String, Option<String>)> = page
        .papers
        .iter()
        .map(|p| {
            (
                p.external_id.clone(),
                p.title.clone(),
                p.abstract_text.clone(),
            )
        })
        .collect();
    if let Ok(db) = state.db.lock() {
        let _ = zoro_db::queries::papers_cool_cache::upsert_paper_texts_batch(&db.conn, &texts);
    }
}

fn is_today(date: Option<&str>) -> bool {
    match date {
        None => true,
        Some(d) => {
            let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
            d == today
        }
    }
}

// ── Type mapping ────────────────────────────────────────────────────────────

fn map_page(page: papers_cool::PapersCoolPage) -> PapersCoolPageResponse {
    PapersCoolPageResponse {
        title: page.title,
        total: page.total,
        papers: page.papers.into_iter().map(map_paper).collect(),
    }
}

fn map_paper(p: papers_cool::PapersCoolPaper) -> PapersCoolPaperResponse {
    PapersCoolPaperResponse {
        external_id: p.external_id,
        title: p.title,
        authors: p.authors,
        abstract_text: p.abstract_text,
        categories: p
            .categories
            .into_iter()
            .map(|c| PapersCoolCategoryResponse {
                code: c.code,
                name: c.name,
            })
            .collect(),
        published_date: p.published_date,
        pdf_url: p.pdf_url,
        abs_url: p.abs_url,
        papers_cool_url: p.papers_cool_url,
        pdf_opens: p.pdf_opens,
        kimi_opens: p.kimi_opens,
        keywords: p.keywords,
    }
}

fn map_index(index: papers_cool::PapersCoolIndex) -> PapersCoolIndexResponse {
    PapersCoolIndexResponse {
        arxiv_groups: index
            .arxiv_groups
            .into_iter()
            .map(|g| ArxivGroupResponse {
                name: g.name,
                categories: g
                    .categories
                    .into_iter()
                    .map(|c| ArxivCategoryResponse {
                        code: c.code,
                        name: c.name,
                    })
                    .collect(),
            })
            .collect(),
        venues: index
            .venues
            .into_iter()
            .map(|v| VenueConferenceResponse {
                name: v.name,
                editions: v
                    .editions
                    .into_iter()
                    .map(|e| VenueEditionResponse {
                        key: e.key,
                        year: e.year,
                        groups: e
                            .groups
                            .into_iter()
                            .map(|g| VenueGroupResponse {
                                name: g.name,
                                query: g.query,
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect(),
    }
}
