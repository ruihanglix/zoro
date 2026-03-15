// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import type { ComponentType } from "react";

// =====================================================
// Plugin Manifest Types (mirror of Rust PluginManifest)
// =====================================================

export interface PluginManifest {
  id: string;
  name: string;
  version: string;
  description: string;
  author?: string;
  icon?: string;
  min_host_version?: string;
  main: string;
  style?: string;
  sidecar?: SidecarConfig;
  permissions: string[];
  contributions: PluginContributions;
}

export interface SidecarConfig {
  command: string;
  args?: string[];
}

export interface PluginContributions {
  reader_sidebar_tabs?: ContributionItem[];
  reader_toolbar_actions?: ContributionItem[];
  reader_overlays?: OverlayContribution[];
  settings_sections?: ContributionItem[];
  sidebar_nav_items?: ContributionItem[];
}

export interface ContributionItem {
  id: string;
  titleKey: string;
  icon: string;
  component: string;
}

export interface OverlayContribution {
  id: string;
  trigger: string;
  component: string;
}

// =====================================================
// Plugin Info (runtime, from backend)
// =====================================================

export interface PluginInfo {
  manifest: PluginManifest;
  mode: "installed" | "dev";
  path: string;
  enabled: boolean;
}

// =====================================================
// Data Types — structured data returned from SDK APIs
// =====================================================

export interface ChatMessage {
  role: "system" | "user" | "assistant";
  content: string;
}

export interface AIOptions {
  model?: string;
  providerId?: string;
  temperature?: number;
  maxTokens?: number;
}

export interface ParagraphInfo {
  index: number;
  text: string;
  element?: HTMLElement;
  page?: number;
}

export interface TextSelectionInfo {
  text: string;
  paragraphIndex: number;
  startOffset: number;
  endOffset: number;
}

export interface PaperData {
  id: string;
  slug: string;
  title: string;
  short_title?: string;
  authors: Array<{ name: string; affiliation?: string }>;
  abstract_text?: string;
  doi?: string;
  arxiv_id?: string;
  url?: string;
  pdf_url?: string;
  html_url?: string;
  published_date?: string;
  added_date: string;
  modified_date: string;
  source?: string;
  read_status: string;
  rating: number | null;
  tags: TagData[];
  has_pdf: boolean;
  has_html: boolean;
  notes: string[];
  entry_type?: string;
  journal?: string;
  volume?: string;
  issue?: string;
  pages?: string;
  publisher?: string;
}

export interface TagData {
  id: string;
  name: string;
  color: string | null;
}

export interface CollectionData {
  id: string;
  name: string;
  slug: string;
  parent_id: string | null;
  paper_count: number;
  description: string | null;
}

export interface NoteData {
  id: string;
  paper_id: string;
  content: string;
  created_date: string;
  modified_date: string;
}

export interface AnnotationData {
  id: string;
  paper_id: string;
  type: string;
  color: string;
  comment?: string;
  selected_text?: string;
  image_data?: string;
  position_json: string;
  page_number: number;
  created_date: string;
  modified_date: string;
}

export interface AnnotationInput {
  paper_id: string;
  type: string;
  color: string;
  comment?: string;
  selected_text?: string;
  position_json: string;
  page_number: number;
}

export interface CitationData {
  text: string;
  style: string;
  cached: boolean;
}

export interface SubscriptionData {
  id: string;
  source_type: string;
  name: string;
  enabled: boolean;
  poll_interval_minutes: number;
  last_polled: string | null;
}

export interface FeedItemData {
  id: string;
  external_id: string;
  title: string;
  authors: Array<{ name: string; affiliation?: string }>;
  abstract_text?: string;
  url?: string;
  pdf_url?: string;
  published_at?: string;
  added_to_library: boolean;
  upvotes?: number;
  ai_summary?: string;
  ai_keywords?: string[];
}

// =====================================================
// Plugin SDK — API exposed to plugins by the host
// =====================================================

