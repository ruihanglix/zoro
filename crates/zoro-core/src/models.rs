// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paper {
    pub id: String, // UUID
    pub slug: String,
    pub title: String,
    pub short_title: Option<String>,
    pub authors: Vec<Author>,
    #[serde(rename = "abstract")]
    pub abstract_text: Option<String>,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    pub url: Option<String>,
    pub pdf_url: Option<String>,
    pub html_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub published_date: Option<String>,
    pub added_date: String,
    pub modified_date: String,
    pub source: Option<String>, // "browser-extension", "subscription", "manual", "import"
    pub tags: Vec<String>,
    pub collections: Vec<String>,
    pub attachments: Vec<AttachmentInfo>,
    pub notes: Vec<String>,
    pub read_status: ReadStatus,
    pub rating: Option<u8>,
    pub extra: serde_json::Value,
    // Citation metadata fields
    pub entry_type: Option<String>, // "article", "inproceedings", "book", etc.
    pub journal: Option<String>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub pages: Option<String>,
    pub publisher: Option<String>,
    pub issn: Option<String>,
    pub isbn: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    pub name: String,
    pub affiliation: Option<String>,
    pub orcid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentInfo {
    pub filename: String,
    #[serde(rename = "type")]
    pub attachment_type: String,
    pub created: String,
}

/// Annotation metadata for sync export/import within metadata.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationMetadata {
    pub id: String,
    #[serde(rename = "type")]
    pub annotation_type: String,
    pub color: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_text: Option<String>,
    pub position_json: String,
    pub page_number: i64,
    pub source_file: String,
    pub created_date: String,
    pub modified_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ReadStatus {
    #[default]
    Unread,
    Reading,
    Read,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub parent_id: Option<String>,
    pub position: i32,
    pub created_date: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String,
    pub paper_id: String,
    pub filename: String,
    pub file_type: String,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
    pub relative_path: String,
    pub created_date: String,
    pub modified_date: String,
    pub source: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: String,
    pub source_type: String,
    pub name: String,
    pub config: Option<serde_json::Value>,
    pub enabled: bool,
    pub poll_interval_minutes: i32,
    pub last_polled: Option<String>,
    pub created_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionItem {
    pub id: String,
    pub subscription_id: String,
    pub paper_id: Option<String>,
    pub external_id: String,
    pub title: String,
    pub authors: Vec<Author>,
    pub abstract_text: Option<String>,
    pub url: Option<String>,
    pub pdf_url: Option<String>,
    pub html_url: Option<String>,
    pub upvotes: Option<i32>,
    pub data: Option<serde_json::Value>,
    pub fetched_date: String,
    pub added_to_library: bool,
}

/// Metadata JSON file that lives alongside each paper directory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperMetadata {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub short_title: Option<String>,
    pub authors: Vec<Author>,
    #[serde(rename = "abstract")]
    pub abstract_text: Option<String>,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    pub url: Option<String>,
    pub pdf_url: Option<String>,
    pub html_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub published_date: Option<String>,
    pub added_date: String,
    pub source: Option<String>,
    pub tags: Vec<String>,
    pub collections: Vec<String>,
    pub attachments: Vec<AttachmentInfo>,
    pub notes: Vec<String>,
    pub read_status: ReadStatus,
    pub rating: Option<u8>,
    pub extra: serde_json::Value,
    // Citation metadata fields
    pub entry_type: Option<String>,
    pub journal: Option<String>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub pages: Option<String>,
    pub publisher: Option<String>,
    pub issn: Option<String>,
    pub isbn: Option<String>,
    /// Annotations (highlights, underlines, notes) — synced in metadata.json.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<AnnotationMetadata>,
}

/// A cached translation of a text field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Translation {
    pub id: String,
    pub entity_type: String, // "paper", "subscription_item", "note"
    pub entity_id: String,
    pub field: String, // "title", "abstract_text"
    pub target_lang: String,
    pub translated_text: String,
    pub model: Option<String>,
    pub created_date: String,
    pub modified_date: String,
}

