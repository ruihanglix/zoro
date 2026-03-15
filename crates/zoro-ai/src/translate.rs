// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::client::ChatClient;
use crate::error::AiError;
use zoro_core::models::{AiConfig, TranslationPrompts};

/// A glossary entry: source term → translated term.
#[derive(Debug, Clone)]
pub struct GlossaryEntry {
    pub source: String,
    pub translated: String,
}

/// Map a language code to a human-readable name for use in prompts.
pub fn lang_code_to_name(code: &str) -> &str {
    match code {
        "zh" => "Chinese",
        "ja" => "Japanese",
        "ko" => "Korean",
        "es" => "Spanish",
        "fr" => "French",
        "de" => "German",
        "pt" => "Portuguese",
        "ru" => "Russian",
        "ar" => "Arabic",
        "it" => "Italian",
        "nl" => "Dutch",
        "pl" => "Polish",
        "tr" => "Turkish",
        "vi" => "Vietnamese",
        "th" => "Thai",
        "id" => "Indonesian",
        "uk" => "Ukrainian",
        "cs" => "Czech",
        "sv" => "Swedish",
        "hi" => "Hindi",
        "en" => "English",
        other => other,
    }
}

/// Apply template substitution: replace `{{text}}` and `{{target_lang}}`.
fn render_template(template: &str, text: &str, target_lang_name: &str) -> String {
    template
        .replace("{{text}}", text)
        .replace("{{target_lang}}", target_lang_name)
}

/// Filter glossary entries to only those whose source term appears in the text
/// (case-insensitive substring match).
pub fn filter_glossary_for_text(glossary: &[GlossaryEntry], text: &str) -> Vec<GlossaryEntry> {
    let lower = text.to_lowercase();
    glossary
        .iter()
        .filter(|e| lower.contains(&e.source.to_lowercase()))
        .cloned()
        .collect()
}

/// Build the glossary instruction block to append to a system prompt.
fn build_glossary_prompt(entries: &[GlossaryEntry]) -> String {
    if entries.is_empty() {
        return String::new();
    }
    let mut lines =
        String::from("\n\nUse the following glossary for consistent terminology translation:\n");
    for e in entries {
        lines.push_str(&format!("- {} → {}\n", e.source, e.translated));
    }
    lines
}

/// Translate a single text field using the configured LLM.
///
/// `field_type` should be `"title"` or `"abstract_text"` to select the
/// appropriate prompt templates. `glossary` entries matching the source text
/// are appended to the system prompt for consistent terminology.
pub async fn translate_text(
    config: &AiConfig,
    prompts: &TranslationPrompts,
    field_type: &str,
    text: &str,
    target_lang: &str,
    glossary: &[GlossaryEntry],
) -> Result<String, AiError> {
    if config.base_url.is_empty() || config.api_key.is_empty() || config.model.is_empty() {
        return Err(AiError::NotConfigured(
            "AI base_url, api_key, and model must all be set".to_string(),
        ));
    }

    if text.trim().is_empty() {
        return Ok(String::new());
    }

    let lang_name = lang_code_to_name(target_lang);

    // Dedicated prompt for short AI summaries: faithful translation without expansion
    let summary_system: String = [
        "You are a professional academic translator. ",
        "Translate the following AI-generated paper summary to {{target_lang}}. ",
        "Maintain academic tone and technical accuracy. ",
        "Translate faithfully \u{2014} do NOT add, elaborate, or expand beyond the original content.\n\n",
        "IMPORTANT formatting rules:\n",
        "- Split the translation into short paragraphs of 2\u{2013}3 sentences each, ",
        "separated by a blank line.\n",
        "- This improves readability for bilingual side-by-side display.\n\n",
        "Output ONLY the translated text, nothing else.",
    ]
    .concat();
    let summary_user = "{{text}}".to_string();

    // Fall back to default prompts when the configured ones are empty
    let defaults = TranslationPrompts::default();
    let (system_template, user_template) = match field_type {
        "title" => (
            if prompts.title_system.is_empty() {
                &defaults.title_system
            } else {
                &prompts.title_system
            },
            if prompts.title_user.is_empty() {
                &defaults.title_user
            } else {
                &prompts.title_user
            },
        ),
        "ai_summary" => (&summary_system, &summary_user),
        _ => (
            if prompts.abstract_system.is_empty() {
                &defaults.abstract_system
            } else {
                &prompts.abstract_system
            },
            if prompts.abstract_user.is_empty() {
                &defaults.abstract_user
            } else {
                &prompts.abstract_user
            },
        ),
    };

    let mut system_prompt = render_template(system_template, text, lang_name);

    let matched = filter_glossary_for_text(glossary, text);
    system_prompt.push_str(&build_glossary_prompt(&matched));

    let user_prompt = render_template(user_template, text, lang_name);

    tracing::debug!(
        field_type = %field_type,
        target_lang = %target_lang,
        lang_name = %lang_name,
        source_text_len = text.len(),
        glossary_terms = matched.len(),
        "Translating field"
    );

    let client = ChatClient::new(&config.base_url, &config.api_key, &config.model);
    let result = client.chat(&system_prompt, &user_prompt, 0.3, None).await?;

    tracing::debug!(
        field_type = %field_type,
        target_lang = %target_lang,
        result_len = result.len(),
        result = %result,
        "Translation complete"
    );

    Ok(result)
}

