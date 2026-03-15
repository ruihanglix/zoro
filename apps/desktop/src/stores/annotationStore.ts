// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import * as commands from "@/lib/commands";
import type { AnnotationResponse, ReaderStateResponse } from "@/lib/commands";
import type { PDFDocumentProxy } from "pdfjs-dist";
import type {
	Content,
	IHighlight,
	ScaledPosition,
} from "react-pdf-highlighter";
import { create } from "zustand";

// Extend IHighlight with our custom fields
export interface ZoroHighlight extends IHighlight {
	type: AnnotationType;
	color: string;
	paperId: string;
	selectedText: string | null;
	imageData: string | null;
	pageNumber: number;
	createdDate: string;
	modifiedDate: string;
}

// Expanded annotation colors (8 colors, matching Zotero)
export const ANNOTATION_COLORS = [
	{ name: "Yellow", value: "#ffe28f" },
	{ name: "Red", value: "#f5a3a3" },
	{ name: "Green", value: "#a8e6a3" },
	{ name: "Blue", value: "#a3d1e6" },
	{ name: "Purple", value: "#cba3e6" },
	{ name: "Magenta", value: "#e6a3d1" },
	{ name: "Orange", value: "#f5c88a" },
	{ name: "Gray", value: "#c4c4c4" },
] as const;

export const DEFAULT_ANNOTATION_COLOR = "#ffe28f";

export type AnnotationType =
	| "highlight"
	| "underline"
	| "area"
	| "note"
	| "ink";

// Active tool in the reader toolbar
export type ReaderTool = "cursor" | "highlight" | "underline" | "note" | "ink";

// Ink annotation path data
export interface InkPoint {
	x: number;
	y: number;
}

export interface InkStroke {
	points: InkPoint[];
	strokeWidth: number;
}

export interface InkAnnotationData {
	strokes: InkStroke[];
	boundingRect: { x1: number; y1: number; x2: number; y2: number };
}

// Left panel view mode
export type LeftPanelView = "thumbnails" | "outline" | "annotations";

// HTML heading item for outline panel
export interface HtmlHeadingItem {
	level: number; // 1-6 corresponding to h1-h6
	text: string;
	id: string; // unique id for scrolling
	children: HtmlHeadingItem[];
}

interface AnnotationState {
	annotations: ZoroHighlight[];
	loading: boolean;
	error: string | null;

	// Current tool state
	activeColor: string;
	activeType: AnnotationType;
	activeTool: ReaderTool;

	// Ink drawing state
	inkStrokeWidth: number;
	inkEraserActive: boolean;

	// Left panel view
	leftPanelView: LeftPanelView;

	// PDF document reference (shared across components)
	pdfDocument: PDFDocumentProxy | null;

	// Page navigation state
	currentPage: number;
	totalPages: number;

	// Navigation history (for back/forward)
	navigationHistory: number[];
	historyIndex: number;

	// Timestamp of last programmatic navigation (suppresses jump detection in setCurrentPage)
	_lastNavigateTime: number;

	// Source page recorded when an internal PDF link is clicked (Table/Figure/Citation refs).
	// setCurrentPage uses this for debounced jump detection during smooth scrolling.
	_linkJumpSourcePage: number | null;

	// Reader state
	readerState: ReaderStateResponse | null;

	// Scroll-to function ref (set by PdfHighlighter's scrollRef)
	scrollToHighlight: ((highlight: ZoroHighlight) => void) | null;

	// Scroll-to-page function ref (set by PdfAnnotationViewer)
	scrollToPage: ((pageNumber: number) => void) | null;

	// PDF viewer ref for search (set by PdfAnnotationViewer)
	pdfViewer: Record<string, unknown> | null;

