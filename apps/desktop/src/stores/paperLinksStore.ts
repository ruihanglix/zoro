// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import * as commands from "@/lib/commands";
import type { PaperLinkResponse } from "@/lib/commands";
import { logger } from "@/lib/logger";
import { create } from "zustand";

interface PaperLinksState {
	links: PaperLinkResponse[];
	loading: boolean;

	fetchLinks: (paperId: string) => Promise<void>;
	addLink: (
		paperId: string,
		url: string,
		title?: string | null,
		favicon?: string | null,
	) => Promise<PaperLinkResponse | null>;
	removeLink: (id: string) => Promise<void>;
	updateLink: (
		id: string,
		url?: string | null,
		title?: string | null,
		favicon?: string | null,
	) => Promise<void>;
}

export const usePaperLinksStore = create<PaperLinksState>()((set) => ({
	links: [],
	loading: false,

	fetchLinks: async (paperId) => {
		set({ loading: true });
		try {
			const links = await commands.listPaperLinks(paperId);
			set({ links, loading: false });
		} catch (e) {
			logger.error("paperLinks", "Failed to fetch paper links", e);
			set({ loading: false });
		}
	},

	addLink: async (paperId, url, title, favicon) => {
		try {
			const link = await commands.addPaperLink(paperId, url, title, favicon);
			set((s) => ({ links: [link, ...s.links] }));
			return link;
		} catch (e) {
			logger.error("paperLinks", "Failed to add paper link", e);
			return null;
		}
	},

	removeLink: async (id) => {
		try {
			await commands.deletePaperLink(id);
			set((s) => ({ links: s.links.filter((l) => l.id !== id) }));
		} catch (e) {
			logger.error("paperLinks", "Failed to delete paper link", e);
		}
	},

	updateLink: async (id, url, title, favicon) => {
		try {
			const updated = await commands.updatePaperLink(id, url, title, favicon);
			set((s) => ({
				links: s.links.map((l) => (l.id === id ? updated : l)),
			}));
		} catch (e) {
			logger.error("paperLinks", "Failed to update paper link", e);
		}
	},
}));
