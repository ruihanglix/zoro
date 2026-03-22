// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { invoke } from "@tauri-apps/api/core";

// Types matching the Rust backend
export interface PaperResponse {
	id: string;
	slug: string;
	title: string;
	short_title: string | null;
	authors: AuthorResponse[];
	abstract_text: string | null;
	doi: string | null;
	arxiv_id: string | null;
	url: string | null;
	pdf_url: string | null;
	html_url: string | null;
	thumbnail_url: string | null;
	published_date: string | null;
	added_date: string;
	modified_date: string;
	source: string | null;
	read_status: string;
	rating: number | null;
	tags: TagResponse[];
	attachments: AttachmentResponse[];
	has_pdf: boolean;
	has_html: boolean;
	notes: string[];
	extra_json: string | null;
	entry_type: string | null;
	journal: string | null;
	volume: string | null;
	issue: string | null;
	pages: string | null;
	publisher: string | null;
	issn: string | null;
	isbn: string | null;
	pdf_downloaded: boolean;
	html_downloaded: boolean;
}

export interface AuthorResponse {
	name: string;
	affiliation: string | null;
}

export interface TagResponse {
	id: string;
	name: string;
	color: string | null;
}

export interface AttachmentResponse {
	id: string;
	filename: string;
	file_type: string;
	file_size: number | null;
	source: string;
	is_local: boolean;
	created_date: string;
}

export interface CollectionResponse {
	id: string;
	name: string;
	slug: string;
	parent_id: string | null;
	paper_count: number;
	description: string | null;
}

export interface SubscriptionResponse {
	id: string;
	source_type: string;
	name: string;
	enabled: boolean;
	poll_interval_minutes: number;
	last_polled: string | null;
}

export interface FeedItemResponse {
	id: string;
	external_id: string;
	title: string;
	authors: { name: string; affiliation: string | null }[];
	abstract_text: string | null;
	url: string | null;
	pdf_url: string | null;
	html_url: string | null;
	upvotes: number | null;
	published_at: string | null;
	fetched_date: string;
	added_to_library: boolean;
	// New metadata fields from HF API
	thumbnail_url: string | null;
	ai_summary: string | null;
	ai_keywords: string[] | null;
	project_page: string | null;
	github_repo: string | null;
	github_stars: number | null;
	num_comments: number | null;
	/** Author-uploaded media (images, videos, gifs) */
	media_urls: string[];
	/** Local path to a cached thumbnail image (if available) */
	cached_thumbnail_path: string | null;
	/** Organization that claimed this paper on HuggingFace */
	organization: {
		name: string | null;
		fullname: string | null;
		avatar: string | null;
	} | null;
}

export interface AddPaperInput {
	title: string;
	short_title?: string;
	authors: { name: string; affiliation?: string }[];
	abstract_text?: string;
	doi?: string;
	arxiv_id?: string;
	url?: string;
	pdf_url?: string;
	html_url?: string;
	published_date?: string;
	source?: string;
	tags?: string[];
	entry_type?: string;
	journal?: string;
	volume?: string;
	issue?: string;
	pages?: string;
	publisher?: string;
	issn?: string;
	isbn?: string;
}

// Library commands
export const addPaper = (input: AddPaperInput) =>
	invoke<PaperResponse>("add_paper", { input });

export const getPaper = (id: string) =>
	invoke<PaperResponse>("get_paper", { id });

export const listPapers = (params?: {
	collectionId?: string;
	tagName?: string;
	readStatus?: string;
	uncategorized?: boolean;
	sortBy?: string;
	sortOrder?: string;
	limit?: number;
	offset?: number;
}) =>
	invoke<PaperResponse[]>("list_papers", {
		collectionId: params?.collectionId ?? null,
		tagName: params?.tagName ?? null,
		readStatus: params?.readStatus ?? null,
		uncategorized: params?.uncategorized ?? null,
		sortBy: params?.sortBy ?? null,
		sortOrder: params?.sortOrder ?? null,
		limit: params?.limit ?? null,
		offset: params?.offset ?? null,
	});

export const deletePaper = (id: string) => invoke<void>("delete_paper", { id });

export const updatePaperStatus = (id: string, readStatus: string) =>
	invoke<void>("update_paper_status", { id, readStatus });

export const updatePaperRating = (id: string, rating: number | null) =>
	invoke<void>("update_paper_rating", { id, rating });

// Search
export const searchPapers = (
	query: string,
	limit?: number,
	wholeWord?: boolean,
) =>
	invoke<PaperResponse[]>("search_papers", {
		query,
		limit: limit ?? null,
		wholeWord: wholeWord ?? null,
	});

// Collections
export const createCollection = (
	name: string,
	parentId?: string,
	description?: string,
) =>
	invoke<CollectionResponse>("create_collection", {
		name,
		parentId: parentId ?? null,
		description: description ?? null,
	});

export const listCollections = () =>
	invoke<CollectionResponse[]>("list_collections");

export const deleteCollection = (id: string) =>
	invoke<void>("delete_collection", { id });

