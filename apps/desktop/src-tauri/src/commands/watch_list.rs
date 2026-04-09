// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::AppState;
use tauri::State;
use zoro_db::queries::watch_lists as wl_queries;

// ── Response types ──────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
pub struct WatchListResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub poll_interval_minutes: i32,
    pub last_polled: Option<String>,
    pub created_date: String,
    pub item_count: i64,
    pub new_result_count: i64,
}

#[derive(Debug, serde::Serialize)]
pub struct WatchListItemResponse {
    pub id: String,
    pub list_id: String,
    pub item_type: String,
    pub external_id: String,
    pub source: String,
    pub display_name: String,
    pub config: Option<serde_json::Value>,
    pub last_checked: Option<String>,
    pub created_date: String,
}

#[derive(Debug, serde::Serialize)]
pub struct WatchListResultResponse {
    pub id: String,
    pub list_id: String,
    pub item_id: String,
    pub item_type: String,
    pub external_id: String,
    pub title: String,
    pub authors: Vec<WatchListResultAuthor>,
    pub abstract_text: Option<String>,
    pub url: Option<String>,
    pub pdf_url: Option<String>,
    pub published_date: Option<String>,
    pub fetched_date: String,
    pub added_to_library: bool,
    pub paper_id: Option<String>,
    /// Display name of the watch list item that triggered this result.
    pub source_display_name: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct WatchListResultAuthor {
    pub name: String,
}

/// Helper to extract an optional string from a JSON value.
fn json_str(data: &serde_json::Value, key: &str) -> Option<String> {
    data.get(key).and_then(|v| v.as_str()).map(String::from)
}

fn map_result_row(
    r: wl_queries::WatchListResultRow,
    items_map: &std::collections::HashMap<String, String>,
) -> WatchListResultResponse {
    let data: serde_json::Value = r
        .data_json
        .as_deref()
        .and_then(|d| serde_json::from_str(d).ok())
        .unwrap_or(serde_json::json!({}));

    let authors = data
        .get("authors")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| {
                    a.get("name")
                        .or_else(|| a.as_str().map(|_| a))
                        .and_then(|n| n.as_str())
                        .map(|name| WatchListResultAuthor {
                            name: name.to_string(),
                        })
                })
                .collect()
        })
        .unwrap_or_default();

    WatchListResultResponse {
        id: r.id,
        list_id: r.list_id,
        item_id: r.item_id.clone(),
        item_type: r.item_type,
        external_id: r.external_id,
        title: r.title,
        authors,
        abstract_text: json_str(&data, "abstract_text"),
        url: json_str(&data, "url"),
        pdf_url: json_str(&data, "pdf_url"),
        published_date: r
            .published_date
            .or_else(|| json_str(&data, "published_date")),
        fetched_date: r.fetched_date,
        added_to_library: r.added_to_library,
        paper_id: r.paper_id,
        source_display_name: items_map.get(&r.item_id).cloned(),
    }
}

// ── Watch List CRUD commands ────────────────────────────────────────────────

#[tauri::command]
pub async fn list_watch_lists(
    state: State<'_, AppState>,
) -> Result<Vec<WatchListResponse>, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let lists = wl_queries::list_watch_lists(&db.conn).map_err(|e| format!("{}", e))?;

    let mut result = Vec::new();
    for list in lists {
        let items =
            wl_queries::list_watch_list_items(&db.conn, &list.id).map_err(|e| format!("{}", e))?;
        let (_, new_count) = wl_queries::count_watch_list_results(&db.conn, Some(&list.id))
            .map_err(|e| format!("{}", e))?;
        result.push(WatchListResponse {
            id: list.id,
            name: list.name,
            description: list.description,
            poll_interval_minutes: list.poll_interval_minutes,
            last_polled: list.last_polled,
            created_date: list.created_date,
            item_count: items.len() as i64,
            new_result_count: new_count,
        });
    }
    Ok(result)
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateWatchListInput {
    pub name: String,
    pub description: Option<String>,
    pub poll_interval_minutes: Option<i32>,
}

#[tauri::command]
pub async fn create_watch_list(
    state: State<'_, AppState>,
    input: CreateWatchListInput,
) -> Result<WatchListResponse, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let list = wl_queries::create_watch_list(
        &db.conn,
        &input.name,
        input.description.as_deref(),
        input.poll_interval_minutes.unwrap_or(360),
    )
    .map_err(|e| format!("{}", e))?;
    Ok(WatchListResponse {
        id: list.id,
        name: list.name,
        description: list.description,
        poll_interval_minutes: list.poll_interval_minutes,
        last_polled: list.last_polled,
        created_date: list.created_date,
        item_count: 0,
        new_result_count: 0,
    })
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateWatchListInput {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub poll_interval_minutes: Option<i32>,
}

