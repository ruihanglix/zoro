// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import type {
	AiConfigResponse,
	BatchTranslationResponse,
	HtmlTranslationProgress,
	TranslationResponse,
} from "@/lib/commands";
import * as commands from "@/lib/commands";
import { logger } from "@/lib/logger";
import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";

type DisplayMode = "bilingual" | "translated" | "original";

/** Cache key: "paper:id" or "subscription_item:id" */
function cacheKey(entityType: string, entityId: string): string {
	return `${entityType}:${entityId}`;
}

interface HtmlTranslationState {
	translating: boolean;
	progress: HtmlTranslationProgress | null;
}

interface HtmlTranslationCompletePayload {
	paperId: string;
	status: string;
	totalParagraphs?: number;
	translated?: number;
	skipped?: number;
	failed?: number;
	error?: string;
}

interface PdfTranslationState {
	translating: boolean;
	message: string | null;
}

interface BackgroundTaskPayload {
	task_id: string;
	paper_id: string;
	paper_title: string;
	task_type: string;
	status: string;
	message: string | null;
}

interface TranslationState {
	// Translation cache: key -> field translations
	cache: Record<string, TranslationResponse[]>;
	loading: Record<string, boolean>;
	error: Record<string, string | null>;

	// Display preference
	displayMode: DisplayMode;

	// AI config (cached from backend)
	aiConfig: AiConfigResponse | null;
	aiConfigLoading: boolean;

	// HTML translation background tasks: paperId -> state
	htmlTranslations: Record<string, HtmlTranslationState>;
	// Callbacks registered by HtmlReader to reload content on completion
	htmlTranslationCompleteCallbacks: Record<string, () => void>;

	// PDF translation background tasks: paperId -> state
	pdfTranslations: Record<string, PdfTranslationState>;

	// Actions
	fetchTranslations: (
		entityType: string,
		entityId: string,
	) => Promise<TranslationResponse[]>;
	translateFields: (
		entityType: string,
		entityId: string,
		fields: string[],
	) => Promise<TranslationResponse[]>;
	ensureTranslated: (
		entityType: string,
		entityId: string,
		fields: string[],
	) => Promise<void>;
	ensureTranslatedBatch: (
		entityType: string,
		entityIds: string[],
		fields: string[],
	) => Promise<void>;
	fetchTranslationsBatch: (
		entityType: string,
		entityIds: string[],
	) => Promise<void>;
	deleteTranslations: (entityType: string, entityId: string) => Promise<void>;
	setDisplayMode: (mode: DisplayMode) => void;
	fetchAiConfig: () => Promise<void>;
	getTranslatedText: (
		entityType: string,
		entityId: string,
		field: string,
	) => string | null;
	isTranslationLoading: (entityType: string, entityId: string) => boolean;

	// HTML translation background task actions
	startHtmlTranslation: (paperId: string) => Promise<void>;
	isHtmlTranslating: (paperId: string) => boolean;
	getHtmlTranslationProgress: (
		paperId: string,
	) => HtmlTranslationProgress | null;
	registerHtmlTranslationComplete: (
		paperId: string,
		callback: () => void,
	) => void;
	unregisterHtmlTranslationComplete: (paperId: string) => void;
	syncActiveHtmlTranslations: () => Promise<void>;

	// PDF translation state selectors
	isPdfTranslating: (paperId: string) => boolean;
	getPdfTranslationMessage: (paperId: string) => string | null;
	setPdfTranslating: (paperId: string, translating: boolean) => void;
}