export const addPaperToCollection = (paperId: string, collectionId: string) =>
	invoke<void>("add_paper_to_collection", { paperId, collectionId });

export const removePaperFromCollection = (
	paperId: string,
	collectionId: string,
) => invoke<void>("remove_paper_from_collection", { paperId, collectionId });

// Tags
export const listTags = () => invoke<TagResponse[]>("list_tags");

export const addTagToPaper = (paperId: string, tagName: string) =>
	invoke<void>("add_tag_to_paper", { paperId, tagName });

export const removeTagFromPaper = (paperId: string, tagName: string) =>
	invoke<void>("remove_tag_from_paper", { paperId, tagName });

// Subscriptions
export const listSubscriptions = () =>
	invoke<SubscriptionResponse[]>("list_subscriptions");

export const listFeedItems = (
	subscriptionId: string,
	limit?: number,
	offset?: number,
) =>
	invoke<FeedItemResponse[]>("list_feed_items", {
		subscriptionId,
		limit: limit ?? null,
		offset: offset ?? null,
	});

export const addFeedItemToLibrary = (itemId: string) =>
	invoke<string>("add_feed_item_to_library", { itemId });

export const refreshSubscription = (subscriptionId: string) =>
	invoke<number>("refresh_subscription", { subscriptionId });

export const toggleSubscription = (id: string, enabled: boolean) =>
	invoke<void>("toggle_subscription", { id, enabled });

export const fetchFeedItemsByDate = (
	subscriptionId: string,
	date: string,
	forceRefresh?: boolean,
) =>
	invoke<FeedItemResponse[]>("fetch_feed_items_by_date", {
		subscriptionId,
		date,
		forceRefresh: forceRefresh ?? null,
	});

export const getLatestFeedDate = () =>
	invoke<string | null>("get_latest_feed_date", {});

export interface StorageInfoResponse {
	data_dir: string;
	total_papers: number;
	feed_cache_items: number;
	feed_total_items: number;
	feed_cache_retention_days: number;
}

export const getStorageInfo = () =>
	invoke<StorageInfoResponse>("get_storage_info");

export const clearFeedCache = (subscriptionId?: string) =>
	invoke<number>("clear_feed_cache", {
		subscriptionId: subscriptionId ?? null,
	});

export const changeDataDir = (newPath: string, moveData: boolean) =>
	invoke<void>("change_data_dir", { newPath, moveData });

export interface SubscriptionsConfigResponse {
	feed_cache_retention_days: number;
	poll_interval_minutes: number;
}

export const getSubscriptionsConfig = () =>
	invoke<SubscriptionsConfigResponse>("get_subscriptions_config");

export const updateSubscriptionsConfig = (feedCacheRetentionDays?: number) =>
	invoke<void>("update_subscriptions_config", {
		feedCacheRetentionDays: feedCacheRetentionDays ?? null,
	});

export const fetchRemotePdf = (url: string) =>
	invoke<string>("fetch_remote_pdf", { url });

// Import/Export
export const importBibtex = (content: string) =>
	invoke<number>("import_bibtex", { content });

export const exportBibtex = (paperIds?: string[]) =>
	invoke<string>("export_bibtex", { paperIds: paperIds ?? null });

export const importRis = (content: string) =>
	invoke<number>("import_ris", { content });

export const exportRis = (paperIds?: string[]) =>
	invoke<string>("export_ris", { paperIds: paperIds ?? null });

export const exportAnnotatedPdf = (
	paperId: string,
	sourceFile?: string | null,
) =>
	invoke<void>("export_annotated_pdf", {
		paperId,
		sourceFile: sourceFile ?? null,
	});

export const exportAnnotatedHtml = (paperId: string) =>
	invoke<void>("export_annotated_html", { paperId });

export const exportPdf = (paperId: string, sourceFile?: string | null) =>
	invoke<void>("export_pdf", {
		paperId,
		sourceFile: sourceFile ?? null,
	});

export const exportHtml = (paperId: string) =>
	invoke<void>("export_html", { paperId });

export const showPaperFolder = (paperId: string) =>
	invoke<void>("show_paper_folder", { paperId });

export const showAttachmentInFolder = (paperId: string, filename: string) =>
	invoke<void>("show_attachment_in_folder", { paperId, filename });

// Zotero Import
export interface ZoteroScanResult {
	valid: boolean;
	error: string | null;
	totalItems: number;
	totalCollections: number;
	totalTags: number;
	totalAttachments: number;
	totalNotes: number;
	totalAnnotations: number;
	cloudAttachments: number;
}

export interface ZoteroImportOptions {
	zoteroDir: string;
	importCollections: boolean;
	importNotes: boolean;
	importAttachments: boolean;
	importAnnotations: boolean;
}

export interface ZoteroImportProgress {
	phase: string;
	current: number;
	total: number;
	message: string;
}

export interface ZoteroImportResult {
	papersImported: number;
	papersSkipped: number;
	collectionsImported: number;
	notesImported: number;
	attachmentsCopied: number;
	attachmentsMissing: number;
	annotationsImported: number;
	errors: string[];
}

