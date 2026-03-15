// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::sync::Arc;

use rmcp::model::*;
use serde_json::json;

use crate::state::AppState;
use zoro_db::queries::{papers, subscriptions, translations};

// ---------------------------------------------------------------------------
// Input structs
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetTranslationsInput {
    /// Entity type: "paper" or "subscription_item"
    pub entity_type: String,
    /// Entity ID (paper ID or subscription item ID)
    pub entity_id: String,
    /// Target language code (e.g. "zh", "ja"). If omitted, uses the configured native_lang.
    pub target_lang: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TranslatePaperInput {
    /// Paper ID to translate
    pub paper_id: String,
    /// Target language code (e.g. "zh", "ja"). If omitted, uses the configured native_lang.
    pub target_lang: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TranslateFeedItemInput {
    /// Subscription item ID to translate
    pub item_id: String,
    /// Target language code (e.g. "zh", "ja"). If omitted, uses the configured native_lang.
    pub target_lang: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DeleteTranslationsInput {
    /// Entity type: "paper" or "subscription_item"
    pub entity_type: String,
    /// Entity ID (paper ID or subscription item ID)
    pub entity_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchTranslatedTextInput {
    /// Search query for FTS5 full-text search across translated content
    pub query: String,
    /// Entity type to search: "paper" or "subscription_item". Defaults to "paper".
    pub entity_type: Option<String>,
    /// Maximum number of results. Defaults to 20.
    pub limit: Option<i64>,
}

// ---------------------------------------------------------------------------
// Helper: resolve target language from input or config
// ---------------------------------------------------------------------------

fn resolve_target_lang(
    state: &AppState,
    input_lang: Option<&str>,
) -> Result<String, rmcp::ErrorData> {
    if let Some(lang) = input_lang {
        if !lang.is_empty() {
            return Ok(lang.to_string());
        }
    }
    let config = state.load_config();
    let lang = config.general.native_lang;
    if lang.is_empty() {
        return Err(rmcp::ErrorData::invalid_params(
            "No target language specified and no native_lang configured in settings. \
             Please set a native language in the Zoro settings first.",
            None,
        ));
    }
    Ok(lang)
}

// ---------------------------------------------------------------------------
// Tool: get_translations
// ---------------------------------------------------------------------------

pub fn tool_get_translations(
    state: &Arc<AppState>,
    input: GetTranslationsInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let target_lang = resolve_target_lang(state, input.target_lang.as_deref())?;

    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let rows = translations::get_translations(
        &db.conn,
        &input.entity_type,
        &input.entity_id,
        &target_lang,
    )
    .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    let result: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.id,
                "entity_type": r.entity_type,
                "entity_id": r.entity_id,
                "field": r.field,
                "target_lang": r.target_lang,
                "translated_text": r.translated_text,
                "model": r.model,
                "created_date": r.created_date,
                "modified_date": r.modified_date,
            })
        })
        .collect();

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&result).unwrap_or_default(),
    )]))
}

// ---------------------------------------------------------------------------
// Tool: translate_paper (async — calls LLM)
// ---------------------------------------------------------------------------