	// Actions
	fetchAnnotations: (paperId: string, sourceFile?: string) => Promise<void>;
	addAnnotation: (
		paperId: string,
		type: AnnotationType,
		color: string,
		position: ScaledPosition,
		content: Content,
		sourceFile?: string,
	) => Promise<ZoroHighlight | null>;
	addInkAnnotation: (
		paperId: string,
		color: string,
		pageNumber: number,
		inkData: InkAnnotationData,
		sourceFile?: string,
	) => Promise<ZoroHighlight | null>;
	updateAnnotation: (
		id: string,
		color?: string | null,
		comment?: string | null,
	) => Promise<void>;
	deleteAnnotation: (id: string, paperId: string) => Promise<void>;
	updateAnnotationType: (id: string, newType: AnnotationType) => Promise<void>;
	setActiveColor: (color: string) => void;
	setActiveType: (type: AnnotationType) => void;
	setActiveTool: (tool: ReaderTool) => void;
	setInkStrokeWidth: (width: number) => void;
	setInkEraserActive: (active: boolean) => void;
	setLeftPanelView: (view: LeftPanelView) => void;
	setScrollToHighlight: (
		fn: ((highlight: ZoroHighlight) => void) | null,
	) => void;
	setScrollToPage: (fn: ((pageNumber: number) => void) | null) => void;
	setPdfDocument: (doc: PDFDocumentProxy | null) => void;
	setPdfViewer: (viewer: Record<string, unknown> | null) => void;
	setCurrentPage: (page: number) => void;
	setTotalPages: (total: number) => void;

	// Navigation history actions
	navigateToPage: (page: number) => void;
	navigateBack: () => void;
	navigateForward: () => void;
	canNavigateBack: () => boolean;
	canNavigateForward: () => boolean;
	markInternalLinkJump: () => void;

	// Annotation navigation
	navigateToPrevAnnotation: () => void;
	navigateToNextAnnotation: () => void;

	// Zoom
	zoomLevel: number;
	setZoomLevel: (level: number) => void;

	// Reader state actions
	fetchReaderState: (paperId: string) => Promise<void>;
	saveReaderState: (
		paperId: string,
		scrollPosition?: number | null,
		scale?: number | null,
	) => Promise<void>;

	// HTML annotation actions
	addHtmlAnnotation: (
		paperId: string,
		type: AnnotationType,
		color: string,
		positionJson: string,
		selectedText: string | null,
		comment: string | null,
		sourceFile?: string,
	) => Promise<ZoroHighlight | null>;
	addHtmlInkAnnotation: (
		paperId: string,
		color: string,
		inkData: {
			strokes: InkStroke[];
			boundingRect: { x1: number; y1: number; x2: number; y2: number };
			contentHeight: number;
		},
		sourceFile?: string,
	) => Promise<ZoroHighlight | null>;

	// Pending HTML citation jump (set by notes panel, consumed by HTML reader)
	pendingHtmlCitationJump: string | null;
	setPendingHtmlCitationJump: (positionJson: string | null) => void;

	// Pending HTML annotation scroll (set by side panel when switching from PDF to HTML mode)
	pendingHtmlAnnotationScrollId: string | null;
	setPendingHtmlAnnotationScrollId: (id: string | null) => void;

	// HTML outline headings (extracted from iframe)
	htmlHeadings: HtmlHeadingItem[];
	setHtmlHeadings: (headings: HtmlHeadingItem[]) => void;

	// Function ref: scroll to a heading in HTML iframe
	scrollToHtmlHeading: ((headingId: string) => void) | null;
	setScrollToHtmlHeading: (fn: ((headingId: string) => void) | null) => void;

	// Reset state (when switching papers)
	resetReaderState: () => void;
}

function responseToHighlight(resp: AnnotationResponse): ZoroHighlight {
	const position: ScaledPosition = JSON.parse(resp.position_json);
	return {
		id: resp.id,
		type: resp.type as AnnotationType,
		color: resp.color,
		paperId: resp.paper_id,
		selectedText: resp.selected_text,
		imageData: resp.image_data,
		pageNumber: resp.page_number,
		createdDate: resp.created_date,
		modifiedDate: resp.modified_date,
		position,
		content: {
			text: resp.selected_text ?? undefined,
			image: resp.image_data ?? undefined,
		},
		comment: {
			text: resp.comment ?? "",
			emoji: "",
		},
	};
}

// Module-level timer for debounced link jump detection (not in Zustand state
// to avoid serialization and unnecessary re-render concerns).
let _linkJumpSettleTimer: ReturnType<typeof setTimeout> | null = null;

