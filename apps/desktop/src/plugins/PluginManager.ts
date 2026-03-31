// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import * as commands from "@/lib/commands";
import type { PluginInfoResponse } from "@/lib/commands";
import { logger } from "@/lib/logger";
import { convertFileSrc } from "@tauri-apps/api/core";
import { readFile } from "@tauri-apps/plugin-fs";
import type { LoadedPluginModule, PluginSDKInstance } from "./types";

type ParagraphResult = {
	index: number;
	text: string;
	page?: number;
	element?: HTMLElement;
};

// =====================================================
// Global Event Bus
// =====================================================

type EventHandler = (...args: unknown[]) => void;
const globalEventBus = new Map<string, Set<EventHandler>>();

/** Subscribe to an event on the global bus. */
function busOn(event: string, handler: EventHandler): () => void {
	if (!globalEventBus.has(event)) {
		globalEventBus.set(event, new Set());
	}
	globalEventBus.get(event)!.add(handler);
	return () => {
		globalEventBus.get(event)?.delete(handler);
	};
}

/** Emit an event on the global bus. */
function busEmit(event: string, ...args: unknown[]) {
	const handlers = globalEventBus.get(event);
	if (!handlers) return;
	for (const handler of handlers) {
		try {
			handler(...args);
		} catch (e) {
			logger.error("plugin", `EventBus error in handler for '${event}'`, e);
		}
	}
}

/** Public helper — host code can emit built-in events through this. */
export function emitPluginEvent(event: string, ...args: unknown[]) {
	busEmit(event, ...args);
}

// =====================================================
// Reader Event Hub
// =====================================================

type ParagraphHoverHandler = (paragraph: ParagraphResult | null) => void;
type TextSelectedHandler = (info: {
	text: string;
	paragraphIndex: number;
	startOffset: number;
	endOffset: number;
}) => void;

const paragraphHoverListeners = new Set<ParagraphHoverHandler>();
const textSelectedListeners = new Set<TextSelectedHandler>();

/** Called by host to dispatch paragraph hover events to all plugin listeners. */
export function emitParagraphHover(paragraph: ParagraphResult | null) {
	for (const cb of paragraphHoverListeners) {
		try {
			cb(paragraph);
		} catch (e) {
			logger.error("plugin", "Paragraph hover handler error", e);
		}
	}
}

/** Called by host to dispatch text selection events to all plugin listeners. */
export function emitTextSelected(info: {
	text: string;
	paragraphIndex: number;
	startOffset: number;
	endOffset: number;
}) {
	for (const cb of textSelectedListeners) {
		try {
			cb(info);
		} catch (e) {
			logger.error("plugin", "Text selected handler error", e);
		}
	}
}

// =====================================================
// Paragraph extraction (unchanged logic)
// =====================================================