/// Config for the app (config.toml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub connector: ConnectorConfig,
    pub subscriptions: SubscriptionsConfig,
    pub ai: AiConfig,
    #[serde(default)]
    pub sync: SyncConfig,
    #[serde(default)]
    pub mcp: McpConfig,
    #[serde(default)]
    pub chat: ChatConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub data_dir: String,
    pub language: String,
    /// The user's native language code for translation targets (e.g. "zh", "ja", "ko").
    /// Defaults to the system locale on first launch. Users can override or
    /// disable translation by setting this to empty in the settings UI.
    #[serde(default)]
    pub native_lang: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorConfig {
    pub port: u16,
    pub enabled: bool,
    #[serde(default = "default_zotero_compat_enabled")]
    pub zotero_compat_enabled: bool,
    #[serde(default = "default_zotero_compat_port")]
    pub zotero_compat_port: u16,
}

fn default_zotero_compat_enabled() -> bool {
    true
}

fn default_zotero_compat_port() -> u16 {
    23119
}

fn default_feed_cache_retention_days() -> i32 {
    7
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionsConfig {
    pub poll_interval_minutes: i32,
    #[serde(default = "default_feed_cache_retention_days")]
    pub feed_cache_retention_days: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub provider: String,
    pub api_key: String,
    /// OpenAI-compatible API base URL (e.g. "https://api.openai.com/v1")
    #[serde(default)]
    pub base_url: String,
    /// Model name for translation / chat (e.g. "gpt-4o-mini")
    #[serde(default)]
    pub model: String,
    /// Automatically translate title/abstract when viewing a paper
    #[serde(default)]
    pub auto_translate: bool,
    /// Custom prompts for translation
    #[serde(default)]
    pub translation_prompts: TranslationPrompts,
    /// Max concurrent requests for HTML paragraph translation (default 8)
    #[serde(default = "default_html_concurrency")]
    pub html_concurrency: usize,
    /// PDF translation via external BabelDOC CLI
    #[serde(default)]
    pub pdf_translation: PdfTranslationConfig,
    /// Enable the glossary for consistent term translation
    #[serde(default = "default_true")]
    pub glossary_enabled: bool,
    /// Minimum occurrence count before an auto-extracted term becomes active
    #[serde(default = "default_glossary_threshold")]
    pub glossary_threshold: u32,
    /// Additional AI providers for multi-provider chat support.
    /// The main fields above act as the "default" provider; entries here
    /// let the user switch providers mid-conversation.
    #[serde(default)]
    pub providers: Vec<AiProvider>,
    /// Per-task model overrides for different translation scenarios.
    /// Falls back to the main `model` field when a task model is empty.
    #[serde(default)]
    pub task_model_defaults: TaskModelDefaults,
}

/// An additional OpenAI-compatible AI provider that can be selected per-message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProvider {
    pub id: String,
    pub name: String,
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub models: Vec<String>,
}

impl AiConfig {
    /// Return a clone of this config with base_url / api_key / model resolved
    /// for the given model name. If the model belongs to one of the additional
    /// `providers` (matched via its `models` list), the returned config will
    /// use that provider's base_url and api_key. Otherwise, the config is
    /// returned with only the model field updated.
    ///
    /// Lab models are handled by the local LLM proxy server (`zoro-llm-proxy`)
    /// and appear as a regular provider with `base_url` pointing to
    /// `http://127.0.0.1:{PORT}/v1`.
    pub fn resolve_for_model(&self, model: &str) -> AiConfig {
        let mut cfg = self.clone();
        cfg.model = model.to_string();

        if model.is_empty() {
            return cfg;
        }

        // Special case: ACP Proxy model — resolve to the local proxy server.
        // The port is read from the ACP proxy config file in the data dir,
        // but as a lightweight fallback we use the default port 29171.
        if model == "Zoro-ACP-Proxy" {
            // Check providers first (the virtual provider may have been persisted)
            for p in &self.providers {
                if p.id == "__acp_proxy__" || p.models.contains(&model.to_string()) {
                    if !p.base_url.is_empty() {
                        cfg.base_url = p.base_url.clone();
                    }
                    cfg.api_key = "acp-proxy".to_string();
                    return cfg;
                }
            }
            // Fallback to default port
            cfg.base_url = "http://127.0.0.1:29171/v1".to_string();
            cfg.api_key = "acp-proxy".to_string();
            return cfg;
        }

        // Check if this model belongs to an additional provider
        for p in &self.providers {
            if p.models.contains(&model.to_string()) {
                if !p.base_url.is_empty() {
                    cfg.base_url = p.base_url.clone();
                }
                if !p.api_key.is_empty() {
                    cfg.api_key = p.api_key.clone();
                }
                break;
            }
        }

        cfg
    }
}