pub async fn tool_translate_paper(
    state: &Arc<AppState>,
    input: TranslatePaperInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let target_lang = resolve_target_lang(state, input.target_lang.as_deref())?;

    // Load config for AI settings (before acquiring DB lock)
    let config = state.load_config();
    if config.ai.base_url.is_empty() || config.ai.api_key.is_empty() || config.ai.model.is_empty() {
        return Err(rmcp::ErrorData::invalid_params(
            "AI translation is not configured. Please set base_url, api_key, and model \
             in the Zoro AI settings first.",
            None,
        ));
    }

    // Read paper data from DB, then release lock
    let (title, abstract_text) = {
        let db = state
            .db
            .lock()
            .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;
        let row = papers::get_paper(&db.conn, &input.paper_id)
            .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;
        (row.title.clone(), row.abstract_text.clone())
    };

    let prompts = &config.ai.translation_prompts;
    let mut translated_fields = Vec::new();

    // Resolve the per-task model for normal (title/abstract) translation
    let normal_model = config
        .ai
        .task_model_defaults
        .resolve("normal", &config.ai.model);
    let mut task_config = config.ai.clone();
    task_config.model = normal_model;

    let glossary = if config.ai.glossary_enabled {
        let db = state
            .db
            .lock()
            .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;
        zoro_db::queries::glossary::list_active_glossary(
            &db.conn,
            &target_lang,
            config.ai.glossary_threshold as i64,
        )
        .unwrap_or_default()
        .into_iter()
        .map(|r| zoro_ai::translate::GlossaryEntry {
            source: r.source_term,
            translated: r.translated_term,
        })
        .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    // Translate title
    let translated_title = zoro_ai::translate::translate_text(
        &task_config,
        prompts,
        "title",
        &title,
        &target_lang,
        &glossary,
    )
    .await
    .map_err(|e| {
        rmcp::ErrorData::internal_error(format!("Title translation failed: {}", e), None)
    })?;

    // Translate abstract if present
    let translated_abstract = if let Some(ref abs) = abstract_text {
        if !abs.trim().is_empty() {
            Some(
                zoro_ai::translate::translate_text(
                    &task_config,
                    prompts,
                    "abstract_text",
                    abs,
                    &target_lang,
                    &glossary,
                )
                .await
                .map_err(|e| {
                    rmcp::ErrorData::internal_error(
                        format!("Abstract translation failed: {}", e),
                        None,
                    )
                })?,
            )
        } else {
            None
        }
    } else {
        None
    };

    // Save translations to DB
    {
        let db = state
            .db
            .lock()
            .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

        translations::upsert_translation(
            &db.conn,
            "paper",
            &input.paper_id,
            "title",
            &target_lang,
            &translated_title,
            Some(&task_config.model),
        )
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

        translated_fields.push(json!({
            "field": "title",
            "original": title,
            "translated": translated_title,
        }));

        if let Some(ref trans_abs) = translated_abstract {
            translations::upsert_translation(
                &db.conn,
                "paper",
                &input.paper_id,
                "abstract_text",
                &target_lang,
                trans_abs,
                Some(&task_config.model),
            )
            .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

            translated_fields.push(json!({
                "field": "abstract_text",
                "original": abstract_text,
                "translated": trans_abs,
            }));
        }
    }

    let result = json!({
        "paper_id": input.paper_id,
        "target_lang": target_lang,
        "model": task_config.model,
        "translations": translated_fields,
    });

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&result).unwrap_or_default(),
    )]))
}

// ---------------------------------------------------------------------------
// Tool: translate_feed_item (async — calls LLM)
// ---------------------------------------------------------------------------