export const detectZoteroDir = () => invoke<string | null>("detect_zotero_dir");

export const validateZoteroDir = (path: string) =>
	invoke<boolean>("validate_zotero_dir", { path });

export const scanZoteroLibrary = (path: string) =>
	invoke<ZoteroScanResult>("scan_zotero_library", { path });

export const importZoteroLibrary = (options: ZoteroImportOptions) =>
	invoke<ZoteroImportResult>("import_zotero_library", { options });

// Connector
export const getConnectorStatus = () =>
	invoke<{
		enabled: boolean;
		port: number;
		running: boolean;
		zotero_compat_enabled: boolean;
		zotero_compat_port: number;
		zotero_compat_running: boolean;
		zotero_compat_error: string | null;
	}>("get_connector_status");

export interface ConnectorConfigResponse {
	port: number;
	enabled: boolean;
	zotero_compat_enabled: boolean;
	zotero_compat_port: number;
}

export const getConnectorConfig = () =>
	invoke<ConnectorConfigResponse>("get_connector_config");

export const updateConnectorConfig = (input: {
	zotero_compat_enabled?: boolean;
	zotero_compat_port?: number;
}) => invoke<ConnectorConfigResponse>("update_connector_config", { input });

// --- New commands ---

// Update paper metadata
export interface UpdatePaperInput {
	title?: string;
	short_title?: string | null;
	abstract_text?: string | null;
	doi?: string | null;
	arxiv_id?: string | null;
	url?: string | null;
	pdf_url?: string | null;
	html_url?: string | null;
	published_date?: string | null;
	source?: string | null;
	entry_type?: string | null;
	journal?: string | null;
	volume?: string | null;
	issue?: string | null;
	pages?: string | null;
	publisher?: string | null;
	issn?: string | null;
	isbn?: string | null;
}

export const updatePaper = (id: string, input: UpdatePaperInput) =>
	invoke<PaperResponse>("update_paper", { id, input });

export const updatePaperAuthors = (paperId: string, authorNames: string[]) =>
	invoke<PaperResponse>("update_paper_authors", { paperId, authorNames });

// Update collection
export interface UpdateCollectionInput {
	name?: string;
	parent_id?: string | null;
	description?: string | null;
	position?: number;
}

export const updateCollection = (id: string, input: UpdateCollectionInput) =>
	invoke<void>("update_collection", { id, input });

// Get collections for a paper
export const getCollectionsForPaper = (paperId: string) =>
	invoke<CollectionResponse[]>("get_collections_for_paper", { paperId });

// Count uncategorized papers
export const countUncategorizedPapers = () =>
	invoke<number>("count_uncategorized_papers");

// Reorder collections
export const reorderCollections = (items: { id: string; position: number }[]) =>
	invoke<void>("reorder_collections", { items });

// Delete tag
export const deleteTag = (id: string) => invoke<void>("delete_tag", { id });

// Update tag
export interface UpdateTagInput {
	name?: string;
	color?: string | null;
}

export const updateTag = (id: string, input: UpdateTagInput) =>
	invoke<void>("update_tag", { id, input });

// Tag search/autocomplete
export const searchTags = (prefix: string, limit?: number) =>
	invoke<TagResponse[]>("search_tags", { prefix, limit: limit ?? null });

// Debug / Logging
export interface LogEntry {
	id: number;
	timestamp: string;
	level: string;
	target: string;
	message: string;
}

export const getLogs = (sinceId?: number) =>
	invoke<LogEntry[]>("get_logs", { sinceId: sinceId ?? null });

export const setDebugMode = (enabled: boolean) =>
	invoke<void>("set_debug_mode", { enabled });

export const clearLogs = () => invoke<void>("clear_logs");

// Citation
export const enrichPaperMetadata = (paperId: string) =>
	invoke<PaperResponse>("enrich_paper_metadata", { paperId });

export interface MetadataCandidate {
	source: string;
	title: string | null;
	authors: string[] | null;
	year: number | null;
	venue: string | null;
	doi: string | null;
	arxiv_id: string | null;
	abstract_text: string | null;
}

export interface MetadataSearchParams {
	title?: string | null;
	author?: string | null;
	doi?: string | null;
	arxiv_id?: string | null;
	year?: string | null;
	journal?: string | null;
	isbn?: string | null;
}

export const searchMetadataCandidates = (params: MetadataSearchParams) =>
	invoke<MetadataCandidate[]>("search_metadata_candidates", { params });

export const applyMetadataCandidate = (
	paperId: string,
	doi: string | null,
	arxivId: string | null,
) =>
	invoke<PaperResponse>("apply_metadata_candidate", {
		paperId,
		doi,
		arxivId,
	});

export interface CitationSource {
	provider: string;
	doi: string | null;
	request_url: string | null;
	accept_header: string | null;
	style: string;
}

export interface HttpDebugInfo {
	method: string;
	request_url: string;
	request_headers: Record<string, string>;
	status_code: number;
	final_url: string;
	response_headers: Record<string, string>;
	body: string;
}