/// Extract technical terms and their translations from academic text using an LLM.
/// Returns a list of (source_term, translated_term) pairs.
pub async fn extract_glossary_terms(
    config: &AiConfig,
    text: &str,
    target_lang: &str,
) -> Result<Vec<(String, String)>, AiError> {
    if config.base_url.is_empty() || config.api_key.is_empty() || config.model.is_empty() {
        return Err(AiError::NotConfigured(
            "AI base_url, api_key, and model must all be set".to_string(),
        ));
    }

    if text.trim().is_empty() {
        return Ok(Vec::new());
    }

    let lang_name = lang_code_to_name(target_lang);

    let system_prompt = format!(
        concat!(
            "You are a terminology extraction assistant for academic papers. ",
            "Extract important technical terms, proper nouns, and domain-specific ",
            "terminology from the given text. For each term, provide its translation to {}. ",
            "Return ONLY a JSON array of objects with \"source\" and \"translated\" fields. ",
            "Keep terms concise (1-4 words). Extract 5-20 terms maximum. ",
            "Do NOT include common words or generic phrases. ",
            "Example: [{{\"source\": \"attention mechanism\", \"translated\": \"注意力机制\"}}]",
        ),
        lang_name
    );

    let client = ChatClient::new(&config.base_url, &config.api_key, &config.model);
    let result = client.chat(&system_prompt, text, 0.3, None).await?;

    parse_extracted_terms(&result)
}

/// Parse the JSON response from the LLM into term pairs.
fn parse_extracted_terms(response: &str) -> Result<Vec<(String, String)>, AiError> {
    let trimmed = response.trim();

    // Try to extract JSON array from the response (handle markdown code blocks)
    let json_str = if let Some(start) = trimmed.find('[') {
        if let Some(end) = trimmed.rfind(']') {
            &trimmed[start..=end]
        } else {
            trimmed
        }
    } else {
        trimmed
    };

    let parsed: Vec<serde_json::Value> = serde_json::from_str(json_str).unwrap_or_default();

    let terms: Vec<(String, String)> = parsed
        .into_iter()
        .filter_map(|v| {
            let source = v.get("source")?.as_str()?.trim().to_string();
            let translated = v.get("translated")?.as_str()?.trim().to_string();
            if source.is_empty() || translated.is_empty() {
                None
            } else {
                Some((source, translated))
            }
        })
        .collect();

    Ok(terms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lang_code_to_name() {
        assert_eq!(lang_code_to_name("zh"), "Chinese");
        assert_eq!(lang_code_to_name("ja"), "Japanese");
        assert_eq!(lang_code_to_name("unknown"), "unknown");
    }

    #[test]
    fn test_render_template() {
        let result = render_template(
            "Translate to {{target_lang}}: {{text}}",
            "Hello world",
            "Chinese",
        );
        assert_eq!(result, "Translate to Chinese: Hello world");
    }

    #[test]
    fn test_filter_glossary_for_text() {
        let glossary = vec![
            GlossaryEntry {
                source: "Transformer".to_string(),
                translated: "Transformer".to_string(),
            },
            GlossaryEntry {
                source: "attention".to_string(),
                translated: "注意力".to_string(),
            },
            GlossaryEntry {
                source: "RLHF".to_string(),
                translated: "人类反馈强化学习".to_string(),
            },
        ];
        let text = "We use a Transformer with cross-attention layers.";
        let matched = filter_glossary_for_text(&glossary, text);
        assert_eq!(matched.len(), 2);
    }

    #[test]
    fn test_build_glossary_prompt() {
        let entries = vec![GlossaryEntry {
            source: "attention".to_string(),
            translated: "注意力".to_string(),
        }];
        let prompt = build_glossary_prompt(&entries);
        assert!(prompt.contains("attention → 注意力"));
    }

    #[test]
    fn test_parse_extracted_terms() {
        let json = r#"[{"source": "Transformer", "translated": "Transformer"}, {"source": "RLHF", "translated": "人类反馈强化学习"}]"#;
        let terms = parse_extracted_terms(json).unwrap();
        assert_eq!(terms.len(), 2);
        assert_eq!(terms[0].0, "Transformer");

        // With markdown wrapper
        let md = "```json\n[{\"source\": \"test\", \"translated\": \"测试\"}]\n```";
        let terms = parse_extracted_terms(md).unwrap();
        assert_eq!(terms.len(), 1);
    }
}