pub async fn tool_translate_feed_item(
    state: &Arc<AppState>,
    input: TranslateFeedItemInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let target_lang = resolve_target_lang(state, input.target_lang.as_deref())?;

    let config = state.load_config();
    if config.ai.base_url.is_empty() || config.ai.api_key.is_empty() || config.ai.model.is_empty() {
        return Err(rmcp::ErrorData::invalid_params(
            "AI translation is not configured. Please set base_url, api_key, and model \
             in the Zoro AI settings first.",
            None,
        ));
    }

    // Read feed item from DB, then release lock
    let (title, abstract_text) = {
        let db = state
            .db
            .lock()
            .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;
        let item = subscriptions::get_subscription_item(&db.conn, &input.item_id)
            .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

        // Abstract is stored in data_json blob
        let abstract_text: Option<String> = item.data_json.as_ref().and_then(|dj| {
            serde_json::from_str::<serde_json::Value>(dj)
                .ok()
                .and_then(|v| v.get("abstract")?.as_str().map(String::from))
        });

        (item.title.clone(), abstract_text)
    };

    let prompts = &config.ai.translation_prompts;
    let mut translated_fields = Vec::new();

    // Resolve the per-task model for normal (title/abstract) translation
    let normal_model = config
        .ai
        .task_model_defaults
        .resolve("normal", &config.ai.model);
    let mut task_config = config.ai.clone();
    task_config.model = normal_model;

    let glossary = if config.ai.glossary_enabled {
        let db = state
            .db
            .lock()
            .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;
        zoro_db::queries::glossary::list_active_glossary(
            &db.conn,
            &target_lang,
            config.ai.glossary_threshold as i64,
        )
        .unwrap_or_default()
        .into_iter()
        .map(|r| zoro_ai::translate::GlossaryEntry {
            source: r.source_term,
            translated: r.translated_term,
        })
        .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    // Translate title
    let translated_title = zoro_ai::translate::translate_text(
        &task_config,
        prompts,
        "title",
        &title,
        &target_lang,
        &glossary,
    )
    .await
    .map_err(|e| {
        rmcp::ErrorData::internal_error(format!("Title translation failed: {}", e), None)
    })?;

    // Translate abstract if present
    let translated_abstract = if let Some(ref abs) = abstract_text {
        if !abs.trim().is_empty() {
            Some(
                zoro_ai::translate::translate_text(
                    &task_config,
                    prompts,
                    "abstract_text",
                    abs,
                    &target_lang,
                    &glossary,
                )
                .await
                .map_err(|e| {
                    rmcp::ErrorData::internal_error(
                        format!("Abstract translation failed: {}", e),
                        None,
                    )
                })?,
            )
        } else {
            None
        }
    } else {
        None
    };

    // Save translations to DB
    {
        let db = state
            .db
            .lock()
            .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

        translations::upsert_translation(
            &db.conn,
            "subscription_item",
            &input.item_id,
            "title",
            &target_lang,
            &translated_title,
            Some(&task_config.model),
        )
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

        translated_fields.push(json!({
            "field": "title",
            "original": title,
            "translated": translated_title,
        }));

        if let Some(ref trans_abs) = translated_abstract {
            translations::upsert_translation(
                &db.conn,
                "subscription_item",
                &input.item_id,
                "abstract_text",
                &target_lang,
                trans_abs,
                Some(&task_config.model),
            )
            .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

            translated_fields.push(json!({
                "field": "abstract_text",
                "original": abstract_text,
                "translated": trans_abs,
            }));
        }
    }

    let result = json!({
        "item_id": input.item_id,
        "target_lang": target_lang,
        "model": task_config.model,
        "translations": translated_fields,
    });

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&result).unwrap_or_default(),
    )]))
}

// ---------------------------------------------------------------------------
// Tool: delete_translations
// ---------------------------------------------------------------------------

pub fn tool_delete_translations(
    state: &Arc<AppState>,
    input: DeleteTranslationsInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let deleted = translations::delete_translations(&db.conn, &input.entity_type, &input.entity_id)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Deleted {} translation(s) for {} '{}'",
        deleted, input.entity_type, input.entity_id
    ))]))
}

// ---------------------------------------------------------------------------
// Tool: search_translated_text
// ---------------------------------------------------------------------------

pub fn tool_search_translated_text(
    state: &Arc<AppState>,
    input: SearchTranslatedTextInput,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let entity_type = input.entity_type.as_deref().unwrap_or("paper");
    let limit = input.limit.unwrap_or(20);

    let db = state
        .db
        .lock()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("DB lock error: {}", e), None))?;

    let entity_ids =
        translations::search_translations_fts(&db.conn, &input.query, entity_type, limit)
            .map_err(|e| rmcp::ErrorData::internal_error(format!("{}", e), None))?;

    let result = json!({
        "query": input.query,
        "entity_type": entity_type,
        "matching_entity_ids": entity_ids,
        "count": entity_ids.len(),
    });

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&result).unwrap_or_default(),
    )]))
}