#[tauri::command]
pub async fn update_watch_list(
    state: State<'_, AppState>,
    id: String,
    input: UpdateWatchListInput,
) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    wl_queries::update_watch_list(
        &db.conn,
        &id,
        input.name.as_deref(),
        input.description.as_ref().map(|d| d.as_deref()),
        input.poll_interval_minutes,
    )
    .map_err(|e| format!("{}", e))
}

#[tauri::command]
pub async fn delete_watch_list(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    wl_queries::delete_watch_list(&db.conn, &id).map_err(|e| format!("{}", e))
}

// ── Watch List Items commands ───────────────────────────────────────────────

#[tauri::command]
pub async fn list_watch_list_items(
    state: State<'_, AppState>,
    list_id: String,
) -> Result<Vec<WatchListItemResponse>, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let items =
        wl_queries::list_watch_list_items(&db.conn, &list_id).map_err(|e| format!("{}", e))?;
    Ok(items
        .into_iter()
        .map(|i| WatchListItemResponse {
            id: i.id,
            list_id: i.list_id,
            item_type: i.item_type,
            external_id: i.external_id,
            source: i.source,
            display_name: i.display_name,
            config: i.config_json.and_then(|s| serde_json::from_str(&s).ok()),
            last_checked: i.last_checked,
            created_date: i.created_date,
        })
        .collect())
}

#[derive(Debug, serde::Deserialize)]
pub struct AddWatchListItemInput {
    pub list_id: String,
    pub item_type: String,
    pub external_id: String,
    pub source: String,
    pub display_name: String,
    pub config: Option<serde_json::Value>,
}

#[tauri::command]
pub async fn add_watch_list_item(
    state: State<'_, AppState>,
    input: AddWatchListItemInput,
) -> Result<WatchListItemResponse, String> {
    let config_str = input
        .config
        .as_ref()
        .and_then(|c| serde_json::to_string(c).ok());
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let item = wl_queries::add_watch_list_item(
        &db.conn,
        &input.list_id,
        &input.item_type,
        &input.external_id,
        &input.source,
        &input.display_name,
        config_str.as_deref(),
    )
    .map_err(|e| format!("{}", e))?;
    Ok(WatchListItemResponse {
        id: item.id,
        list_id: item.list_id,
        item_type: item.item_type,
        external_id: item.external_id,
        source: item.source,
        display_name: item.display_name,
        config: input.config,
        last_checked: item.last_checked,
        created_date: item.created_date,
    })
}

#[tauri::command]
pub async fn delete_watch_list_item(
    state: State<'_, AppState>,
    item_id: String,
) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    wl_queries::delete_watch_list_item(&db.conn, &item_id).map_err(|e| format!("{}", e))
}

// ── Watch List Results commands ─────────────────────────────────────────────

#[tauri::command]
pub async fn list_watch_list_results(
    state: State<'_, AppState>,
    list_id: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<WatchListResultResponse>, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;

    let lim = limit.unwrap_or(100);
    let off = offset.unwrap_or(0);

    let rows = if let Some(lid) = &list_id {
        wl_queries::list_watch_list_results(&db.conn, lid, lim, off)
            .map_err(|e| format!("{}", e))?
    } else {
        wl_queries::list_all_watch_list_results(&db.conn, lim, off).map_err(|e| format!("{}", e))?
    };

    // Build a map of item_id -> display_name for source attribution
    let mut items_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    if let Some(lid) = &list_id {
        if let Ok(items) = wl_queries::list_watch_list_items(&db.conn, lid) {
            for item in items {
                items_map.insert(item.id, item.display_name);
            }
        }
    } else {
        // For "all" view, we need items from all lists
        if let Ok(lists) = wl_queries::list_watch_lists(&db.conn) {
            for list in lists {
                if let Ok(items) = wl_queries::list_watch_list_items(&db.conn, &list.id) {
                    for item in items {
                        items_map.insert(item.id, item.display_name);
                    }
                }
            }
        }
    }

    Ok(rows
        .into_iter()
        .map(|r| map_result_row(r, &items_map))
        .collect())
}

