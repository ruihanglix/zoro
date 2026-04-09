// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import type {
	AuthorSearchResultResponse,
	WatchListItemResponse,
	WatchListResponse,
	WatchListResultResponse,
} from "@/lib/commands";
import * as commands from "@/lib/commands";
import { create } from "zustand";

interface WatchListState {
	// Data
	watchLists: WatchListResponse[];
	items: WatchListItemResponse[];
	results: WatchListResultResponse[];
	authorSearchResults: AuthorSearchResultResponse[];

	// Loading states
	loading: boolean;
	resultsLoading: boolean;
	refreshing: boolean;
	searchingAuthors: boolean;
	error: string | null;

	// Actions — Watch Lists
	fetchWatchLists: () => Promise<void>;
	createWatchList: (
		name: string,
		description?: string,
		pollIntervalMinutes?: number,
	) => Promise<WatchListResponse>;
	updateWatchList: (
		id: string,
		input: commands.UpdateWatchListInput,
	) => Promise<void>;
	deleteWatchList: (id: string) => Promise<void>;

	// Actions — Items
	fetchItems: (listId: string) => Promise<void>;
	addItem: (input: commands.AddWatchListItemInput) => Promise<void>;
	deleteItem: (itemId: string, listId: string) => Promise<void>;

	// Actions — Results
	fetchResults: (listId?: string | null) => Promise<void>;
	addResultToLibrary: (resultId: string) => Promise<string>;
	refreshWatchList: (listId: string) => Promise<number>;

	// Actions — Author Search
	searchAuthors: (query: string) => Promise<void>;
	clearAuthorSearch: () => void;
}

export const useWatchListStore = create<WatchListState>((set) => ({
	watchLists: [],
	items: [],
	results: [],
	authorSearchResults: [],
	loading: false,
	resultsLoading: false,
	refreshing: false,
	searchingAuthors: false,
	error: null,

	fetchWatchLists: async () => {
		set({ loading: true, error: null });
		try {
			const watchLists = await commands.listWatchLists();
			set({ watchLists, loading: false });
		} catch (e) {
			set({ error: String(e), loading: false });
		}
	},

	createWatchList: async (name, description, pollIntervalMinutes) => {
		const result = await commands.createWatchList({
			name,
			description: description ?? null,
			pollIntervalMinutes: pollIntervalMinutes ?? null,
		});
		const watchLists = await commands.listWatchLists();
		set({ watchLists });
		return result;
	},

	updateWatchList: async (id, input) => {
		await commands.updateWatchList(id, input);
		const watchLists = await commands.listWatchLists();
		set({ watchLists });
	},

	deleteWatchList: async (id) => {
		await commands.deleteWatchList(id);
		const watchLists = await commands.listWatchLists();
		set({ watchLists });
	},

	fetchItems: async (listId) => {
		try {
			const items = await commands.listWatchListItems(listId);
			set({ items });
		} catch (e) {
			set({ error: String(e) });
		}
	},

	addItem: async (input) => {
		await commands.addWatchListItem(input);
		const items = await commands.listWatchListItems(input.listId);
		set({ items });
		// Refresh watch lists to update item_count
		const watchLists = await commands.listWatchLists();
		set({ watchLists });
	},

	deleteItem: async (itemId, listId) => {
		await commands.deleteWatchListItem(itemId);
		const items = await commands.listWatchListItems(listId);
		set({ items });
		const watchLists = await commands.listWatchLists();
		set({ watchLists });
	},

	fetchResults: async (listId) => {
		set({ resultsLoading: true });
		try {
			const results = await commands.listWatchListResults(listId, 200);
			set({ results, resultsLoading: false });
		} catch (e) {
			set({ error: String(e), resultsLoading: false });
		}
	},

	addResultToLibrary: async (resultId) => {
		const paperId = await commands.addWatchListResultToLibrary(resultId);
		// Re-fetch results to update added_to_library flag
		set((s) => ({
			results: s.results.map((r) =>
				r.id === resultId
					? { ...r, added_to_library: true, paper_id: paperId }
					: r,
			),
		}));
		// Refresh watch lists to update new_result_count
		const watchLists = await commands.listWatchLists();
		set({ watchLists });
		return paperId;
	},

	refreshWatchList: async (listId) => {
		set({ refreshing: true });
		try {
			const newCount = await commands.refreshWatchList(listId);
			// Re-fetch everything
			const watchLists = await commands.listWatchLists();
			const results = await commands.listWatchListResults(listId, 200);
			set({ watchLists, results, refreshing: false });
			return newCount;
		} catch (e) {
			set({ error: String(e), refreshing: false });
			return 0;
		}
	},

	searchAuthors: async (query) => {
		if (!query.trim()) {
			set({ authorSearchResults: [] });
			return;
		}
		set({ searchingAuthors: true });
		try {
			const authorSearchResults =
				await commands.searchAuthorsForWatchList(query);
			set({ authorSearchResults, searchingAuthors: false });
		} catch (e) {
			set({ error: String(e), searchingAuthors: false });
		}
	},

	clearAuthorSearch: () => set({ authorSearchResults: [] }),
}));
