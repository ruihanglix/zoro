// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import {
	COLUMN_DEF_MAP,
	type ColumnState,
	getDefaultColumnState,
	loadColumnState,
	saveColumnState,
} from "@/lib/columnConfig";
import i18n from "@/lib/i18n";
import type { SupportedLanguage } from "@/lib/i18n";
import { useEffect, useState } from "react";
import { create } from "zustand";

const ONBOARDING_KEY = "zoro-onboarding-completed";

type View = "library" | "feed" | "papers-cool" | "plugins";
type ListMode = "list" | "card";
export type Theme = "light" | "dark" | "system";
export type CitationPreviewMode = "text" | "image" | "off";

export type HtmlReaderFontFamily =
	| "system"
	| "serif"
	| "sans-serif"
	| "cjk"
	| "custom";

export interface HtmlReaderTypography {
	fontFamily: HtmlReaderFontFamily;
	customFontFamily: string;
	fontSize: number; // px, 12–24
	lineHeight: number; // 1.2–2.4
	fontWeight: number; // 300–700
	maxWidth: number; // px, 0 = unlimited, 600–1200
}

// localStorage helpers
function loadSetting<T>(key: string, fallback: T): T {
	try {
		const raw = localStorage.getItem(key);
		if (raw === null) return fallback;
		return JSON.parse(raw) as T;
	} catch {
		return fallback;
	}
}

function saveSetting<T>(key: string, value: T): void {
	try {
		localStorage.setItem(key, JSON.stringify(value));
	} catch {
		// silently ignore
	}
}

type PanelLayout = Record<string, number>;

function layoutsEqual(a: PanelLayout, b: PanelLayout): boolean {
	const keysA = Object.keys(a);
	const keysB = Object.keys(b);
	if (keysA.length !== keysB.length) return false;
	return keysA.every((k) => Math.abs((a[k] ?? 0) - (b[k] ?? 0)) < 0.1);
}

// Apply global UI scale to the document root element
export function applyUiScale(scale: number): void {
	document.documentElement.style.fontSize = `${scale * 100}%`;
}

export function applyTheme(theme: Theme): void {
	const root = document.documentElement;
	if (theme === "dark") {
		root.classList.add("dark");
	} else if (theme === "light") {
		root.classList.remove("dark");
	} else {
		const prefersDark = window.matchMedia(
			"(prefers-color-scheme: dark)",
		).matches;
		root.classList.toggle("dark", prefersDark);
	}
}

export function useIsDarkMode(): boolean {
	const theme = useUiStore((s) => s.theme);
	const [isDark, setIsDark] = useState(() => {
		if (theme === "dark") return true;
		if (theme === "light") return false;
		return window.matchMedia("(prefers-color-scheme: dark)").matches;
	});

	useEffect(() => {
		if (theme !== "system") {
			setIsDark(theme === "dark");
			return;
		}
		const mq = window.matchMedia("(prefers-color-scheme: dark)");
		setIsDark(mq.matches);
		const handler = (e: MediaQueryListEvent) => setIsDark(e.matches);
		mq.addEventListener("change", handler);
		return () => mq.removeEventListener("change", handler);
	}, [theme]);

	return isDark;
}

const DEFAULT_HTML_READER_TYPOGRAPHY: HtmlReaderTypography = {
	fontFamily: "system",
	customFontFamily: "",
	fontSize: 16,
	lineHeight: 1.6,
	fontWeight: 400,
	maxWidth: 800,
};

interface UiState {
	view: View;
	sidebarOpen: boolean;
	listMode: ListMode;
	feedListMode: ListMode;
	addPaperDialogOpen: boolean;
	importDialogOpen: boolean;
	metadataSearchPaperId: string | null;
	showUncategorized: boolean;
	showBackgroundTasks: boolean;
	debugMode: boolean;
	disableNativeContextMenu: boolean;

	// Persisted preferences
	theme: Theme;
	confirmBeforeDelete: boolean;
	defaultView: View;
	defaultListMode: ListMode;
	defaultSortBy: string;
	defaultSortOrder: string;
	citationPreviewMode: CitationPreviewMode;
	showReaderTerminal: boolean;
	language: SupportedLanguage;
	uiScale: number;

	// HTML reader typography
	htmlReaderTypography: HtmlReaderTypography;

	// Column configuration (order = array order)
	columns: ColumnState[];

	// Panel layout persistence (synced across same-type tabs)
	readerPanelLayout: PanelLayout;

	// Onboarding
	showOnboarding: boolean;