export const useTranslationStore = create<TranslationState>((set, get) => ({
	cache: {},
	loading: {},
	error: {},
	displayMode:
		(localStorage.getItem("zoro-display-mode") as DisplayMode) || "bilingual",
	aiConfig: null,
	aiConfigLoading: false,
	htmlTranslations: {},
	htmlTranslationCompleteCallbacks: {},
	pdfTranslations: {},

	fetchTranslations: async (entityType, entityId) => {
		const key = cacheKey(entityType, entityId);
		// If already cached, return immediately
		if (get().cache[key]) {
			return get().cache[key];
		}

		set((s) => ({
			loading: { ...s.loading, [key]: true },
			error: { ...s.error, [key]: null },
		}));

		try {
			const translations = await commands.getTranslations(entityType, entityId);
			set((s) => ({
				cache: { ...s.cache, [key]: translations },
				loading: { ...s.loading, [key]: false },
			}));
			return translations;
		} catch (e) {
			set((s) => ({
				error: { ...s.error, [key]: String(e) },
				loading: { ...s.loading, [key]: false },
			}));
			return [];
		}
	},

	translateFields: async (entityType, entityId, fields) => {
		const key = cacheKey(entityType, entityId);
		set((s) => ({
			loading: { ...s.loading, [key]: true },
			error: { ...s.error, [key]: null },
		}));

		try {
			const translations = await commands.translateFields(
				entityType,
				entityId,
				fields,
			);
			// Merge with existing cache (replace fields that were re-translated)
			const existing = get().cache[key] || [];
			const merged = [...existing];
			for (const t of translations) {
				const idx = merged.findIndex((m) => m.field === t.field);
				if (idx >= 0) {
					merged[idx] = t;
				} else {
					merged.push(t);
				}
			}
			set((s) => ({
				cache: { ...s.cache, [key]: merged },
				loading: { ...s.loading, [key]: false },
			}));
			return translations;
		} catch (e) {
			set((s) => ({
				error: { ...s.error, [key]: String(e) },
				loading: { ...s.loading, [key]: false },
			}));
			return [];
		}
	},

	ensureTranslated: async (entityType, entityId, fields) => {
		const config = get().aiConfig;
		if (!config || !config.nativeLang || !config.apiKeySet) {
			return;
		}
		// Only auto-translate when display mode needs translations
		if (get().displayMode === "original") {
			return;
		}

		const key = cacheKey(entityType, entityId);
		// Skip if already loading
		if (get().loading[key]) return;

		// First try to fetch from cache/DB
		let cached = get().cache[key];
		if (!cached) {
			cached = await get().fetchTranslations(entityType, entityId);
		}

		// Find fields that haven't been translated yet
		const cachedFields = new Set(cached.map((t) => t.field));
		const missing = fields.filter((f) => !cachedFields.has(f));

		if (missing.length > 0) {
			await get().translateFields(entityType, entityId, missing);
		}
	},

	ensureTranslatedBatch: async (entityType, entityIds, fields) => {
		const config = get().aiConfig;
		if (!config || !config.nativeLang || !config.apiKeySet) {
			return;
		}
		if (get().displayMode === "original") {
			return;
		}
		if (entityIds.length === 0) return;

		// First, batch-fetch existing translations from DB
		await get().fetchTranslationsBatch(entityType, entityIds);

		// Find entities that are missing requested fields
		const toTranslate: string[] = [];
		for (const eid of entityIds) {
			const key = cacheKey(entityType, eid);
			const cached = get().cache[key] || [];
			const cachedFields = new Set(cached.map((t) => t.field));
			const missing = fields.filter((f) => !cachedFields.has(f));
			if (missing.length > 0 && !get().loading[key]) {
				toTranslate.push(eid);
			}
		}

		if (toTranslate.length === 0) return;

		// Translate missing fields with concurrency limit to avoid flooding LLM
		const CONCURRENCY = 3;
		for (let i = 0; i < toTranslate.length; i += CONCURRENCY) {
			const batch = toTranslate.slice(i, i + CONCURRENCY);
			await Promise.all(
				batch.map((eid) => {
					const key = cacheKey(entityType, eid);
					const cached = get().cache[key] || [];
					const cachedFields = new Set(cached.map((t) => t.field));
					const missing = fields.filter((f) => !cachedFields.has(f));
					if (missing.length > 0) {
						return get().translateFields(entityType, eid, missing);
					}
					return Promise.resolve([]);
				}),
			);
		}
	},

	fetchTranslationsBatch: async (entityType, entityIds) => {
		if (entityIds.length === 0) return;

		try {
			const results = await commands.getTranslationsBatch(
				entityType,
				entityIds,
			);
			// Group by entityId
			const grouped: Record<string, BatchTranslationResponse[]> = {};
			for (const r of results) {
				if (!grouped[r.entityId]) {
					grouped[r.entityId] = [];
				}
				grouped[r.entityId].push(r);
			}

			// Update cache
			set((s) => {
				const newCache = { ...s.cache };
				for (const [eid, items] of Object.entries(grouped)) {
					const key = cacheKey(entityType, eid);
					const existing = newCache[key] || [];
					const merged = [...existing];
					for (const item of items) {
						const idx = merged.findIndex((m) => m.field === item.field);
						const entry: TranslationResponse = {
							field: item.field,
							originalText: "",
							translatedText: item.translatedText,
							model: null,
							createdDate: "",
						};
						if (idx >= 0) {
							merged[idx] = entry;
						} else {
							merged.push(entry);
						}
					}
					newCache[key] = merged;
				}
				return { cache: newCache };
			});
		} catch (e) {
			logger.error("translation", "Failed to batch fetch translations", e);
		}
	},

	deleteTranslations: async (entityType, entityId) => {
		const key = cacheKey(entityType, entityId);
		try {
			await commands.deleteTranslations(entityType, entityId);
			set((s) => {
				const newCache = { ...s.cache };
				delete newCache[key];
				return { cache: newCache };
			});
		} catch (e) {
			logger.error("translation", "Failed to delete translations", e);
		}
	},

	setDisplayMode: (mode) => {
		localStorage.setItem("zoro-display-mode", mode);
		set({ displayMode: mode });
	},

	fetchAiConfig: async () => {
		set({ aiConfigLoading: true });
		try {
			const config = await commands.getAiConfig();
			set({ aiConfig: config, aiConfigLoading: false });
		} catch (e) {
			logger.error("translation", "Failed to fetch AI config", e);
			set({ aiConfigLoading: false });
		}
	},

	getTranslatedText: (entityType, entityId, field) => {
		const key = cacheKey(entityType, entityId);
		const cached = get().cache[key];
		if (!cached) return null;
		const entry = cached.find((t) => t.field === field);
		return entry?.translatedText ?? null;
	},

	isTranslationLoading: (entityType, entityId) => {
		const key = cacheKey(entityType, entityId);
		return get().loading[key] ?? false;
	},

	startHtmlTranslation: async (paperId) => {
		if (get().htmlTranslations[paperId]?.translating) return;

		set((s) => ({
			htmlTranslations: {
				...s.htmlTranslations,
				[paperId]: { translating: true, progress: null },
			},
		}));

		try {
			await commands.translatePaperHtml(paperId);
		} catch (e) {
			logger.error("translation", "Failed to start HTML translation", e);
			set((s) => {
				const next = { ...s.htmlTranslations };
				delete next[paperId];
				return { htmlTranslations: next };
			});
		}
	},

	isHtmlTranslating: (paperId) => {
		return get().htmlTranslations[paperId]?.translating ?? false;
	},

	getHtmlTranslationProgress: (paperId) => {
		return get().htmlTranslations[paperId]?.progress ?? null;
	},

	registerHtmlTranslationComplete: (paperId, callback) => {
		set((s) => ({
			htmlTranslationCompleteCallbacks: {
				...s.htmlTranslationCompleteCallbacks,
				[paperId]: callback,
			},
		}));
	},

	unregisterHtmlTranslationComplete: (paperId) => {
		set((s) => {
			const next = { ...s.htmlTranslationCompleteCallbacks };
			delete next[paperId];
			return { htmlTranslationCompleteCallbacks: next };
		});
	},

	syncActiveHtmlTranslations: async () => {
		try {
			const active = await commands.getActiveHtmlTranslations();
			const current = get().htmlTranslations;
			const updated: Record<string, HtmlTranslationState> = { ...current };
			for (const pid of active) {
				if (!updated[pid]) {
					updated[pid] = { translating: true, progress: null };
				}
			}
			set({ htmlTranslations: updated });
		} catch (e) {
			logger.error("translation", "Failed to sync active HTML translations", e);
		}
	},

	isPdfTranslating: (paperId) => {
		return get().pdfTranslations[paperId]?.translating ?? false;
	},

	getPdfTranslationMessage: (paperId) => {
		return get().pdfTranslations[paperId]?.message ?? null;
	},

	setPdfTranslating: (paperId, translating) => {
		if (translating) {
			set((s) => ({
				pdfTranslations: {
					...s.pdfTranslations,
					[paperId]: { translating: true, message: null },
				},
			}));
		} else {
			set((s) => {
				const next = { ...s.pdfTranslations };
				delete next[paperId];
				return { pdfTranslations: next };
			});
		}
	},
}));

