// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
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

        // Sleep for poll interval (default 60 min)
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}