#[tauri::command]
pub async fn add_watch_list_result_to_library(
    state: State<'_, AppState>,
    result_id: String,
) -> Result<String, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;

    let result =
        wl_queries::get_watch_list_result(&db.conn, &result_id).map_err(|e| format!("{}", e))?;

    if result.added_to_library {
        return Err("Result already added to library".to_string());
    }

    let data: serde_json::Value = result
        .data_json
        .as_deref()
        .and_then(|d| serde_json::from_str(d).ok())
        .unwrap_or(serde_json::json!({}));

    let title = result.title.clone();
    let doi = json_str(&data, "doi");
    let arxiv_id = json_str(&data, "arxiv_id");
    let url = json_str(&data, "url");
    let pdf_url = json_str(&data, "pdf_url");
    let abstract_text = json_str(&data, "abstract_text");
    let published_date = result
        .published_date
        .clone()
        .or_else(|| json_str(&data, "published_date"));

    let authors_data: Vec<(String, Option<String>, Option<String>)> = data
        .get("authors")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| {
                    let name = a
                        .get("name")
                        .and_then(|n| n.as_str())
                        .or_else(|| a.as_str())
                        .map(String::from)?;
                    Some((name, None, None))
                })
                .collect()
        })
        .unwrap_or_default();

    let slug = zoro_core::slug_utils::generate_paper_slug(
        &title,
        doi.as_deref()
            .or(arxiv_id.as_deref())
            .unwrap_or(&result.external_id),
        published_date.as_deref(),
    );

    let papers_dir = state.data_dir.join("library/papers");
    let paper_dir = crate::storage::paper_dir::create_paper_dir(&papers_dir, &slug)
        .map_err(|e| format!("Failed to create paper dir: {}", e))?;

    let db_input = zoro_db::queries::papers::CreatePaperInput {
        slug: slug.clone(),
        title: title.clone(),
        short_title: None,
        abstract_text: abstract_text.clone(),
        doi: doi.clone(),
        arxiv_id: arxiv_id.clone(),
        url: url.clone(),
        pdf_url: pdf_url.clone(),
        html_url: None,
        thumbnail_url: None,
        published_date: published_date.clone(),
        source: Some("watch-list".to_string()),
        dir_path: format!("papers/{}", slug),
        extra_json: None,
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

    let row = zoro_db::queries::papers::insert_paper(&db.conn, &db_input)
        .map_err(|e| format!("Failed to insert paper: {}", e))?;

    zoro_db::queries::papers::set_paper_authors(&db.conn, &row.id, &authors_data)
        .map_err(|e| format!("Failed to set authors: {}", e))?;

    // Write metadata.json
    let metadata = zoro_core::models::PaperMetadata {
        id: row.id.clone(),
        slug: slug.clone(),
        title,
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
        doi: doi.clone(),
        arxiv_id: arxiv_id.clone(),
        url,
        pdf_url: pdf_url.clone(),
        html_url: None,
        thumbnail_url: None,
        published_date,
        added_date: row.added_date.clone(),
        source: Some("watch-list".to_string()),
        tags: Vec::new(),
        collections: Vec::new(),
        attachments: Vec::new(),
        notes: Vec::new(),
        read_status: zoro_core::models::ReadStatus::Unread,
        rating: None,
        extra: serde_json::json!({}),
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
    let _ = crate::storage::paper_dir::write_metadata(&paper_dir, &metadata);

    wl_queries::mark_watch_list_result_added(&db.conn, &result_id, &row.id)
        .map_err(|e| format!("{}", e))?;

    crate::storage::sync::rebuild_library_index(&db, &state.data_dir);

    // Background: download PDF if available
    if let Some(pdf) = pdf_url {
        let pdf_path = paper_dir.join("paper.pdf");
        let paper_id_clone = row.id.clone();
        let db_path = state.data_dir.join("library.db");
        tokio::spawn(async move {
            if let Ok(()) = crate::storage::attachments::download_file(&pdf, &pdf_path).await {
                let file_size = crate::storage::attachments::get_file_size(&pdf_path);
                if let Ok(dl_db) = zoro_db::Database::open(&db_path) {
                    let _ = zoro_db::queries::attachments::insert_attachment(
                        &dl_db.conn,
                        &paper_id_clone,
                        "paper.pdf",
                        "pdf",
                        Some("application/pdf"),
                        file_size,
                        "paper.pdf",
                        "watch-list",
                    );
                }
            }
        });
    }

    // Background: enrich metadata
    if arxiv_id.is_some() || doi.is_some() {
        let enrich_paper_id = row.id.clone();
        let enrich_doi = doi;
        let enrich_arxiv = arxiv_id;
        let enrich_db_path = state.data_dir.join("library.db");
        tokio::spawn(async move {
            match zoro_metadata::enrich_paper(enrich_doi.as_deref(), enrich_arxiv.as_deref()).await
            {
                Ok(enrichment) => {
                    if let Ok(enrich_db) = zoro_db::Database::open(&enrich_db_path) {
                        if let Ok(current) =
                            zoro_db::queries::papers::get_paper(&enrich_db.conn, &enrich_paper_id)
                        {
                            let update = zoro_db::queries::papers::UpdatePaperInput {
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
                            let _ = zoro_db::queries::papers::update_paper(
                                &enrich_db.conn,
                                &enrich_paper_id,
                                &update,
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!("Background enrichment failed for watch list result: {}", e);
                }
            }
        });
    }

    Ok(row.id)
}

// ── Search commands (for adding authors/papers to watch lists) ──────────────

#[derive(Debug, serde::Serialize)]
pub struct AuthorSearchResult {
    pub name: String,
    pub external_id: String,
    pub source: String,
    pub notes: Option<String>,
    pub paper_count: Option<i32>,
    pub citation_count: Option<i32>,
}

#[tauri::command]
pub async fn search_authors_for_watch_list(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<AuthorSearchResult>, String> {
    let mut results = Vec::new();

    // Always search DBLP (free, no key needed)
    match zoro_subscriptions::dblp::search_authors(&query, 10).await {
        Ok(suggestions) => {
            for s in suggestions {
                results.push(AuthorSearchResult {
                    name: s.name,
                    external_id: s.url,
                    source: "dblp".to_string(),
                    notes: s.notes,
                    paper_count: None,
                    citation_count: None,
                });
            }
        }
        Err(e) => tracing::warn!("DBLP author search failed: {}", e),
    }

    // Also search Semantic Scholar if API key is configured
    let s2_key = {
        let config = state
            .config
            .lock()
            .map_err(|e| format!("Config lock error: {}", e))?;
        config
            .subscriptions
            .watch_list_api_keys
            .semantic_scholar
            .clone()
    };

    if !s2_key.is_empty() {
        match zoro_subscriptions::semantic_scholar::search_authors(&query, &s2_key, 10).await {
            Ok(suggestions) => {
                for s in suggestions {
                    results.push(AuthorSearchResult {
                        name: s.name,
                        external_id: s.author_id,
                        source: "semantic-scholar".to_string(),
                        notes: if s.affiliations.is_empty() {
                            None
                        } else {
                            Some(s.affiliations.join(", "))
                        },
                        paper_count: Some(s.paper_count),
                        citation_count: Some(s.citation_count),
                    });
                }
            }
            Err(e) => tracing::warn!("Semantic Scholar author search failed: {}", e),
        }
    }

    Ok(results)
}

// ── Refresh (manual poll) command ───────────────────────────────────────────

#[tauri::command]
pub async fn refresh_watch_list(
    state: State<'_, AppState>,
    list_id: String,
) -> Result<i32, String> {
    let (items, s2_key) = {
        let db = state
            .db
            .lock()
            .map_err(|e| format!("DB lock error: {}", e))?;
        let items =
            wl_queries::list_watch_list_items(&db.conn, &list_id).map_err(|e| format!("{}", e))?;
        let config = state
            .config
            .lock()
            .map_err(|e| format!("Config lock error: {}", e))?;
        let s2_key = config
            .subscriptions
            .watch_list_api_keys
            .semantic_scholar
            .clone();
        (items, s2_key)
    };

    let mut total_new = 0i32;

    for item in &items {
        let new_count =
            crate::subscriptions::poll_watch_list_item(&state, &list_id, item, &s2_key).await;
        total_new += new_count;
    }

    // Update last_polled
    {
        let db = state
            .db
            .lock()
            .map_err(|e| format!("DB lock error: {}", e))?;
        let _ = wl_queries::update_watch_list_last_polled(&db.conn, &list_id);
    }

    Ok(total_new)
}

// ── API Keys config commands ────────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
pub struct WatchListApiKeysResponse {
    pub semantic_scholar_set: bool,
    pub openalex_email: String,
}

#[tauri::command]
pub async fn get_watch_list_api_keys(
    state: State<'_, AppState>,
) -> Result<WatchListApiKeysResponse, String> {
    let config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?;
    Ok(WatchListApiKeysResponse {
        semantic_scholar_set: !config
            .subscriptions
            .watch_list_api_keys
            .semantic_scholar
            .is_empty(),
        openalex_email: config
            .subscriptions
            .watch_list_api_keys
            .openalex_email
            .clone(),
    })
}

#[tauri::command]
pub async fn update_watch_list_api_keys(
    state: State<'_, AppState>,
    semantic_scholar: Option<String>,
    openalex_email: Option<String>,
) -> Result<(), String> {
    let mut config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?;

    if let Some(key) = semantic_scholar {
        config.subscriptions.watch_list_api_keys.semantic_scholar = key;
    }
    if let Some(email) = openalex_email {
        config.subscriptions.watch_list_api_keys.openalex_email = email;
    }

    crate::storage::config::save_config(&state.data_dir, &config)
        .map_err(|e| format!("Failed to save config: {}", e))
}
