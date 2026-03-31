// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import * as commands from "@/lib/commands";
import type { NoteResponse } from "@/lib/commands";
import { logger } from "@/lib/logger";
import { create } from "zustand";

export interface CitationClipboard {
	format: "pdf" | "html";
	selectedText: string;
	position: string;
	pageNumber: number;
}

interface NoteState {
	notes: NoteResponse[];
	activeNoteId: string | null;
	loading: boolean;
	saving: boolean;
	citationClipboard: CitationClipboard | null;

	fetchNotes: (paperId: string) => Promise<void>;
	createNote: (paperId: string) => Promise<NoteResponse | null>;
	updateNote: (id: string, content: string) => Promise<void>;
	deleteNote: (id: string, paperId: string) => Promise<void>;
	setActiveNote: (id: string | null) => void;
	setCitationClipboard: (data: CitationClipboard | null) => void;
}

export const useNoteStore = create<NoteState>()((set, get) => ({
	notes: [],
	activeNoteId: null,
	loading: false,
	saving: false,
	citationClipboard: null,

	fetchNotes: async (paperId) => {
		set({ loading: true });
		try {
			const notes = await commands.listNotes(paperId);
			set({ notes, loading: false });
		} catch (e) {
			logger.error("note", "Failed to fetch notes", e);
			set({ loading: false });
		}
	},

	createNote: async (paperId) => {
		try {
			const note = await commands.addNote(paperId, "");
			set((s) => ({ notes: [note, ...s.notes], activeNoteId: note.id }));
			return note;
		} catch (e) {
			logger.error("note", "Failed to create note", e);
			return null;
		}
	},

	updateNote: async (id, content) => {
		set({ saving: true });
		try {
			const updated = await commands.updateNote(id, content);
			set((s) => ({
				notes: s.notes.map((n) => (n.id === id ? updated : n)),
				saving: false,
			}));
		} catch (e) {
			logger.error("note", "Failed to update note", e);
			set({ saving: false });
		}
	},

	deleteNote: async (id, _paperId) => {
		try {
			await commands.deleteNote(id);
			const { activeNoteId } = get();
			set((s) => ({
				notes: s.notes.filter((n) => n.id !== id),
				activeNoteId: activeNoteId === id ? null : activeNoteId,
			}));
		} catch (e) {
			logger.error("note", "Failed to delete note", e);
		}
	},

	setActiveNote: (id) => set({ activeNoteId: id }),

	setCitationClipboard: (data) => set({ citationClipboard: data }),
}));