async function extractParagraphsFromReader(
	paperId: string | undefined,
): Promise<ParagraphResult[]> {
	const paragraphs: ParagraphResult[] = [];

	// --- Strategy 1: PDF textLayer (grouped by page) ---
	const textLayers = document.querySelectorAll(".textLayer");
	if (textLayers.length > 0) {
		let idx = 0;
		for (const layer of textLayers) {
			const pageEl = layer.closest(".page") as HTMLElement | null;
			const pageNum = pageEl
				? Number.parseInt(pageEl.getAttribute("data-page-number") ?? "0", 10)
				: undefined;
			const spans = layer.querySelectorAll("span");
			let currentBlock = "";
			for (const span of spans) {
				const t = span.textContent?.trim() ?? "";
				if (!t) {
					if (currentBlock.length > 0) {
						paragraphs.push({
							index: idx++,
							text: currentBlock,
							page: pageNum,
						});
						currentBlock = "";
					}
					continue;
				}
				currentBlock += (currentBlock ? " " : "") + t;
			}
			if (currentBlock.length > 0) {
				paragraphs.push({ index: idx++, text: currentBlock, page: pageNum });
			}
		}
		if (paragraphs.length > 0) return paragraphs;
	}

	// --- Strategy 2: HTML iframe ---
	const iframe = document.querySelector(
		"iframe[title]",
	) as HTMLIFrameElement | null;
	if (iframe?.contentDocument) {
		const doc = iframe.contentDocument;
		const pElements = doc.querySelectorAll("p, h1, h2, h3, h4, h5, h6, li");
		let idx = 0;
		for (const el of pElements) {
			const text = (el.textContent ?? "").trim();
			if (text.length > 10) {
				paragraphs.push({ index: idx++, text });
			}
		}
		if (paragraphs.length > 0) return paragraphs;
	}

	// --- Strategy 3: Read HTML file from disk ---
	if (paperId) {
		try {
			const htmlPath = await commands.getPaperHtmlPath(paperId);
			const data = await readFile(htmlPath);
			const html = new TextDecoder().decode(data);
			const parser = new DOMParser();
			const doc = parser.parseFromString(html, "text/html");
			const pElements = doc.querySelectorAll("p, h1, h2, h3, h4, h5, h6, li");
			let idx = 0;
			for (const el of pElements) {
				const text = (el.textContent ?? "").trim();
				if (text.length > 10) {
					paragraphs.push({ index: idx++, text });
				}
			}
		} catch (e) {
			logger.warn("plugin", "Failed to read paper HTML", e);
		}
	}

	return paragraphs;
}

// =====================================================
// Plugin Module Loading
// =====================================================

export async function loadPluginModule(
	pluginInfo: PluginInfoResponse,
): Promise<LoadedPluginModule> {
	const mainPath = `${pluginInfo.path}/${pluginInfo.manifest.main}`;
	const assetUrl = convertFileSrc(mainPath);
	const urlWithCacheBust = `${assetUrl}?t=${Date.now()}`;

	const module = await import(/* @vite-ignore */ urlWithCacheBust);
	const plugin = module.default;

	if (!plugin || typeof plugin.activate !== "function") {
		throw new Error(
			`Plugin ${pluginInfo.manifest.id}: invalid module — missing activate()`,
		);
	}

	return {
		pluginId: pluginInfo.manifest.id,
		activate: plugin.activate,
		deactivate: plugin.deactivate ?? (() => {}),
		components: plugin.components ?? {},
	};
}

// =====================================================
// SDK Factory — creates a sandboxed SDK for each plugin
// =====================================================

