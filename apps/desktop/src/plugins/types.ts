// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import type {
	ContributionItemResponse,
	OverlayContributionResponse,
} from "@/lib/commands";
import type { ComponentType } from "react";

// =====================================================
// Plugin Component Props
// =====================================================

export interface PluginComponentProps {
	sdk: PluginSDKInstance;
	context?: Record<string, unknown>;
}

// =====================================================
// Loaded Plugin Module (runtime)
// =====================================================

export interface LoadedPluginModule {
	pluginId: string;
	activate: (sdk: PluginSDKInstance) => void;
	deactivate: () => void;
	components: Record<string, ComponentType<PluginComponentProps>>;
}

// =====================================================
// Slot / Contribution helpers
// =====================================================

export type SlotLocation =
	| "reader_sidebar"
	| "reader_toolbar"
	| "reader_overlay"
	| "settings"
	| "sidebar_nav";

export interface ContributionWithPlugin {
	pluginId: string;
	pluginName: string;
	contribution: ContributionItemResponse | OverlayContributionResponse;
	component: ComponentType<PluginComponentProps>;
}

// =====================================================
// SDK Instance (what gets passed to plugins)
// =====================================================

export interface PluginSDKInstance {
	papers: {
		getCurrent(): Promise<Record<string, unknown> | null>;
		getById(id: string): Promise<Record<string, unknown>>;
		list(filter?: Record<string, unknown>): Promise<Record<string, unknown>[]>;
		search(query: string, limit?: number): Promise<Record<string, unknown>[]>;
		updateStatus(id: string, status: string): Promise<void>;
		updateRating(id: string, rating: number | null): Promise<void>;
		update(
			id: string,
			data: Record<string, unknown>,
		): Promise<Record<string, unknown>>;
		delete(id: string): Promise<void>;
		getPdfPath(id: string): Promise<string>;
		getHtmlPath(id: string): Promise<string>;
	};
	tags: {
		list(): Promise<Record<string, unknown>[]>;
		search(prefix: string, limit?: number): Promise<Record<string, unknown>[]>;
		addToPaper(paperId: string, tagName: string): Promise<void>;
		removeFromPaper(paperId: string, tagName: string): Promise<void>;
		update(
			id: string,
			data: { name?: string; color?: string | null },
		): Promise<void>;
		delete(id: string): Promise<void>;
	};
	collections: {
		list(): Promise<Record<string, unknown>[]>;
		create(
			name: string,
			parentId?: string,
			description?: string,
		): Promise<Record<string, unknown>>;
		update(id: string, data: Record<string, unknown>): Promise<void>;
		delete(id: string): Promise<void>;
		addPaper(paperId: string, collectionId: string): Promise<void>;
		removePaper(paperId: string, collectionId: string): Promise<void>;
		getForPaper(paperId: string): Promise<Record<string, unknown>[]>;
	};
	notes: {
		list(paperId: string): Promise<Record<string, unknown>[]>;
		add(paperId: string, content: string): Promise<Record<string, unknown>>;
		update(id: string, content: string): Promise<Record<string, unknown>>;
		delete(id: string): Promise<void>;
	};
	annotations: {
		list(paperId: string): Promise<Record<string, unknown>[]>;
		add(input: Record<string, unknown>): Promise<Record<string, unknown>>;
		update(
			id: string,
			data: Record<string, unknown>,
		): Promise<Record<string, unknown>>;
		delete(id: string): Promise<void>;
	};
	citations: {
		format(
			paperId: string,
			style: string,
		): Promise<{ text: string; style: string; cached: boolean }>;
		getBibtex(paperId: string): Promise<string>;
	};
	importExport: {
		importBibtex(content: string): Promise<number>;
		exportBibtex(paperIds?: string[]): Promise<string>;
		importRis(content: string): Promise<number>;
		exportRis(paperIds?: string[]): Promise<string>;
	};
	ai: {
		chat(
			messages: Array<{ role: string; content: string }>,
			options?: Record<string, unknown>,
		): Promise<string>;
		chatStream(
			messages: Array<{ role: string; content: string }>,
			onChunk: (text: string) => void,
			options?: Record<string, unknown>,
		): Promise<void>;
		translate(text: string, targetLang: string): Promise<string>;
		getModels(): Promise<Array<{ id: string; name: string; models: string[] }>>;
	};
	storage: {
		get<T = unknown>(key: string): Promise<T | null>;
		set(key: string, value: unknown): Promise<void>;
		delete(key: string): Promise<void>;
	};
	ui: {
		showToast(message: string, type?: "info" | "success" | "error"): void;
		showConfirm(message: string, title?: string): Promise<boolean>;
		showPrompt(message: string, defaultValue?: string): Promise<string | null>;
		getTheme(): "light" | "dark";
		onThemeChange(cb: (theme: "light" | "dark") => void): () => void;
		t(key: string, params?: Record<string, string>): string;
		getLocale(): string;
		openUrl(url: string): Promise<void>;
		copyToClipboard(text: string): Promise<void>;
	};
	reader: {
		getParagraphs(): Promise<
			Array<{ index: number; text: string; page?: number }>
		>;
		getViewportParagraphs(): Array<{
			index: number;
			text: string;
			page?: number;
		}>;
		scrollToParagraph(index: number): void;
		highlightParagraphs(indices: number[], color?: string): void;
		clearHighlights(): void;
		onParagraphHover(
			cb: (
				paragraph: { index: number; text: string; page?: number } | null,
			) => void,
		): () => void;
		onTextSelected(
			cb: (info: {
				text: string;
				paragraphIndex: number;
				startOffset: number;
				endOffset: number;
			}) => void,
		): () => void;
		getSelectedText(): string | null;
	};
	subscriptions: {
		list(): Promise<Record<string, unknown>[]>;
		getFeedItems(
			subscriptionId: string,
			limit?: number,
			offset?: number,
		): Promise<Record<string, unknown>[]>;
	};
	http: {
		fetch(
			url: string,
			options?: {
				method?: string;
				headers?: Record<string, string>;
				body?: string;
			},
		): Promise<{
			status: number;
			headers: Record<string, string>;
			body: string;
		}>;
	};
	events: {
		on(event: string, handler: (...args: unknown[]) => void): () => void;
		emit(event: string, ...args: unknown[]): void;
	};
	plugin: {
		id: string;
		version: string;
		dataDir: string;
	};
}
