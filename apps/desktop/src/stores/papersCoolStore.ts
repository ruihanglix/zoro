// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import * as commands from "@/lib/commands";
import type {
	PapersCoolIndexResponse,
	PapersCoolPageResponse,
} from "@/lib/commands";
import { create } from "zustand";

type BrowseMode = "arxiv" | "venue" | "search";

const BOOKMARKS_KEY = "zoro-papers-cool-bookmarks";

export interface PapersCoolBookmark {
	type: "arxiv" | "venue";
	key: string;
	label: string;
	venueGroup?: string;
}

interface PapersCoolState {
	index: PapersCoolIndexResponse | null;
	indexLoading: boolean;

	mode: BrowseMode;
	selectedCategory: string | null;
	selectedVenue: string | null;
	selectedGroup: string | null;
	searchQuery: string;
	currentDate: string | null;

	page: PapersCoolPageResponse | null;
	loading: boolean;
	error: string | null;

	bookmarks: PapersCoolBookmark[];

	fetchIndex: (force?: boolean) => Promise<void>;
	browseArxiv: (
		category: string,
		date?: string,
		force?: boolean,
	) => Promise<void>;
	browseVenue: (
		venueKey: string,
		group?: string,
		force?: boolean,
	) => Promise<void>;
	search: (query: string, force?: boolean) => Promise<void>;
	setDate: (date: string) => void;
	setMode: (mode: BrowseMode) => void;
	addBookmark: (bookmark: PapersCoolBookmark) => void;
	removeBookmark: (type: string, key: string, venueGroup?: string) => void;
	isBookmarked: (type: string, key: string, venueGroup?: string) => boolean;
}

function todayStr(): string {
	const d = new Date();
	const y = d.getFullYear();
	const m = String(d.getMonth() + 1).padStart(2, "0");
	const day = String(d.getDate()).padStart(2, "0");
	return `${y}-${m}-${day}`;
}

function loadBookmarks(): PapersCoolBookmark[] {
	try {
		const raw = localStorage.getItem(BOOKMARKS_KEY);
		if (raw) return JSON.parse(raw);
	} catch {
		// ignore
	}
	return [];
}

function saveBookmarks(bookmarks: PapersCoolBookmark[]) {
	localStorage.setItem(BOOKMARKS_KEY, JSON.stringify(bookmarks));
}

function bookmarkMatch(
	b: PapersCoolBookmark,
	type: string,
	key: string,
	venueGroup?: string,
): boolean {
	return (
		b.type === type &&
		b.key === key &&
		(b.venueGroup ?? "") === (venueGroup ?? "")
	);
}

export const usePapersCoolStore = create<PapersCoolState>((set, get) => ({
	index: null,
	indexLoading: false,

	mode: "arxiv",
	selectedCategory: null,
	selectedVenue: null,
	selectedGroup: null,
	searchQuery: "",
	currentDate: null,

	page: null,
	loading: false,
	error: null,

	bookmarks: loadBookmarks(),

	fetchIndex: async (force) => {
		if (get().indexLoading) return;
		set({ indexLoading: true });
		try {
			const index = await commands.papersCoolIndex(force);
			set({ index, indexLoading: false });
		} catch (e) {
			set({ indexLoading: false, error: String(e) });
		}
	},

	browseArxiv: async (category, date, force) => {
		const d = date ?? get().currentDate ?? todayStr();
		set({
			loading: true,
			error: null,
			mode: "arxiv",
			selectedCategory: category,
			selectedVenue: null,
			selectedGroup: null,
			currentDate: d,
		});
		try {
			const page = await commands.papersCoolBrowseArxiv(category, d, force);
			set({ page, loading: false });
		} catch (e) {
			set({ loading: false, error: String(e) });
		}
	},

	browseVenue: async (venueKey, group, force) => {
		set({
			loading: true,
			error: null,
			mode: "venue",
			selectedVenue: venueKey,
			selectedGroup: group ?? null,
			selectedCategory: null,
		});
		try {
			const page = await commands.papersCoolBrowseVenue(venueKey, group, force);
			set({ page, loading: false });
		} catch (e) {
			set({ loading: false, error: String(e) });
		}
	},

	search: async (query, force) => {
		if (!query.trim()) return;
		set({
			loading: true,
			error: null,
			mode: "search",
			searchQuery: query,
			selectedCategory: null,
			selectedVenue: null,
			selectedGroup: null,
		});
		try {
			const page = await commands.papersCoolSearch(query, force);
			set({ page, loading: false });
		} catch (e) {
			set({ loading: false, error: String(e) });
		}
	},

	setDate: (date) => {
		set({ currentDate: date });
		const { selectedCategory } = get();
		if (selectedCategory) {
			get().browseArxiv(selectedCategory, date);
		}
	},

	setMode: (mode) => set({ mode }),

	addBookmark: (bookmark) => {
		const bookmarks = [...get().bookmarks, bookmark];
		saveBookmarks(bookmarks);
		set({ bookmarks });
	},

	removeBookmark: (type, key, venueGroup) => {
		const bookmarks = get().bookmarks.filter(
			(b) => !bookmarkMatch(b, type, key, venueGroup),
		);
		saveBookmarks(bookmarks);
		set({ bookmarks });
	},

	isBookmarked: (type, key, venueGroup) => {
		return get().bookmarks.some((b) => bookmarkMatch(b, type, key, venueGroup));
	},
}));
