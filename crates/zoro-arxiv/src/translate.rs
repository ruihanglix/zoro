// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::ArxivError;
use futures::stream::{self, StreamExt};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use zoro_ai::client::ChatClient;
use zoro_core::models::AiConfig;

const TRANSLATION_CLASS: &str = "zr-translation-block";
const TRANSLATION_ATTR: &str = "data-zotero-translation";
const TRANSLATION_STYLE_ID: &str = "zotero-arxiv-translation-style";

const TRANSLATION_STYLE: &str = r#"
.zr-translation-block {
  border: 1.5px dashed #9ca3af;
  border-radius: 6px;
  padding: 8px 12px;
  margin-top: 6px;
  margin-bottom: 4px;
}
.zr-translation-block,
.zr-translation-block[data-zotero-translation] * {
  color: #374151;
}
html.zr-dark .zr-translation-block {
  border-color: #4b5563;
}
html.zr-dark .zr-translation-block,
html.zr-dark .zr-translation-block[data-zotero-translation] * {
  color: #c8cfd8 !important;
}
"#;

const DEFAULT_HTML_SYSTEM_PROMPT: &str = concat!(
    "You are a professional academic translator. ",
    "Translate the following paragraph from an academic paper to {{target_lang}}. ",
    "Maintain academic tone and technical accuracy. ",
    "Keep all [MATH_N] placeholders exactly as they are — do NOT translate, ",
    "modify, or remove them. ",
    "Preserve all formatting markers: **bold** and __italic__. ",
    "If the original text has **bold text**, the translation must keep the same ",
    "words wrapped in ** markers. Same for __italic__. ",
    "Output ONLY the translated text, nothing else.",
);