export function createPluginSDK(
	pluginInfo: PluginInfoResponse,
	readerContext?: { paperId?: string },
): PluginSDKInstance {
	const pluginId = pluginInfo.manifest.id;

	return {
		// ── Papers ────────────────────────────────────────────
		papers: {
			async getCurrent() {
				const paperId = readerContext?.paperId;
				if (!paperId) return null;
				try {
					const paper = await commands.getPaper(paperId);
					return paper as unknown as Record<string, unknown>;
				} catch {
					return null;
				}
			},
			async getById(id: string) {
				const paper = await commands.getPaper(id);
				return paper as unknown as Record<string, unknown>;
			},
			async list(filter?: Record<string, unknown>) {
				const papers = await commands.listPapers({
					collectionId: filter?.collectionId as string | undefined,
					tagName: filter?.tagName as string | undefined,
					readStatus: filter?.readStatus as string | undefined,
					uncategorized: filter?.uncategorized as boolean | undefined,
					sortBy: filter?.sortBy as string | undefined,
					sortOrder: filter?.sortOrder as string | undefined,
					limit: filter?.limit as number | undefined,
					offset: filter?.offset as number | undefined,
				});
				return papers as unknown as Record<string, unknown>[];
			},
			async search(query: string, limit?: number) {
				const results = await commands.searchPapers(query, limit);
				return results as unknown as Record<string, unknown>[];
			},
			async updateStatus(id: string, status: string) {
				await commands.updatePaperStatus(id, status);
				busEmit("paper:statusChanged", { paperId: id, status });
			},
			async updateRating(id: string, rating: number | null) {
				await commands.updatePaperRating(id, rating);
			},
			async update(id: string, data: Record<string, unknown>) {
				const result = await commands.updatePaper(
					id,
					data as Parameters<typeof commands.updatePaper>[1],
				);
				return result as unknown as Record<string, unknown>;
			},
			async delete(id: string) {
				await commands.deletePaper(id);
				busEmit("paper:deleted", { paperId: id });
			},
			async getPdfPath(id: string) {
				return await commands.getPaperPdfPath(id);
			},
			async getHtmlPath(id: string) {
				return await commands.getPaperHtmlPath(id);
			},
		},

		// ── Tags ─────────────────────────────────────────────
		tags: {
			async list() {
				const tags = await commands.listTags();
				return tags as unknown as Record<string, unknown>[];
			},
			async search(prefix: string, limit?: number) {
				const tags = await commands.searchTags(prefix, limit);
				return tags as unknown as Record<string, unknown>[];
			},
			async addToPaper(paperId: string, tagName: string) {
				await commands.addTagToPaper(paperId, tagName);
				busEmit("tag:changed", { paperId, tagName, action: "added" });
			},
			async removeFromPaper(paperId: string, tagName: string) {
				await commands.removeTagFromPaper(paperId, tagName);
				busEmit("tag:changed", { paperId, tagName, action: "removed" });
			},
			async update(id: string, data: { name?: string; color?: string | null }) {
				await commands.updateTag(id, data);
			},
			async delete(id: string) {
				await commands.deleteTag(id);
			},
		},

		// ── Collections ──────────────────────────────────────
		collections: {
			async list() {
				const collections = await commands.listCollections();
				return collections as unknown as Record<string, unknown>[];
			},
			async create(name: string, parentId?: string, description?: string) {
				const collection = await commands.createCollection(
					name,
					parentId,
					description,
				);
				return collection as unknown as Record<string, unknown>;
			},
			async update(id: string, data: Record<string, unknown>) {
				await commands.updateCollection(
					id,
					data as Parameters<typeof commands.updateCollection>[1],
				);
			},
			async delete(id: string) {
				await commands.deleteCollection(id);
			},
			async addPaper(paperId: string, collectionId: string) {
				await commands.addPaperToCollection(paperId, collectionId);
				busEmit("collection:changed", {
					paperId,
					collectionId,
					action: "added",
				});
			},
			async removePaper(paperId: string, collectionId: string) {
				await commands.removePaperFromCollection(paperId, collectionId);
				busEmit("collection:changed", {
					paperId,
					collectionId,
					action: "removed",
				});
			},
			async getForPaper(paperId: string) {
				const collections = await commands.getCollectionsForPaper(paperId);
				return collections as unknown as Record<string, unknown>[];
			},
		},

		// ── Notes ────────────────────────────────────────────
		notes: {
			async list(paperId: string) {
				const notes = await commands.listNotes(paperId);
				return notes as unknown as Record<string, unknown>[];
			},
			async add(paperId: string, content: string) {
				const note = await commands.addNote(paperId, content);
				return note as unknown as Record<string, unknown>;
			},
			async update(id: string, content: string) {
				const note = await commands.updateNote(id, content);
				return note as unknown as Record<string, unknown>;
			},
			async delete(id: string) {
				await commands.deleteNote(id);
			},
		},

		// ── Annotations ──────────────────────────────────────
		annotations: {
			async list(paperId: string) {
				const anns = await commands.listAnnotations(paperId);
				return anns as unknown as Record<string, unknown>[];
			},
			async add(input: Record<string, unknown>) {
				const result = await commands.addAnnotation(
					input.paper_id as string,
					input.type as string,
					input.color as string,
					input.position_json as string,
					input.page_number as number,
					(input.comment as string) ?? null,
					(input.selected_text as string) ?? null,
				);
				busEmit("annotation:added", {
					annotationId: result.id,
					paperId: input.paper_id,
				});
				return result as unknown as Record<string, unknown>;
			},
			async update(id: string, data: Record<string, unknown>) {
				const result = await commands.updateAnnotation(
					id,
					(data.color as string | undefined) ?? null,
					data.comment as string | undefined,
				);
				return result as unknown as Record<string, unknown>;
			},
			async delete(id: string) {
				await commands.deleteAnnotation(id);
				busEmit("annotation:deleted", { annotationId: id });
			},
		},

		// ── Citations ────────────────────────────────────────
		citations: {
			async format(paperId: string, style: string) {
				const result = await commands.getFormattedCitation(paperId, style);
				return {
					text: result.text,
					style: result.source.style,
					cached: result.cached,
				};
			},
			async getBibtex(paperId: string) {
				const result = await commands.getPaperBibtex(paperId);
				return result.text;
			},
		},

		// ── Import / Export ──────────────────────────────────
		importExport: {
			async importBibtex(content: string) {
				return await commands.importBibtex(content);
			},
			async exportBibtex(paperIds?: string[]) {
				return await commands.exportBibtex(paperIds);
			},
			async importRis(content: string) {
				return await commands.importRis(content);
			},
			async exportRis(paperIds?: string[]) {
				return await commands.exportRis(paperIds);
			},
		},

		// ── AI ───────────────────────────────────────────────
		ai: {
			async chat(
				messages: Array<{ role: string; content: string }>,
				options?: Record<string, unknown>,
			) {
				const result = await commands.pluginAiChat({
					messages,
					model: options?.model as string | undefined,
					providerId: options?.providerId as string | undefined,
					temperature: options?.temperature as number | undefined,
					maxTokens: options?.maxTokens as number | undefined,
				});
				return result;
			},
			async chatStream(
				messages: Array<{ role: string; content: string }>,
				onChunk: (text: string) => void,
				options?: Record<string, unknown>,
			) {
				const { listen } = await import("@tauri-apps/api/event");
				const requestId = `${pluginId}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;

				const unlistenChunk = await listen<string>(
					`plugin-ai-stream-${requestId}`,
					(event) => {
						onChunk(event.payload);
					},
				);

				const donePromise = new Promise<string>((resolve, reject) => {
					listen<{ ok: boolean; content?: string; error?: string }>(
						`plugin-ai-done-${requestId}`,
						(event) => {
							unlistenChunk();
							if (event.payload.ok) {
								resolve(event.payload.content ?? "");
							} else {
								reject(new Error(event.payload.error ?? "AI stream failed"));
							}
						},
					);
				});

				await commands.pluginAiChatStream(
					{
						messages,
						model: options?.model as string | undefined,
						providerId: options?.providerId as string | undefined,
						temperature: options?.temperature as number | undefined,
						maxTokens: options?.maxTokens as number | undefined,
					},
					requestId,
				);

				await donePromise;
			},
			async translate(text: string, _targetLang: string) {
				const result = await commands.translateSelection(text);
				return result;
			},
			async getModels() {
				return await commands.pluginAiGetModels();
			},
		},

		// ── Storage ──────────────────────────────────────────
		storage: {
			async get<T = unknown>(key: string): Promise<T | null> {
				const value = await commands.pluginStorageGet(pluginId, key);
				if (value === null) return null;
				try {
					return JSON.parse(value) as T;
				} catch {
					return value as unknown as T;
				}
			},
			async set(key: string, value: unknown) {
				await commands.pluginStorageSet(pluginId, key, JSON.stringify(value));
			},
			async delete(key: string) {
				await commands.pluginStorageDelete(pluginId, key);
			},
		},

		// ── UI ───────────────────────────────────────────────
		ui: {
			showToast(message: string, type?: "info" | "success" | "error") {
				// Dispatch a custom DOM event that the host toast system can pick up
				const detail = { message, type: type ?? "info", pluginId };
				window.dispatchEvent(new CustomEvent("zoro:toast", { detail }));
			logger.info("plugin", `Toast(${type ?? "info"}): ${message}`);
			},
			async showConfirm(message: string, _title?: string) {
				return window.confirm(message);
			},
			async showPrompt(message: string, defaultValue?: string) {
				return window.prompt(message, defaultValue);
			},
			getTheme() {
				return document.documentElement.classList.contains("dark")
					? "dark"
					: "light";
			},
			onThemeChange(cb: (theme: "light" | "dark") => void) {
				const observer = new MutationObserver(() => {
					const isDark = document.documentElement.classList.contains("dark");
					cb(isDark ? "dark" : "light");
				});
				observer.observe(document.documentElement, {
					attributes: true,
					attributeFilter: ["class"],
				});
				return () => observer.disconnect();
			},
			t(key: string, _params?: Record<string, string>) {
				// Plugins provide their own translations; host i18n is not exposed
				return key;
			},
			getLocale() {
				return document.documentElement.lang || navigator.language || "en";
			},
			async openUrl(url: string) {
				const { open } = await import("@tauri-apps/plugin-shell");
				await open(url);
			},
			async copyToClipboard(text: string) {
				const { writeText } = await import(
					"@tauri-apps/plugin-clipboard-manager"
				);
				await writeText(text);
			},
		},

		// ── Reader ───────────────────────────────────────────
		reader: (() => {
			let cachedParagraphs: ParagraphResult[] | null = null;
			const currentPaperId = readerContext?.paperId;

			return {
				async getParagraphs() {
					if (cachedParagraphs && cachedParagraphs.length > 0) {
						return cachedParagraphs;
					}
					const result = await extractParagraphsFromReader(currentPaperId);
					cachedParagraphs = result;
					return result;
				},

				getViewportParagraphs() {
					const results: ParagraphResult[] = [];
					const textLayers = document.querySelectorAll(".textLayer");
					if (textLayers.length > 0) {
						let idx = 0;
						for (const layer of textLayers) {
							const rect = layer.getBoundingClientRect();
							const isVisible =
								rect.bottom > 0 && rect.top < window.innerHeight;
							const pageEl = layer.closest(".page") as HTMLElement | null;
							const pageNum = pageEl
								? Number.parseInt(
										pageEl.getAttribute("data-page-number") ?? "0",
										10,
									)
								: undefined;
							if (isVisible) {
								const text = (layer.textContent ?? "").trim();
								if (text) {
									results.push({ index: idx, text, page: pageNum });
								}
							}
							idx++;
						}
						return results;
					}

					const iframe = document.querySelector(
						"iframe[title]",
					) as HTMLIFrameElement | null;
					if (iframe?.contentDocument) {
						const pElements = iframe.contentDocument.querySelectorAll(
							"p, h1, h2, h3, h4, h5, h6",
						);
						let idx = 0;
						for (const el of pElements) {
							const text = (el.textContent ?? "").trim();
							if (text.length <= 10) continue;
							const rect = el.getBoundingClientRect();
							if (rect.bottom > 0 && rect.top < window.innerHeight) {
								results.push({ index: idx, text });
							}
							idx++;
						}
					}
					return results;
				},

				scrollToParagraph(index: number) {
					const textLayers = document.querySelectorAll(".textLayer");
					if (textLayers.length > 0 && index < textLayers.length) {
						const layer = textLayers[index];
						layer.scrollIntoView({ behavior: "smooth", block: "center" });
						return;
					}
					const iframe = document.querySelector(
						"iframe[title]",
					) as HTMLIFrameElement | null;
					if (iframe?.contentDocument) {
						const pElements = iframe.contentDocument.querySelectorAll(
							"p, h1, h2, h3, h4, h5, h6",
						);
						const validEls = Array.from(pElements).filter(
							(el) => (el.textContent ?? "").trim().length > 10,
						);
						if (index < validEls.length) {
							validEls[index].scrollIntoView({
								behavior: "smooth",
								block: "center",
							});
						}
					}
				},

				highlightParagraphs(indices: number[], color?: string) {
					const highlightColor = color ?? "rgba(254, 240, 138, 0.5)";
					const iframe = document.querySelector(
						"iframe[title]",
					) as HTMLIFrameElement | null;
					if (iframe?.contentDocument) {
						const pElements = iframe.contentDocument.querySelectorAll(
							"p, h1, h2, h3, h4, h5, h6",
						);
						const validEls = Array.from(pElements).filter(
							(el) => (el.textContent ?? "").trim().length > 10,
						);
						for (const i of indices) {
							if (i < validEls.length) {
								(validEls[i] as HTMLElement).style.backgroundColor =
									highlightColor;
								(validEls[i] as HTMLElement).dataset.pluginHighlight = "true";
							}
						}
					}
				},

				clearHighlights() {
					const iframe = document.querySelector(
						"iframe[title]",
					) as HTMLIFrameElement | null;
					if (iframe?.contentDocument) {
						const highlighted = iframe.contentDocument.querySelectorAll(
							'[data-plugin-highlight="true"]',
						);
						for (const el of highlighted) {
							(el as HTMLElement).style.backgroundColor = "";
							delete (el as HTMLElement).dataset.pluginHighlight;
						}
					}
				},

				onParagraphHover(
					cb: (
						paragraph: { index: number; text: string; page?: number } | null,
					) => void,
				) {
					paragraphHoverListeners.add(cb);
					return () => {
						paragraphHoverListeners.delete(cb);
					};
				},

				onTextSelected(
					cb: (info: {
						text: string;
						paragraphIndex: number;
						startOffset: number;
						endOffset: number;
					}) => void,
				) {
					textSelectedListeners.add(cb);
					return () => {
						textSelectedListeners.delete(cb);
					};
				},

				getSelectedText() {
					// Check iframe selection first (HTML reader mode)
					const iframe = document.querySelector(
						"iframe[title]",
					) as HTMLIFrameElement | null;
					if (iframe?.contentDocument) {
						const sel = iframe.contentDocument.getSelection();
						if (sel && sel.toString().trim()) return sel.toString().trim();
					}
					// Fallback to main document selection (PDF mode)
					const sel = document.getSelection();
					if (sel && sel.toString().trim()) return sel.toString().trim();
					return null;
				},
			};
		})() as PluginSDKInstance["reader"],

		// ── Subscriptions ────────────────────────────────────
		subscriptions: {
			async list() {
				const subs = await commands.listSubscriptions();
				return subs as unknown as Record<string, unknown>[];
			},
			async getFeedItems(
				subscriptionId: string,
				limit?: number,
				offset?: number,
			) {
				const items = await commands.listFeedItems(
					subscriptionId,
					limit,
					offset,
				);
				return items as unknown as Record<string, unknown>[];
			},
		},

		// ── HTTP proxy ───────────────────────────────────────
		http: {
			async fetch(
				url: string,
				options?: {
					method?: string;
					headers?: Record<string, string>;
					body?: string;
				},
			) {
				// Use Tauri's fetch (bypasses CORS) — plugins are sandboxed by manifest permissions
				const response = await window.fetch(url, {
					method: options?.method ?? "GET",
					headers: options?.headers,
					body: options?.body,
				});
				const body = await response.text();
				const headers: Record<string, string> = {};
				response.headers.forEach((value, key) => {
					headers[key] = value;
				});
				return { status: response.status, headers, body };
			},
		},

		// ── Events ───────────────────────────────────────────
		events: {
			on(event: string, handler: (...args: unknown[]) => void): () => void {
				return busOn(event, handler);
			},
			emit(event: string, ...args: unknown[]) {
				busEmit(event, ...args);
			},
		},

		// ── Plugin Identity ──────────────────────────────────
		plugin: {
			id: pluginId,
			version: pluginInfo.manifest.version,
			dataDir: pluginInfo.path,
		},
	};
}
