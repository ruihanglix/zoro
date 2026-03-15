// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import type {
	CollectionResponse,
	FeedItemResponse,
	ImportResult,
	PaperResponse,
	SubscriptionResponse,
	TagResponse,
	UpdatePaperInput,
} from "@/lib/commands";
import * as commands from "@/lib/commands";
import { create } from "zustand";

function loadJsonSetting<T>(key: string, fallback: T): T {
	try {
		const raw = localStorage.getItem(key);
		if (raw === null) return fallback;
		return JSON.parse(raw) as T;
	} catch {
		return fallback;
	}
}

interface LibraryState {
	// Papers
	papers: PaperResponse[];
	selectedPaper: PaperResponse | null;
	selectedPaperIds: Set<string>;
	loading: boolean;
	error: string | null;

	// Collections
	collections: CollectionResponse[];
	uncategorizedCount: number;

	// Tags
	tags: TagResponse[];

	// Subscriptions
	subscriptions: SubscriptionResponse[];
	feedItems: FeedItemResponse[];
	feedLoading: boolean;
	feedDate: string | null; // YYYY-MM-DD, null means "latest from remote"
	latestFeedDate: string | null; // latest available date from HF API

	// Filters
	currentCollectionId: string | null;
	currentTagName: string | null;
	currentUncategorized: boolean;
	searchQuery: string;
	searchWholeWord: boolean;
	sortBy: string;
	sortOrder: string;

	// Actions
	fetchPapers: () => Promise<void>;
	fetchPaper: (id: string) => Promise<void>;
	addPaper: (input: commands.AddPaperInput) => Promise<PaperResponse>;
	deletePaper: (id: string) => Promise<void>;
	updatePaperStatus: (id: string, status: string) => Promise<void>;
	updatePaperRating: (id: string, rating: number | null) => Promise<void>;
	updatePaper: (id: string, input: UpdatePaperInput) => Promise<void>;
	updatePaperAuthors: (id: string, authorNames: string[]) => Promise<void>;
	searchPapers: (query: string, wholeWord?: boolean) => Promise<void>;
	setSearchWholeWord: (wholeWord: boolean) => void;

	// Collections
	fetchCollections: () => Promise<void>;
	createCollection: (
		name: string,
		parentId?: string,
		description?: string,
	) => Promise<void>;
	updateCollection: (
		id: string,
		input: commands.UpdateCollectionInput,
	) => Promise<void>;
	deleteCollection: (id: string) => Promise<void>;
	setCurrentCollection: (id: string | null) => void;
	setCurrentUncategorized: (value: boolean) => void;

	// Tags
	fetchTags: () => Promise<void>;
	addTagToPaper: (paperId: string, tagName: string) => Promise<void>;
	removeTagFromPaper: (paperId: string, tagName: string) => Promise<void>;
	deleteTag: (id: string) => Promise<void>;
	setCurrentTag: (name: string | null) => void;

	// Subscriptions
	fetchSubscriptions: () => Promise<void>;
	fetchFeedItems: (subscriptionId: string) => Promise<void>;
	fetchFeedByDate: (
		subscriptionId: string,
		date: string,
		forceRefresh?: boolean,
	) => Promise<void>;
	refreshSubscription: (subscriptionId: string) => Promise<number>;
	addFeedItemToLibrary: (itemId: string) => Promise<void>;
	setFeedDate: (date: string | null) => void;
	fetchLatestFeedDate: () => Promise<string | null>;

	// Attachments
	addAttachmentFiles: (paperId: string, filePaths: string[]) => Promise<void>;

	// Import/Export
	importBibtex: (content: string) => Promise<number>;
	exportBibtex: (paperIds?: string[]) => Promise<string>;
	importLocalFiles: (filePaths: string[]) => Promise<ImportResult>;
	importing: boolean;

	// Standalone notes
	createStandaloneNote: (
		collectionId?: string,
	) => Promise<PaperResponse | null>;