export interface CitationResponse {
	text: string;
	source: CitationSource;
	cached: boolean;
	fetched_date: string | null;
	http_debug: HttpDebugInfo | null;
}

export const getFormattedCitation = (paperId: string, style: string) =>
	invoke<CitationResponse>("get_formatted_citation", { paperId, style });

export const getPaperBibtex = (paperId: string) =>
	invoke<CitationResponse>("get_paper_bibtex", { paperId });

// Notes
export interface NoteResponse {
	id: string;
	paper_id: string;
	content: string;
	created_date: string;
	modified_date: string;
}

export const addNote = (paperId: string, content: string) =>
	invoke<NoteResponse>("add_note", { paperId, content });

export const listNotes = (paperId: string) =>
	invoke<NoteResponse[]>("list_notes", { paperId });

export const updateNote = (id: string, content: string) =>
	invoke<NoteResponse>("update_note", { id, content });

export const deleteNote = (id: string) => invoke<void>("delete_note", { id });

// Annotations
export interface AnnotationResponse {
	id: string;
	paper_id: string;
	type: "highlight" | "underline" | "area" | "note" | "ink";
	color: string;
	comment: string | null;
	selected_text: string | null;
	image_data: string | null;
	position_json: string;
	page_number: number;
	created_date: string;
	modified_date: string;
}

export const addAnnotation = (
	paperId: string,
	annotationType: string,
	color: string,
	positionJson: string,
	pageNumber: number,
	comment?: string | null,
	selectedText?: string | null,
	imageData?: string | null,
	sourceFile?: string | null,
) =>
	invoke<AnnotationResponse>("add_annotation", {
		paperId,
		annotationType,
		color,
		comment: comment ?? null,
		selectedText: selectedText ?? null,
		imageData: imageData ?? null,
		positionJson,
		pageNumber,
		sourceFile: sourceFile ?? null,
	});

export const listAnnotations = (paperId: string, sourceFile?: string | null) =>
	invoke<AnnotationResponse[]>("list_annotations", {
		paperId,
		sourceFile: sourceFile ?? null,
	});

export const updateAnnotation = (
	id: string,
	color?: string | null,
	comment?: string | null,
) =>
	invoke<AnnotationResponse>("update_annotation", {
		id,
		color: color ?? null,
		comment: comment === undefined ? null : comment,
	});

export const deleteAnnotation = (id: string) =>
	invoke<void>("delete_annotation", { id });

export const updateAnnotationType = (id: string, annotationType: string) =>
	invoke<AnnotationResponse>("update_annotation_type", { id, annotationType });

// Reader State
export interface ReaderStateResponse {
	paper_id: string;
	scroll_position: number | null;
	scale: number | null;
	modified_date: string;
}

export const getReaderState = (paperId: string) =>
	invoke<ReaderStateResponse | null>("get_reader_state", { paperId });

export const saveReaderState = (
	paperId: string,
	scrollPosition?: number | null,
	scale?: number | null,
) =>
	invoke<ReaderStateResponse>("save_reader_state", {
		paperId,
		scrollPosition: scrollPosition ?? null,
		scale: scale ?? null,
	});

// Attachments
export const addAttachmentFiles = (paperId: string, filePaths: string[]) =>
	invoke<PaperResponse>("add_attachment_files", { paperId, filePaths });

// File access
export const getPaperPdfPath = (paperId: string) =>
	invoke<string>("get_paper_pdf_path", { paperId });

export const getPaperHtmlPath = (paperId: string) =>
	invoke<string>("get_paper_html_path", { paperId });

export const getPaperFilePath = (paperId: string, filename: string) =>
	invoke<string>("get_paper_file_path", { paperId, filename });

// Local file import
export interface ImportSkipped {
	path: string;
	reason: string;
}

export interface ImportResult {
	imported: PaperResponse[];
	skipped: ImportSkipped[];
}

export const importLocalFiles = (filePaths: string[]) =>
	invoke<ImportResult>("import_local_files", { filePaths });

// Sync
export interface SyncConfig {
	enabled: boolean;
	url: string;
	username: string;
	password: string;
	remote_path: string;
	device_id: string;
	device_name: string;
	interval_minutes: number;
	// Sync content options
	sync_collections: boolean;
	sync_tags: boolean;
	sync_annotations: boolean;
	sync_reader_state: boolean;
	sync_notes: boolean;
	sync_attachments: boolean;
	max_file_size_mb: number;
	pdf_download_mode: string; // "on_demand" | "full"
	// Conflict strategy
	conflict_strategy: string; // "auto_merge" | "prefer_local" | "prefer_remote"
}

export interface SyncProgress {
	phase: string;
	current: number;
	total: number;
	message: string;
}

export interface DeviceSyncInfo {
	device_id: string;
	device_name: string;
	last_sync_time: string | null;
	last_sequence: number;
}

export interface SyncStatus {
	enabled: boolean;
	syncing: boolean;
	last_sync_time: string | null;
	last_error: string | null;
	progress: SyncProgress | null;
	devices: DeviceSyncInfo[];
}

