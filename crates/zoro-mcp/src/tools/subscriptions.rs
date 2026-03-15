// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::sync::Arc;

use rmcp::model::*;
use serde_json::json;

use crate::state::AppState;
use zoro_db::queries::subscriptions as sub_queries;
use zoro_subscriptions::{build_item_data_json, HuggingFaceDailyPapers};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListFeedItemsInput {
    /// Subscription ID
    pub subscription_id: String,
    /// Maximum number of results (default 50)
    pub limit: Option<i64>,
    /// Offset for pagination (default 0)
    pub offset: Option<i64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AddFeedItemToLibraryInput {
    /// Feed item ID
    pub item_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RefreshSubscriptionInput {
    /// Subscription ID
    pub subscription_id: String,
}

fn json_str(data: &serde_json::Value, key: &str) -> Option<String> {
    data.get(key).and_then(|v| v.as_str()).map(String::from)
}

fn json_i32(data: &serde_json::Value, key: &str) -> Option<i32> {
    data.get(key).and_then(|v| v.as_i64()).map(|v| v as i32)
}

fn feed_item_to_json(item: &sub_queries::SubscriptionItemRow) -> serde_json::Value {
    let data: serde_json::Value = item
        .data_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(|| json!({}));

    let authors: Vec<serde_json::Value> = data
        .get("authors")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .map(|a| {
                    json!({
                        "name": a.get("name").and_then(|n| n.as_str()).unwrap_or(""),
                        "affiliation": a.get("affiliation").and_then(|n| n.as_str()),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    json!({
        "id": item.id,
        "external_id": item.external_id,
        "title": item.title,
        "authors": authors,
        "abstract_text": json_str(&data, "abstract_text"),
        "url": json_str(&data, "url"),
        "pdf_url": json_str(&data, "pdf_url"),
        "html_url": json_str(&data, "html_url"),
        "upvotes": json_i32(&data, "upvotes"),
        "fetched_date": item.fetched_date,
        "added_to_library": item.added_to_library,
        "thumbnail_url": json_str(&data, "thumbnail"),
        "ai_summary": json_str(&data, "ai_summary"),
        "project_page": json_str(&data, "project_page"),
        "github_repo": json_str(&data, "github_repo"),
        "github_stars": json_i32(&data, "github_stars"),
        "num_comments": json_i32(&data, "num_comments"),
    })
}

pub fn tool_list_subscriptions(state: &Arc<AppState>) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let subs = sub_queries::list_subscriptions(&db.conn)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    let results: Vec<serde_json::Value> = subs
        .iter()
        .map(|s| {
            json!({
                "id": s.id,
                "source_type": s.source_type,
                "name": s.name,
                "enabled": s.enabled,
                "poll_interval_minutes": s.poll_interval_minutes,
                "last_polled": s.last_polled,
            })
        })
        .collect();

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&results).unwrap_or_default(),
    )]))
}

pub fn tool_list_feed_items(
    state: &Arc<AppState>,
    input: ListFeedItemsInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let items = sub_queries::list_subscription_items(
        &db.conn,
        &input.subscription_id,
        input.limit.unwrap_or(50),
        input.offset.unwrap_or(0),
    )
    .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    let results: Vec<serde_json::Value> = items.iter().map(feed_item_to_json).collect();

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&results).unwrap_or_default(),
    )]))
}

pub fn tool_add_feed_item_to_library(
    state: &Arc<AppState>,
    input: AddFeedItemToLibraryInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let item = sub_queries::get_subscription_item(&db.conn, &input.item_id)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    if item.added_to_library {
        return Ok(CallToolResult::success(vec![Content::text(
            "Item is already in the library",
        )]));
    }

    // Parse data JSON for authors and extra fields
    let data: serde_json::Value = item
        .data_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(|| json!({}));

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

    let authors: Vec<(String, Option<String>, Option<String>)> = data
        .get("authors")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| {
                    a.get("name").and_then(|n| n.as_str()).map(|name| {
                        let aff = a
                            .get("affiliation")
                            .and_then(|n| n.as_str())
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

    let db_input = zoro_db::queries::papers::CreatePaperInput {
        slug: slug.clone(),
        title: title.clone(),
        short_title: None,
        abstract_text: abstract_text.clone(),
        doi: None,
        arxiv_id: Some(arxiv_id),
        url: Some(url),
        pdf_url: Some(pdf_url),
        html_url: Some(html_url),
        thumbnail_url,
        published_date: json_str(&data, "published_at"),
        source: Some("subscription".to_string()),
        dir_path: format!("papers/{}", slug),
        extra_json: item.data_json.clone(),
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
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    let _ = zoro_db::queries::papers::set_paper_authors(&db.conn, &row.id, &authors);
    let _ = sub_queries::mark_item_added_to_library(&db.conn, &input.item_id, &row.id);

    let papers_dir = state.data_dir.join("library/papers");
    let _ = zoro_storage::paper_dir::create_paper_dir(&papers_dir, &slug);

    zoro_storage::sync::sync_paper_metadata(&db, &state.data_dir, &row.id);
    zoro_storage::sync::rebuild_library_index(&db, &state.data_dir);

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Added '{}' to library (paper ID: {})",
        title, row.id
    ))]))
}

pub async fn tool_refresh_subscription(
    state: &Arc<AppState>,
    input: RefreshSubscriptionInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    // Read subscription info
    let sub = {
        let db = state
            .db
            .lock()
            .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;
        let subs = sub_queries::list_subscriptions(&db.conn)
            .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;
        subs.into_iter()
            .find(|s| s.id == input.subscription_id)
            .ok_or_else(|| rmcp::ErrorData::invalid_params("Subscription not found", None))?
    };

    // Fetch items from source (network call, no DB lock)
    // Use fetch_by_date with the latest date so results match the date-based API.
    let items = match sub.source_type.as_str() {
        "huggingface-daily" => {
            let source = HuggingFaceDailyPapers::new();
            let latest_date = zoro_subscriptions::fetch_latest_date()
                .await
                .map_err(|e| {
                    rmcp::ErrorData::internal_error(
                        format!("Failed to fetch latest date: {}", e),
                        None,
                    )
                })?
                .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
            source.fetch_by_date(&latest_date).await.map_err(|e| {
                rmcp::ErrorData::internal_error(format!("Fetch failed: {}", e), None)
            })?
        }
        _ => {
            return Err(rmcp::ErrorData::invalid_params(
                format!("Unknown source type: {}", sub.source_type),
                None,
            ));
        }
    };

    // Store items in DB
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let mut new_count = 0;
    for item in &items {
        let data_json = build_item_data_json(item);
        let source_date = zoro_subscriptions::extract_source_date(item);
        if sub_queries::insert_subscription_item(
            &db.conn,
            &input.subscription_id,
            &item.external_id,
            &item.title,
            data_json.as_deref(),
            source_date.as_deref(),
        )
        .is_ok()
        {
            new_count += 1;
        }
    }

    let _ = sub_queries::update_last_polled(&db.conn, &input.subscription_id);

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Refreshed subscription: {} new items (total fetched: {})",
        new_count,
        items.len()
    ))]))
}