const DEFAULT_HTML_USER_PROMPT: &str = "{{text}}";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateProgress {
    pub total: usize,
    pub done: usize,
    pub failed: usize,
    pub status: String,
    pub current_paragraph: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateResult {
    pub total_paragraphs: usize,
    pub translated: usize,
    pub skipped: usize,
    pub failed: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslatedParagraph {
    pub tag: String,
    pub original_text_snippet: String,
    pub translation_html: String,
}

const TRANSLATABLE_SELECTOR: &str = "p, h1, h2, h3, h4, h5, figcaption, blockquote";

/// A translatable element extracted from the HTML.
#[derive(Debug, Clone)]
struct TranslatableElement {
    tag: String,
    /// Plain text for locating the element in raw HTML
    text: String,
    /// Text with `[MATH_N]` placeholders sent to the LLM
    text_for_llm: String,
    /// Original `<math>...</math>` blocks to restore after translation
    math_blocks: Vec<String>,
}

/// Process inner HTML for translation:
/// - Replace `<math>...</math>` with `[MATH_N]` placeholders (restored after translation)
/// - Remove `<cite>...</cite>` blocks (citations omitted from translation)
///
/// Returns (text_for_llm, math_blocks_to_restore).
fn prepare_for_translation(inner_html: &str) -> (String, Vec<String>) {
    // Remove <cite>...</cite> blocks (inline citations like "(Wu et al., 2025)")
    let cite_re = regex::Regex::new(r"(?si)<cite\b[^>]*>.*?</cite>").unwrap();
    let no_cites = cite_re.replace_all(inner_html, "").to_string();

    // Replace <math>...</math> with placeholders
    let math_re = regex::Regex::new(r"(?si)<math\b[^>]*>.*?</math>").unwrap();

    let mut math_blocks = Vec::new();
    for m in math_re.find_iter(&no_cites) {
        math_blocks.push(m.as_str().to_string());
    }

    let mut idx = 0;
    let replaced = math_re
        .replace_all(&no_cites, |_caps: &regex::Captures| {
            let placeholder = format!("[MATH_{}]", idx);
            idx += 1;
            placeholder
        })
        .to_string();

    let clean = strip_tags_keep_formatting(&replaced);
    // Collapse multiple spaces left by removed citations
    let collapsed = regex::Regex::new(r"\s{2,}")
        .unwrap()
        .replace_all(clean.trim(), " ")
        .to_string();
    (collapsed, math_blocks)
}

/// Extract translatable elements from HTML that haven't been translated yet.
fn extract_untranslated_elements(html: &str) -> Vec<TranslatableElement> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse(TRANSLATABLE_SELECTOR).unwrap();

    let mut elements = Vec::new();

    for el in doc.select(&sel) {
        if el.value().attr(TRANSLATION_ATTR) == Some("true") {
            continue;
        }

        let text = el.text().collect::<String>().trim().to_string();
        if text.is_empty() {
            continue;
        }

        let tag = el.value().name().to_string();
        let min_len = if tag == "p" || tag == "figcaption" || tag == "blockquote" {
            10
        } else {
            2
        };
        if text.len() < min_len {
            continue;
        }

        let inner_html = el.inner_html();
        let (text_for_llm, math_blocks) = prepare_for_translation(&inner_html);

        elements.push(TranslatableElement {
            tag,
            text,
            text_for_llm,
            math_blocks,
        });
    }

    deduplicate_elements(elements)
}

/// Ensure the translation style block is present in the HTML.
fn ensure_translation_style(html: &str) -> String {
    if html.contains(TRANSLATION_STYLE_ID) {
        return html.to_string();
    }

    let style_tag = format!(
        "<style id=\"{}\">{}</style>",
        TRANSLATION_STYLE_ID,
        TRANSLATION_STYLE.trim()
    );

    if let Some(pos) = html.find("</head>") {
        let mut result = html.to_string();
        result.insert_str(pos, &style_tag);
        result
    } else if let Some(pos) = html.find("<body") {
        let mut result = html.to_string();
        result.insert_str(pos, &format!("<head>{}</head>", style_tag));
        result
    } else {
        format!("{}{}", style_tag, html)
    }
}

/// Build the translation block HTML to insert after an element.
/// Uses the same tag so it inherits the same styles as the original.
/// Restores `[MATH_N]` placeholders with the original `<math>` HTML,
/// and converts markdown `**bold**` / `__italic__` back to HTML tags.
fn build_translation_block(tag: &str, translated_text: &str, math_blocks: &[String]) -> String {
    let mut text = translated_text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");

    // Convert markdown-style formatting to HTML (`*` is not HTML-escaped)
    text = restore_formatting(&text);

    // Restore math blocks (raw HTML from original document, safe to inject)
    for (i, math_html) in math_blocks.iter().enumerate() {
        let placeholder = format!("[MATH_{}]", i);
        text = text.replace(&placeholder, math_html);
    }

    format!(
        "<{} class=\"{}\" {}=\"true\">{}</{}>",
        tag, TRANSLATION_CLASS, TRANSLATION_ATTR, text, tag
    )
}

/// Translate a single paragraph using the AI client.
async fn translate_paragraph(
    client: &ChatClient,
    text: &str,
    system_prompt: &str,
    user_template: &str,
    temperature: f32,
) -> Result<String, ArxivError> {
    let user_prompt = user_template.replace("{{text}}", text);
    client
        .chat(system_prompt, &user_prompt, temperature, None)
        .await
        .map_err(|e| ArxivError::Translation(e.to_string()))
}

/// Render prompt templates with target language.
fn render_prompt(template: &str, target_lang: &str) -> String {
    template.replace(
        "{{target_lang}}",
        zoro_ai::translate::lang_code_to_name(target_lang),
    )
}

/// Strip HTML tags, leaving only text content.
fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            out.push(ch);
        }
    }
    out
}

