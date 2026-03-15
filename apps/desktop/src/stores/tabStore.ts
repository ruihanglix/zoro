// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import type { FeedItemResponse } from "@/lib/commands";
import { create } from "zustand";

export interface Tab {
	id: string;
	type: "home" | "reader" | "settings" | "note" | "webview" | "agent";
	/** Paper ID for library reader tabs or note tabs */
	paperId?: string;
	/** Feed item for feed reader tabs */
	feedItem?: FeedItemResponse;
	/** Reader mode: pdf or html */
	readerMode?: "pdf" | "html";
	/** Specific PDF filename to open (e.g. "paper.zh.pdf") instead of default paper.pdf */
	pdfFilename?: string;
	/** Bilingual side-by-side PDF mode */
	bilingualMode?: boolean;
	/** Sync scrolling between bilingual panes (default true) */
	bilingualSyncScroll?: boolean;
	/** Translation PDF filename for bilingual mode (e.g. "paper.zh.pdf") */
	bilingualTranslationFile?: string;
	/** URL for webview tabs */
	url?: string;
	/** Display title for the tab */
	title: string;
}

interface TabState {
	tabs: Tab[];
	activeTabId: string;

	openTab: (tab: Omit<Tab, "id"> & { id?: string }) => void;
	closeTab: (id: string) => void;
	setActiveTab: (id: string) => void;
	updateTab: (id: string, partial: Partial<Tab>) => void;
}

const AGENT_TAB: Tab = {
	id: "agent",
	type: "agent",
	title: "Agent",
};

const HOME_TAB: Tab = {
	id: "home",
	type: "home",
	title: "Home",
};

/** Generate a unique tab ID for a reader/note tab. */
function makeTabId(tab: Omit<Tab, "id">): string {
	if (tab.type === "note" && tab.paperId) return `note-${tab.paperId}`;
	if (tab.type === "webview" && tab.url) return `web-${tab.url}`;
	if (tab.paperId && tab.pdfFilename)
		return `paper-${tab.paperId}-${tab.pdfFilename}`;
	if (tab.paperId) return `paper-${tab.paperId}`;
	if (tab.feedItem) return `feed-${tab.feedItem.id}`;
	return `tab-${Date.now()}`;
}

export const useTabStore = create<TabState>((set, get) => ({
	tabs: [AGENT_TAB, HOME_TAB],
	activeTabId: "home",

	openTab: (tabInput) => {
		const id = tabInput.id ?? makeTabId(tabInput);
		const { tabs } = get();

		// If a tab with this ID already exists, just focus it
		const existing = tabs.find((t) => t.id === id);
		if (existing) {
			// Also update readerMode if it changed (e.g., switching PDF -> HTML)
			set({
				activeTabId: id,
				tabs: tabs.map((t) =>
					t.id === id
						? { ...t, readerMode: tabInput.readerMode ?? t.readerMode }
						: t,
				),
			});
			return;
		}

		// Create new tab
		const newTab: Tab = { ...tabInput, id };
		set({
			tabs: [...tabs, newTab],
			activeTabId: id,
		});
	},

	closeTab: (id) => {
		if (id === "home" || id === "agent") return;

		const { tabs, activeTabId } = get();
		const idx = tabs.findIndex((t) => t.id === id);
		if (idx === -1) return;

		const newTabs = tabs.filter((t) => t.id !== id);

		// If closing the active tab, switch to the adjacent tab
		let newActiveId = activeTabId;
		if (activeTabId === id) {
			// Prefer the tab to the right, then left, then home
			if (idx < newTabs.length) {
				newActiveId = newTabs[idx].id;
			} else if (idx > 0) {
				newActiveId = newTabs[idx - 1].id;
			} else {
				newActiveId = "home";
			}
		}

		set({ tabs: newTabs, activeTabId: newActiveId });
	},

	setActiveTab: (id) => {
		set({ activeTabId: id });
	},

	updateTab: (id, partial) => {
		set((s) => ({
			tabs: s.tabs.map((t) => (t.id === id ? { ...t, ...partial } : t)),
		}));
	},
}));