export interface ZoroPluginSDK {
  // === Paper data ===
  papers: {
    /** Get the paper currently open in the reader (null if not in reader context). */
    getCurrent(): Promise<PaperData | null>;
    /** Get paper by ID. */
    getById(id: string): Promise<PaperData>;
    /** List papers with optional filtering. */
    list(filter?: {
      readStatus?: string;
      tagName?: string;
      collectionId?: string;
      uncategorized?: boolean;
      sortBy?: string;
      sortOrder?: string;
      limit?: number;
      offset?: number;
    }): Promise<PaperData[]>;
    /** Full-text search across the library. */
    search(query: string, limit?: number): Promise<PaperData[]>;
    /** Update the read status of a paper. */
    updateStatus(id: string, status: "unread" | "reading" | "read"): Promise<void>;
    /** Update the rating of a paper (1-5 or null to clear). */
    updateRating(id: string, rating: number | null): Promise<void>;
    /** Update paper metadata fields. */
    update(id: string, data: Partial<Omit<PaperData, "id" | "slug" | "added_date" | "modified_date" | "tags" | "has_pdf" | "has_html" | "notes">>): Promise<PaperData>;
    /** Delete a paper. */
    delete(id: string): Promise<void>;
    /** Get the local PDF file path for a paper. */
    getPdfPath(id: string): Promise<string>;
    /** Get the local HTML file path for a paper. */
    getHtmlPath(id: string): Promise<string>;
  };

  // === Tags ===
  tags: {
    /** List all tags in the library. */
    list(): Promise<TagData[]>;
    /** Search/autocomplete tags by prefix. */
    search(prefix: string, limit?: number): Promise<TagData[]>;
    /** Add a tag to a paper (creates the tag if it doesn't exist). */
    addToPaper(paperId: string, tagName: string): Promise<void>;
    /** Remove a tag from a paper. */
    removeFromPaper(paperId: string, tagName: string): Promise<void>;
    /** Update a tag's name or color. */
    update(id: string, data: { name?: string; color?: string | null }): Promise<void>;
    /** Delete a tag from the system. */
    delete(id: string): Promise<void>;
  };

  // === Collections ===
  collections: {
    /** List all collections. */
    list(): Promise<CollectionData[]>;
    /** Create a new collection. */
    create(name: string, parentId?: string, description?: string): Promise<CollectionData>;
    /** Update a collection. */
    update(id: string, data: { name?: string; parent_id?: string | null; description?: string | null }): Promise<void>;
    /** Delete a collection. */
    delete(id: string): Promise<void>;
    /** Add a paper to a collection. */
    addPaper(paperId: string, collectionId: string): Promise<void>;
    /** Remove a paper from a collection. */
    removePaper(paperId: string, collectionId: string): Promise<void>;
    /** Get all collections a paper belongs to. */
    getForPaper(paperId: string): Promise<CollectionData[]>;
  };

  // === Notes ===
  notes: {
    /** List all notes for a paper. */
    list(paperId: string): Promise<NoteData[]>;
    /** Add a new note to a paper. */
    add(paperId: string, content: string): Promise<NoteData>;
    /** Update the content of a note. */
    update(id: string, content: string): Promise<NoteData>;
    /** Delete a note. */
    delete(id: string): Promise<void>;
  };

  // === Annotations ===
  annotations: {
    list(paperId: string): Promise<AnnotationData[]>;
    add(input: AnnotationInput): Promise<AnnotationData>;
    update(id: string, data: { color?: string; comment?: string }): Promise<AnnotationData>;
    delete(id: string): Promise<void>;
  };

  // === Citations ===
  citations: {
    /** Get a formatted citation for a paper. */
    format(paperId: string, style: string): Promise<CitationData>;
    /** Get the BibTeX entry for a paper. */
    getBibtex(paperId: string): Promise<string>;
  };

  // === Import / Export ===
  importExport: {
    /** Import papers from BibTeX content string. Returns count of imported papers. */
    importBibtex(content: string): Promise<number>;
    /** Export papers as BibTeX string. Pass paper IDs or omit to export all. */
    exportBibtex(paperIds?: string[]): Promise<string>;
    /** Import papers from RIS content string. Returns count of imported papers. */
    importRis(content: string): Promise<number>;
    /** Export papers as RIS string. */
    exportRis(paperIds?: string[]): Promise<string>;
  };

  // === AI services (reuse host config) ===
  ai: {
    chat(messages: ChatMessage[], options?: AIOptions): Promise<string>;
    chatStream(
      messages: ChatMessage[],
      onChunk: (text: string) => void,
      options?: AIOptions,
    ): Promise<void>;
    translate(text: string, targetLang: string): Promise<string>;
    getModels(): Promise<Array<{ id: string; name: string; models: string[] }>>;
  };

