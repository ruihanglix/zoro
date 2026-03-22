// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use zoro_db::queries::attachments;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationResponse {
    pub field: String,
    pub original_text: String,
    pub translated_text: String,
    pub model: Option<String>,
    pub created_date: String,
}

/// Get cached translations for an entity. Returns empty vec if none exist.
#[tauri::command]
pub async fn get_translations(
    state: State<'_, AppState>,
    entity_type: String,
    entity_id: String,
) -> Result<Vec<TranslationResponse>, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?;
    let target_lang = &config.general.native_lang;

    if target_lang.is_empty() {
        return Ok(Vec::new());
    }

    let rows = zoro_db::queries::translations::get_translations(
        &db.conn,
        &entity_type,
        &entity_id,
        target_lang,
    )
    .map_err(|e| format!("DB error: {}", e))?;

    // Fetch original texts to pair with translations
    let originals = get_original_texts(&db.conn, &entity_type, &entity_id);

    let result = rows
        .into_iter()
        .map(|r| {
            let original = originals.get(&r.field).cloned().unwrap_or_default();
            TranslationResponse {
                field: r.field,
                original_text: original,
                translated_text: r.translated_text,
                model: r.model,
                created_date: r.created_date,
            }
        })
        .collect();

    Ok(result)
}

/// Batch get translations for multiple entities.
#[tauri::command]
pub async fn get_translations_batch(
    state: State<'_, AppState>,
    entity_type: String,
    entity_ids: Vec<String>,
) -> Result<Vec<BatchTranslationResponse>, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?;
    let target_lang = &config.general.native_lang;

    if target_lang.is_empty() || entity_ids.is_empty() {
        return Ok(Vec::new());
    }

    let rows = zoro_db::queries::translations::get_translations_batch(
        &db.conn,
        &entity_type,
        &entity_ids,
        target_lang,
    )
    .map_err(|e| format!("DB error: {}", e))?;

    let result = rows
        .into_iter()
        .map(|r| BatchTranslationResponse {
            entity_id: r.entity_id,
            field: r.field,
            translated_text: r.translated_text,
        })
        .collect();

    Ok(result)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchTranslationResponse {
    pub entity_id: String,
    pub field: String,
    pub translated_text: String,
}