export const testWebdavConnection = (
	url: string,
	username: string,
	password: string,
) => invoke<string>("test_webdav_connection", { url, username, password });

export const saveSyncConfig = (config: SyncConfig) =>
	invoke<void>("save_sync_config", { config });

export const triggerSync = () => invoke<void>("trigger_sync");

export interface SyncConfigResponse {
	enabled: boolean;
	url: string;
	username: string;
	password_set: boolean;
	remote_path: string;
	interval_minutes: number;
	device_id: string;
	device_name: string;
	sync_collections: boolean;
	sync_tags: boolean;
	sync_annotations: boolean;
	sync_reader_state: boolean;
	sync_notes: boolean;
	sync_attachments: boolean;
	max_file_size_mb: number;
	pdf_download_mode: string;
	conflict_strategy: string;
}

export const getSyncConfig = () =>
	invoke<SyncConfigResponse>("get_sync_config");

export const getSyncStatus = () => invoke<SyncStatus>("get_sync_status");

export const downloadPaperFile = (paperId: string, fileType: string) =>
	invoke<string>("download_paper_file", { paperId, fileType });

export const cancelSync = () => invoke<void>("cancel_sync");

// MCP Server
export interface McpStatusResponse {
	enabled: boolean;
	running: boolean;
	transport: string;
	port: number;
	pid: number | null;
	binary_found: boolean;
}

export interface UpdateMcpConfigInput {
	enabled?: boolean;
	transport?: string;
	port?: number;
}

export const getMcpStatus = () => invoke<McpStatusResponse>("get_mcp_status");

export const updateMcpConfig = (input: UpdateMcpConfigInput) =>
	invoke<McpStatusResponse>("update_mcp_config", { input });

export const startMcpServer = () =>
	invoke<McpStatusResponse>("start_mcp_server");

export const stopMcpServer = () => invoke<McpStatusResponse>("stop_mcp_server");

export const restartMcpServer = () =>
	invoke<McpStatusResponse>("restart_mcp_server");

// Translation / AI
export interface TranslationResponse {
	field: string;
	originalText: string;
	translatedText: string;
	model: string | null;
	createdDate: string;
}

export interface BatchTranslationResponse {
	entityId: string;
	field: string;
	translatedText: string;
}

export interface PdfTranslationConfigResponse {
	enabled: boolean;
	babeldocCommand: string;
	useAiConfig: boolean;
	customApiKeySet: boolean;
	customBaseUrl: string;
	customModel: string;
	qps: number;
	extraArgs: string;
}

export interface AiProviderResponse {
	id: string;
	name: string;
	baseUrl: string;
	apiKeySet: boolean;
	models: string[];
}

export interface AiConfigResponse {
	provider: string;
	baseUrl: string;
	apiKeySet: boolean;
	model: string;
	autoTranslate: boolean;
	nativeLang: string;
	translationPrompts: TranslationPromptsResponse;
	htmlConcurrency: number;
	pdfTranslation: PdfTranslationConfigResponse;
	glossaryEnabled: boolean;
	glossaryThreshold: number;
	providers: AiProviderResponse[];
	taskModelDefaults: TaskModelDefaultsResponse;
}

export interface TaskModelDefaultsResponse {
	quickTranslation: string;
	normalTranslation: string;
	heavyTranslation: string;
	glossaryExtraction: string;
}

export interface TranslationPromptsResponse {
	titleSystem: string;
	titleUser: string;
	abstractSystem: string;
	abstractUser: string;
	htmlSystem: string;
	htmlUser: string;
}

// arXiv HTML
export interface HtmlTranslateResult {
	totalParagraphs: number;
	translated: number;
	skipped: number;
	failed: number;
	error: string | null;
}

export interface HtmlTranslationProgress {
	paperId: string;
	total: number;
	done: number;
	failed: number;
	status: string;
	currentParagraph: string | null;
}

export interface UpdatePdfTranslationInput {
	enabled?: boolean;
	babeldocCommand?: string;
	useAiConfig?: boolean;
	customApiKey?: string;
	customBaseUrl?: string;
	customModel?: string;
	qps?: number;
	extraArgs?: string;
}

export interface UpdateAiConfigInput {
	provider?: string;
	baseUrl?: string;
	apiKey?: string;
	model?: string;
	autoTranslate?: boolean;
	nativeLang?: string;
	htmlConcurrency?: number;
	translationPrompts?: {
		titleSystem?: string;
		titleUser?: string;
		abstractSystem?: string;
		abstractUser?: string;
		htmlSystem?: string;
		htmlUser?: string;
	};
	pdfTranslation?: UpdatePdfTranslationInput;
	glossaryEnabled?: boolean;
	glossaryThreshold?: number;
	providers?: UpdateAiProviderInput[];
	taskModelDefaults?: {
		quickTranslation?: string;
		normalTranslation?: string;
		heavyTranslation?: string;
		glossaryExtraction?: string;
	};
}

