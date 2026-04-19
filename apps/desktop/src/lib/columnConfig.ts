// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import type { PaperResponse } from "@/lib/commands";

// --- Column definition (static, never changes) ---

export interface ColumnDef {
	/** Unique column identifier */
	id: string;
	/** Display label shown in the header */
	label: string;
	/** Whether the column is visible by default */
	defaultVisible: boolean;
	/** Default width in pixels. -1 means flex-1 (fill remaining space) */
	defaultWidth: number;
	/** Minimum width in pixels when resizing */
	minWidth: number;
	/** Backend sort field name. undefined = not sortable */
	sortField?: string;
	/** If true, column cannot be hidden (only title) */
	pinned?: boolean;
}

/** All available columns in their default order */
export const COLUMN_DEFS: ColumnDef[] = [
	{
		id: "shortTitle",
		label: "columns.shortTitle",
		defaultVisible: true,
		defaultWidth: 120,
		minWidth: 60,
	},
	{
		id: "title",
		label: "columns.title",
		defaultVisible: true,
		defaultWidth: -1,
		minWidth: 120,
		sortField: "title",
		pinned: true,
	},
	{
		id: "authors",
		label: "columns.authors",
		defaultVisible: true,
		defaultWidth: 180,
		minWidth: 80,
		sortField: "authors",
	},
	{
		id: "year",
		label: "columns.year",
		defaultVisible: false,
		defaultWidth: 60,
		minWidth: 40,
		sortField: "published_date",
	},
	{
		id: "source",
		label: "columns.source",
		defaultVisible: false,
		defaultWidth: 80,
		minWidth: 50,
	},
	{
		id: "readStatus",
		label: "columns.status",
		defaultVisible: false,
		defaultWidth: 70,
		minWidth: 50,
	},
	{
		id: "addedDate",
		label: "columns.added",
		defaultVisible: true,
		defaultWidth: 100,
		minWidth: 60,
		sortField: "added_date",
	},
	{
		id: "modifiedDate",
		label: "columns.modified",
		defaultVisible: false,
		defaultWidth: 100,
		minWidth: 60,
		sortField: "modified_date",
	},
	{
		id: "doi",
		label: "columns.doi",
		defaultVisible: false,
		defaultWidth: 150,
		minWidth: 80,
	},
	{
		id: "arxivId",
		label: "columns.arxivId",
		defaultVisible: false,
		defaultWidth: 120,
		minWidth: 80,
	},
	{
		id: "journal",
		label: "columns.journal",
		defaultVisible: false,
		defaultWidth: 150,
		minWidth: 80,
	},
	{
		id: "volume",
		label: "columns.volume",
		defaultVisible: false,
		defaultWidth: 60,
		minWidth: 40,
	},
	{
		id: "issue",
		label: "columns.issue",
		defaultVisible: false,
		defaultWidth: 60,
		minWidth: 40,
	},
	{
		id: "pages",
		label: "columns.pages",
		defaultVisible: false,
		defaultWidth: 80,
		minWidth: 50,
	},
	{
		id: "publisher",
		label: "columns.publisher",
		defaultVisible: false,
		defaultWidth: 120,
		minWidth: 80,
	},
	{
		id: "entryType",
		label: "columns.type",
		defaultVisible: false,
		defaultWidth: 100,
		minWidth: 60,
	},
	{
		id: "rating",
		label: "columns.rating",
		defaultVisible: false,
		defaultWidth: 80,
		minWidth: 60,
	},
	{
		id: "tags",
		label: "columns.tags",
		defaultVisible: false,
		defaultWidth: 150,
		minWidth: 80,
	},
	{
		id: "attachments",
		label: "columns.attachments",
		defaultVisible: false,
		defaultWidth: 80,
		minWidth: 50,
	},
	{
		id: "pdfStatus",
		label: "columns.pdf",
		defaultVisible: false,
		defaultWidth: 32,
		minWidth: 28,
	},
	{
		id: "abstract",
		label: "columns.abstract",
		defaultVisible: false,
		defaultWidth: 300,
		minWidth: 100,
	},
	{
		id: "url",
		label: "columns.url",
		defaultVisible: false,
		defaultWidth: 200,
		minWidth: 80,
	},
];