	setView: (view: View) => void;
	toggleSidebar: () => void;
	setListMode: (mode: ListMode) => void;
	setFeedListMode: (mode: ListMode) => void;
	setAddPaperDialogOpen: (open: boolean) => void;
	setImportDialogOpen: (open: boolean) => void;
	openMetadataSearchDialog: (paperId: string) => void;
	closeMetadataSearchDialog: () => void;
	setShowUncategorized: (show: boolean) => void;
	setShowBackgroundTasks: (show: boolean) => void;
	setDebugMode: (enabled: boolean) => void;
	setDisableNativeContextMenu: (v: boolean) => void;

	// Persisted preference actions
	setTheme: (theme: Theme) => void;
	setConfirmBeforeDelete: (v: boolean) => void;
	setDefaultView: (view: View) => void;
	setDefaultListMode: (mode: ListMode) => void;
	setDefaultSortBy: (field: string) => void;
	setDefaultSortOrder: (order: string) => void;
	setCitationPreviewMode: (mode: CitationPreviewMode) => void;
	setShowReaderTerminal: (v: boolean) => void;
	setLanguage: (lang: SupportedLanguage) => void;
	setUiScale: (scale: number) => void;

	// HTML reader typography actions
	setHtmlReaderTypography: (typography: Partial<HtmlReaderTypography>) => void;
	resetHtmlReaderTypography: () => void;

	// Column actions
	setColumns: (columns: ColumnState[]) => void;
	toggleColumnVisibility: (columnId: string) => void;
	reorderColumns: (fromIndex: number, toIndex: number) => void;
	resizeColumn: (columnId: string, width: number) => void;
	resetColumns: () => void;

	// Panel layout actions
	setReaderPanelLayout: (layout: PanelLayout) => void;

	// Onboarding actions
	setShowOnboarding: (show: boolean) => void;
	restartOnboarding: () => void;
}

const savedDefaultView = loadSetting<View>("zoro-default-view", "library");
const savedDefaultListMode = loadSetting<ListMode>(
	"zoro-default-list-mode",
	"list",
);