fn default_html_concurrency() -> usize {
    8
}

/// Per-task model defaults for different translation / AI scenarios.
/// Each field can be a model name string. When empty, the global default
/// `AiConfig::model` is used as a fallback.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskModelDefaults {
    /// Model for quick, inline translations (e.g. selected-text translation,
    /// subscription feed item translation). Should be fast and cheap.
    #[serde(default)]
    pub quick_translation: String,
    /// Model for standard translations (title, abstract). Can be slightly
    /// slower but higher quality.
    #[serde(default)]
    pub normal_translation: String,
    /// Model for heavy / full-text translations (arXiv HTML, PDF).
    /// Can use a larger, slower model for best quality.
    #[serde(default)]
    pub heavy_translation: String,
    /// Model for glossary term extraction.
    #[serde(default)]
    pub glossary_extraction: String,
}

impl TaskModelDefaults {
    /// Resolve the effective model for a given task, falling back to
    /// the global default model when the task-specific model is empty.
    pub fn resolve(&self, task: &str, global_model: &str) -> String {
        let m = match task {
            "quick" => &self.quick_translation,
            "normal" => &self.normal_translation,
            "heavy" => &self.heavy_translation,
            "glossary" => &self.glossary_extraction,
            _ => &self.normal_translation,
        };
        if m.is_empty() {
            global_model.to_string()
        } else {
            m.clone()
        }
    }
}

/// Configuration for PDF translation via the external BabelDOC CLI tool.
/// BabelDOC is invoked as a separate subprocess to avoid AGPL license coupling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfTranslationConfig {
    #[serde(default)]
    pub enabled: bool,
    /// Path or name of the babeldoc command (default: "babeldoc")
    #[serde(default = "default_babeldoc_command")]
    pub babeldoc_command: String,
    /// When true, reuse the main AI config (api_key, base_url, model)
    #[serde(default = "default_true")]
    pub use_ai_config: bool,
    #[serde(default)]
    pub custom_api_key: String,
    #[serde(default)]
    pub custom_base_url: String,
    #[serde(default)]
    pub custom_model: String,
    /// Queries-per-second limit for the translation API (default 4)
    #[serde(default = "default_qps")]
    pub qps: u32,
    /// Extra command-line arguments appended to the babeldoc invocation.
    /// Users can put any additional flags here (e.g. "--no-dual --skip-clean").
    #[serde(default = "default_extra_args")]
    pub extra_args: String,
}

fn default_babeldoc_command() -> String {
    "babeldoc".to_string()
}

fn default_true() -> bool {
    true
}

fn default_glossary_threshold() -> u32 {
    5
}

fn default_qps() -> u32 {
    4
}

fn default_extra_args() -> String {
    "--no-dual".to_string()
}

impl Default for PdfTranslationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            babeldoc_command: default_babeldoc_command(),
            use_ai_config: true,
            custom_api_key: String::new(),
            custom_base_url: String::new(),
            custom_model: String::new(),
            qps: default_qps(),
            extra_args: default_extra_args(),
        }
    }
}

/// User-customisable prompt templates for LLM translation.
/// `{{text}}` is replaced with the source text, `{{target_lang}}` with the
/// target language name (e.g. "Chinese", "Japanese").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationPrompts {
    pub title_system: String,
    pub title_user: String,
    pub abstract_system: String,
    pub abstract_user: String,
    /// System prompt for HTML paragraph translation (arXiv reader)
    #[serde(default = "default_html_system_prompt")]
    pub html_system: String,
    /// User prompt template for HTML paragraph translation (arXiv reader)
    #[serde(default = "default_html_user_prompt")]
    pub html_user: String,
}

fn default_html_system_prompt() -> String {
    concat!(
        "You are a professional academic translator. ",
        "Translate the following paragraph from an academic paper to {{target_lang}}. ",
        "Maintain academic tone and technical accuracy. ",
        "Keep mathematical notation, citations, and references unchanged. ",
        "Output ONLY the translated text, nothing else.",
    )
    .to_string()
}

fn default_html_user_prompt() -> String {
    "{{text}}".to_string()
}

