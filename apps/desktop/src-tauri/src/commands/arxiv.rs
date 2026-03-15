// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::AppState;
use tauri::{Emitter, State};
use zoro_db::queries::{attachments, papers};

/// Fetch arXiv HTML for a paper as a background task.
/// Returns immediately; emits `background-task` and `paper-updated` events.
#[tauri::command]
pub async fn fetch_arxiv_html(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    paper_id: String,
) -> Result<(), String> {
    tracing::info!(paper_id = %paper_id, "fetch_arxiv_html: spawning background task");

    let (arxiv_id, paper_title, paper_dir, db_path) = {
        let db = state
            .db
            .lock()
            .map_err(|e| format!("DB lock error: {}", e))?;
        let row = papers::get_paper(&db.conn, &paper_id).map_err(|e| {
            tracing::error!(paper_id = %paper_id, error = %e, "fetch_arxiv_html: get_paper failed");
            format!("{}", e)
        })?;

        let aid = zoro_arxiv::arxiv_id::find_arxiv_id(
            row.arxiv_id.as_deref(),
            row.url.as_deref(),
            row.doi.as_deref(),
            None,
        )
        .ok_or_else(|| {
            tracing::error!(paper_id = %paper_id, "fetch_arxiv_html: no arXiv ID found");
            "No arXiv ID found for this paper".to_string()
        })?;

        let dir = state.data_dir.join("library").join(&row.dir_path);
        let db_path = state.data_dir.join("library.db");
        (aid, row.title.clone(), dir, db_path)
    };

    let pid = paper_id.clone();
    tokio::spawn(async move {
        let task_id = format!("html-{}", pid);
        let html_path = paper_dir.join("paper.html");

        let _ = app.emit(
            "background-task",
            serde_json::json!({
                "task_id": task_id,
                "paper_id": pid,
                "paper_title": paper_title,
                "task_type": "html-download",
                "status": "running",
                "message": "Fetching arXiv HTML",
            }),
        );

        match zoro_arxiv::fetch::fetch_and_save(&arxiv_id, &html_path).await {
            Ok(()) => {
                let _ = zoro_arxiv::clean::clean_html_file(&html_path, &[]).await;
                let file_size = html_path.metadata().map(|m| m.len() as i64).ok();

                if let Ok(db) = zoro_db::Database::open(&db_path) {
                    if let Ok(atts) =
                        zoro_db::queries::attachments::get_paper_attachments(&db.conn, &pid)
                    {
                        for att in &atts {
                            if att.file_type == "html" {
                                let _ = zoro_db::queries::attachments::delete_attachment(
                                    &db.conn, &att.id,
                                );
                            }
                        }
                    }
                    let _ = zoro_db::queries::attachments::insert_attachment(
                        &db.conn,
                        &pid,
                        "paper.html",
                        "html",
                        Some("text/html"),
                        file_size,
                        "paper.html",
                        "arxiv-fetch",
                    );
                }

                let _ = app.emit(
                    "background-task",
                    serde_json::json!({
                        "task_id": task_id,
                        "paper_id": pid,
                        "paper_title": paper_title,
                        "task_type": "html-download",
                        "status": "completed",
                        "message": null,
                    }),
                );
                let _ = app.emit("paper-updated", &pid);
            }
            Err(e) => {
                tracing::error!(
                    arxiv_id = %arxiv_id, error = %e,
                    "fetch_arxiv_html: background fetch failed"
                );
                let _ = app.emit(
                    "background-task",
                    serde_json::json!({
                        "task_id": task_id,
                        "paper_id": pid,
                        "paper_title": paper_title,
                        "task_type": "html-download",
                        "status": "failed",
                        "message": format!("{}", e),
                    }),
                );
            }
        }
    });

    Ok(())
}

/// Clean an HTML paper by hiding distracting elements.
#[tauri::command]
pub async fn clean_paper_html(
    state: State<'_, AppState>,
    paper_id: String,
    extra_selectors: Option<Vec<String>>,
) -> Result<usize, String> {
    let html_path = resolve_html_path(&state, &paper_id)?;
    let selectors = extra_selectors.unwrap_or_default();

    zoro_arxiv::clean::clean_html_file(&html_path, &selectors)
        .await
        .map_err(|e| format!("Failed to clean HTML: {}", e))
}