// Global event listeners for PDF translation progress via background-task events
listen<BackgroundTaskPayload>("background-task", (event) => {
	const p = event.payload;
	if (p.task_type !== "pdf-translation") return;

	if (p.status === "running") {
		useTranslationStore.setState((s) => ({
			pdfTranslations: {
				...s.pdfTranslations,
				[p.paper_id]: { translating: true, message: p.message },
			},
		}));
	} else if (p.status === "completed" || p.status === "failed") {
		useTranslationStore.setState((s) => {
			const next = { ...s.pdfTranslations };
			delete next[p.paper_id];
			return { pdfTranslations: next };
		});
	}
});

// Global event listeners for HTML translation progress & completion
listen<HtmlTranslationProgress>("html-translation-progress", (event) => {
	const p = event.payload;
	const store = useTranslationStore.getState();
	if (store.htmlTranslations[p.paperId]) {
		useTranslationStore.setState((s) => ({
			htmlTranslations: {
				...s.htmlTranslations,
				[p.paperId]: { translating: true, progress: p },
			},
		}));
	}
});

listen<HtmlTranslationCompletePayload>("html-translation-complete", (event) => {
	const p = event.payload;
	useTranslationStore.setState((s) => {
		const next = { ...s.htmlTranslations };
		delete next[p.paperId];
		return { htmlTranslations: next };
	});

	const cb =
		useTranslationStore.getState().htmlTranslationCompleteCallbacks[p.paperId];
	if (cb) cb();
});

// ---------------------------------------------------------------------------
// Reactive selector hooks — these subscribe to the actual cache/loading data
// so components re-render when translations arrive.
// ---------------------------------------------------------------------------

/** Get a single translated field value, reactively. */
export function useTranslatedText(
	entityType: string,
	entityId: string,
	field: string,
): string | null {
	return useTranslationStore((s) => {
		const key = cacheKey(entityType, entityId);
		const cached = s.cache[key];
		if (!cached) return null;
		const entry = cached.find((t) => t.field === field);
		return entry?.translatedText ?? null;
	});
}

/** Check if a translation request is in-flight, reactively. */
export function useTranslationLoading(
	entityType: string,
	entityId: string,
): boolean {
	return useTranslationStore((s) => {
		const key = cacheKey(entityType, entityId);
		return s.loading[key] ?? false;
	});
}