/// Strip HTML tags but convert bold/italic formatting to markdown markers.
/// Uses a stack to track which `</span>` closes which formatting span.
fn strip_tags_keep_formatting(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    let mut fmt_stack: Vec<&str> = Vec::new();

    while let Some(ch) = chars.next() {
        if ch != '<' {
            out.push(ch);
            continue;
        }
        let mut tag = String::from('<');
        for c in chars.by_ref() {
            tag.push(c);
            if c == '>' {
                break;
            }
        }
        let lower = tag.to_lowercase();
        if lower.starts_with("<b>") || lower.starts_with("<b ") || lower.starts_with("<strong") {
            out.push_str("**");
            fmt_stack.push("bold");
        } else if lower.starts_with("</b>") || lower.starts_with("</strong>") {
            out.push_str("**");
        } else if lower.starts_with("<em") || lower.starts_with("<i>") || lower.starts_with("<i ") {
            out.push_str("__");
            fmt_stack.push("italic");
        } else if lower.starts_with("</em>") || lower.starts_with("</i>") {
            out.push_str("__");
        } else if lower.contains("ltx_font_bold") {
            out.push_str("**");
            fmt_stack.push("bold");
        } else if lower.contains("ltx_font_italic") {
            out.push_str("__");
            fmt_stack.push("italic");
        } else if lower.starts_with("</span>") {
            if let Some(fmt) = fmt_stack.pop() {
                match fmt {
                    "bold" => out.push_str("**"),
                    "italic" => out.push_str("__"),
                    _ => {}
                }
            }
        } else if lower.starts_with("<span") {
            fmt_stack.push("other");
        }
        // all other tags are silently dropped
    }
    out
}

/// Convert markdown-style bold/italic back to HTML tags.
fn restore_formatting(text: &str) -> String {
    let bold_re = regex::Regex::new(r"\*\*(.+?)\*\*").unwrap();
    let result = bold_re.replace_all(text, "<b>$1</b>").to_string();
    let italic_re = regex::Regex::new(r"__(.+?)__").unwrap();
    italic_re.replace_all(&result, "<i>$1</i>").to_string()
}

/// Remove elements whose text is fully contained in another element's text
/// (e.g. individual list items that are already part of a larger paragraph).
fn deduplicate_elements(elements: Vec<TranslatableElement>) -> Vec<TranslatableElement> {
    let texts: Vec<String> = elements.iter().map(|e| e.text.clone()).collect();
    elements
        .into_iter()
        .enumerate()
        .filter(|(i, elem)| {
            !texts.iter().enumerate().any(|(j, other)| {
                j != *i && other.len() > elem.text.len() && other.contains(elem.text.as_str())
            })
        })
        .map(|(_, elem)| elem)
        .collect()
}

