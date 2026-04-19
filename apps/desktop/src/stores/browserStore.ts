// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { loadSetting, saveSetting } from "@/stores/uiStore";
import { create } from "zustand";

export interface Bookmark {
	id: string;
	name: string;
	url: string;
	icon?: string;
	isPreset?: boolean;
}

const DEFAULT_BOOKMARKS: Bookmark[] = [
	{
		id: "preset-gemini",
		name: "Gemini",
		url: "https://gemini.google.com",
		icon: "✦",
		isPreset: true,
	},
	{
		id: "preset-chatgpt",
		name: "ChatGPT",
		url: "https://chatgpt.com",
		icon: "◉",
		isPreset: true,
	},
	{
		id: "preset-claude",
		name: "Claude",
		url: "https://claude.ai",
		icon: "◈",
		isPreset: true,
	},
	{
		id: "preset-kimi",
		name: "Kimi",
		url: "https://kimi.moonshot.cn",
		icon: "☾",
		isPreset: true,
	},
	{
		id: "preset-deepseek",
		name: "DeepSeek",
		url: "https://chat.deepseek.com",
		icon: "◇",
		isPreset: true,
	},
];

const BOOKMARKS_KEY = "zoro-browser-bookmarks";

interface BrowserStoreState {
	bookmarks: Bookmark[];
	addBookmark: (name: string, url: string, icon?: string) => void;
	removeBookmark: (id: string) => void;
	updateBookmark: (id: string, updates: Partial<Bookmark>) => void;
	resetToDefaults: () => void;
}

export const useBrowserStore = create<BrowserStoreState>((set) => ({
	bookmarks: loadSetting<Bookmark[]>(BOOKMARKS_KEY, DEFAULT_BOOKMARKS),

	addBookmark: (name, url, icon) => {
		set((state) => {
			const newBookmark: Bookmark = {
				id: `bm-${Date.now()}`,
				name,
				url,
				icon,
			};
			const bookmarks = [...state.bookmarks, newBookmark];
			saveSetting(BOOKMARKS_KEY, bookmarks);
			return { bookmarks };
		});
	},

	removeBookmark: (id) => {
		set((state) => {
			const bookmarks = state.bookmarks.filter((b) => b.id !== id);
			saveSetting(BOOKMARKS_KEY, bookmarks);
			return { bookmarks };
		});
	},

	updateBookmark: (id, updates) => {
		set((state) => {
			const bookmarks = state.bookmarks.map((b) =>
				b.id === id ? { ...b, ...updates } : b,
			);
			saveSetting(BOOKMARKS_KEY, bookmarks);
			return { bookmarks };
		});
	},

	resetToDefaults: () => {
		saveSetting(BOOKMARKS_KEY, DEFAULT_BOOKMARKS);
		set({ bookmarks: [...DEFAULT_BOOKMARKS] });
	},
}));