export interface UpdateAiProviderInput {
	id: string;
	name: string;
	baseUrl: string;
	apiKey?: string;
	models: string[];
}

export const getTranslations = (entityType: string, entityId: string) =>
	invoke<TranslationResponse[]>("get_translations", { entityType, entityId });

export const getTranslationsBatch = (entityType: string, entityIds: string[]) =>
	invoke<BatchTranslationResponse[]>("get_translations_batch", {
		entityType,
		entityIds,
	});

export const translateFields = (
	entityType: string,
	entityId: string,
	fields: string[],
) =>
	invoke<TranslationResponse[]>("translate_fields", {
		entityType,
		entityId,
		fields,
	});

export const deleteTranslations = (entityType: string, entityId: string) =>
	invoke<number>("delete_translations", { entityType, entityId });

export const getAiConfig = () => invoke<AiConfigResponse>("get_ai_config");

export const updateAiConfig = (input: UpdateAiConfigInput) =>
	invoke<void>("update_ai_config", { input });

export const testAiConnection = () => invoke<string>("test_ai_connection");

export const resetTranslationPrompts = () =>
	invoke<void>("reset_translation_prompts");

export const translateSelection = (text: string) =>
	invoke<string>("translate_selection", { text });

export const translatePdf = (paperId: string, pdfFilename?: string) =>
	invoke<void>("translate_pdf", { paperId, pdfFilename: pdfFilename ?? null });

export const testBabeldoc = () => invoke<string>("test_babeldoc");

// arXiv HTML commands
export const fetchArxivHtml = (paperId: string) =>
	invoke<void>("fetch_arxiv_html", { paperId });

export const cleanPaperHtml = (paperId: string, extraSelectors?: string[]) =>
	invoke<number>("clean_paper_html", {
		paperId,
		extraSelectors: extraSelectors ?? null,
	});

export const fixPaperHtmlStyle = (paperId: string) =>
	invoke<void>("fix_paper_html_style", { paperId });

export const translatePaperHtml = (paperId: string) =>
	invoke<void>("translate_paper_html", { paperId });

// Glossary
export interface GlossaryTermResponse {
	id: string;
	sourceTerm: string;
	translatedTerm: string;
	targetLang: string;
	source: string;
	occurrenceCount: number;
	createdDate: string;
	updatedDate: string;
}

export const getGlossary = () => invoke<GlossaryTermResponse[]>("get_glossary");

export const addGlossaryTerm = (input: {
	sourceTerm: string;
	translatedTerm: string;
}) => invoke<GlossaryTermResponse>("add_glossary_term", { input });

export const updateGlossaryTerm = (input: {
	id: string;
	translatedTerm: string;
}) => invoke<void>("update_glossary_term", { input });

export const promoteGlossaryTerm = (id: string) =>
	invoke<void>("promote_glossary_term", { id });

export const deleteGlossaryTerm = (id: string) =>
	invoke<void>("delete_glossary_term", { id });

export const clearGlossary = () => invoke<number>("clear_glossary");

export const getActiveHtmlTranslations = () =>
	invoke<string[]>("get_active_html_translations");

export const saveHtmlTranslationEdit = (
	paperId: string,
	blockIndex: number,
	newText: string,
) =>
	invoke<void>("save_html_translation_edit", {
		paperId,
		blockIndex,
		newText,
	});

export const countHtmlUntranslated = (paperId: string) =>
	invoke<number>("count_html_untranslated", { paperId });

// Terminal
export const spawnTerminal = (paperId: string) =>
	invoke<string>("spawn_terminal", { paperId });

export const writeTerminal = (terminalId: string, data: string) =>
	invoke<void>("write_terminal", { terminalId, data });

export const resizeTerminal = (
	terminalId: string,
	cols: number,
	rows: number,
) => invoke<void>("resize_terminal", { terminalId, cols, rows });

export const closeTerminal = (terminalId: string) =>
	invoke<void>("close_terminal", { terminalId });

export const getTerminalHistory = (terminalId: string) =>
	invoke<string>("get_terminal_history", { terminalId });

// ── Papers.cool ─────────────────────────────────────────────────────────────

export interface PapersCoolPaperResponse {
	external_id: string;
	title: string;
	authors: string[];
	abstract_text: string | null;
	categories: PapersCoolCategoryResponse[];
	published_date: string | null;
	pdf_url: string | null;
	abs_url: string | null;
	papers_cool_url: string;
	pdf_opens: number;
	kimi_opens: number;
	keywords: string[];
}

export interface PapersCoolCategoryResponse {
	code: string;
	name: string;
}

export interface PapersCoolPageResponse {
	title: string;
	total: number;
	papers: PapersCoolPaperResponse[];
}

export interface ArxivCategoryResponse {
	code: string;
	name: string;
}

export interface ArxivGroupResponse {
	name: string;
	categories: ArxivCategoryResponse[];
}

export interface VenueGroupResponse {
	name: string;
	query: string;
}

export interface VenueEditionResponse {
	key: string;
	year: string;
	groups: VenueGroupResponse[];
}