/// Truncate a string to at most `max_bytes` on a valid UTF-8 char boundary.
fn safe_truncate(s: &str, max_bytes: usize) -> &str {
    if max_bytes >= s.len() {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Find the position right after the closing tag for an element
/// identified by tag name and a snippet of its text content.
/// Tag-aware: strips inline tags (`<b>`, `<a>`, etc.) before matching.
fn find_element_end(html: &str, tag: &str, text: &str) -> Option<usize> {
    let snippet = safe_truncate(text, 50);
    let open_re = regex::Regex::new(&format!(r"<{}[\s>]", regex::escape(tag))).unwrap();
    let close_tag = format!("</{}>", tag);

    for m in open_re.find_iter(html) {
        let tag_start = m.start();
        let tag_end = tag_start + html[tag_start..].find('>')?;
        let content_start = tag_end + 1;

        let close_pos = html[content_start..].find(&close_tag)?;
        let abs_close = content_start + close_pos;

        let inner = &html[content_start..abs_close];
        let plain = strip_tags(inner);

        if plain.contains(snippet) {
            return Some(abs_close + close_tag.len());
        }
    }
    None
}

/// Add `data-zotero-translation="true"` to an element's opening tag,
/// identified by tag name and text content.
fn add_translation_attr(html: &str, tag: &str, text: &str) -> String {
    let snippet = safe_truncate(text, 50);
    let open_re = regex::Regex::new(&format!(r"<{}[\s>]", regex::escape(tag))).unwrap();
    let close_tag = format!("</{}>", tag);

    for m in open_re.find_iter(html) {
        let tag_start = m.start();
        let tag_end_offset = match html[tag_start..].find('>') {
            Some(pos) => pos,
            None => continue,
        };
        let tag_end_pos = tag_start + tag_end_offset;
        let content_start = tag_end_pos + 1;

        let close_pos = match html[content_start..].find(&close_tag) {
            Some(pos) => content_start + pos,
            None => continue,
        };

        let inner = &html[content_start..close_pos];
        let plain = strip_tags(inner);

        if plain.contains(snippet) {
            let opening = &html[tag_start..=tag_end_pos];
            if opening.contains(TRANSLATION_ATTR) {
                return html.to_string();
            }
            let mut result = html.to_string();
            result.insert_str(tag_end_pos, &format!(" {}=\"true\"", TRANSLATION_ATTR));
            return result;
        }
    }

    html.to_string()
}

const DEFAULT_CONCURRENCY: usize = 8;

/// Translate all untranslated paragraphs in an HTML string.
/// Sends up to `concurrency` translation requests in parallel.
/// Calls `on_progress` after each paragraph completes, and `on_inserted`
/// after each translation is inserted into the HTML (with current HTML and paragraph info).
/// `glossary_prompt` is an optional pre-built glossary block to append to the system prompt.
#[allow(clippy::too_many_arguments)]
pub async fn translate_html<F, G>(
    html: &str,
    ai_config: &AiConfig,
    target_lang: &str,
    html_system_prompt: Option<&str>,
    html_user_prompt: Option<&str>,
    glossary_prompt: Option<&str>,
    concurrency: usize,
    on_progress: F,
    mut on_inserted: G,
) -> Result<(String, TranslateResult), ArxivError>
where
    F: FnMut(TranslateProgress) + Send + 'static,
    G: FnMut(&str, &TranslatedParagraph) + Send + 'static,
{
    let concurrency = if concurrency == 0 {
        DEFAULT_CONCURRENCY
    } else {
        concurrency
    };
    if ai_config.base_url.is_empty() || ai_config.api_key.is_empty() || ai_config.model.is_empty() {
        return Err(ArxivError::AiNotConfigured(
            "AI base_url, api_key, and model must all be set".to_string(),
        ));
    }

    let elements = extract_untranslated_elements(html);
    let total = elements.len();

    let on_progress = Arc::new(Mutex::new(on_progress));

    let mut result = TranslateResult {
        total_paragraphs: total,
        translated: 0,
        skipped: 0,
        failed: 0,
        error: None,
    };

    if total == 0 {
        on_progress.lock().await(TranslateProgress {
            total: 0,
            done: 0,
            failed: 0,
            status: "no_paragraphs".to_string(),
            current_paragraph: None,
        });
        return Ok((html.to_string(), result));
    }

    let sys_template = html_system_prompt.unwrap_or(DEFAULT_HTML_SYSTEM_PROMPT);
    let usr_template = html_user_prompt.unwrap_or(DEFAULT_HTML_USER_PROMPT);
    let mut system_prompt = render_prompt(sys_template, target_lang);
    if let Some(gp) = glossary_prompt {
        system_prompt.push_str(gp);
    }
    let user_template = render_prompt(usr_template, target_lang);

    let client = Arc::new(ChatClient::new(
        &ai_config.base_url,
        &ai_config.api_key,
        &ai_config.model,
    ));

    {
        on_progress.lock().await(TranslateProgress {
            total,
            done: 0,
            failed: 0,
            status: "translating".to_string(),
            current_paragraph: None,
        });
    }

    tracing::info!(
        total = total,
        concurrency = concurrency,
        "Starting concurrent translation"
    );

    let done_counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let failed_counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let indexed: Vec<(usize, TranslatableElement)> = elements.into_iter().enumerate().collect();

    let mut stream = stream::iter(indexed.into_iter().map(|(idx, elem)| {
        let client = Arc::clone(&client);
        let system_prompt = system_prompt.clone();
        let user_template = user_template.clone();
        let progress_cb = Arc::clone(&on_progress);
        let done_counter = Arc::clone(&done_counter);
        let failed_counter = Arc::clone(&failed_counter);

        async move {
            let preview = if elem.text.len() > 80 {
                format!("{}...", &elem.text[..80])
            } else {
                elem.text.clone()
            };

            match translate_paragraph(
                &client,
                &elem.text_for_llm,
                &system_prompt,
                &user_template,
                0.3,
            )
            .await
            {
                Ok(translated) => {
                    let done = done_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    let failed = failed_counter.load(std::sync::atomic::Ordering::Relaxed);
                    progress_cb.lock().await(TranslateProgress {
                        total,
                        done,
                        failed,
                        status: "translating".to_string(),
                        current_paragraph: Some(preview),
                    });
                    (idx, elem, Some(translated))
                }
                Err(e) => {
                    tracing::warn!(idx = idx, error = %e, "Translation failed");
                    let failed =
                        failed_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    let done = done_counter.load(std::sync::atomic::Ordering::Relaxed);
                    progress_cb.lock().await(TranslateProgress {
                        total,
                        done,
                        failed,
                        status: "translating".to_string(),
                        current_paragraph: Some(preview),
                    });
                    (idx, elem, None)
                }
            }
        }
    }))
    .buffer_unordered(concurrency);

    let mut current_html = ensure_translation_style(html);

    while let Some((_idx, elem, translated)) = stream.next().await {
        match translated {
            Some(translated_text) => {
                let block = build_translation_block(&elem.tag, &translated_text, &elem.math_blocks);
                if let Some(insert_pos) = find_element_end(&current_html, &elem.tag, &elem.text) {
                    current_html.insert_str(insert_pos, &block);
                    current_html = add_translation_attr(&current_html, &elem.tag, &elem.text);
                    result.translated += 1;
                    let para = TranslatedParagraph {
                        tag: elem.tag.clone(),
                        original_text_snippet: safe_truncate(&elem.text, 80).to_string(),
                        translation_html: block,
                    };
                    on_inserted(&current_html, &para);
                } else {
                    result.skipped += 1;
                    tracing::debug!(
                        tag = %elem.tag,
                        text_preview = &elem.text[..elem.text.len().min(60)],
                        "Could not locate element in HTML, skipping"
                    );
                }
            }
            None => {
                result.failed += 1;
            }
        }
    }

    {
        on_progress.lock().await(TranslateProgress {
            total,
            done: result.translated,
            failed: result.failed,
            status: if result.failed > 0 {
                "partial".to_string()
            } else {
                "done".to_string()
            },
            current_paragraph: None,
        });
    }

    Ok((current_html, result))
}

/// Translate an HTML file in place.
/// Writes to disk after each paragraph is inserted so the file stays up to date.
/// Calls `on_inserted` after each paragraph is written.
#[allow(clippy::too_many_arguments)]
pub async fn translate_html_file<F, G>(
    path: &std::path::Path,
    ai_config: &AiConfig,
    target_lang: &str,
    html_system_prompt: Option<&str>,
    html_user_prompt: Option<&str>,
    glossary_prompt: Option<&str>,
    concurrency: usize,
    on_progress: F,
    mut on_inserted: G,
) -> Result<TranslateResult, ArxivError>
where
    F: FnMut(TranslateProgress) + Send + 'static,
    G: FnMut(&TranslatedParagraph) + Send + 'static,
{
    let html = tokio::fs::read_to_string(path).await?;
    let path_buf = path.to_path_buf();

    let wrapped = move |current_html: &str, para: &TranslatedParagraph| {
        let _ = std::fs::write(&path_buf, current_html);
        on_inserted(para);
    };

    let (_, result) = translate_html(
        &html,
        ai_config,
        target_lang,
        html_system_prompt,
        html_user_prompt,
        glossary_prompt,
        concurrency,
        on_progress,
        wrapped,
    )
    .await?;

    Ok(result)
}

/// Update the text of a specific translation block identified by its index.
/// `block_index` is the 0-based index among all .zr-translation-block elements.
pub fn update_translation_block(
    html: &str,
    block_index: usize,
    new_text: &str,
) -> Result<String, ArxivError> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse(&format!(".{}", TRANSLATION_CLASS)).unwrap();
    let blocks: Vec<_> = doc.select(&sel).collect();

    if block_index >= blocks.len() {
        return Err(ArxivError::ParseError(format!(
            "Translation block index {} out of range (total {})",
            block_index,
            blocks.len()
        )));
    }

    let block = &blocks[block_index];
    let tag = block.value().name();
    let old_html = block.html();
    let escaped = new_text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let new_block = format!(
        "<{} class=\"{}\" {}=\"true\">{}</{}>",
        tag, TRANSLATION_CLASS, TRANSLATION_ATTR, escaped, tag
    );

    Ok(html.replacen(&old_html, &new_block, 1))
}

/// Count the number of untranslated elements in an HTML string.
pub fn count_untranslated(html: &str) -> usize {
    extract_untranslated_elements(html).len()
}