impl Default for TranslationPrompts {
    fn default() -> Self {
        Self {
            title_system: "You are a professional academic translator. Translate the following paper title to {{target_lang}}. Keep technical terms accurate. Output only the translation, nothing else.".to_string(),
            title_user: "{{text}}".to_string(),
            abstract_system: concat!(
                "You are a professional academic translator. ",
                "Translate the following paper abstract to {{target_lang}}. ",
                "Maintain academic tone and technical accuracy.\n\n",
                "IMPORTANT formatting rules:\n",
                "- Split the translation into short paragraphs of 2–3 sentences each, ",
                "separated by a blank line.\n",
                "- If the original already has multiple paragraphs, split each paragraph ",
                "into 2–3 sentence groups as well.\n",
                "- This improves readability for bilingual side-by-side display.\n\n",
                "Output ONLY the translated text, nothing else. ",
                "Do NOT include the original text. Do NOT add any commentary or labels.",
            ).to_string(),
            abstract_user: "{{text}}".to_string(),
            html_system: default_html_system_prompt(),
            html_user: default_html_user_prompt(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    /// Whether WebDAV sync is enabled
    #[serde(default)]
    pub enabled: bool,
    /// WebDAV server URL (e.g. "https://dav.jianguoyun.com/dav/")
    #[serde(default)]
    pub url: String,
    /// WebDAV username
    #[serde(default)]
    pub username: String,
    /// Encrypted password (stored via platform keychain or AES)
    #[serde(default)]
    pub password: String,
    /// Remote root path on the WebDAV server
    #[serde(default = "default_remote_path")]
    pub remote_path: String,
    /// Sync interval in minutes
    #[serde(default = "default_sync_interval")]
    pub interval_minutes: i32,
    /// Unique device identifier (auto-generated UUID)
    #[serde(default)]
    pub device_id: String,
    /// Human-readable device name
    #[serde(default)]
    pub device_name: String,

    // -- Sync content options --
    /// Sync Collections (folder structure). Default: true
    #[serde(default = "default_true")]
    pub sync_collections: bool,
    /// Sync Tags. Default: true
    #[serde(default = "default_true")]
    pub sync_tags: bool,
    /// Sync Annotations (highlights & notes). Default: true
    #[serde(default = "default_true")]
    pub sync_annotations: bool,
    /// Sync Reader State (scroll position, zoom level). Default: false
    #[serde(default)]
    pub sync_reader_state: bool,
    /// Sync small note files (notes/*.md). Default: true
    #[serde(default = "default_true")]
    pub sync_notes: bool,
    /// Sync attachment files (notes, images, small PDFs, etc.). Default: false
    #[serde(default)]
    pub sync_attachments: bool,
    /// Maximum file size for attachment sync in MB. 0 = unlimited.
    #[serde(default)]
    pub max_file_size_mb: u32,
    /// PDF/HTML download mode: "on_demand" or "full"
    #[serde(default = "default_pdf_download_mode")]
    pub pdf_download_mode: String,

    // -- Conflict resolution --
    /// Conflict strategy: "auto_merge", "prefer_local", "prefer_remote"
    #[serde(default = "default_conflict_strategy")]
    pub conflict_strategy: String,
}

fn default_pdf_download_mode() -> String {
    "on_demand".to_string()
}

fn default_conflict_strategy() -> String {
    "auto_merge".to_string()
}

fn default_remote_path() -> String {
    "/".to_string()
}

fn default_sync_interval() -> i32 {
    5
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            url: String::new(),
            username: String::new(),
            password: String::new(),
            remote_path: default_remote_path(),
            interval_minutes: default_sync_interval(),
            device_id: String::new(),
            device_name: String::new(),
            sync_collections: true,
            sync_tags: true,
            sync_annotations: true,
            sync_reader_state: false,
            sync_notes: true,
            sync_attachments: false,
            max_file_size_mb: 0,
            pdf_download_mode: default_pdf_download_mode(),
            conflict_strategy: default_conflict_strategy(),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                data_dir: "~/.zoro".to_string(),
                language: "en".to_string(),
                native_lang: String::new(),
            },
            connector: ConnectorConfig {
                port: 23120,
                enabled: true,
                zotero_compat_enabled: true,
                zotero_compat_port: 23119,
            },
            subscriptions: SubscriptionsConfig {
                poll_interval_minutes: 60,
                feed_cache_retention_days: 7,
            },
            ai: AiConfig {
                provider: String::new(),
                api_key: String::new(),
                base_url: String::new(),
                model: String::new(),
                auto_translate: false,
                translation_prompts: TranslationPrompts::default(),
                html_concurrency: default_html_concurrency(),
                pdf_translation: PdfTranslationConfig::default(),
                glossary_enabled: true,
                glossary_threshold: default_glossary_threshold(),
                providers: Vec::new(),
                task_model_defaults: TaskModelDefaults::default(),
            },
            sync: SyncConfig::default(),
            mcp: McpConfig::default(),
            chat: ChatConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_mcp_transport")]
    pub transport: String,
    #[serde(default = "default_mcp_port")]
    pub port: u16,
}

fn default_mcp_transport() -> String {
    "http".to_string()
}

fn default_mcp_port() -> u16 {
    23121
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            transport: default_mcp_transport(),
            port: default_mcp_port(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemPromptPreset {
    pub name: String,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatConfig {
    #[serde(default = "default_chat_active_preset")]
    pub active_preset: String,
    #[serde(default = "default_true")]
    pub confirm_tool_calls: bool,
    #[serde(default = "default_chat_presets")]
    pub presets: Vec<SystemPromptPreset>,
}

fn default_chat_active_preset() -> String {
    "Research Assistant".to_string()
}

fn default_chat_presets() -> Vec<SystemPromptPreset> {
    vec![
        SystemPromptPreset {
            name: "Research Assistant".to_string(),
            prompt: concat!(
                "You are a helpful academic research assistant integrated into Zoro, ",
                "a literature management application. You can help users search, organize, ",
                "and understand their paper library. You have access to tools for searching papers, ",
                "reading notes and annotations, managing tags and collections, and more. ",
                "Be concise and precise in your responses. When discussing papers, ",
                "cite titles and authors when possible.",
            )
            .to_string(),
        },
        SystemPromptPreset {
            name: "Paper Reviewer".to_string(),
            prompt: concat!(
                "You are an expert academic peer reviewer. Help the user critically analyze ",
                "papers in their library. Identify strengths, weaknesses, methodological issues, ",
                "and contributions. Be constructive and specific. Reference the paper's content ",
                "when available (abstract, notes, annotations).",
            )
            .to_string(),
        },
        SystemPromptPreset {
            name: "Literature Survey".to_string(),
            prompt: concat!(
                "You are a literature survey assistant. Help the user identify themes, ",
                "connections, and gaps across papers in their library. Synthesize findings, ",
                "compare methodologies, and suggest potential research directions. ",
                "Use the search and listing tools to find relevant papers.",
            )
            .to_string(),
        },
    ]
}

impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            active_preset: default_chat_active_preset(),
            confirm_tool_calls: true,
            presets: default_chat_presets(),
        }
    }
}

// Implement conversion from Paper to PaperMetadata
impl From<&Paper> for PaperMetadata {
    fn from(p: &Paper) -> Self {
        Self {
            id: p.id.clone(),
            slug: p.slug.clone(),
            title: p.title.clone(),
            short_title: p.short_title.clone(),
            authors: p.authors.clone(),
            abstract_text: p.abstract_text.clone(),
            doi: p.doi.clone(),
            arxiv_id: p.arxiv_id.clone(),
            url: p.url.clone(),
            pdf_url: p.pdf_url.clone(),
            html_url: p.html_url.clone(),
            thumbnail_url: p.thumbnail_url.clone(),
            published_date: p.published_date.clone(),
            added_date: p.added_date.clone(),
            source: p.source.clone(),
            tags: p.tags.clone(),
            collections: p.collections.clone(),
            attachments: p.attachments.clone(),
            notes: p.notes.clone(),
            read_status: p.read_status.clone(),
            rating: p.rating,
            extra: p.extra.clone(),
            entry_type: p.entry_type.clone(),
            journal: p.journal.clone(),
            volume: p.volume.clone(),
            issue: p.issue.clone(),
            pages: p.pages.clone(),
            publisher: p.publisher.clone(),
            issn: p.issn.clone(),
            isbn: p.isbn.clone(),
            annotations: Vec::new(), // Annotations are loaded separately, not from Paper struct
        }
    }
}