export const useAnnotationStore = create<AnnotationState>()((set, get) => ({
	annotations: [],
	loading: false,
	error: null,
	activeColor: DEFAULT_ANNOTATION_COLOR,
	activeType: "highlight",
	activeTool: "cursor",
	inkStrokeWidth: 2,
	inkEraserActive: false,
	leftPanelView: "annotations",
	pdfDocument: null,
	currentPage: 1,
	totalPages: 0,
	navigationHistory: [],
	historyIndex: -1,
	_lastNavigateTime: 0,
	_linkJumpSourcePage: null,
	readerState: null,
	scrollToHighlight: null,
	scrollToPage: null,
	pdfViewer: null,
	zoomLevel: 1,
	pendingHtmlCitationJump: null,
	pendingHtmlAnnotationScrollId: null,
	htmlHeadings: [],
	scrollToHtmlHeading: null,

	setPendingHtmlCitationJump: (positionJson) =>
		set({ pendingHtmlCitationJump: positionJson }),

	setPendingHtmlAnnotationScrollId: (id) =>
		set({ pendingHtmlAnnotationScrollId: id }),

	setHtmlHeadings: (headings) => set({ htmlHeadings: headings }),
	setScrollToHtmlHeading: (fn) => set({ scrollToHtmlHeading: fn }),

	setZoomLevel: (level) =>
		set({ zoomLevel: Math.max(0.25, Math.min(5, level)) }),

	fetchAnnotations: async (paperId: string, sourceFile?: string) => {
		set({ loading: true, error: null, annotations: [] });
		try {
			const resp = await commands.listAnnotations(paperId, sourceFile);
			set({ annotations: resp.map(responseToHighlight), loading: false });
		} catch (e) {
			set({ error: String(e), loading: false });
		}
	},

	addAnnotation: async (
		paperId,
		type,
		color,
		position,
		content,
		sourceFile,
	) => {
		try {
			const resp = await commands.addAnnotation(
				paperId,
				type,
				color,
				JSON.stringify(position),
				position.pageNumber,
				undefined,
				content.text ?? null,
				content.image ?? null,
				sourceFile,
			);
			const highlight = responseToHighlight(resp);
			set((s) => ({ annotations: [...s.annotations, highlight] }));
			return highlight;
		} catch (e) {
			console.error("Failed to add annotation:", e);
			return null;
		}
	},

	addInkAnnotation: async (paperId, color, pageNumber, inkData, sourceFile) => {
		try {
			const positionJson = JSON.stringify({
				pageNumber,
				boundingRect: {
					x1: inkData.boundingRect.x1,
					y1: inkData.boundingRect.y1,
					x2: inkData.boundingRect.x2,
					y2: inkData.boundingRect.y2,
					width: 100,
					height: 100,
					pageNumber,
				},
				rects: [],
				usePdfCoordinates: false,
				inkStrokes: inkData.strokes,
			});
			const resp = await commands.addAnnotation(
				paperId,
				"ink",
				color,
				positionJson,
				pageNumber,
				undefined,
				undefined,
				undefined,
				sourceFile,
			);
			const highlight = responseToHighlight(resp);
			set((s) => ({ annotations: [...s.annotations, highlight] }));
			return highlight;
		} catch (e) {
			console.error("Failed to add ink annotation:", e);
			return null;
		}
	},

	updateAnnotation: async (id, color, comment) => {
		try {
			const resp = await commands.updateAnnotation(id, color, comment);
			const updated = responseToHighlight(resp);
			set((s) => ({
				annotations: s.annotations.map((a) => (a.id === id ? updated : a)),
			}));
		} catch (e) {
			console.error("Failed to update annotation:", e);
		}
	},

	deleteAnnotation: async (id, _paperId) => {
		try {
			await commands.deleteAnnotation(id);
			set((s) => ({
				annotations: s.annotations.filter((a) => a.id !== id),
			}));
		} catch (e) {
			console.error("Failed to delete annotation:", e);
		}
	},

	updateAnnotationType: async (id, newType) => {
		try {
			const resp = await commands.updateAnnotationType(id, newType);
			const updated = responseToHighlight(resp);
			set((s) => ({
				annotations: s.annotations.map((a) => (a.id === id ? updated : a)),
			}));
		} catch (e) {
			console.error("Failed to update annotation type:", e);
		}
	},

	setActiveColor: (color) => set({ activeColor: color }),
	setActiveType: (type) => set({ activeType: type }),
	setActiveTool: (tool) => set({ activeTool: tool, inkEraserActive: false }),
	setInkStrokeWidth: (width) => set({ inkStrokeWidth: width }),
	setInkEraserActive: (active) => set({ inkEraserActive: active }),
	setLeftPanelView: (view) => set({ leftPanelView: view }),
	setScrollToHighlight: (fn) => set({ scrollToHighlight: fn }),
	setScrollToPage: (fn) => set({ scrollToPage: fn }),
	setPdfDocument: (doc) =>
		set({ pdfDocument: doc, totalPages: doc?.numPages ?? 0 }),
	setPdfViewer: (viewer) => set({ pdfViewer: viewer }),
	setCurrentPage: (page) => {
		const state = get();
		if (page === state.currentPage) return;

		if (Date.now() - state._lastNavigateTime < 2000) {
			set({ currentPage: page });
			return;
		}

		// When an internal PDF link click was detected, use debounced detection:
		// PDF.js smooth-scrolls to the target, producing many small-delta page
		// changes. We wait for scrolling to settle, then compare against the
		// source page recorded at click time.
		if (state._linkJumpSourcePage !== null) {
			set({ currentPage: page });
			if (_linkJumpSettleTimer) clearTimeout(_linkJumpSettleTimer);
			_linkJumpSettleTimer = setTimeout(() => {
				_linkJumpSettleTimer = null;
				const current = get();
				const src = current._linkJumpSourcePage;
				if (src === null) return;
				const dest = current.currentPage;
				if (dest !== src) {
					const newHistory = current.navigationHistory.slice(
						0,
						current.historyIndex + 1,
					);
					if (
						newHistory.length === 0 ||
						newHistory[newHistory.length - 1] !== src
					) {
						newHistory.push(src);
					}
					newHistory.push(dest);
					set({
						navigationHistory: newHistory,
						historyIndex: newHistory.length - 1,
						_lastNavigateTime: Date.now(),
						_linkJumpSourcePage: null,
					});
				} else {
					set({ _linkJumpSourcePage: null });
				}
			}, 500);
			return;
		}

		// Normal scrolling: immediate delta > 2 detection (handles instant jumps
		// from PDF.js that don't use smooth scrolling).
		if (Math.abs(page - state.currentPage) > 2) {
			const newHistory = state.navigationHistory.slice(
				0,
				state.historyIndex + 1,
			);
			if (
				newHistory.length === 0 ||
				newHistory[newHistory.length - 1] !== state.currentPage
			) {
				newHistory.push(state.currentPage);
			}
			newHistory.push(page);
			set({
				currentPage: page,
				navigationHistory: newHistory,
				historyIndex: newHistory.length - 1,
			});
		} else {
			set({ currentPage: page });
		}
	},
	setTotalPages: (total) => set({ totalPages: total }),

	navigateToPage: (page) => {
		const { historyIndex, navigationHistory, scrollToPage, currentPage } =
			get();
		if (page === currentPage) return;
		const newHistory = navigationHistory.slice(0, historyIndex + 1);
		// Push source page so the user can navigate back to it
		if (
			newHistory.length === 0 ||
			newHistory[newHistory.length - 1] !== currentPage
		) {
			newHistory.push(currentPage);
		}
		newHistory.push(page);
		set({
			navigationHistory: newHistory,
			historyIndex: newHistory.length - 1,
			currentPage: page,
			_lastNavigateTime: Date.now(),
		});
		scrollToPage?.(page);
	},

	navigateBack: () => {
		const { historyIndex, navigationHistory, scrollToPage } = get();
		if (historyIndex <= 0) return;
		const newIndex = historyIndex - 1;
		const page = navigationHistory[newIndex];
		set({
			historyIndex: newIndex,
			currentPage: page,
			_lastNavigateTime: Date.now(),
		});
		scrollToPage?.(page);
	},

	navigateForward: () => {
		const { historyIndex, navigationHistory, scrollToPage } = get();
		if (historyIndex >= navigationHistory.length - 1) return;
		const newIndex = historyIndex + 1;
		const page = navigationHistory[newIndex];
		set({
			historyIndex: newIndex,
			currentPage: page,
			_lastNavigateTime: Date.now(),
		});
		scrollToPage?.(page);
	},

	canNavigateBack: () => get().historyIndex > 0,
	canNavigateForward: () =>
		get().historyIndex < get().navigationHistory.length - 1,

	markInternalLinkJump: () => {
		set({ _linkJumpSourcePage: get().currentPage });
	},

	navigateToPrevAnnotation: () => {
		const { annotations, currentPage, scrollToHighlight } = get();
		if (annotations.length === 0 || !scrollToHighlight) return;

		// Find the last annotation before the current page, or the last annotation on the current page
		// that is before the current scroll position
		const sorted = [...annotations].sort((a, b) => a.pageNumber - b.pageNumber);
		const beforeCurrent = sorted.filter((a) => a.pageNumber < currentPage);
		const onCurrent = sorted.filter((a) => a.pageNumber === currentPage);

		// Try annotations on current page first (in reverse), then previous pages
		const candidates = [...onCurrent.reverse(), ...beforeCurrent.reverse()];
		if (candidates.length > 0) {
			scrollToHighlight(candidates[0]);
		} else if (sorted.length > 0) {
			// Wrap around to the last annotation
			scrollToHighlight(sorted[sorted.length - 1]);
		}
	},

	navigateToNextAnnotation: () => {
		const { annotations, currentPage, scrollToHighlight } = get();
		if (annotations.length === 0 || !scrollToHighlight) return;

		const sorted = [...annotations].sort((a, b) => a.pageNumber - b.pageNumber);
		const afterCurrent = sorted.filter((a) => a.pageNumber > currentPage);
		const onCurrent = sorted.filter((a) => a.pageNumber === currentPage);

		// Try annotations on current page first, then next pages
		const candidates = [...onCurrent, ...afterCurrent];
		if (candidates.length > 0) {
			scrollToHighlight(candidates[0]);
		} else if (sorted.length > 0) {
			// Wrap around to the first annotation
			scrollToHighlight(sorted[0]);
		}
	},

	fetchReaderState: async (paperId) => {
		try {
			const state = await commands.getReaderState(paperId);
			set({ readerState: state });
		} catch (e) {
			console.error("Failed to fetch reader state:", e);
		}
	},

	saveReaderState: async (paperId, scrollPosition, scale) => {
		try {
			const state = await commands.saveReaderState(
				paperId,
				scrollPosition,
				scale,
			);
			set({ readerState: state });
		} catch (e) {
			console.error("Failed to save reader state:", e);
		}
	},

	addHtmlAnnotation: async (
		paperId,
		type,
		color,
		positionJson,
		selectedText,
		comment,
		sourceFile,
	) => {
		try {
			const resp = await commands.addAnnotation(
				paperId,
				type,
				color,
				positionJson,
				0,
				comment ?? undefined,
				selectedText ?? undefined,
				undefined,
				sourceFile,
			);
			const highlight = responseToHighlight(resp);
			set((s) => ({ annotations: [...s.annotations, highlight] }));
			return highlight;
		} catch (e) {
			console.error("Failed to add HTML annotation:", e);
			return null;
		}
	},

	addHtmlInkAnnotation: async (paperId, color, inkData, sourceFile) => {
		try {
			const positionJson = JSON.stringify({
				format: "html",
				inkStrokes: inkData.strokes,
				contentHeight: inkData.contentHeight,
				pageNumber: 0,
				boundingRect: {
					x1: inkData.boundingRect.x1,
					y1: inkData.boundingRect.y1,
					x2: inkData.boundingRect.x2,
					y2: inkData.boundingRect.y2,
					width: 1,
					height: 1,
					pageNumber: 0,
				},
				rects: [],
			});
			const resp = await commands.addAnnotation(
				paperId,
				"ink",
				color,
				positionJson,
				0,
				undefined,
				undefined,
				undefined,
				sourceFile,
			);
			const highlight = responseToHighlight(resp);
			set((s) => ({ annotations: [...s.annotations, highlight] }));
			return highlight;
		} catch (e) {
			console.error("Failed to add HTML ink annotation:", e);
			return null;
		}
	},

	resetReaderState: () => {
		if (_linkJumpSettleTimer) {
			clearTimeout(_linkJumpSettleTimer);
			_linkJumpSettleTimer = null;
		}
		set({
			annotations: [],
			pdfDocument: null,
			pdfViewer: null,
			currentPage: 1,
			totalPages: 0,
			navigationHistory: [],
			historyIndex: -1,
			_lastNavigateTime: 0,
			_linkJumpSourcePage: null,
			readerState: null,
			scrollToHighlight: null,
			scrollToPage: null,
			activeTool: "cursor",
			inkStrokeWidth: 2,
			inkEraserActive: false,
			leftPanelView: "annotations",
			zoomLevel: 1,
			pendingHtmlCitationJump: null,
			pendingHtmlAnnotationScrollId: null,
			htmlHeadings: [],
			scrollToHtmlHeading: null,
		});
	},
}));