export const useUiStore = create<UiState>((set, get) => ({
	view: savedDefaultView,
	sidebarOpen: true,
	listMode: savedDefaultListMode,
	feedListMode: "card",
	addPaperDialogOpen: false,
	importDialogOpen: false,
	metadataSearchPaperId: null,
	showUncategorized: true,
	showBackgroundTasks: loadSetting<boolean>("zoro-show-bg-tasks", false),
	debugMode: loadSetting<boolean>("zoro-debug-mode", false),
	disableNativeContextMenu: loadSetting<boolean>(
		"zoro-disable-native-ctx",
		false,
	),
	columns: loadColumnState(),
	readerPanelLayout: loadSetting<PanelLayout>("zoro-reader-layout", {
		"reader-left": 22,
		"reader-center": 48,
		"reader-right": 30,
	}),

	showOnboarding: localStorage.getItem(ONBOARDING_KEY) !== "true",

	theme: loadSetting<Theme>("zoro-theme", "system"),
	confirmBeforeDelete: loadSetting<boolean>("zoro-confirm-delete", true),
	defaultView: savedDefaultView,
	defaultListMode: savedDefaultListMode,
	defaultSortBy: loadSetting<string>("zoro-default-sort-by", "added_date"),
	defaultSortOrder: loadSetting<string>("zoro-default-sort-order", "desc"),
	citationPreviewMode: loadSetting<CitationPreviewMode>(
		"zoro-citation-preview-mode",
		"text",
	),
	showReaderTerminal: loadSetting<boolean>("zoro-show-reader-terminal", true),
	language: (localStorage.getItem("zoro-ui-language") ??
		i18n.language ??
		"en") as SupportedLanguage,
	uiScale: loadSetting<number>("zoro-ui-scale", 1),

	htmlReaderTypography: loadSetting<HtmlReaderTypography>(
		"zoro-html-reader-typography",
		DEFAULT_HTML_READER_TYPOGRAPHY,
	),

	setView: (view) => set({ view }),
	toggleSidebar: () => set((s) => ({ sidebarOpen: !s.sidebarOpen })),
	setListMode: (mode) => set({ listMode: mode }),
	setFeedListMode: (mode) => set({ feedListMode: mode }),
	setAddPaperDialogOpen: (open) => set({ addPaperDialogOpen: open }),
	setImportDialogOpen: (open) => set({ importDialogOpen: open }),
	openMetadataSearchDialog: (paperId) =>
		set({ metadataSearchPaperId: paperId }),
	closeMetadataSearchDialog: () => set({ metadataSearchPaperId: null }),
	setShowUncategorized: (show) => set({ showUncategorized: show }),
	setShowBackgroundTasks: (show) => {
		saveSetting("zoro-show-bg-tasks", show);
		set({ showBackgroundTasks: show });
	},
	setDebugMode: (enabled) => {
		saveSetting("zoro-debug-mode", enabled);
		set({ debugMode: enabled });
	},
	setDisableNativeContextMenu: (v) => {
		saveSetting("zoro-disable-native-ctx", v);
		set({ disableNativeContextMenu: v });
	},

	setTheme: (theme) => {
		saveSetting("zoro-theme", theme);
		applyTheme(theme);
		set({ theme });
	},
	setConfirmBeforeDelete: (v) => {
		saveSetting("zoro-confirm-delete", v);
		set({ confirmBeforeDelete: v });
	},
	setDefaultView: (view) => {
		saveSetting("zoro-default-view", view);
		set({ defaultView: view });
	},
	setDefaultListMode: (mode) => {
		saveSetting("zoro-default-list-mode", mode);
		set({ defaultListMode: mode });
	},
	setDefaultSortBy: (field) => {
		saveSetting("zoro-default-sort-by", field);
		set({ defaultSortBy: field });
	},
	setDefaultSortOrder: (order) => {
		saveSetting("zoro-default-sort-order", order);
		set({ defaultSortOrder: order });
	},
	setCitationPreviewMode: (mode) => {
		saveSetting("zoro-citation-preview-mode", mode);
		set({ citationPreviewMode: mode });
	},
	setShowReaderTerminal: (v) => {
		saveSetting("zoro-show-reader-terminal", v);
		set({ showReaderTerminal: v });
	},
	setLanguage: (lang) => {
		localStorage.setItem("zoro-ui-language", lang);
		i18n.changeLanguage(lang);
		set({ language: lang });
	},
	setUiScale: (scale) => {
		const clamped = Math.max(0.5, Math.min(2, scale));
		saveSetting("zoro-ui-scale", clamped);
		applyUiScale(clamped);
		set({ uiScale: clamped });
	},

	setHtmlReaderTypography: (partial) => {
		const current = get().htmlReaderTypography;
		const updated = { ...current, ...partial };
		saveSetting("zoro-html-reader-typography", updated);
		set({ htmlReaderTypography: updated });
	},
	resetHtmlReaderTypography: () => {
		saveSetting("zoro-html-reader-typography", DEFAULT_HTML_READER_TYPOGRAPHY);
		set({ htmlReaderTypography: DEFAULT_HTML_READER_TYPOGRAPHY });
	},

	setColumns: (columns) => {
		saveColumnState(columns);
		set({ columns });
	},

	toggleColumnVisibility: (columnId) =>
		set((s) => {
			// Don't allow hiding pinned columns
			const def = COLUMN_DEF_MAP[columnId];
			if (def?.pinned) return s;

			const columns = s.columns.map((col) =>
				col.id === columnId ? { ...col, visible: !col.visible } : col,
			);
			saveColumnState(columns);
			return { columns };
		}),

	reorderColumns: (fromIndex, toIndex) =>
		set((s) => {
			if (fromIndex === toIndex) return s;
			const columns = [...s.columns];
			const [moved] = columns.splice(fromIndex, 1);
			columns.splice(toIndex, 0, moved);
			saveColumnState(columns);
			return { columns };
		}),

	resizeColumn: (columnId, width) =>
		set((s) => {
			const def = COLUMN_DEF_MAP[columnId];
			const minWidth = def?.minWidth ?? 30;
			const clampedWidth = Math.max(minWidth, Math.round(width));

			const columns = s.columns.map((col) =>
				col.id === columnId ? { ...col, width: clampedWidth } : col,
			);
			saveColumnState(columns);
			return { columns };
		}),

	resetColumns: () => {
		const columns = getDefaultColumnState();
		saveColumnState(columns);
		set({ columns });
	},

	setShowOnboarding: (show) => set({ showOnboarding: show }),
	restartOnboarding: () => {
		localStorage.removeItem(ONBOARDING_KEY);
		set({ showOnboarding: true });
	},

	setReaderPanelLayout: (layout) => {
		if (layoutsEqual(get().readerPanelLayout, layout)) return;
		saveSetting("zoro-reader-layout", layout);
		set({ readerPanelLayout: layout });
	},
}));