export interface VenueConferenceResponse {
	name: string;
	editions: VenueEditionResponse[];
}

export interface PapersCoolIndexResponse {
	arxiv_groups: ArxivGroupResponse[];
	venues: VenueConferenceResponse[];
}

export const papersCoolIndex = (forceRefresh?: boolean) =>
	invoke<PapersCoolIndexResponse>("papers_cool_index", {
		forceRefresh: forceRefresh ?? null,
	});

export const papersCoolBrowseArxiv = (
	category: string,
	date?: string,
	forceRefresh?: boolean,
) =>
	invoke<PapersCoolPageResponse>("papers_cool_browse_arxiv", {
		category,
		date: date ?? null,
		forceRefresh: forceRefresh ?? null,
	});

export const papersCoolBrowseVenue = (
	venueKey: string,
	group?: string,
	forceRefresh?: boolean,
) =>
	invoke<PapersCoolPageResponse>("papers_cool_browse_venue", {
		venueKey,
		group: group ?? null,
		forceRefresh: forceRefresh ?? null,
	});

export const papersCoolSearch = (query: string, forceRefresh?: boolean) =>
	invoke<PapersCoolPageResponse>("papers_cool_search", {
		query,
		forceRefresh: forceRefresh ?? null,
	});

// Browser (native webview) commands
export const createBrowserWebview = (
	label: string,
	url: string,
	x: number,
	y: number,
	width: number,
	height: number,
) =>
	invoke<void>("create_browser_webview", { label, url, x, y, width, height });

export const closeBrowserWebview = (label: string) =>
	invoke<void>("close_browser_webview", { label });

export const showBrowserWebview = (label: string) =>
	invoke<void>("show_browser_webview", { label });

export const hideBrowserWebview = (label: string) =>
	invoke<void>("hide_browser_webview", { label });

export const resizeBrowserWebview = (
	label: string,
	x: number,
	y: number,
	width: number,
	height: number,
) => invoke<void>("resize_browser_webview", { label, x, y, width, height });

export const browserNavigate = (label: string, url: string) =>
	invoke<void>("browser_navigate", { label, url });

export const browserGoBack = (label: string) =>
	invoke<void>("browser_go_back", { label });

export const browserGoForward = (label: string) =>
	invoke<void>("browser_go_forward", { label });

export const browserReload = (label: string) =>
	invoke<void>("browser_reload", { label });

export const browserGetUrl = (label: string) =>
	invoke<string>("browser_get_url", { label });

// ── ACP Agent ──────────────────────────────────────────────────────────────

export interface AgentInfoResponse {
	name: string;
	title: string;
	description: string;
	hasSession: boolean;
}

export interface ImageInput {
	base64Data: string;
	mimeType: string;
}

export const acpListAgents = () =>
	invoke<AgentInfoResponse[]>("acp_list_agents");

export const acpStartSession = (agentName: string, cwd?: string) =>
	invoke<string>("acp_start_session", { agentName, cwd: cwd ?? null });

export const acpGetPaperDir = (paperId: string) =>
	invoke<string>("acp_get_paper_dir", { paperId });

export const acpSendPrompt = (
	agentName: string,
	message: string,
	images?: ImageInput[],
) =>
	invoke<void>("acp_send_prompt", {
		agentName,
		message,
		images: images ?? null,
	});

export const acpCancelPrompt = (agentName: string) =>
	invoke<void>("acp_cancel_prompt", { agentName });

export const acpStopSession = (agentName: string) =>
	invoke<void>("acp_stop_session", { agentName });

export interface ConfigOptionValue {
	value: string;
	name: string;
	description: string | null;
}

export interface ConfigOptionInfo {
	id: string;
	name: string;
	description: string | null;
	category: string | null;
	current_value: string;
	options: ConfigOptionValue[];
}

export const acpSetConfigOption = (
	agentName: string,
	configId: string,
	value: string,
) =>
	invoke<ConfigOptionInfo[]>("acp_set_config_option", {
		agentName,
		configId,
		value,
	});

export interface SaveAgentConfigInput {
	name: string;
	title: string;
	command: string;
	args: string[];
}

export const acpSaveAgentConfig = (agents: SaveAgentConfigInput[]) =>
	invoke<void>("acp_save_agent_config", { agents });

// ── ACP Chat Session Persistence ────────────────────────────────────────────

export interface ChatSessionMeta {
	id: string;
	agentName: string;
	title: string;
	messageCount: number;
	createdAt: string;
	updatedAt: string;
	cwd: string | null;
}

export interface ChatSessionFile {
	id: string;
	agentName: string;
	title: string;
	messages: Record<string, unknown>[];
	createdAt: string;
	updatedAt: string;
	cwd?: string | null;
}

export const acpListChatSessions = () =>
	invoke<ChatSessionMeta[]>("acp_list_chat_sessions");

export const acpSaveChatSession = (session: ChatSessionFile) =>
	invoke<void>("acp_save_chat_session", { session });