/** Lookup map for quick access by column id */
export const COLUMN_DEF_MAP: Record<string, ColumnDef> = Object.fromEntries(
	COLUMN_DEFS.map((def) => [def.id, def]),
);

// --- Persisted column state ---

export interface ColumnState {
	/** Column id (must match a ColumnDef.id) */
	id: string;
	/** Whether the column is currently visible */
	visible: boolean;
	/** Current width in pixels. -1 means flex-1 */
	width: number;
}

const STORAGE_KEY = "zoro-column-config";

/** Build the default column state from COLUMN_DEFS */
export function getDefaultColumnState(): ColumnState[] {
	return COLUMN_DEFS.map((def) => ({
		id: def.id,
		visible: def.defaultVisible,
		width: def.defaultWidth,
	}));
}

/** Load column state from localStorage, falling back to defaults */
export function loadColumnState(): ColumnState[] {
	try {
		const raw = localStorage.getItem(STORAGE_KEY);
		if (!raw) return getDefaultColumnState();

		const saved: ColumnState[] = JSON.parse(raw);
		if (!Array.isArray(saved) || saved.length === 0) {
			return getDefaultColumnState();
		}

		// Merge with current COLUMN_DEFS to handle added/removed columns
		const savedMap = new Map(saved.map((s) => [s.id, s]));
		const knownIds = new Set(COLUMN_DEFS.map((d) => d.id));

		// Start with saved columns that still exist (preserves order)
		const merged: ColumnState[] = [];
		for (const s of saved) {
			if (knownIds.has(s.id)) {
				merged.push(s);
			}
		}

		// Append any new columns that weren't in saved state
		for (const def of COLUMN_DEFS) {
			if (!savedMap.has(def.id)) {
				merged.push({
					id: def.id,
					visible: def.defaultVisible,
					width: def.defaultWidth,
				});
			}
		}

		return merged;
	} catch {
		return getDefaultColumnState();
	}
}

/** Save column state to localStorage */
export function saveColumnState(state: ColumnState[]): void {
	try {
		localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
	} catch {
		// Silently ignore storage errors
	}
}

// --- Cell value extractors ---

/** Extract a display-ready string value from a paper for a given column */
export function getCellValue(columnId: string, paper: PaperResponse): string {
	switch (columnId) {
		case "title":
			return paper.title;
		case "shortTitle":
			return paper.short_title ?? "";
		case "authors": {
			const first = paper.authors[0]?.name ?? "";
			return paper.authors.length > 1 ? `${first} et al.` : first;
		}
		case "year":
			return paper.published_date
				? String(new Date(paper.published_date).getFullYear())
				: "";
		case "source":
			return paper.source ?? "";
		case "readStatus":
			return paper.read_status;
		case "addedDate":
			return new Date(paper.added_date).toLocaleString(undefined, {
				year: "numeric",
				month: "short",
				day: "numeric",
				hour: "2-digit",
				minute: "2-digit",
			});
		case "modifiedDate":
			return new Date(paper.modified_date).toLocaleString(undefined, {
				year: "numeric",
				month: "short",
				day: "numeric",
				hour: "2-digit",
				minute: "2-digit",
			});
		case "doi":
			return paper.doi ?? "";
		case "arxivId":
			return paper.arxiv_id ?? "";
		case "journal":
			return paper.journal ?? "";
		case "volume":
			return paper.volume ?? "";
		case "issue":
			return paper.issue ?? "";
		case "pages":
			return paper.pages ?? "";
		case "publisher":
			return paper.publisher ?? "";
		case "entryType":
			return paper.entry_type ?? "";
		case "rating":
			return paper.rating != null ? String(paper.rating) : "";
		case "tags":
			return paper.tags.map((t) => t.name).join(", ");
		case "attachments":
			return paper.attachments.length > 0
				? String(paper.attachments.length)
				: "";
		case "pdfStatus":
			return ""; // Rendered as icon, not text
		case "abstract":
			return paper.abstract_text ?? "";
		case "url":
			return paper.url ?? "";
		default:
			return "";
	}
}