/// Inject ar5iv CSS into an HTML paper for better rendering.
#[tauri::command]
pub async fn fix_paper_html_style(
    state: State<'_, AppState>,
    paper_id: String,
) -> Result<(), String> {
    let html_path = resolve_html_path(&state, &paper_id)?;

    zoro_arxiv::style::fix_html_file_style(&html_path)
        .await
        .map_err(|e| format!("Failed to fix HTML style: {}", e))
}

/// Translate all untranslated paragraphs in the HTML paper as a background task.
/// Returns immediately after spawning; emits `html-translation-progress` events
/// during translation and `html-translation-complete` when finished.
#[tauri::command]
pub async fn translate_paper_html(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    paper_id: String,
) -> Result<(), String> {
    {
        let active = state
            .active_html_translations
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        if active.contains(&paper_id) {
            return Err("Translation already in progress for this paper".into());
        }
    }

    let html_path = resolve_html_path(&state, &paper_id)?;

    let (ai_config, target_lang, html_system, html_user, concurrency) = {
        let config = state
            .config
            .lock()
            .map_err(|e| format!("Config lock error: {}", e))?;

        if config.general.native_lang.is_empty() {
            return Err(
                "Native language not configured. Set it in Settings > AI / Translation.".into(),
            );
        }
        if config.ai.base_url.is_empty()
            || config.ai.api_key.is_empty()
            || config.ai.model.is_empty()
        {
            return Err("AI not configured. Set base URL, API key, and model in Settings.".into());
        }

        (
            config.ai.clone(),
            config.general.native_lang.clone(),
            config.ai.translation_prompts.html_system.clone(),
            config.ai.translation_prompts.html_user.clone(),
            config.ai.html_concurrency,
        )
    };

    // Resolve the per-task model for heavy (full-text) translation
    let mut heavy_config = ai_config.clone();
    heavy_config.model = ai_config
        .task_model_defaults
        .resolve("heavy", &ai_config.model);

    // Build glossary prompt for HTML translation
    let glossary_prompt = if ai_config.glossary_enabled {
        let db = state
            .db
            .lock()
            .map_err(|e| format!("DB lock error: {}", e))?;
        let rows = zoro_db::queries::glossary::list_active_glossary(
            &db.conn,
            &target_lang,
            ai_config.glossary_threshold as i64,
        )
        .unwrap_or_default();
        if rows.is_empty() {
            None
        } else {
            let mut prompt = String::from(
                "\n\nUse the following glossary for consistent terminology translation:\n",
            );
            for r in &rows {
                prompt.push_str(&format!("- {} → {}\n", r.source_term, r.translated_term));
            }
            Some(prompt)
        }
    } else {
        None
    };

    let paper_title = {
        let db = state
            .db
            .lock()
            .map_err(|e| format!("DB lock error: {}", e))?;
        papers::get_paper(&db.conn, &paper_id)
            .map(|p| p.title)
            .unwrap_or_else(|_| paper_id.clone())
    };

    {
        let mut active = state
            .active_html_translations
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        active.insert(paper_id.clone());
    }

    let active_translations = std::sync::Arc::clone(&state.active_html_translations);
    let pid = paper_id.clone();

    tokio::spawn(async move {
        let task_id = format!("html-translate-{}", pid);

        let _ = app.emit(
            "background-task",
            serde_json::json!({
                "task_id": task_id,
                "paper_id": pid,
                "paper_title": paper_title,
                "task_type": "html-translation",
                "status": "running",
                "message": null,
            }),
        );

        let sys_owned = if html_system.is_empty() {
            None
        } else {
            Some(html_system)
        };
        let usr_owned = if html_user.is_empty() {
            None
        } else {
            Some(html_user)
        };

        let app_progress = app.clone();
        let progress_pid = pid.clone();

        let app_inserted = app.clone();
        let inserted_pid = pid.clone();

        let result = zoro_arxiv::translate::translate_html_file(
            &html_path,
            &heavy_config,
            &target_lang,
            sys_owned.as_deref(),
            usr_owned.as_deref(),
            glossary_prompt.as_deref(),
            concurrency,
            move |progress| {
                let _ = app_progress.emit(
                    "html-translation-progress",
                    serde_json::json!({
                        "paperId": progress_pid,
                        "total": progress.total,
                        "done": progress.done,
                        "failed": progress.failed,
                        "status": progress.status,
                        "currentParagraph": progress.current_paragraph,
                    }),
                );
            },
            move |para| {
                let _ = app_inserted.emit(
                    "html-translation-insert",
                    serde_json::json!({
                        "paperId": inserted_pid,
                        "tag": para.tag,
                        "originalTextSnippet": para.original_text_snippet,
                        "translationHtml": para.translation_html,
                    }),
                );
            },
        )
        .await;

        if let Ok(mut active) = active_translations.lock() {
            active.remove(&pid);
        }

        match result {
            Ok(tr) => {
                let status = if tr.failed > 0 { "partial" } else { "done" };
                let _ = app.emit(
                    "html-translation-complete",
                    serde_json::json!({
                        "paperId": pid,
                        "status": status,
                        "totalParagraphs": tr.total_paragraphs,
                        "translated": tr.translated,
                        "skipped": tr.skipped,
                        "failed": tr.failed,
                    }),
                );
                let msg = format!("{}/{} paragraphs", tr.translated, tr.total_paragraphs);
                let _ = app.emit(
                    "background-task",
                    serde_json::json!({
                        "task_id": task_id,
                        "paper_id": pid,
                        "paper_title": paper_title,
                        "task_type": "html-translation",
                        "status": "completed",
                        "message": msg,
                    }),
                );
            }
            Err(e) => {
                tracing::error!(paper_id = %pid, error = %e, "Background HTML translation failed");
                let _ = app.emit(
                    "html-translation-complete",
                    serde_json::json!({
                        "paperId": pid,
                        "status": "error",
                        "error": e.to_string(),
                    }),
                );
                let _ = app.emit(
                    "background-task",
                    serde_json::json!({
                        "task_id": task_id,
                        "paper_id": pid,
                        "paper_title": paper_title,
                        "task_type": "html-translation",
                        "status": "failed",
                        "message": e.to_string(),
                    }),
                );
            }
        }
    });

    Ok(())
}