/// Translate specified fields for an entity using the configured LLM.
/// Results are cached in the DB and returned.
#[tauri::command]
pub async fn translate_fields(
    state: State<'_, AppState>,
    entity_type: String,
    entity_id: String,
    fields: Vec<String>,
) -> Result<Vec<TranslationResponse>, String> {
    // Read config (release lock immediately)
    let (ai_config, target_lang) = {
        let config = state
            .config
            .lock()
            .map_err(|e| format!("Config lock error: {}", e))?;
        (config.ai.clone(), config.general.native_lang.clone())
    };

    if target_lang.is_empty() {
        return Err(
            "Native language not configured. Set it in Settings > AI / Translation.".into(),
        );
    }

    // Resolve the per-task model for title/abstract ("normal" translation)
    let normal_model = ai_config
        .task_model_defaults
        .resolve("normal", &ai_config.model);
    // resolve_for_model also resolves provider-specific base_url/api_key
    let task_ai_config = ai_config.resolve_for_model(&normal_model);

    // Load glossary terms if enabled
    let glossary = if ai_config.glossary_enabled {
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
        rows.into_iter()
            .map(|r| zoro_ai::translate::GlossaryEntry {
                source: r.source_term,
                translated: r.translated_term,
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    // Get original texts (need DB lock briefly)
    let originals = {
        let db = state
            .db
            .lock()
            .map_err(|e| format!("DB lock error: {}", e))?;
        get_original_texts(&db.conn, &entity_type, &entity_id)
    };

    let mut results = Vec::new();
    let mut all_original_text = String::new();

    for field in &fields {
        let original = match originals.get(field.as_str()) {
            Some(text) if !text.is_empty() => text.clone(),
            _ => continue,
        };

        all_original_text.push_str(&original);
        all_original_text.push('\n');

        let translated = zoro_ai::translate::translate_text(
            &task_ai_config,
            &ai_config.translation_prompts,
            field,
            &original,
            &target_lang,
            &glossary,
        )
        .await
        .map_err(|e| format!("Translation error: {}", e))?;

        let row = {
            let db = state
                .db
                .lock()
                .map_err(|e| format!("DB lock error: {}", e))?;
            zoro_db::queries::translations::upsert_translation(
                &db.conn,
                &entity_type,
                &entity_id,
                field,
                &target_lang,
                &translated,
                Some(&task_ai_config.model),
            )
            .map_err(|e| format!("DB error: {}", e))?
        };

        results.push(TranslationResponse {
            field: row.field,
            original_text: original,
            translated_text: row.translated_text,
            model: row.model,
            created_date: row.created_date,
        });
    }

    // Auto-extract glossary terms in the background
    if ai_config.glossary_enabled && !all_original_text.trim().is_empty() {
        let db = std::sync::Arc::clone(&state.db);
        // resolve_for_model also resolves provider-specific base_url/api_key
        let glossary_ai = ai_config.resolve_for_model(
            &ai_config.task_model_defaults.resolve("glossary", &ai_config.model),
        );
        let lang = target_lang.clone();
        let eid = entity_id.clone();
        tokio::spawn(async move {
            match zoro_ai::translate::extract_glossary_terms(
                &glossary_ai,
                &all_original_text,
                &lang,
            )
            .await
            {
                Ok(terms) if !terms.is_empty() => {
                    if let Ok(db) = db.lock() {
                        let _ = zoro_db::queries::glossary::upsert_extracted_terms(
                            &db.conn, &terms, &lang, &eid,
                        );
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "Glossary term extraction failed");
                }
            }
        });
    }

    Ok(results)
}

/// Delete all cached translations for an entity (e.g. before re-translating).
#[tauri::command]
pub async fn delete_translations(
    state: State<'_, AppState>,
    entity_type: String,
    entity_id: String,
) -> Result<usize, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    zoro_db::queries::translations::delete_translations(&db.conn, &entity_type, &entity_id)
        .map_err(|e| format!("DB error: {}", e))
}

/// Get the current AI configuration.
#[tauri::command]
pub async fn get_ai_config(state: State<'_, AppState>) -> Result<AiConfigResponse, String> {
    let config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?;
    Ok(AiConfigResponse {
        provider: config.ai.provider.clone(),
        base_url: config.ai.base_url.clone(),
        api_key_set: !config.ai.api_key.is_empty(),
        model: config.ai.model.clone(),
        auto_translate: config.ai.auto_translate,
        native_lang: config.general.native_lang.clone(),
        translation_prompts: TranslationPromptsResponse {
            title_system: config.ai.translation_prompts.title_system.clone(),
            title_user: config.ai.translation_prompts.title_user.clone(),
            abstract_system: config.ai.translation_prompts.abstract_system.clone(),
            abstract_user: config.ai.translation_prompts.abstract_user.clone(),
            html_system: config.ai.translation_prompts.html_system.clone(),
            html_user: config.ai.translation_prompts.html_user.clone(),
        },
        html_concurrency: config.ai.html_concurrency,
        glossary_enabled: config.ai.glossary_enabled,
        glossary_threshold: config.ai.glossary_threshold,
        pdf_translation: PdfTranslationConfigResponse {
            enabled: config.ai.pdf_translation.enabled,
            babeldoc_command: config.ai.pdf_translation.babeldoc_command.clone(),
            use_ai_config: config.ai.pdf_translation.use_ai_config,
            custom_api_key_set: !config.ai.pdf_translation.custom_api_key.is_empty(),
            custom_base_url: config.ai.pdf_translation.custom_base_url.clone(),
            custom_model: config.ai.pdf_translation.custom_model.clone(),
            qps: config.ai.pdf_translation.qps,
            extra_args: config.ai.pdf_translation.extra_args.clone(),
        },
        providers: config
            .ai
            .providers
            .iter()
            .map(|p| AiProviderResponse {
                id: p.id.clone(),
                name: p.name.clone(),
                base_url: p.base_url.clone(),
                api_key_set: !p.api_key.is_empty(),
                models: p.models.clone(),
            })
            .collect(),
        task_model_defaults: TaskModelDefaultsResponse {
            quick_translation: config.ai.task_model_defaults.quick_translation.clone(),
            normal_translation: config.ai.task_model_defaults.normal_translation.clone(),
            heavy_translation: config.ai.task_model_defaults.heavy_translation.clone(),
            glossary_extraction: config.ai.task_model_defaults.glossary_extraction.clone(),
        },
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfigResponse {
    pub provider: String,
    pub base_url: String,
    pub api_key_set: bool,
    pub model: String,
    pub auto_translate: bool,
    pub native_lang: String,
    pub translation_prompts: TranslationPromptsResponse,
    pub html_concurrency: usize,
    pub pdf_translation: PdfTranslationConfigResponse,
    pub glossary_enabled: bool,
    pub glossary_threshold: u32,
    pub providers: Vec<AiProviderResponse>,
    pub task_model_defaults: TaskModelDefaultsResponse,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskModelDefaultsResponse {
    pub quick_translation: String,
    pub normal_translation: String,
    pub heavy_translation: String,
    pub glossary_extraction: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderResponse {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key_set: bool,
    pub models: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfTranslationConfigResponse {
    pub enabled: bool,
    pub babeldoc_command: String,
    pub use_ai_config: bool,
    pub custom_api_key_set: bool,
    pub custom_base_url: String,
    pub custom_model: String,
    pub qps: u32,
    pub extra_args: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationPromptsResponse {
    pub title_system: String,
    pub title_user: String,
    pub abstract_system: String,
    pub abstract_user: String,
    pub html_system: String,
    pub html_user: String,
}

/// Update AI configuration. API key is only updated if provided (non-empty).
#[tauri::command]
pub async fn update_ai_config(
    state: State<'_, AppState>,
    input: UpdateAiConfigInput,
) -> Result<(), String> {
    let mut config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?;

    if let Some(provider) = input.provider {
        config.ai.provider = provider;
    }
    if let Some(base_url) = input.base_url {
        config.ai.base_url = base_url;
    }
    if let Some(api_key) = input.api_key {
        if !api_key.is_empty() {
            config.ai.api_key = api_key;
        }
    }
    if let Some(model) = input.model {
        config.ai.model = model;
    }
    if let Some(auto_translate) = input.auto_translate {
        config.ai.auto_translate = auto_translate;
    }
    if let Some(native_lang) = input.native_lang {
        config.general.native_lang = native_lang;
    }
    if let Some(concurrency) = input.html_concurrency {
        config.ai.html_concurrency = concurrency.clamp(1, 32);
    }
    if let Some(prompts) = input.translation_prompts {
        if let Some(v) = prompts.title_system {
            config.ai.translation_prompts.title_system = v;
        }
        if let Some(v) = prompts.title_user {
            config.ai.translation_prompts.title_user = v;
        }
        if let Some(v) = prompts.abstract_system {
            config.ai.translation_prompts.abstract_system = v;
        }
        if let Some(v) = prompts.abstract_user {
            config.ai.translation_prompts.abstract_user = v;
        }
        if let Some(v) = prompts.html_system {
            config.ai.translation_prompts.html_system = v;
        }
        if let Some(v) = prompts.html_user {
            config.ai.translation_prompts.html_user = v;
        }
    }
    if let Some(glossary_enabled) = input.glossary_enabled {
        config.ai.glossary_enabled = glossary_enabled;
    }
    if let Some(threshold) = input.glossary_threshold {
        config.ai.glossary_threshold = threshold.max(1);
    }
    if let Some(pdf) = input.pdf_translation {
        if let Some(v) = pdf.enabled {
            config.ai.pdf_translation.enabled = v;
        }
        if let Some(v) = pdf.babeldoc_command {
            config.ai.pdf_translation.babeldoc_command = v;
        }
        if let Some(v) = pdf.use_ai_config {
            config.ai.pdf_translation.use_ai_config = v;
        }
        if let Some(v) = pdf.custom_api_key {
            if !v.is_empty() {
                config.ai.pdf_translation.custom_api_key = v;
            }
        }
        if let Some(v) = pdf.custom_base_url {
            config.ai.pdf_translation.custom_base_url = v;
        }
        if let Some(v) = pdf.custom_model {
            config.ai.pdf_translation.custom_model = v;
        }
        if let Some(v) = pdf.qps {
            config.ai.pdf_translation.qps = v.max(1);
        }
        if let Some(v) = pdf.extra_args {
            config.ai.pdf_translation.extra_args = v;
        }
    }
    if let Some(tmd) = input.task_model_defaults {
        if let Some(v) = tmd.quick_translation {
            config.ai.task_model_defaults.quick_translation = v;
        }
        if let Some(v) = tmd.normal_translation {
            config.ai.task_model_defaults.normal_translation = v;
        }
        if let Some(v) = tmd.heavy_translation {
            config.ai.task_model_defaults.heavy_translation = v;
        }
        if let Some(v) = tmd.glossary_extraction {
            config.ai.task_model_defaults.glossary_extraction = v;
        }
    }
    if let Some(new_providers) = input.providers {
        config.ai.providers = new_providers
            .into_iter()
            .map(|p| {
                // Preserve existing api_key if no new one is provided
                let existing_key = config
                    .ai
                    .providers
                    .iter()
                    .find(|ep| ep.id == p.id)
                    .map(|ep| ep.api_key.clone())
                    .unwrap_or_default();
                zoro_core::models::AiProvider {
                    id: p.id,
                    name: p.name,
                    base_url: p.base_url,
                    api_key: match p.api_key {
                        Some(k) if !k.is_empty() => k,
                        _ => existing_key,
                    },
                    models: p.models,
                }
            })
            .collect();
    }

    crate::storage::config::save_config(&state.data_dir, &config)
        .map_err(|e| format!("Failed to save config: {}", e))?;

    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAiConfigInput {
    pub provider: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub auto_translate: Option<bool>,
    pub native_lang: Option<String>,
    pub translation_prompts: Option<UpdateTranslationPromptsInput>,
    pub html_concurrency: Option<usize>,
    pub pdf_translation: Option<UpdatePdfTranslationInput>,
    pub glossary_enabled: Option<bool>,
    pub glossary_threshold: Option<u32>,
    pub providers: Option<Vec<UpdateAiProviderInput>>,
    pub task_model_defaults: Option<UpdateTaskModelDefaultsInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTaskModelDefaultsInput {
    pub quick_translation: Option<String>,
    pub normal_translation: Option<String>,
    pub heavy_translation: Option<String>,
    pub glossary_extraction: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAiProviderInput {
    pub id: String,
    pub name: String,
    pub base_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub models: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePdfTranslationInput {
    pub enabled: Option<bool>,
    pub babeldoc_command: Option<String>,
    pub use_ai_config: Option<bool>,
    pub custom_api_key: Option<String>,
    pub custom_base_url: Option<String>,
    pub custom_model: Option<String>,
    pub qps: Option<u32>,
    pub extra_args: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTranslationPromptsInput {
    pub title_system: Option<String>,
    pub title_user: Option<String>,
    pub abstract_system: Option<String>,
    pub abstract_user: Option<String>,
    pub html_system: Option<String>,
    pub html_user: Option<String>,
}

/// Test the AI connection by sending a simple request.
#[tauri::command]
pub async fn test_ai_connection(state: State<'_, AppState>) -> Result<String, String> {
    let resolved = {
        let config = state
            .config
            .lock()
            .map_err(|e| format!("Config lock error: {}", e))?;

        if config.ai.model.is_empty() {
            return Err("AI not configured. Set a default model first.".into());
        }

        // Resolve the correct base_url/api_key for the selected model
        // (e.g. lab free models route to their own provider endpoints)
        config.ai.resolve_for_model(&config.ai.model)
    };

    if resolved.base_url.is_empty() || resolved.api_key.is_empty() {
        return Err("AI not configured. Set base URL, API key, and model first.".into());
    }

    let client = zoro_ai::client::ChatClient::new(&resolved.base_url, &resolved.api_key, &resolved.model);

    client
        .test_connection()
        .await
        .map_err(|e| format!("Connection test failed: {}", e))
}

/// Translate arbitrary selected text using the configured LLM.
/// Unlike `translate_fields`, this does not look up entity texts from the DB —
/// it translates the provided text directly and returns the result without
/// persisting it.
#[tauri::command]
pub async fn translate_selection(
    state: State<'_, AppState>,
    text: String,
) -> Result<String, String> {
    let (ai_config, target_lang) = {
        let config = state
            .config
            .lock()
            .map_err(|e| format!("Config lock error: {}", e))?;
        (config.ai.clone(), config.general.native_lang.clone())
    };

    if target_lang.is_empty() {
        return Err(
            "Native language not configured. Set it in Settings > AI / Translation.".into(),
        );
    }

    if text.trim().is_empty() {
        return Ok(String::new());
    }

    // Resolve the per-task model for quick (inline) translation
    // resolve_for_model also resolves provider-specific base_url/api_key
    let quick_config = ai_config.resolve_for_model(
        &ai_config.task_model_defaults.resolve("quick", &ai_config.model),
    );

    let glossary = if ai_config.glossary_enabled {
        let db = state
            .db
            .lock()
            .map_err(|e| format!("DB lock error: {}", e))?;
        zoro_db::queries::glossary::list_active_glossary(
            &db.conn,
            &target_lang,
            ai_config.glossary_threshold as i64,
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

    let translated = zoro_ai::translate::translate_text(
        &quick_config,
        &ai_config.translation_prompts,
        "abstract_text",
        &text,
        &target_lang,
        &glossary,
    )
    .await
    .map_err(|e| format!("Translation error: {}", e))?;

    Ok(translated)
}

/// Reset translation prompts to defaults.
#[tauri::command]
pub async fn reset_translation_prompts(state: State<'_, AppState>) -> Result<(), String> {
    let mut config = state
        .config
        .lock()
        .map_err(|e| format!("Config lock error: {}", e))?;

    config.ai.translation_prompts = zoro_core::models::TranslationPrompts::default();

    crate::storage::config::save_config(&state.data_dir, &config)
        .map_err(|e| format!("Failed to save config: {}", e))?;

    Ok(())
}

// --- PDF Translation via BabelDOC CLI ---

/// Resolve a command name to an absolute path by probing the user's login shell PATH.
/// GUI apps on macOS / Linux don't inherit the interactive-shell PATH, so plain
/// `Command::new("babeldoc")` fails with "No such file or directory".
/// On Windows, use `where` command and probe Windows-specific install locations.
/// If the command is already an absolute path we just check it exists.
fn resolve_command_path(cmd: &str) -> Option<std::path::PathBuf> {
    let path = std::path::Path::new(cmd);

    // Already absolute – just verify it exists
    if path.is_absolute() {
        if path.exists() {
            return Some(path.to_path_buf());
        }
        return None;
    }

    #[cfg(windows)]
    {
        // On Windows, use `where` to locate the command on PATH
        if let Ok(output) = std::process::Command::new("where").arg(cmd).output() {
            if output.status.success() {
                // `where` may return multiple lines; take the first one
                let out = String::from_utf8_lossy(&output.stdout);
                if let Some(first_line) = out.lines().next() {
                    let p = first_line.trim().to_string();
                    if !p.is_empty() {
                        return Some(std::path::PathBuf::from(p));
                    }
                }
            }
        }

        // Also try with common extensions appended (.exe, .cmd, .bat)
        for ext in &[".exe", ".cmd", ".bat"] {
            let cmd_with_ext = format!("{}{}", cmd, ext);
            if let Ok(output) = std::process::Command::new("where")
                .arg(&cmd_with_ext)
                .output()
            {
                if output.status.success() {
                    let out = String::from_utf8_lossy(&output.stdout);
                    if let Some(first_line) = out.lines().next() {
                        let p = first_line.trim().to_string();
                        if !p.is_empty() {
                            return Some(std::path::PathBuf::from(p));
                        }
                    }
                }
            }
        }

        // Probe well-known Windows locations where pip / pipx / uv install tools
        let userprofile = std::env::var("USERPROFILE").unwrap_or_default();
        let localappdata = std::env::var("LOCALAPPDATA").unwrap_or_default();
        let candidates = [
            format!(
                "{}\\AppData\\Local\\Programs\\Python\\Scripts\\{}.exe",
                userprofile, cmd
            ),
            format!("{}\\Programs\\Python\\Scripts\\{}.exe", localappdata, cmd),
            format!(
                "{}\\AppData\\Roaming\\Python\\Scripts\\{}.exe",
                userprofile, cmd
            ),
            format!("{}\\.local\\bin\\{}.exe", userprofile, cmd),
            format!("{}\\.cargo\\bin\\{}.exe", userprofile, cmd),
            // pipx default install location
            format!(
                "{}\\pipx\\venvs\\{}\\Scripts\\{}.exe",
                localappdata, cmd, cmd
            ),
            // uv tool install location
            format!("{}\\uv\\tools\\{}\\Scripts\\{}.exe", localappdata, cmd, cmd),
        ];
        for c in &candidates {
            let p = std::path::PathBuf::from(c);
            if p.exists() {
                return Some(p);
            }
        }
    }

    #[cfg(not(windows))]
    {
        // First, try the current (restricted) process PATH
        if let Ok(output) = std::process::Command::new("which").arg(cmd).output() {
            if output.status.success() {
                let p = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !p.is_empty() {
                    return Some(std::path::PathBuf::from(p));
                }
            }
        }

        // Ask the user's login shell for PATH and re-try `which`
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        if let Ok(output) = std::process::Command::new(&shell)
            .arg("-lc")
            .arg(format!("which {}", cmd))
            .output()
        {
            if output.status.success() {
                let p = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !p.is_empty() {
                    return Some(std::path::PathBuf::from(p));
                }
            }
        }

        // Probe well-known locations where uv / pipx typically install tools
        let home = std::env::var("HOME").unwrap_or_default();
        let candidates = [
            format!("{}/.local/bin/{}", home, cmd),
            format!("{}/.cargo/bin/{}", home, cmd),
            format!("/usr/local/bin/{}", cmd),
            format!("/opt/homebrew/bin/{}", cmd),
        ];
        for c in &candidates {
            let p = std::path::PathBuf::from(c);
            if p.exists() {
                return Some(p);
            }
        }
    }

    None
}

/// Get the full shell PATH from the user's login shell.
/// This is needed because GUI apps on macOS don't inherit the user's PATH.
/// On Windows, GUI apps already inherit the user's PATH, so we just return it.
fn get_shell_path() -> Option<String> {
    #[cfg(windows)]
    {
        // On Windows, the PATH is inherited from the user's environment
        std::env::var("PATH").ok()
    }

    #[cfg(not(windows))]
    {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        if let Ok(output) = std::process::Command::new(&shell)
            .arg("-lc")
            .arg("echo $PATH")
            .output()
        {
            if output.status.success() {
                let p = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !p.is_empty() {
                    return Some(p);
                }
            }
        }
        None
    }
}

/// Test whether the configured babeldoc command is reachable.
/// Returns the version string on success.
#[tauri::command]
pub async fn test_babeldoc(state: State<'_, AppState>) -> Result<String, String> {
    let cmd = {
        let config = state
            .config
            .lock()
            .map_err(|e| format!("Config lock error: {}", e))?;
        config.ai.pdf_translation.babeldoc_command.clone()
    };

    let resolved = resolve_command_path(&cmd).ok_or_else(|| {
        format!(
            "Command '{}' not found. Make sure BabelDOC is installed and the path is correct.\n\n\
             Install with: uv tool install --python 3.12 BabelDOC\n\
             Or specify the full path in the settings.",
            cmd
        )
    })?;

    let mut command = tokio::process::Command::new(&resolved);
    command.arg("--version");

    // Inject the user's shell PATH so babeldoc can find its own dependencies
    if let Some(path) = get_shell_path() {
        command.env("PATH", &path);
    }

    let output = command
        .output()
        .await
        .map_err(|e| format!("Failed to run '{}': {}", resolved.display(), e))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let version_text = if !stdout.is_empty() { stdout } else { stderr };
        Ok(format!(
            "PDF translation server is working!\nPath: {}\n{}",
            resolved.display(),
            version_text
        ))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(format!(
            "PDF translation server command found at {} but exited with error:\n{}",
            resolved.display(),
            stderr
        ))
    }
}

/// Map the app's native_lang code to PDF translation server's --lang-out value.
fn babeldoc_lang_out(native_lang: &str) -> &str {
    match native_lang {
        "zh" => "zh-CN",
        "en" => "en",
        "ja" => "ja",
        "ko" => "ko",
        "es" => "es",
        "fr" => "fr",
        "de" => "de",
        "pt" => "pt",
        "ru" => "ru",
        "ar" => "ar",
        "it" => "it",
        "vi" => "vi",
        "th" => "th",
        "id" => "id",
        "hi" => "hi",
        other => other,
    }
}

/// Translate a paper's PDF using the external PDF translation server.
/// Spawns babeldoc as a separate subprocess (AGPL isolation) and emits
/// background-task events for progress tracking.
#[tauri::command]
pub async fn translate_pdf(
    state: State<'_, AppState>,
    app: AppHandle,
    paper_id: String,
    pdf_filename: Option<String>,
) -> Result<(), String> {
    let (pdf_path, paper_dir, paper_title, native_lang, pdf_cfg, ai_base_url, ai_api_key, ai_model) = {
        let config = state
            .config
            .lock()
            .map_err(|e| format!("Config lock error: {}", e))?;

        if !config.ai.pdf_translation.enabled {
            return Err(
                "PDF translation is not enabled. Enable it in Settings > AI / Translation.".into(),
            );
        }

        let native_lang = config.general.native_lang.clone();
        if native_lang.is_empty() {
            return Err(
                "Native language not configured. Set it in Settings > AI / Translation.".into(),
            );
        }

        let db = state
            .db
            .lock()
            .map_err(|e| format!("DB lock error: {}", e))?;
        let row = zoro_db::queries::papers::get_paper(&db.conn, &paper_id)
            .map_err(|e| format!("{}", e))?;
        let paper_dir = state.data_dir.join("library").join(&row.dir_path);
        let paper_title = row.title.clone();

        // Find the PDF to translate.
        // If a specific filename is given, resolve it; otherwise fall back to
        // paper.pdf, then the first available PDF attachment.
        let pdf_path = if let Some(ref fname) = pdf_filename {
            // Try root-level first (e.g. paper.pdf), then attachments/ subdir
            let root = paper_dir.join(fname);
            if root.exists() {
                root
            } else {
                let att = paper_dir.join("attachments").join(fname);
                if att.exists() {
                    att
                } else {
                    return Err(format!("PDF file not found: {}", fname));
                }
            }
        } else {
            let default_path = paper_dir.join("paper.pdf");
            if default_path.exists() {
                default_path
            } else {
                let mut found: Option<std::path::PathBuf> = None;
                if let Ok(atts) = attachments::get_paper_attachments(&db.conn, &paper_id) {
                    for att in &atts {
                        if att.file_type == "pdf" {
                            let att_path = paper_dir.join(&att.relative_path);
                            if att_path.exists() {
                                found = Some(att_path);
                                break;
                            }
                        }
                    }
                }
                found.ok_or_else(|| "No PDF file found for this paper.".to_string())?
            }
        };

        let pdf_cfg = config.ai.pdf_translation.clone();
        // Resolve the correct provider base_url/api_key for the current model
        let resolved_ai = config.ai.resolve_for_model(&config.ai.model);
        let ai_base_url = resolved_ai.base_url.clone();
        let ai_api_key = resolved_ai.api_key.clone();
        let ai_model = resolved_ai.model.clone();

        (
            pdf_path,
            paper_dir,
            paper_title,
            native_lang,
            pdf_cfg,
            ai_base_url,
            ai_api_key,
            ai_model,
        )
    };

    let lang_out = babeldoc_lang_out(&native_lang).to_string();
    // Derive output filename from the source PDF stem: {stem}.{lang}.pdf
    let source_stem = pdf_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("paper");
    let output_filename = format!("{}.{}.pdf", source_stem, native_lang);
    let output_path = paper_dir.join(&output_filename);

    let (api_key, base_url, model) = if pdf_cfg.use_ai_config {
        (ai_api_key, ai_base_url, ai_model)
    } else {
        (
            pdf_cfg.custom_api_key.clone(),
            pdf_cfg.custom_base_url.clone(),
            pdf_cfg.custom_model.clone(),
        )
    };

    if api_key.is_empty() || base_url.is_empty() || model.is_empty() {
        return Err(
            "AI API not configured for PDF translation. Set API key, base URL, and model.".into(),
        );
    }

    let pid = paper_id.clone();
    let task_id = format!("pdf-translate-{}", pid);
    let cmd = pdf_cfg.babeldoc_command.clone();
    let qps = pdf_cfg.qps;
    let debug_mode = state.debug_mode.load(std::sync::atomic::Ordering::Relaxed);
    let db_handle = state.db.clone();
    let output_filename_clone = output_filename.clone();

    tokio::spawn(async move {
        let _ = app.emit(
            "background-task",
            serde_json::json!({
                "task_id": task_id,
                "paper_id": pid,
                "paper_title": paper_title,
                "task_type": "pdf-translation",
                "status": "running",
                "message": "Starting PDF translation…",
            }),
        );

        // Build a temp output dir so babeldoc writes there.
        // Clean up any stale files from previous runs first (e.g. debug-mode
        // PDFs that were never properly cleaned up).
        let temp_out = paper_dir.join("_babeldoc_temp");
        let _ = std::fs::remove_dir_all(&temp_out);
        let _ = std::fs::create_dir_all(&temp_out);

        // Resolve the babeldoc command path (GUI apps may not have the user's PATH)
        let resolved_cmd =
            resolve_command_path(&cmd).unwrap_or_else(|| std::path::PathBuf::from(&cmd));

        use std::process::Stdio;

        // In debug mode, redirect babeldoc output to a log file and tail it
        // for clean, complete lines. This avoids the fragmented output caused
        // by the terminal-width line splitting when reading piped bytes.
        let log_file_path = temp_out.join("babeldoc.log");

        let mut command = tokio::process::Command::new(&resolved_cmd);

        // Inject the user's shell PATH so babeldoc can find its own dependencies
        if let Some(path) = get_shell_path() {
            command.env("PATH", &path);
        }

        // Force babeldoc to use a very wide terminal so it doesn't wrap lines
        command.env("COLUMNS", "9999");

        command
            .arg("--openai")
            .arg("--openai-model")
            .arg(&model)
            .arg("--openai-base-url")
            .arg(&base_url)
            .arg("--openai-api-key")
            .arg(&api_key)
            .arg("--files")
            .arg(&pdf_path)
            .arg("--output")
            .arg(&temp_out)
            .arg("--lang-out")
            .arg(&lang_out)
            .arg("--qps")
            .arg(qps.to_string())
            .arg("--watermark-output-mode")
            .arg("no_watermark");

        // Append user-supplied extra arguments (e.g. "--no-dual --skip-clean")
        let extra_args = pdf_cfg.extra_args.clone();
        for token in extra_args.split_whitespace() {
            command.arg(token);
        }

        if debug_mode {
            command.arg("--debug").arg("--ignore-cache");
        }

        // Log the full command line for debugging (mask the API key)
        {
            let prog = resolved_cmd.to_string_lossy();
            let args: Vec<String> = command
                .as_std()
                .get_args()
                .map(|a| {
                    let s = a.to_string_lossy().to_string();
                    s
                })
                .collect();
            // Mask --openai-api-key value (the arg right after the flag)
            let mut masked = Vec::with_capacity(args.len());
            let mut mask_next = false;
            for arg in &args {
                if mask_next {
                    masked.push("***".to_string());
                    mask_next = false;
                } else if arg == "--openai-api-key" {
                    masked.push(arg.clone());
                    mask_next = true;
                } else {
                    masked.push(arg.clone());
                }
            }
            tracing::debug!("[babeldoc] command: {} {}", prog, masked.join(" "));
        }

        // Create the log file and redirect stdout/stderr to it
        let log_file = match std::fs::File::create(&log_file_path) {
            Ok(f) => f,
            Err(e) => {
                let _ = app.emit(
                    "background-task",
                    serde_json::json!({
                        "task_id": task_id,
                        "paper_id": pid,
                        "task_type": "pdf-translation",
                        "status": "failed",
                        "message": format!("Failed to create log file: {}", e),
                    }),
                );
                return;
            }
        };
        let log_file_clone = log_file.try_clone().unwrap();
        command
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(log_file_clone));

        let spawn_result = command.spawn();

        match spawn_result {
            Ok(mut child) => {
                // Spawn an async task that tails the log file, reading complete
                // lines and forwarding them via tracing::debug!
                let tail_log_path = log_file_path.clone();
                let stop_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let stop_flag_clone = stop_flag.clone();

                let tail_task = tokio::spawn(async move {
                    use tokio::io::AsyncBufReadExt;

                    // Wait briefly for the file to be created / first bytes written
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

                    let file = match tokio::fs::File::open(&tail_log_path).await {
                        Ok(f) => f,
                        Err(_) => return,
                    };
                    let mut reader = tokio::io::BufReader::new(file);
                    let mut line = String::new();

                    loop {
                        line.clear();
                        match reader.read_line(&mut line).await {
                            Ok(0) => {
                                // EOF — check if the process has finished
                                if stop_flag_clone.load(std::sync::atomic::Ordering::Relaxed) {
                                    // Do one final drain pass
                                    loop {
                                        line.clear();
                                        match reader.read_line(&mut line).await {
                                            Ok(0) | Err(_) => break,
                                            Ok(_) => {
                                                let trimmed = line.trim();
                                                if !trimmed.is_empty() {
                                                    tracing::debug!("[babeldoc] {}", trimmed);
                                                }
                                            }
                                        }
                                    }
                                    break;
                                }
                                // Process still running — wait a bit and retry
                                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            }
                            Ok(_) => {
                                let trimmed = line.trim();
                                if !trimmed.is_empty() {
                                    tracing::debug!("[babeldoc] {}", trimmed);
                                }
                            }
                            Err(_) => break,
                        }
                    }
                });

                // Wait for the process to finish
                let status = child.wait().await;
                // Signal the tail task to stop and drain remaining lines
                stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                let _ = tail_task.await;

                // Read stderr content from the log file for error reporting
                let stderr_str = std::fs::read_to_string(&log_file_path).unwrap_or_default();

                match status {
                    Ok(exit_status) if exit_status.success() => {
                        // BabelDOC outputs translated file(s) in the temp dir — find the translated PDF
                        let found = find_translated_pdf(&temp_out);
                        match found {
                            Some(src) => {
                                if let Err(e) = std::fs::rename(&src, &output_path) {
                                    if let Err(e2) = std::fs::copy(&src, &output_path) {
                                        let _ = app.emit(
                                        "background-task",
                                        serde_json::json!({
                                            "task_id": task_id,
                                            "paper_id": pid,
                                            "task_type": "pdf-translation",
                                            "status": "failed",
                                            "message": format!("Failed to move output: {} / {}", e, e2),
                                        }),
                                    );
                                        let _ = std::fs::remove_dir_all(&temp_out);
                                        return;
                                    }
                                }
                                let _ = std::fs::remove_dir_all(&temp_out);

                                // Register the translated PDF as an attachment in DB
                                let file_size =
                                    std::fs::metadata(&output_path).map(|m| m.len() as i64).ok();
                                if let Ok(db) = db_handle.lock() {
                                    let _ = attachments::insert_attachment(
                                        &db.conn,
                                        &pid,
                                        &output_filename_clone,
                                        "pdf",
                                        Some("application/pdf"),
                                        file_size,
                                        &output_filename_clone,
                                        "pdf-translation",
                                    );
                                }

                                let _ = app.emit(
                                    "background-task",
                                    serde_json::json!({
                                        "task_id": task_id,
                                        "paper_id": pid,
                                        "task_type": "pdf-translation",
                                        "status": "completed",
                                        "message": "PDF translation completed",
                                    }),
                                );

                                // Emit dedicated completion event for UI to refresh
                                let _ = app.emit(
                                    "pdf-translation-complete",
                                    serde_json::json!({
                                        "paperId": pid,
                                        "outputFilename": output_filename_clone,
                                        "status": "completed",
                                    }),
                                );

                                // Emit paper-updated so the library list refreshes
                                let _ = app.emit(
                                    "paper-updated",
                                    serde_json::json!({
                                        "paperId": pid,
                                    }),
                                );
                            }
                            None => {
                                let _ = std::fs::remove_dir_all(&temp_out);
                                let _ = app.emit(
                                "background-task",
                                serde_json::json!({
                                    "task_id": task_id,
                                    "paper_id": pid,
                    "task_type": "pdf-translation",
                                    "status": "failed",
                                    "message": "PDF translation server finished but no translated PDF found in output",
                                }),
                            );
                            }
                        }
                    }
                    Ok(exit_status) => {
                        let _ = std::fs::remove_dir_all(&temp_out);
                        let msg = if stderr_str.len() > 300 {
                            format!("{}…", &stderr_str[..300])
                        } else {
                            stderr_str
                        };
                        let _ = app.emit(
                            "background-task",
                            serde_json::json!({
                                "task_id": task_id,
                                "paper_id": pid,
                                "task_type": "pdf-translation",
                                "status": "failed",
                                "message": format!("PDF translation server exited with code {}: {}", exit_status, msg),
                            }),
                        );
                    }
                    Err(e) => {
                        let _ = std::fs::remove_dir_all(&temp_out);
                        let _ = app.emit(
                            "background-task",
                            serde_json::json!({
                                "task_id": task_id,
                                "paper_id": pid,
                                "task_type": "pdf-translation",
                                "status": "failed",
                                "message": format!("Failed to wait for PDF translation server: {}", e),
                            }),
                        );
                    }
                }
            }
            Err(e) => {
                let _ = std::fs::remove_dir_all(&temp_out);
                let _ = app.emit(
                    "background-task",
                    serde_json::json!({
                        "task_id": task_id,
                        "paper_id": pid,
                        "task_type": "pdf-translation",
                        "status": "failed",
                        "message": format!("Failed to run PDF translation server: {}. Is it installed?", e),
                    }),
                );
            }
        }
    });

    Ok(())
}

/// Find the translated PDF output by PDF translation server in the temp directory.
/// PDF translation server typically creates files like `paper_translated.pdf` or `paper_dual.pdf`.
fn find_translated_pdf(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut pdfs: Vec<std::path::PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("pdf"))
                .unwrap_or(false)
        })
        .collect();

    // Prefer files with "translated" or "dual" in the name
    pdfs.sort_by(|a, b| {
        let a_name = a
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        let b_name = b
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        let a_prio = a_name.contains("translate");
        let b_prio = b_name.contains("translate");
        b_prio.cmp(&a_prio)
    });

    pdfs.into_iter().next()
}

// --- Helpers ---

/// Fetch original text fields for an entity from the DB.
fn get_original_texts(
    conn: &rusqlite::Connection,
    entity_type: &str,
    entity_id: &str,
) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();

    match entity_type {
        "paper" => {
            if let Ok(paper) = zoro_db::queries::papers::get_paper(conn, entity_id) {
                map.insert("title".to_string(), paper.title);
                if let Some(abs) = paper.abstract_text {
                    map.insert("abstract_text".to_string(), abs);
                }
            }
        }
        "subscription_item" => {
            if let Ok(item) =
                zoro_db::queries::subscriptions::get_subscription_item(conn, entity_id)
            {
                map.insert("title".to_string(), item.title);
                if let Some(ref data_json) = item.data_json {
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(data_json) {
                        if let Some(abs) = data.get("abstract_text").and_then(|v| v.as_str()) {
                            map.insert("abstract_text".to_string(), abs.to_string());
                        }
                        if let Some(summary) = data.get("ai_summary").and_then(|v| v.as_str()) {
                            map.insert("ai_summary".to_string(), summary.to_string());
                        }
                    }
                }
            }
        }
        "note" => {
            if let Ok(note) = zoro_db::queries::notes::get_note(conn, entity_id) {
                map.insert("content".to_string(), note.content);
            }
        }
        "papers_cool_paper" => {
            if let Ok(Some((title, abs))) =
                zoro_db::queries::papers_cool_cache::get_paper_text(conn, entity_id)
            {
                map.insert("title".to_string(), title);
                if let Some(abs) = abs {
                    map.insert("abstract_text".to_string(), abs);
                }
            }
        }
        _ => {}
    }

    map
}

// --- Glossary Commands ---

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlossaryTermResponse {
    pub id: String,
    pub source_term: String,
    pub translated_term: String,
    pub target_lang: String,
    pub source: String,
    pub occurrence_count: i64,
    pub created_date: String,
    pub updated_date: String,
}

impl From<zoro_db::queries::glossary::GlossaryRow> for GlossaryTermResponse {
    fn from(r: zoro_db::queries::glossary::GlossaryRow) -> Self {
        Self {
            id: r.id,
            source_term: r.source_term,
            translated_term: r.translated_term,
            target_lang: r.target_lang,
            source: r.source,
            occurrence_count: r.occurrence_count,
            created_date: r.created_date,
            updated_date: r.updated_date,
        }
    }
}

#[tauri::command]
pub async fn get_glossary(state: State<'_, AppState>) -> Result<Vec<GlossaryTermResponse>, String> {
    let target_lang = {
        let config = state
            .config
            .lock()
            .map_err(|e| format!("Config lock error: {}", e))?;
        config.general.native_lang.clone()
    };
    if target_lang.is_empty() {
        return Ok(Vec::new());
    }
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let rows = zoro_db::queries::glossary::list_glossary(&db.conn, &target_lang)
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(rows.into_iter().map(GlossaryTermResponse::from).collect())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddGlossaryTermInput {
    pub source_term: String,
    pub translated_term: String,
}

#[tauri::command]
pub async fn add_glossary_term(
    state: State<'_, AppState>,
    input: AddGlossaryTermInput,
) -> Result<GlossaryTermResponse, String> {
    let target_lang = {
        let config = state
            .config
            .lock()
            .map_err(|e| format!("Config lock error: {}", e))?;
        config.general.native_lang.clone()
    };
    if target_lang.is_empty() {
        return Err("Native language not configured.".into());
    }
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    let row = zoro_db::queries::glossary::add_glossary_term(
        &db.conn,
        &input.source_term,
        &input.translated_term,
        &target_lang,
        "manual",
    )
    .map_err(|e| format!("DB error: {}", e))?;
    Ok(GlossaryTermResponse::from(row))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateGlossaryTermInput {
    pub id: String,
    pub translated_term: String,
}

#[tauri::command]
pub async fn update_glossary_term(
    state: State<'_, AppState>,
    input: UpdateGlossaryTermInput,
) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    zoro_db::queries::glossary::update_glossary_term(&db.conn, &input.id, &input.translated_term)
        .map_err(|e| format!("DB error: {}", e))
}

#[tauri::command]
pub async fn promote_glossary_term(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    zoro_db::queries::glossary::promote_glossary_term(&db.conn, &id)
        .map_err(|e| format!("DB error: {}", e))
}

#[tauri::command]
pub async fn delete_glossary_term(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    zoro_db::queries::glossary::delete_glossary_term(&db.conn, &id)
        .map_err(|e| format!("DB error: {}", e))
}

#[tauri::command]
pub async fn clear_glossary(state: State<'_, AppState>) -> Result<usize, String> {
    let target_lang = {
        let config = state
            .config
            .lock()
            .map_err(|e| format!("Config lock error: {}", e))?;
        config.general.native_lang.clone()
    };
    if target_lang.is_empty() {
        return Ok(0);
    }
    let db = state
        .db
        .lock()
        .map_err(|e| format!("DB lock error: {}", e))?;
    zoro_db::queries::glossary::clear_glossary(&db.conn, &target_lang)
        .map_err(|e| format!("DB error: {}", e))
}
