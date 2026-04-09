// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use zoro_db::queries::watch_lists as wl_queries;
use zoro_subscriptions::{build_item_data_json, HuggingFaceDailyPapers};

pub async fn start_poller(app: AppHandle) {
    tracing::info!("Starting subscription poller");

    loop {
        // Wait before first poll
        tokio::time::sleep(Duration::from_secs(30)).await;

        let app_state: tauri::State<crate::AppState> = app.state();

        let subscriptions = {
            let db = match app_state.db.lock() {
                Ok(db) => db,
                Err(_) => continue,
            };
            zoro_db::queries::subscriptions::list_subscriptions(&db.conn).unwrap_or_default()
        };

        // Read retention config for cache cleanup
        let retention_days = app_state
            .config
            .lock()
            .map(|c| c.subscriptions.feed_cache_retention_days)
            .unwrap_or(7);

        for sub in &subscriptions {
            if !sub.enabled {
                continue;
            }

            match sub.source_type.as_str() {
                "huggingface-daily" => {
                    let source = HuggingFaceDailyPapers::new();
                    // Resolve the latest date first, then fetch by date so
                    // poller results are consistent with the date-based API.
                    let latest_date = match zoro_subscriptions::fetch_latest_date().await {
                        Ok(Some(d)) => d,
                        Ok(None) => chrono::Utc::now().format("%Y-%m-%d").to_string(),
                        Err(e) => {
                            tracing::error!("Failed to fetch latest date for {}: {}", sub.name, e);
                            continue;
                        }
                    };
                    match source.fetch_by_date(&latest_date).await {
                        Ok(items) => {
                            // Collect thumbnail URLs before entering DB lock scope
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

                            // Scope the DB lock so it's released before emitting events
                            let new_count = {
                                let db = match app_state.db.lock() {
                                    Ok(db) => db,
                                    Err(_) => continue,
                                };
                                let mut count = 0;
                                for item in &items {
                                    let data_json = build_item_data_json(item);
                                    let source_date = zoro_subscriptions::extract_source_date(item);
                                    match zoro_db::queries::subscriptions::insert_subscription_item(
                                        &db.conn,
                                        &sub.id,
                                        &item.external_id,
                                        &item.title,
                                        data_json.as_deref(),
                                        source_date.as_deref(),
                                    ) {
                                        Ok(_) => count += 1,
                                        Err(zoro_db::DbError::Duplicate(_)) => {}
                                        Err(e) => {
                                            tracing::warn!("Failed to insert item: {}", e)
                                        }
                                    }
                                }
                                let _ = zoro_db::queries::subscriptions::update_last_polled(
                                    &db.conn, &sub.id,
                                );

                                // Cleanup old cached items based on retention policy
                                if retention_days > 0 {
                                    match zoro_db::queries::subscriptions::delete_old_subscription_items(
                                        &db.conn,
                                        &sub.id,
                                        retention_days,
                                    ) {
                                        Ok(deleted) if deleted > 0 => {
                                            tracing::info!(
                                                "Cleaned up {} old cached items for {}",
                                                deleted,
                                                sub.name
                                            );
                                        }
                                        Ok(_) => {}
                                        Err(e) => {
                                            tracing::warn!(
                                                "Failed to cleanup old items for {}: {}",
                                                sub.name,
                                                e
                                            );
                                        }
                                    }
                                }

                                count
                            };
                            // Emit after the lock is released so the frontend
                            // can immediately query list_feed_items without contention
                            if new_count > 0 {
                                tracing::info!("Fetched {} new items for {}", new_count, sub.name);
                                let _ = app.emit("subscription-updated", &sub.id);
                            }

                            // Background-cache thumbnail images for offline access
                            if !thumbnail_urls.is_empty() {
                                let data_dir = app_state.data_dir.clone();
                                tokio::spawn(async move {
                                    for url in thumbnail_urls {
                                        let _ = crate::commands::subscription::download_and_cache_thumbnail_bg(
                                            data_dir.clone(),
                                            url,
                                        )
                                        .await;
                                    }
                                });
                            }
                        }
                        Err(e) => tracing::error!("Failed to fetch {}: {}", sub.name, e),
                    }
                }
                _ => tracing::warn!("Unknown subscription source type: {}", sub.source_type),
            }
        }

        // ── Watch List polling ──────────────────────────────────────────
        poll_all_watch_lists(&app).await;

        // Sleep for poll interval (default 60 min)
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}

// ── Watch List polling helpers ──────────────────────────────────────────────

/// Poll all watch lists that are due for a refresh.
async fn poll_all_watch_lists(app: &AppHandle) {
    let app_state: tauri::State<crate::AppState> = app.state();

    let (watch_lists, s2_key, openalex_email, db_arc) = {
        let db = match app_state.db.lock() {
            Ok(db) => db,
            Err(_) => return,
        };
        let lists = wl_queries::list_watch_lists(&db.conn).unwrap_or_default();
        let config = match app_state.config.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        let key = config
            .subscriptions
            .watch_list_api_keys
            .semantic_scholar
            .clone();
        let email = config
            .subscriptions
            .watch_list_api_keys
            .openalex_email
            .clone();
        drop(db);
        drop(config);
        (lists, key, email, app_state.db.clone())
    };

    if watch_lists.is_empty() {
        return;
    }

    let now = chrono::Utc::now();

    for list in &watch_lists {
        // Check if this list is due for polling based on its interval
        let should_poll = match &list.last_polled {
            Some(last) => {
                if let Ok(last_time) = chrono::DateTime::parse_from_rfc3339(last) {
                    let elapsed = now.signed_duration_since(last_time).num_minutes();
                    elapsed >= list.poll_interval_minutes as i64
                } else {
                    true
                }
            }
            None => true, // Never polled
        };

        if !should_poll {
            continue;
        }

        tracing::info!("Polling watch list: {}", list.name);

        let items = {
            let db = match db_arc.lock() {
                Ok(db) => db,
                Err(_) => continue,
            };
            wl_queries::list_watch_list_items(&db.conn, &list.id).unwrap_or_default()
        };

        let mut total_new = 0i32;
        for item in &items {
            let new_count =
                poll_watch_list_item_core(&db_arc, &list.id, item, &s2_key, &openalex_email).await;
            total_new += new_count;
        }

        // Update last_polled
        {
            let db = match db_arc.lock() {
                Ok(db) => db,
                Err(_) => continue,
            };
            let _ = wl_queries::update_watch_list_last_polled(&db.conn, &list.id);
        }

        if total_new > 0 {
            tracing::info!("Watch list '{}': {} new results", list.name, total_new);
            let _ = app.emit("watch-list-updated", &list.id);
        }
    }
}

/// Poll a single watch list item. Called from the manual `refresh_watch_list`
/// command. Extracts the needed data from `AppState` and delegates to the
/// core polling function.
///
/// Returns the number of new results inserted.
pub async fn poll_watch_list_item(
    state: &tauri::State<'_, crate::AppState>,
    list_id: &str,
    item: &wl_queries::WatchListItemRow,
    s2_key: &str,
) -> i32 {
    let (db_arc, openalex_email) = {
        let email = state
            .config
            .lock()
            .ok()
            .map(|c| c.subscriptions.watch_list_api_keys.openalex_email.clone())
            .unwrap_or_default();
        (state.db.clone(), email)
    };
    poll_watch_list_item_core(&db_arc, list_id, item, s2_key, &openalex_email).await
}

/// Core polling logic for a single watch list item.
/// Accepts `Arc<Mutex<Database>>` instead of `tauri::State` so the future is `Send`.
async fn poll_watch_list_item_core(
    db: &std::sync::Arc<std::sync::Mutex<zoro_db::Database>>,
    list_id: &str,
    item: &wl_queries::WatchListItemRow,
    s2_key: &str,
    openalex_email: &str,
) -> i32 {
    let new_count = match item.item_type.as_str() {
        "author" => poll_author_item(db, list_id, item, s2_key, openalex_email).await,
        "seed-paper" => poll_seed_paper_item(db, list_id, item, s2_key, openalex_email).await,
        other => {
            tracing::warn!("Unknown watch list item type: {}", other);
            0
        }
    };

    // Update last_checked for this item
    {
        let db = match db.lock() {
            Ok(db) => db,
            Err(_) => return new_count,
        };
        let _ = wl_queries::update_watch_list_item_last_checked(&db.conn, &item.id);
    }

    new_count
}

/// Poll an author item: fetch their recent publications.
async fn poll_author_item(
    db: &std::sync::Arc<std::sync::Mutex<zoro_db::Database>>,
    list_id: &str,
    item: &wl_queries::WatchListItemRow,
    s2_key: &str,
    openalex_email: &str,
) -> i32 {
    let use_s2 = !s2_key.is_empty() && item.source == "semantic-scholar";

    if use_s2 {
        // Use Semantic Scholar
        match zoro_subscriptions::semantic_scholar::fetch_author_papers(
            &item.external_id,
            s2_key,
            50,
        )
        .await
        {
            Ok(papers) => insert_s2_papers_as_results(db, list_id, &item.id, "author", &papers),
            Err(e) => {
                tracing::warn!("S2 author poll failed for '{}': {}", item.display_name, e);
                0
            }
        }
    } else {
        // Use DBLP — search by author name
        let query = format!("author:{}", item.display_name);
        match zoro_subscriptions::dblp::search_author_publications(&query, 50).await {
            Ok(papers) => {
                insert_dblp_papers_as_results(db, list_id, &item.id, &papers, openalex_email).await
            }
            Err(e) => {
                tracing::warn!("DBLP author poll failed for '{}': {}", item.display_name, e);
                0
            }
        }
    }
}

/// Poll a seed-paper item: fetch papers that cite it.
async fn poll_seed_paper_item(
    db: &std::sync::Arc<std::sync::Mutex<zoro_db::Database>>,
    list_id: &str,
    item: &wl_queries::WatchListItemRow,
    s2_key: &str,
    openalex_email: &str,
) -> i32 {
    let use_s2 = !s2_key.is_empty();

    if use_s2 {
        // Use Semantic Scholar citations endpoint
        // external_id could be a DOI or S2 paper ID
        match zoro_subscriptions::semantic_scholar::fetch_citations(&item.external_id, s2_key, 100)
            .await
        {
            Ok(papers) => insert_s2_papers_as_results(db, list_id, &item.id, "seed-paper", &papers),
            Err(e) => {
                tracing::warn!("S2 citation poll failed for '{}': {}", item.display_name, e);
                0
            }
        }
    } else {
        // Use OpenCitations (requires DOI)
        let doi = &item.external_id;
        if !doi.starts_with("10.") {
            tracing::warn!(
                "Seed paper '{}' has no DOI (external_id={}), skipping OpenCitations poll",
                item.display_name,
                doi
            );
            return 0;
        }

        match zoro_subscriptions::opencitations::fetch_citations(doi, None).await {
            Ok(records) => {
                insert_citation_records_as_results(db, list_id, &item.id, &records, openalex_email)
                    .await
            }
            Err(e) => {
                tracing::warn!(
                    "OpenCitations poll failed for '{}': {}",
                    item.display_name,
                    e
                );
                0
            }
        }
    }
}

/// Insert Semantic Scholar papers as watch list results.
fn insert_s2_papers_as_results(
    db: &std::sync::Arc<std::sync::Mutex<zoro_db::Database>>,
    list_id: &str,
    item_id: &str,
    item_type: &str,
    papers: &[zoro_subscriptions::semantic_scholar::S2Paper],
) -> i32 {
    let db = match db.lock() {
        Ok(db) => db,
        Err(_) => return 0,
    };

    let mut count = 0;
    for paper in papers {
        if paper.title.is_empty() {
            continue;
        }

        // Use DOI as external_id if available, otherwise S2 paper ID
        let external_id = paper.doi.as_deref().unwrap_or(&paper.paper_id);

        let authors_json: Vec<serde_json::Value> = paper
            .authors
            .iter()
            .map(|a| serde_json::json!({ "name": a.name }))
            .collect();

        let data = serde_json::json!({
            "authors": authors_json,
            "abstract_text": paper.abstract_text,
            "doi": paper.doi,
            "arxiv_id": paper.arxiv_id,
            "url": paper.url,
            "pdf_url": paper.pdf_url,
            "published_date": paper.publication_date,
            "citation_count": paper.citation_count,
            "s2_paper_id": paper.paper_id,
        });
        let data_str = serde_json::to_string(&data).ok();

        match wl_queries::insert_watch_list_result(
            &db.conn,
            list_id,
            item_id,
            item_type,
            external_id,
            &paper.title,
            data_str.as_deref(),
            paper.publication_date.as_deref(),
        ) {
            Ok(_) => count += 1,
            Err(zoro_db::DbError::Duplicate(_)) => {}
            Err(e) => {
                tracing::warn!("Failed to insert S2 watch list result: {}", e);
            }
        }
    }
    count
}

/// Insert DBLP papers as watch list results, then enrich via OpenAlex.
async fn insert_dblp_papers_as_results(
    db: &std::sync::Arc<std::sync::Mutex<zoro_db::Database>>,
    list_id: &str,
    item_id: &str,
    papers: &[zoro_subscriptions::dblp::DblpPaper],
    openalex_email: &str,
) -> i32 {
    let (count, dois_to_enrich) = {
        let db = match db.lock() {
            Ok(db) => db,
            Err(_) => return 0,
        };

        let mut count = 0;
        let mut dois_to_enrich: Vec<String> = Vec::new();

        for paper in papers {
            if paper.title.is_empty() {
                continue;
            }

            // Use DOI as external_id if available, otherwise DBLP key
            let external_id = paper
                .doi
                .as_deref()
                .or(paper.dblp_key.as_deref())
                .unwrap_or(&paper.title);

            let authors_json: Vec<serde_json::Value> = paper
                .authors
                .iter()
                .map(|a| serde_json::json!({ "name": a }))
                .collect();

            let published_date = paper.year.as_deref().map(|y| format!("{}-01-01", y));

            let data = serde_json::json!({
                "authors": authors_json,
                "doi": paper.doi,
                "url": paper.dblp_url,
                "venue": paper.venue,
                "year": paper.year,
                "dblp_key": paper.dblp_key,
                "pub_type": paper.pub_type,
            });
            let data_str = serde_json::to_string(&data).ok();

            match wl_queries::insert_watch_list_result(
                &db.conn,
                list_id,
                item_id,
                "author",
                external_id,
                &paper.title,
                data_str.as_deref(),
                published_date.as_deref(),
            ) {
                Ok(_) => {
                    count += 1;
                    if let Some(doi) = &paper.doi {
                        dois_to_enrich.push(doi.clone());
                    }
                }
                Err(zoro_db::DbError::Duplicate(_)) => {}
                Err(e) => {
                    tracing::warn!("Failed to insert DBLP watch list result: {}", e);
                }
            }
        }

        (count, dois_to_enrich)
    }; // DB lock dropped here

    // Background-enrich new results via OpenAlex
    if !dois_to_enrich.is_empty() {
        let api_key = if openalex_email.is_empty() {
            None
        } else {
            Some(openalex_email)
        };

        // Enrich in batches of 50 (OpenAlex limit)
        for chunk in dois_to_enrich.chunks(50) {
            let doi_refs: Vec<&str> = chunk.iter().map(|s| s.as_str()).collect();
            match zoro_subscriptions::openalex::fetch_works_by_dois(&doi_refs, api_key).await {
                Ok(works) => {
                    let db = match db.lock() {
                        Ok(db) => db,
                        Err(_) => continue,
                    };
                    for work in &works {
                        if let Some(doi) = &work.doi {
                            // Clean DOI (OpenAlex returns full URL)
                            let clean_doi = doi.strip_prefix("https://doi.org/").unwrap_or(doi);
                            enrich_result_with_openalex(&db.conn, list_id, clean_doi, work);
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!("OpenAlex enrichment failed: {}", e);
                }
            }
        }
    }

    count
}

/// Insert OpenCitations citation records as watch list results, then enrich.
async fn insert_citation_records_as_results(
    db: &std::sync::Arc<std::sync::Mutex<zoro_db::Database>>,
    list_id: &str,
    item_id: &str,
    records: &[zoro_subscriptions::opencitations::CitationRecord],
    openalex_email: &str,
) -> i32 {
    let (count, dois_to_enrich) = {
        let db = match db.lock() {
            Ok(db) => db,
            Err(_) => return 0,
        };

        let mut count = 0;
        let mut dois_to_enrich: Vec<String> = Vec::new();

        for record in records {
            for citing_doi in &record.citing_dois {
                if citing_doi.is_empty() {
                    continue;
                }

                let data = serde_json::json!({
                    "doi": citing_doi,
                    "published_date": record.creation,
                });
                let data_str = serde_json::to_string(&data).ok();

                // Use DOI as both external_id and a placeholder title
                match wl_queries::insert_watch_list_result(
                    &db.conn,
                    list_id,
                    item_id,
                    "seed-paper",
                    citing_doi,
                    citing_doi, // Placeholder title — will be enriched
                    data_str.as_deref(),
                    record.creation.as_deref(),
                ) {
                    Ok(_) => {
                        count += 1;
                        dois_to_enrich.push(citing_doi.clone());
                    }
                    Err(zoro_db::DbError::Duplicate(_)) => {}
                    Err(e) => {
                        tracing::warn!("Failed to insert citation watch list result: {}", e);
                    }
                }
            }
        }

        (count, dois_to_enrich)
    }; // DB lock dropped here

    // Enrich via OpenAlex to get actual titles, authors, abstracts
    if !dois_to_enrich.is_empty() {
        let api_key = if openalex_email.is_empty() {
            None
        } else {
            Some(openalex_email)
        };

        for chunk in dois_to_enrich.chunks(50) {
            let doi_refs: Vec<&str> = chunk.iter().map(|s| s.as_str()).collect();
            match zoro_subscriptions::openalex::fetch_works_by_dois(&doi_refs, api_key).await {
                Ok(works) => {
                    let db = match db.lock() {
                        Ok(db) => db,
                        Err(_) => continue,
                    };
                    for work in &works {
                        if let Some(doi) = &work.doi {
                            let clean_doi = doi.strip_prefix("https://doi.org/").unwrap_or(doi);
                            enrich_result_with_openalex(&db.conn, list_id, clean_doi, work);
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!("OpenAlex enrichment failed: {}", e);
                }
            }
        }
    }

    count
}

/// Update an existing watch list result with richer metadata from OpenAlex.
fn enrich_result_with_openalex(
    conn: &rusqlite::Connection,
    list_id: &str,
    doi: &str,
    work: &zoro_subscriptions::openalex::OpenAlexWork,
) {
    let title = work.title.as_deref().unwrap_or(doi);

    let authors_json: Vec<serde_json::Value> = work
        .authors
        .iter()
        .map(|a| serde_json::json!({ "name": a.name }))
        .collect();

    let data = serde_json::json!({
        "authors": authors_json,
        "abstract_text": work.abstract_text,
        "doi": doi,
        "url": work.landing_page_url,
        "pdf_url": work.pdf_url,
        "published_date": work.publication_date,
        "cited_by_count": work.cited_by_count,
        "openalex_id": work.openalex_id,
    });
    let data_str = serde_json::to_string(&data).unwrap_or_default();

    // Update the result row with enriched data
    let _ = conn.execute(
        "UPDATE watch_list_results
            SET title = ?1, data_json = ?2,
                published_date = COALESCE(?3, published_date)
          WHERE list_id = ?4 AND external_id = ?5",
        rusqlite::params![title, data_str, work.publication_date, list_id, doi,],
    );
}