	// Sort
	setSortBy: (sortBy: string) => void;
	setSortOrder: (order: string) => void;
	setSearchQuery: (query: string) => void;
	setSelectedPaper: (paper: PaperResponse | null) => void;

	// Multi-select
	toggleSelectPaper: (id: string) => void;
	selectAllPapers: () => void;
	clearSelection: () => void;
	selectPaperRange: (id: string) => void;
	lastSelectedId: string | null;
}

export const useLibraryStore = create<LibraryState>((set, get) => ({
	papers: [],
	selectedPaper: null,
	selectedPaperIds: new Set<string>(),
	loading: false,
	error: null,
	collections: [],
	uncategorizedCount: 0,
	tags: [],
	subscriptions: [],
	feedItems: [],
	feedLoading: false,
	feedDate: null,
	latestFeedDate: loadJsonSetting<string | null>("zoro-latest-feed-date", null),
	currentCollectionId: null,
	currentTagName: null,
	currentUncategorized: false,
	searchQuery: "",
	searchWholeWord: false,
	sortBy: loadJsonSetting<string>("zoro-default-sort-by", "added_date"),
	sortOrder: loadJsonSetting<string>("zoro-default-sort-order", "desc"),
	importing: false,

	fetchPapers: async () => {
		set({ loading: true, error: null });
		try {
			const {
				currentCollectionId,
				currentTagName,
				currentUncategorized,
				sortBy,
				sortOrder,
				searchQuery,
				searchWholeWord,
			} = get();
			let papers: PaperResponse[];
			if (searchQuery) {
				papers = await commands.searchPapers(
					searchQuery,
					undefined,
					searchWholeWord,
				);
			} else {
				papers = await commands.listPapers({
					collectionId: currentCollectionId ?? undefined,
					tagName: currentTagName ?? undefined,
					uncategorized: currentUncategorized || undefined,
					sortBy,
					sortOrder,
				});
			}
			set({ papers, loading: false });
		} catch (e) {
			set({ error: String(e), loading: false });
		}
	},

	fetchPaper: async (id: string) => {
		try {
			const paper = await commands.getPaper(id);
			set({ selectedPaper: paper });
		} catch (e) {
			set({ error: String(e) });
		}
	},

	addPaper: async (input) => {
		const paper = await commands.addPaper(input);
		await get().fetchPapers();
		await get().fetchCollections();
		return paper;
	},

	deletePaper: async (id) => {
		await commands.deletePaper(id);
		const { selectedPaper } = get();
		if (selectedPaper?.id === id) {
			set({ selectedPaper: null });
		}
		await get().fetchPapers();
		await get().fetchCollections();
	},

	updatePaperStatus: async (id, status) => {
		await commands.updatePaperStatus(id, status);
		await get().fetchPapers();
		const { selectedPaper } = get();
		if (selectedPaper?.id === id) {
			await get().fetchPaper(id);
		}
	},

	updatePaperRating: async (id, rating) => {
		await commands.updatePaperRating(id, rating);
		const { selectedPaper } = get();
		if (selectedPaper?.id === id) {
			await get().fetchPaper(id);
		}
	},

	updatePaper: async (id, input) => {
		await commands.updatePaper(id, input);
		await get().fetchPapers();
		const { selectedPaper } = get();
		if (selectedPaper?.id === id) {
			await get().fetchPaper(id);
		}
	},

	updatePaperAuthors: async (id, authorNames) => {
		await commands.updatePaperAuthors(id, authorNames);
		await get().fetchPapers();
		const { selectedPaper } = get();
		if (selectedPaper?.id === id) {
			await get().fetchPaper(id);
		}
	},

	setSearchWholeWord: (wholeWord) => {
		set({ searchWholeWord: wholeWord });
		const { searchQuery } = get();
		if (searchQuery) {
			get().searchPapers(searchQuery, wholeWord);
		}
	},

	searchPapers: async (query, wholeWord) => {
		const ww = wholeWord ?? get().searchWholeWord;
		set({ searchQuery: query, loading: true });
		try {
			const papers = query
				? await commands.searchPapers(query, undefined, ww)
				: await commands.listPapers({
						sortBy: get().sortBy,
						sortOrder: get().sortOrder,
					});
			set({ papers, loading: false });
		} catch (e) {
			set({ error: String(e), loading: false });
		}
	},

	fetchCollections: async () => {
		try {
			const [collections, uncategorizedCount] = await Promise.all([
				commands.listCollections(),
				commands.countUncategorizedPapers(),
			]);
			set({ collections, uncategorizedCount });
		} catch (e) {
			set({ error: String(e) });
		}
	},

	createCollection: async (name, parentId, description) => {
		await commands.createCollection(name, parentId, description);
		await get().fetchCollections();
	},

	updateCollection: async (id, input) => {
		await commands.updateCollection(id, input);
		await get().fetchCollections();
	},

	deleteCollection: async (id) => {
		await commands.deleteCollection(id);
		if (get().currentCollectionId === id) {
			set({ currentCollectionId: null });
		}
		await get().fetchCollections();
	},

	setCurrentCollection: (id) => {
		set({
			currentCollectionId: id,
			currentTagName: null,
			currentUncategorized: false,
			searchQuery: "",
		});
		get().fetchPapers();
	},

	setCurrentUncategorized: (value) => {
		set({
			currentUncategorized: value,
			currentCollectionId: null,
			currentTagName: null,
			searchQuery: "",
		});
		get().fetchPapers();
	},

	fetchTags: async () => {
		try {
			const tags = await commands.listTags();
			set({ tags });
		} catch (e) {
			set({ error: String(e) });
		}
	},

	addTagToPaper: async (paperId, tagName) => {
		await commands.addTagToPaper(paperId, tagName);
		await get().fetchTags();
		const { selectedPaper } = get();
		if (selectedPaper?.id === paperId) {
			await get().fetchPaper(paperId);
		}
	},

	removeTagFromPaper: async (paperId, tagName) => {
		await commands.removeTagFromPaper(paperId, tagName);
		await get().fetchTags();
		const { selectedPaper } = get();
		if (selectedPaper?.id === paperId) {
			await get().fetchPaper(paperId);
		}
	},

	deleteTag: async (id) => {
		await commands.deleteTag(id);
		await get().fetchTags();
	},

	setCurrentTag: (name) => {
		set({
			currentTagName: name,
			currentCollectionId: null,
			currentUncategorized: false,
			searchQuery: "",
		});
		get().fetchPapers();
	},

	fetchSubscriptions: async () => {
		try {
			const subscriptions = await commands.listSubscriptions();
			set({ subscriptions });
		} catch (e) {
			set({ error: String(e) });
		}
	},

	fetchFeedItems: async (subscriptionId) => {
		set({ feedLoading: true });
		try {
			const feedItems = await commands.listFeedItems(subscriptionId);
			set({ feedItems, feedLoading: false });
		} catch (e) {
			set({ error: String(e), feedLoading: false });
		}
	},

	fetchFeedByDate: async (subscriptionId, date, forceRefresh) => {
		set({ feedLoading: true, feedDate: date });
		try {
			const feedItems = await commands.fetchFeedItemsByDate(
				subscriptionId,
				date,
				forceRefresh,
			);
			set({ feedItems, feedLoading: false });
		} catch (e) {
			set({ error: String(e), feedLoading: false });
		}
	},

	setFeedDate: (date) => {
		set({ feedDate: date });
	},

	fetchLatestFeedDate: async () => {
		try {
			console.log("[Store] fetchLatestFeedDate: calling API...");
			const date = await commands.getLatestFeedDate();
			console.log("[Store] fetchLatestFeedDate: API returned:", date);
			if (date) {
				set({ latestFeedDate: date });
				localStorage.setItem("zoro-latest-feed-date", JSON.stringify(date));
			}
			return date;
		} catch (e) {
			console.error("[Store] Failed to fetch latest feed date:", e);
			return null;
		}
	},

	refreshSubscription: async (subscriptionId) => {
		const count = await commands.refreshSubscription(subscriptionId);
		// Don't auto-fetch here — the caller (feed.tsx) handles re-fetching
		// with the correct date filter via fetchFeedByDate.
		return count;
	},

	addFeedItemToLibrary: async (itemId) => {
		await commands.addFeedItemToLibrary(itemId);
		await get().fetchPapers();
	},

	addAttachmentFiles: async (paperId, filePaths) => {
		await commands.addAttachmentFiles(paperId, filePaths);
		await get().fetchPapers();
		const { selectedPaper } = get();
		if (selectedPaper?.id === paperId) {
			await get().fetchPaper(paperId);
		}
	},

	importBibtex: async (content) => {
		const count = await commands.importBibtex(content);
		await get().fetchPapers();
		return count;
	},

	exportBibtex: async (paperIds) => {
		return await commands.exportBibtex(paperIds);
	},

	importLocalFiles: async (filePaths) => {
		set({ importing: true });
		try {
			const result = await commands.importLocalFiles(filePaths);
			await get().fetchPapers();
			await get().fetchCollections();
			return result;
		} catch (e) {
			set({ error: String(e) });
			throw e;
		} finally {
			set({ importing: false });
		}
	},

	createStandaloneNote: async (collectionId) => {
		try {
			const paper = await commands.addPaper({
				title: "Untitled Note",
				authors: [],
				entry_type: "note",
			});
			await commands.addNote(paper.id, "");
			if (collectionId) {
				await commands.addPaperToCollection(paper.id, collectionId);
			}
			await get().fetchPapers();
			await get().fetchCollections();
			return paper;
		} catch (e) {
			console.error("Failed to create standalone note:", e);
			return null;
		}
	},

	setSortBy: (sortBy) => {
		set({ sortBy });
		get().fetchPapers();
	},

	setSortOrder: (order) => {
		set({ sortOrder: order });
		get().fetchPapers();
	},

	setSearchQuery: (query) => {
		set({ searchQuery: query });
	},

	setSelectedPaper: (paper) => {
		set({ selectedPaper: paper });
	},

	// Multi-select
	lastSelectedId: null,

	toggleSelectPaper: (id) => {
		set((s) => {
			const next = new Set(s.selectedPaperIds);
			// If no multi-selection yet but a paper is "active" via normal click,
			// promote it into the multi-select set so the user doesn't lose it.
			if (next.size === 0 && s.selectedPaper) {
				next.add(s.selectedPaper.id);
			}
			if (next.has(id)) {
				next.delete(id);
			} else {
				next.add(id);
			}
			return { selectedPaperIds: next, lastSelectedId: id };
		});
	},

	selectAllPapers: () => {
		const { papers } = get();
		set({
			selectedPaperIds: new Set(papers.map((p) => p.id)),
			lastSelectedId: null,
		});
	},

	clearSelection: () => {
		set({ selectedPaperIds: new Set<string>(), lastSelectedId: null });
	},

	selectPaperRange: (id) => {
		const { papers, lastSelectedId, selectedPaper, selectedPaperIds } = get();
		// Use lastSelectedId as the anchor; fall back to the active paper
		// so that normal-click → shift-click works as expected.
		const anchor = lastSelectedId ?? selectedPaper?.id ?? null;
		if (!anchor) {
			// No previous selection anchor — just toggle
			const next = new Set(selectedPaperIds);
			next.add(id);
			set({ selectedPaperIds: next, lastSelectedId: id });
			return;
		}
		const startIdx = papers.findIndex((p) => p.id === anchor);
		const endIdx = papers.findIndex((p) => p.id === id);
		if (startIdx === -1 || endIdx === -1) return;
		const from = Math.min(startIdx, endIdx);
		const to = Math.max(startIdx, endIdx);
		const next = new Set(selectedPaperIds);
		for (let i = from; i <= to; i++) {
			next.add(papers[i].id);
		}
		set({ selectedPaperIds: next, lastSelectedId: id });
	},
}));