export const acpLoadChatSession = (sessionId: string) =>
	invoke<ChatSessionFile>("acp_load_chat_session", { sessionId });

export const acpDeleteChatSession = (sessionId: string) =>
	invoke<void>("acp_delete_chat_session", { sessionId });

// ── Built-in Chat ──────────────────────────────────────────────────────────

export interface SystemPromptPreset {
	name: string;
	prompt: string;
}

export interface ProviderInfo {
	id: string;
	name: string;
	models: string[];
}

export interface ChatConfigResponse {
	activePreset: string;
	confirmToolCalls: boolean;
	aiConfigured: boolean;
	defaultModel: string;
	presets: SystemPromptPreset[];
	providers: ProviderInfo[];
}

export interface UpdateChatConfigInput {
	activePreset?: string;
	confirmToolCalls?: boolean;
	presets?: SystemPromptPreset[];
}

export interface ChatHistoryMessage {
	role: string;
	content: string;
}

export interface ChatSendInput {
	messages: ChatHistoryMessage[];
	userMessage: string;
	images?: ImageInput[];
	systemPrompt: string;
	paperId?: string | null;
	model?: string | null;
	providerId?: string | null;
	confirmWrites: boolean;
}

export const chatGetConfig = () =>
	invoke<ChatConfigResponse>("chat_get_config");

export const chatUpdateConfig = (input: UpdateChatConfigInput) =>
	invoke<void>("chat_update_config", { input });

export const chatSendMessage = (input: ChatSendInput) =>
	invoke<void>("chat_send_message", { input });

export const chatConfirmTool = (approved: boolean) =>
	invoke<void>("chat_confirm_tool", { approved });

export const chatCancel = () => invoke<void>("chat_cancel");

// ==================== Plugin Commands ====================

export interface PluginManifestResponse {
	id: string;
	name: string;
	version: string;
	description: string;
	author: string | null;
	icon: string | null;
	min_host_version: string | null;
	main: string;
	style: string | null;
	permissions: string[];
	contributions: {
		reader_sidebar_tabs: ContributionItemResponse[];
		reader_toolbar_actions: ContributionItemResponse[];
		reader_overlays: OverlayContributionResponse[];
		settings_sections: ContributionItemResponse[];
		sidebar_nav_items: ContributionItemResponse[];
	};
}

export interface ContributionItemResponse {
	id: string;
	titleKey: string;
	icon: string;
	component: string;
}

export interface OverlayContributionResponse {
	id: string;
	trigger: string;
	component: string;
}

export interface PluginInfoResponse {
	manifest: PluginManifestResponse;
	mode: "installed" | "dev";
	path: string;
	enabled: boolean;
}

export const listPlugins = () => invoke<PluginInfoResponse[]>("list_plugins");

export const installPluginFromFile = (zcxPath: string) =>
	invoke<PluginInfoResponse>("install_plugin_from_file", { zcxPath });

export const uninstallPlugin = (pluginId: string) =>
	invoke<void>("uninstall_plugin", { pluginId });

export const togglePlugin = (pluginId: string, enabled: boolean) =>
	invoke<void>("toggle_plugin", { pluginId, enabled });

export const loadDevPlugin = (folderPath: string) =>
	invoke<PluginInfoResponse>("load_dev_plugin", { folderPath });

export const unloadDevPlugin = (pluginId: string) =>
	invoke<void>("unload_dev_plugin", { pluginId });

export const reloadDevPlugin = (pluginId: string) =>
	invoke<PluginInfoResponse>("reload_dev_plugin", { pluginId });

export const pluginStorageGet = (pluginId: string, key: string) =>
	invoke<string | null>("plugin_storage_get", { pluginId, key });

export const pluginStorageSet = (
	pluginId: string,
	key: string,
	value: string,
) => invoke<void>("plugin_storage_set", { pluginId, key, value });

export const pluginStorageDelete = (pluginId: string, key: string) =>
	invoke<void>("plugin_storage_delete", { pluginId, key });

// ── Plugin AI (black-box LLM interface for plugins) ──────────────────────────

export interface PluginAiChatInput {
	messages: Array<{ role: string; content: string }>;
	model?: string;
	providerId?: string;
	temperature?: number;
	maxTokens?: number;
}

export interface PluginModelInfo {
	id: string;
	name: string;
	models: string[];
}

export const pluginAiChat = (input: PluginAiChatInput) =>
	invoke<string>("plugin_ai_chat", { input });

export const pluginAiChatStream = (
	input: PluginAiChatInput,
	requestId: string,
) => invoke<void>("plugin_ai_chat_stream", { input, requestId });

export const pluginAiGetModels = () =>
	invoke<PluginModelInfo[]>("plugin_ai_get_models");

// ── HTTP Proxy (bypass browser CORS) ─────────────────────────────────────────

export interface ProxyResponse {
	status: number;
	headers: Record<string, string>;
	body: string;
}

export const httpProxyGet = (
	url: string,
	headers?: Record<string, string>,
) => invoke<ProxyResponse>("http_proxy_get", { url, headers: headers ?? null });