  // === Plugin-scoped persistent storage (KV) ===
  storage: {
    get<T = unknown>(key: string): Promise<T | null>;
    set(key: string, value: unknown): Promise<void>;
    delete(key: string): Promise<void>;
  };

  // === UI utilities ===
  ui: {
    /** Show a toast notification. */
    showToast(message: string, type?: "info" | "success" | "error"): void;
    /** Show a confirmation dialog. Returns true if confirmed. */
    showConfirm(message: string, title?: string): Promise<boolean>;
    /** Show a prompt dialog. Returns the input string or null if cancelled. */
    showPrompt(message: string, defaultValue?: string): Promise<string | null>;
    /** Get the current theme. */
    getTheme(): "light" | "dark";
    /** Listen for theme changes. Returns an unsubscribe function. */
    onThemeChange(cb: (theme: "light" | "dark") => void): () => void;
    /** Translate a key using the host's i18n system. */
    t(key: string, params?: Record<string, string>): string;
    /** Get the current locale code (e.g. "en", "zh-CN"). */
    getLocale(): string;
    /** Open a URL in the system's default browser. */
    openUrl(url: string): Promise<void>;
    /** Copy text to the system clipboard. */
    copyToClipboard(text: string): Promise<void>;
  };

  // === Reader interaction ===
  reader: {
    getParagraphs(): Promise<ParagraphInfo[]>;
    getViewportParagraphs(): ParagraphInfo[];
    scrollToParagraph(index: number): void;
    highlightParagraphs(indices: number[], color?: string): void;
    clearHighlights(): void;
    onParagraphHover(
      cb: (paragraph: ParagraphInfo | null) => void,
    ): () => void;
    onTextSelected(cb: (info: TextSelectionInfo) => void): () => void;
    /** Get the currently selected text in the reader, or null. */
    getSelectedText(): string | null;
  };

  // === Subscriptions (read-only) ===
  subscriptions: {
    /** List all subscription sources. */
    list(): Promise<SubscriptionData[]>;
    /** List feed items for a subscription. */
    getFeedItems(subscriptionId: string, limit?: number, offset?: number): Promise<FeedItemData[]>;
  };

  // === HTTP proxy (subject to permission domain whitelist) ===
  http: {
    /** Perform an HTTP fetch. Domain must be declared in manifest permissions. */
    fetch(url: string, options?: {
      method?: string;
      headers?: Record<string, string>;
      body?: string;
    }): Promise<{ status: number; headers: Record<string, string>; body: string }>;
  };

  // === Event bus ===
  events: {
    /**
     * Subscribe to an event. Returns an unsubscribe function.
     *
     * Built-in events emitted by the host:
     * - "paper:opened"   { paperId: string }
     * - "paper:closed"   { paperId: string }
     * - "paper:added"    { paperId: string }
     * - "paper:deleted"  { paperId: string }
     * - "annotation:added" { annotationId: string, paperId: string }
     * - "annotation:deleted" { annotationId: string, paperId: string }
     * - "tag:changed"    { paperId: string, tagName: string, action: "added" | "removed" }
     * - "collection:changed" { paperId: string, collectionId: string, action: "added" | "removed" }
     * - "theme:changed"  { theme: "light" | "dark" }
     *
     * Plugins can also emit/listen to custom events namespaced by plugin ID.
     */
    on(event: string, handler: (...args: unknown[]) => void): () => void;
    /** Emit an event. Plugins should prefix custom events with their ID. */
    emit(event: string, ...args: unknown[]): void;
  };

  // === Plugin identity ===
  plugin: {
    id: string;
    version: string;
    dataDir: string;
  };
}

// =====================================================
// Plugin Module Interface — what plugins must export
// =====================================================

export interface PluginComponentProps {
  sdk: ZoroPluginSDK;
  context?: Record<string, unknown>;
}

export interface ZoroPlugin {
  activate(sdk: ZoroPluginSDK): void;
  deactivate(): void;
  components: Record<string, ComponentType<PluginComponentProps>>;
}

// =====================================================
// Slot Locations
// =====================================================

export type SlotLocation =
  | "reader_sidebar"
  | "reader_toolbar"
  | "reader_overlay"
  | "settings"
  | "sidebar_nav";

export interface ContributionWithPlugin {
  pluginId: string;
  contribution: ContributionItem | OverlayContribution;
  component: ComponentType<PluginComponentProps>;
}