/// Return the set of paper IDs that currently have a background HTML translation running.
#[tauri::command]
pub async fn get_active_html_translations(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let active = state
        .active_html_translations
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    Ok(active.iter().cloned().collect())
}

/// Update a specific translation block in the HTML file.
#[tauri::command]
pub async fn save_html_translation_edit(
    state: State<'_, AppState>,
    paper_id: String,
    block_index: usize,
    new_text: String,
) -> Result<(), String> {
    let html_path = resolve_html_path(&state, &paper_id)?;

    let html = tokio::fs::read_to_string(&html_path)
        .await
        .map_err(|e| format!("Failed to read HTML: {}", e))?;

    let updated = zoro_arxiv::translate::update_translation_block(&html, block_index, &new_text)
        .map_err(|e| format!("Failed to update translation: {}", e))?;

    tokio::fs::write(&html_path, &updated)
        .await
        .map_err(|e| format!("Failed to write HTML: {}", e))?;

    Ok(())
}

/// Count untranslated paragraphs in the paper's HTML.
#[tauri::command]
pub async fn count_html_untranslated(
    state: State<'_, AppState>,
    paper_id: String,
) -> Result<usize, String> {
    let html_path = resolve_html_path(&state, &paper_id)?;

    let html = tokio::fs::read_to_string(&html_path)
        .await
        .map_err(|e| format!("Failed to read HTML: {}", e))?;

    Ok(zoro_arxiv::translate::count_untranslated(&html))
}

// --- Helpers ---

fn resolve_html_path(
    state: &State<'_, AppState>,
    paper_id: &str,
) -> Result<std::path::PathBuf, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let row = papers::get_paper(&db.conn, paper_id).map_err(|e| format!("{}", e))?;
    let paper_dir = state.data_dir.join("library").join(&row.dir_path);
    let html_path = paper_dir.join("paper.html");
    if html_path.exists() {
        return Ok(html_path);
    }

    let abs_path = paper_dir.join("abs.html");
    if abs_path.exists() {
        return Ok(abs_path);
    }

    // Fall back to HTML attachments
    if let Ok(atts) = attachments::get_paper_attachments(&db.conn, paper_id) {
        for att in &atts {
            if att.file_type == "html" {
                let att_path = paper_dir.join(&att.relative_path);
                if att_path.exists() {
                    return Ok(att_path);
                }
            }
        }
    }

    Err(format!("HTML file not found for paper {}", paper_id))
}
