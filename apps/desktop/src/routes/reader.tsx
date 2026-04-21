// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { BilingualText } from "@/components/BilingualText";
import { DisplayModeToggle } from "@/components/DisplayModeToggle";
import { AnnotationToolbar } from "@/components/reader/AnnotationToolbar";
import { BilingualPdfViewer } from "@/components/reader/BilingualPdfViewer";
import { FeedAnnotationPlaceholder } from "@/components/reader/FeedAnnotationPlaceholder";
import { HighlightPopup } from "@/components/reader/HighlightPopup";
import { PageNavigation } from "@/components/reader/PageNavigation";
import { PdfAnnotationViewer } from "@/components/reader/PdfAnnotationViewer";
import { PdfSearchBar } from "@/components/reader/PdfSearchBar";
import { SidebarTabPanel } from "@/components/reader/SidebarTabPanel";
import { ZoomControls } from "@/components/reader/ZoomControls";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
	ResizableHandle,
	ResizablePanel,
	ResizablePanelGroup,
} from "@/components/ui/resizable";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { useKeybindings } from "@/hooks/useKeybindings";
import { useLocalAnnotations } from "@/hooks/useLocalAnnotations";
import * as commands from "@/lib/commands";
import type {
	FeedItemResponse,
	PaperResponse,
} from "@/lib/commands";
import { getHtmlAnnotationScript } from "@/lib/htmlAnnotation";
import { getAllAvailableTabs } from "@/lib/readerTabs";
import { cn } from "@/lib/utils";
import {
	emitParagraphHover,
	emitTextSelected,
} from "@/plugins/PluginManager";
import { PluginSlot } from "@/plugins/PluginSlot";
import { usePluginStore } from "@/plugins/pluginStore";
import { useAnnotationStore } from "@/stores/annotationStore";
import type {
	AnnotationType,
	ReaderTool,
	ZoroHighlight,
} from "@/stores/annotationStore";
import { ANNOTATION_COLORS } from "@/stores/annotationStore";
import { useLibraryStore } from "@/stores/libraryStore";
import { useNoteStore } from "@/stores/noteStore";
import { useTabStore } from "@/stores/tabStore";
import {
	useTranslatedText,
	useTranslationLoading,
	useTranslationStore,
} from "@/stores/translationStore";
import { useIsDarkMode, useUiStore } from "@/stores/uiStore";
import type {
	HtmlReaderFontFamily,
	HtmlReaderTypography,
} from "@/stores/uiStore";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { readFile } from "@tauri-apps/plugin-fs";
import {
	ArrowLeft,
	ArrowRight,
	BookOpen,
	Check,
	FileText,
	Globe,
	Highlighter,
	Languages,
	Link2,
	Loader2,
	MousePointer2,
	PanelLeft,
	PanelRight,
	Pen,
	RotateCcw,
	Search,
	StickyNote,
	Type,
	Underline,
} from "lucide-react";
import { IconFileTypePdf } from "@tabler/icons-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useGroupRef, usePanelRef } from "react-resizable-panels";
import { useShallow } from "zustand/shallow";

interface ReaderProps {
	tabId: string;
	paperId: string | null;
	feedItem: FeedItemResponse | null;
	readerMode: "pdf" | "html";
	pdfFilename?: string;
}

export function Reader({
	tabId,
	paperId,
	feedItem,
	readerMode,
	pdfFilename,
}: ReaderProps) {
	const { t } = useTranslation();
	const updateTab = useTabStore((s) => s.updateTab);
	const activeTabId = useTabStore((s) => s.activeTabId);
	const tab = useTabStore(
		useShallow((s) => s.tabs.find((t) => t.id === tabId)),
	);
	const isActive = activeTabId === tabId;
	const paper = useLibraryStore(
		useShallow((s) =>
			paperId ? (s.papers.find((p) => p.id === paperId) ?? null) : null,
		),
	);
	const fetchPaper = useLibraryStore((s) => s.fetchPaper);
	const resetReaderState = useAnnotationStore((s) => s.resetReaderState);
	const setHtmlHeadings = useAnnotationStore((s) => s.setHtmlHeadings);
	const setPdfDocument = useAnnotationStore((s) => s.setPdfDocument);

	const [htmlVersion, setHtmlVersion] = useState(0);
	const handleHtmlChanged = useCallback(() => setHtmlVersion((v) => v + 1), []);

	const readerPanelLayout = useUiStore((s) => s.readerPanelLayout);
	const setReaderPanelLayout = useUiStore((s) => s.setReaderPanelLayout);
	const readerSidebarConfig = useUiStore((s) => s.readerSidebarConfig);

	// Plugin sidebar tab contributions (shared between left/right panels)
	const pluginsList = usePluginStore((s) => s.plugins);
	const loadedModules = usePluginStore((s) => s.loadedModules);
	const getContributions = usePluginStore((s) => s.getContributionsForSlot);
	const pluginSidebarTabs = useMemo(() => {
		void pluginsList;
		void loadedModules;
		return getContributions("reader_sidebar");
	}, [pluginsList, loadedModules, getContributions]);

	// Auto-register new plugin tabs into sidebar config
	useEffect(() => {
		const allTabs = getAllAvailableTabs(pluginSidebarTabs);
		const config = useUiStore.getState().readerSidebarConfig;
		const knownIds = new Set([...config.left, ...config.right]);
		const newPluginIds = allTabs
			.filter((t) => t.isPlugin && !knownIds.has(t.id))
			.map((t) => t.id);
		if (newPluginIds.length > 0) {
			useUiStore.getState().setReaderSidebarConfig({
				...config,
				right: [...config.right, ...newPluginIds],
			});
		}
	}, [pluginSidebarTabs]);

	const groupRef = useGroupRef();
	const leftPanelRef = usePanelRef();
	const rightPanelRef = usePanelRef();
	const isLocalResize = useRef(false);

	useEffect(() => {
		if (isLocalResize.current) {
			isLocalResize.current = false;
			return;
		}
		groupRef.current?.setLayout(readerPanelLayout);
	}, [readerPanelLayout, groupRef]);

	const handlePanelLayoutChanged = useCallback(
		(layout: Record<string, number>) => {
			isLocalResize.current = true;
			setReaderPanelLayout(layout);
		},
		[setReaderPanelLayout],
	);

	const toggleLeftPanel = useCallback(() => {
		const panel = leftPanelRef.current;
		if (!panel) return;
		panel.isCollapsed() ? panel.expand() : panel.collapse();
	}, [leftPanelRef]);

	const toggleRightPanel = useCallback(() => {
		const panel = rightPanelRef.current;
		if (!panel) return;
		panel.isCollapsed() ? panel.expand() : panel.collapse();
	}, [rightPanelRef]);

	const isFeedReaderMode = !!feedItem && !paperId;

	const bilingualMode = tab?.bilingualMode ?? false;
	const bilingualSyncScroll = tab?.bilingualSyncScroll ?? true;
	const bilingualTranslationFile = tab?.bilingualTranslationFile;

	// Determine the "source" PDF stem so we can find its translations.
	// E.g. if pdfFilename is "s41586-025-09422-z.pdf", stem is "s41586-025-09422-z".
	// Translations are named "{stem}.{lang}.pdf".
	const sourcePdfStem = useMemo(() => {
		const fname = pdfFilename || "paper.pdf";
		return fname.replace(/\.pdf$/i, "");
	}, [pdfFilename]);

	const translationPdfs = useMemo(() => {
		if (!paper?.attachments) return [];
		const stem = sourcePdfStem;
		// Match {stem}.{lang}.pdf pattern
		const pattern = new RegExp(
			`^${stem.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\.\\w+\\.pdf$`,
			"i",
		);
		return paper.attachments.filter(
			(a) =>
				pattern.test(a.filename) && a.filename !== `${stem}.pdf` && a.is_local,
		);
	}, [paper?.attachments, sourcePdfStem]);

	// Check if the currently opened PDF is itself a translation file,
	// so we can offer switching to bilingual mode with the original.
	const isOpeningTranslation = useMemo(() => {
		if (!pdfFilename) return false;
		// A translation file matches {something}.{lang}.pdf (has at least 2 dots)
		return /^.+\.\w+\.pdf$/i.test(pdfFilename);
	}, [pdfFilename]);

	// The original PDF filename when viewing a translation
	const originalPdfFilename = useMemo(() => {
		if (!isOpeningTranslation || !pdfFilename) return undefined;
		// "paper.zh.pdf" -> "paper.pdf", "s41586-025-09422-z.zh.pdf" -> "s41586-025-09422-z.pdf"
		return pdfFilename.replace(/\.\w+\.pdf$/i, ".pdf");
	}, [isOpeningTranslation, pdfFilename]);

	const translationAnns = useLocalAnnotations(
		bilingualMode && bilingualTranslationFile ? paperId : null,
		bilingualTranslationFile || "paper.pdf",
	);

	const translationScrollRef = useRef<((h: ZoroHighlight) => void) | null>(
		null,
	);

	const handleToggleBilingual = useCallback(() => {
		if (bilingualMode) {
			updateTab(tabId, { bilingualMode: false });
			leftPanelRef.current?.expand();
			rightPanelRef.current?.expand();
		} else if (isOpeningTranslation && originalPdfFilename) {
			// Currently viewing a translation — switch to bilingual with original as left pane
			updateTab(tabId, {
				bilingualMode: true,
				pdfFilename: originalPdfFilename,
				bilingualTranslationFile: pdfFilename,
				bilingualSyncScroll: bilingualSyncScroll,
			});
			leftPanelRef.current?.collapse();
			rightPanelRef.current?.collapse();
		} else {
			const file =
				bilingualTranslationFile ?? translationPdfs[0]?.filename ?? null;
			if (file) {
				updateTab(tabId, {
					bilingualMode: true,
					bilingualTranslationFile: file,
					bilingualSyncScroll: bilingualSyncScroll,
				});
				leftPanelRef.current?.collapse();
				rightPanelRef.current?.collapse();
			}
		}
	}, [
		bilingualMode,
		bilingualTranslationFile,
		bilingualSyncScroll,
		translationPdfs,
		tabId,
		updateTab,
		leftPanelRef,
		rightPanelRef,
		isOpeningTranslation,
		originalPdfFilename,
		pdfFilename,
	]);

	const handleToggleSyncScroll = useCallback(() => {
		updateTab(tabId, { bilingualSyncScroll: !bilingualSyncScroll });
	}, [tabId, bilingualSyncScroll, updateTab]);

	const handleSelectTranslationFile = useCallback(
		(file: string) => {
			updateTab(tabId, { bilingualTranslationFile: file });
		},
		[tabId, updateTab],
	);

	useEffect(() => {
		if (paperId && !paper) {
			fetchPaper(paperId);
		}
	}, [paperId, paper, fetchPaper]);

	useEffect(() => {
		return () => {
			resetReaderState();
		};
	}, [paperId, resetReaderState]);

	// When this tab becomes active, re-sync state to the global store
	// so the side panel shows the correct data for the current tab.
	useEffect(() => {
		if (!isActive) return;
		// Clear stale state from previous tab's different mode
		if (readerMode === "html") {
			setPdfDocument(null);
		} else {
			setHtmlHeadings([]);
		}
	}, [isActive, readerMode, setHtmlHeadings, setPdfDocument]);

	const title = isFeedReaderMode
		? feedItem?.title
		: paper?.title || t("common.loading");

	const pdfUrl = isFeedReaderMode ? (feedItem?.pdf_url ?? null) : null;

	const handleToggleMode = (mode: "pdf" | "html") => {
		updateTab(tabId, { readerMode: mode });
	};

	const setPendingHtmlAnnotationScrollId = useAnnotationStore(
		(s) => s.setPendingHtmlAnnotationScrollId,
	);

	const handleNavigateToHtmlAnnotation = useCallback(
		(ann: ZoroHighlight) => {
			setPendingHtmlAnnotationScrollId(ann.id);
			updateTab(tabId, { readerMode: "html" });
		},
		[tabId, updateTab, setPendingHtmlAnnotationScrollId],
	);

	return (
		<div className="flex h-full w-full flex-col bg-background">
			<ReaderToolbar
				title={title ?? ""}
				paper={paper}
				isFeedReaderMode={isFeedReaderMode}
				readerMode={readerMode}
				pdfFilename={pdfFilename}
				isActive={isActive}
				isOpeningTranslation={isOpeningTranslation}
				onToggleMode={handleToggleMode}
				onHtmlChanged={handleHtmlChanged}
				onToggleLeftPanel={toggleLeftPanel}
				onToggleRightPanel={toggleRightPanel}
				bilingualMode={bilingualMode}
				bilingualSyncScroll={bilingualSyncScroll}
				bilingualTranslationFile={bilingualTranslationFile}
				translationPdfs={translationPdfs}
				onToggleBilingual={handleToggleBilingual}
				onToggleSyncScroll={handleToggleSyncScroll}
				onSelectTranslationFile={handleSelectTranslationFile}
			/>

			<div className="flex-1 overflow-hidden">
				<ResizablePanelGroup
					orientation="horizontal"
					groupRef={groupRef}
					defaultLayout={readerPanelLayout}
					onLayoutChanged={handlePanelLayoutChanged}
				>
					<ResizablePanel
						id="reader-left"
						panelRef={leftPanelRef}
						defaultSize="22%"
						minSize="15%"
						maxSize="35%"
						collapsible
					>
						{isFeedReaderMode ? (
							<FeedAnnotationPlaceholder />
						) : paper ? (
							<SidebarTabPanel
								side="left"
								tabIds={readerSidebarConfig.left}
								paper={paper}
								tabId={tabId}
								readerMode={readerMode}
								paperId={paperId}
								bilingualMode={bilingualMode}
								translationFile={bilingualTranslationFile}
								translationAnnotations={translationAnns.annotations}
								onDeleteTranslationAnnotation={translationAnns.deleteAnnotation}
								onUpdateTranslationAnnotation={translationAnns.updateAnnotation}
								onUpdateTranslationAnnotationType={
									translationAnns.updateAnnotationType
								}
								scrollToTranslationHighlight={translationScrollRef}
								onNavigateToHtmlAnnotation={
									readerMode !== "html"
										? handleNavigateToHtmlAnnotation
										: undefined
								}
								pluginSidebarTabs={pluginSidebarTabs}
							/>
						) : (
							<div className="flex h-full items-center justify-center text-sm text-muted-foreground">
								Loading...
							</div>
						)}
					</ResizablePanel>
					<ResizableHandle />

					<ResizablePanel id="reader-center" defaultSize="48%" minSize="30%">
						<ReaderCenterPanel
							readerMode={readerMode}
							bilingualMode={bilingualMode}
							bilingualTranslationFile={bilingualTranslationFile}
							bilingualSyncScroll={bilingualSyncScroll}
							paperId={paperId}
							pdfUrl={pdfUrl}
							pdfFilename={pdfFilename}
							isActive={isActive}
							translationAnns={translationAnns}
							translationScrollRef={translationScrollRef}
							htmlVersion={htmlVersion}
							tabId={tabId}
						/>
					</ResizablePanel>
					<ResizableHandle />

					<ResizablePanel
						id="reader-right"
						panelRef={rightPanelRef}
						defaultSize="30%"
						minSize="18%"
						maxSize="45%"
						collapsible
					>
						{isFeedReaderMode && feedItem ? (
							<FeedReaderMetadataPanel item={feedItem} />
						) : paper ? (
							<SidebarTabPanel
								side="right"
								tabIds={readerSidebarConfig.right}
								paper={paper}
								tabId={tabId}
								readerMode={readerMode}
								paperId={paperId}
								bilingualMode={bilingualMode}
								translationFile={bilingualTranslationFile}
								translationAnnotations={translationAnns.annotations}
								onDeleteTranslationAnnotation={translationAnns.deleteAnnotation}
								onUpdateTranslationAnnotation={translationAnns.updateAnnotation}
								onUpdateTranslationAnnotationType={
									translationAnns.updateAnnotationType
								}
								scrollToTranslationHighlight={translationScrollRef}
								onNavigateToHtmlAnnotation={
									readerMode !== "html"
										? handleNavigateToHtmlAnnotation
										: undefined
								}
								pluginSidebarTabs={pluginSidebarTabs}
							/>
						) : (
							<div className="flex h-full items-center justify-center text-sm text-muted-foreground">
								Loading...
							</div>
						)}
					</ResizablePanel>
				</ResizablePanelGroup>
			</div>
		</div>
	);
}

/**
 * Center panel: wraps the PDF/HTML viewer with a relative container
 * so plugin overlays can be positioned on top of the content.
 * Also emits paragraph hover and text selection events to plugins.
 */
function ReaderCenterPanel({
	readerMode,
	bilingualMode,
	bilingualTranslationFile,
	bilingualSyncScroll,
	paperId,
	pdfUrl,
	pdfFilename,
	isActive,
	translationAnns,
	translationScrollRef,
	htmlVersion,
	tabId,
}: {
	readerMode: "pdf" | "html";
	bilingualMode: boolean;
	bilingualTranslationFile?: string;
	bilingualSyncScroll: boolean;
	paperId: string | null;
	pdfUrl: string | null;
	pdfFilename?: string;
	isActive: boolean;
	translationAnns: ReturnType<typeof useLocalAnnotations>;
	translationScrollRef: React.MutableRefObject<
		((h: ZoroHighlight) => void) | null
	>;
	htmlVersion: number;
	tabId: string;
}) {
	const containerRef = useRef<HTMLDivElement>(null);
	const lastHoveredRef = useRef<number | null>(null);

	// Set up paragraph hover detection on mousemove
	useEffect(() => {
		const container = containerRef.current;
		if (!container) return;

		let hoverTimer: ReturnType<typeof setTimeout> | null = null;

		const handleMouseMove = (e: MouseEvent) => {
			// Debounce hover detection
			if (hoverTimer) clearTimeout(hoverTimer);
			hoverTimer = setTimeout(() => {
				const target = e.target as HTMLElement;

				// HTML mode: check if hovering over a paragraph in the iframe
				const iframe = container.querySelector(
					"iframe[title]",
				) as HTMLIFrameElement | null;
				if (iframe?.contentDocument) {
					const iframeRect = iframe.getBoundingClientRect();
					const iframeX = e.clientX - iframeRect.left;
					const iframeY = e.clientY - iframeRect.top;
					const el = iframe.contentDocument.elementFromPoint(iframeX, iframeY);
					if (el) {
						const paraEl = el.closest(
							"p, h1, h2, h3, h4, h5, h6, li",
						) as HTMLElement | null;
						if (paraEl) {
							const allParas = iframe.contentDocument.querySelectorAll(
								"p, h1, h2, h3, h4, h5, h6, li",
							);
							const validParas = Array.from(allParas).filter(
								(p) => (p.textContent ?? "").trim().length > 10,
							);
							const idx = validParas.indexOf(paraEl);
							if (idx >= 0 && idx !== lastHoveredRef.current) {
								lastHoveredRef.current = idx;
								emitParagraphHover({
									index: idx,
									text: (paraEl.textContent ?? "").trim(),
									element: paraEl,
								});
							}
							return;
						}
					}
				}

				// PDF mode: check textLayer
				const textLayerSpan = target.closest(".textLayer span");
				if (textLayerSpan) {
					const textLayer = textLayerSpan.closest(".textLayer");
					if (textLayer) {
						const layers = container.querySelectorAll(".textLayer");
						const idx = Array.from(layers).indexOf(textLayer);
						if (idx >= 0 && idx !== lastHoveredRef.current) {
							lastHoveredRef.current = idx;
							emitParagraphHover({
								index: idx,
								text: (textLayer.textContent ?? "").trim(),
							});
						}
						return;
					}
				}

				// Not hovering over any paragraph
				if (lastHoveredRef.current !== null) {
					lastHoveredRef.current = null;
					emitParagraphHover(null);
				}
			}, 100);
		};

		const handleMouseLeave = () => {
			if (lastHoveredRef.current !== null) {
				lastHoveredRef.current = null;
				emitParagraphHover(null);
			}
		};

		container.addEventListener("mousemove", handleMouseMove);
		container.addEventListener("mouseleave", handleMouseLeave);
		return () => {
			container.removeEventListener("mousemove", handleMouseMove);
			container.removeEventListener("mouseleave", handleMouseLeave);
			if (hoverTimer) clearTimeout(hoverTimer);
		};
	}, []);

	// Set up text selection detection
	useEffect(() => {
		const handleMouseUp = () => {
			const selection = window.getSelection();
			if (!selection || selection.isCollapsed) return;

			const text = selection.toString().trim();
			if (text.length < 5) return;

			// Try to find the paragraph index
			const anchorNode = selection.anchorNode;
			if (!anchorNode) return;

			const paraEl = (
				anchorNode.nodeType === Node.ELEMENT_NODE
					? (anchorNode as HTMLElement)
					: anchorNode.parentElement
			)?.closest("p, h1, h2, h3, h4, h5, h6, li, .textLayer");

			emitTextSelected({
				text,
				paragraphIndex: paraEl ? 0 : -1, // Approximate; plugins can refine
				startOffset: selection.anchorOffset,
				endOffset: selection.focusOffset,
			});
		};

		document.addEventListener("mouseup", handleMouseUp);
		return () => document.removeEventListener("mouseup", handleMouseUp);
	}, []);

	return (
		<div ref={containerRef} className="relative h-full w-full">
			{/* Actual reader content */}
			{readerMode === "pdf" && !bilingualMode && (
				<PdfAnnotationViewer
					paperId={paperId}
					pdfUrl={pdfUrl}
					pdfFilename={pdfFilename}
					isActive={isActive}
				/>
			)}
			{readerMode === "pdf" && bilingualMode && bilingualTranslationFile && (
				<BilingualPdfViewer
					paperId={paperId}
					pdfFilename={pdfFilename}
					translationFile={bilingualTranslationFile}
					syncScroll={bilingualSyncScroll}
					translationAnns={translationAnns}
					translationScrollRef={translationScrollRef}
					isActive={isActive}
				/>
			)}
			{readerMode === "html" && (
				<HtmlReader
					paperId={paperId ?? undefined}
					version={htmlVersion}
					isActive={isActive}
				/>
			)}

			{/* Plugin overlays — positioned absolutely on top of reader content */}
			<PluginSlot
				location="reader_overlay"
				context={{ paperId, readerMode, tabId }}
				className="absolute inset-0 pointer-events-none z-40"
			/>
		</div>
	);
}

function ReaderToolbar({
	title,
	paper,
	isFeedReaderMode,
	readerMode,
	pdfFilename,
	isActive,
	isOpeningTranslation,
	onToggleMode,
	onHtmlChanged,
	onToggleLeftPanel,
	onToggleRightPanel,
	bilingualMode,
	bilingualSyncScroll,
	bilingualTranslationFile,
	translationPdfs,
	onToggleBilingual,
	onToggleSyncScroll,
	onSelectTranslationFile,
}: {
	title: string;
	paper: PaperResponse | null;
	isFeedReaderMode: boolean;
	readerMode: "pdf" | "html";
	pdfFilename?: string;
	isActive: boolean;
	isOpeningTranslation: boolean;
	onToggleMode: (mode: "pdf" | "html") => void;
	onHtmlChanged: () => void;
	onToggleLeftPanel: () => void;
	onToggleRightPanel: () => void;
	bilingualMode: boolean;
	bilingualSyncScroll: boolean;
	bilingualTranslationFile?: string;
	translationPdfs: Array<{ filename: string; is_local: boolean }>;
	onToggleBilingual: () => void;
	onToggleSyncScroll: () => void;
	onSelectTranslationFile: (file: string) => void;
}) {
	const [showSearch, setShowSearch] = useState(false);
	const [showInkOptions, setShowInkOptions] = useState(false);
	const activeTool = useAnnotationStore((s) => s.activeTool);
	const setActiveTool = useAnnotationStore((s) => s.setActiveTool);
	const activeColor = useAnnotationStore((s) => s.activeColor);
	const { t } = useTranslation();
	const setActiveColor = useAnnotationStore((s) => s.setActiveColor);
	const inkStrokeWidth = useAnnotationStore((s) => s.inkStrokeWidth);
	const setInkStrokeWidth = useAnnotationStore((s) => s.setInkStrokeWidth);
	const inkEraserActive = useAnnotationStore((s) => s.inkEraserActive);
	const setInkEraserActive = useAnnotationStore((s) => s.setInkEraserActive);
	const navigateBack = useAnnotationStore((s) => s.navigateBack);
	const navigateForward = useAnnotationStore((s) => s.navigateForward);
	const historyIndex = useAnnotationStore((s) => s.historyIndex);
	const historyLength = useAnnotationStore((s) => s.navigationHistory.length);
	const canGoBack = historyIndex > 0;
	const canGoForward = historyIndex < historyLength - 1;
	const [showTypography, setShowTypography] = useState(false);
	const htmlReaderTypography = useUiStore((s) => s.htmlReaderTypography);
	const setHtmlReaderTypography = useUiStore((s) => s.setHtmlReaderTypography);
	const resetHtmlReaderTypography = useUiStore(
		(s) => s.resetHtmlReaderTypography,
	);

	// Ctrl+F / Cmd+F to open search (scope-aware, only fires in active tab)
	const readerSearchHandlers = useMemo(
		() => ({
			"reader.openSearch": () => {
				setShowSearch(true);
			},
		}),
		[],
	);
	useKeybindings("reader", readerSearchHandlers, { enabled: isActive });

	const toolButtons: {
		tool: ReaderTool;
		icon: typeof MousePointer2;
		label: string;
	}[] = [
		{ tool: "cursor", icon: MousePointer2, label: t("reader.select") },
		{ tool: "highlight", icon: Highlighter, label: t("reader.highlight") },
		{ tool: "underline", icon: Underline, label: t("reader.underline") },
		{ tool: "note", icon: StickyNote, label: t("reader.stickyNote") },
		{ tool: "ink", icon: Pen, label: t("reader.freehandDraw") },
	];

	return (
		<header className="flex h-12 items-center gap-2 border-b px-3 shrink-0">
			{/* Toggle left sidebar */}
			<button
				type="button"
				className="rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
				onClick={onToggleLeftPanel}
				title={t("reader.toggleLeftPanel")}
			>
				<PanelLeft className="h-4 w-4" />
			</button>

			<div className="h-5 w-px bg-border" />

			{/* Back / Forward navigation */}
			<div className="flex items-center gap-0.5">
				<button
					type="button"
					className="rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
					onClick={navigateBack}
					disabled={!canGoBack}
					title={t("reader.goBack")}
				>
					<ArrowLeft className="h-4 w-4" />
				</button>
				<button
					type="button"
					className="rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
					onClick={navigateForward}
					disabled={!canGoForward}
					title={t("reader.goForward")}
				>
					<ArrowRight className="h-4 w-4" />
				</button>
			</div>

			{/* Page navigation */}
			{readerMode === "pdf" && (
				<>
					<div className="h-5 w-px bg-border" />
					<PageNavigation />
				</>
			)}

			<div className="h-5 w-px bg-border" />

			{/* Zoom controls */}
			<ZoomControls />

			<div className="h-5 w-px bg-border" />

			{/* Annotation tools */}
			{(readerMode === "pdf" || readerMode === "html") && !isFeedReaderMode && (
				<>
					<div className="flex items-center gap-0.5 rounded-md border p-0.5">
						{toolButtons.map(({ tool, icon: Icon, label }) => (
							<button
								key={tool}
								type="button"
								className={cn(
									"rounded px-1.5 py-1 transition-colors",
									activeTool === tool
										? "bg-primary text-primary-foreground"
										: "text-muted-foreground hover:bg-muted hover:text-foreground",
								)}
								onClick={() => {
									setActiveTool(tool);
									if (tool === "ink") setShowInkOptions(true);
									else setShowInkOptions(false);
								}}
								title={label}
							>
								<Icon className="h-3.5 w-3.5" />
							</button>
						))}
					</div>

					{/* Ink tool options: color swatch + dropdown toggle */}
					{activeTool === "ink" && (
						<div className="relative">
							<button
								type="button"
								className="flex items-center gap-1 rounded-md border px-1.5 py-1 text-xs hover:bg-muted transition-colors"
								onClick={() => setShowInkOptions(!showInkOptions)}
							>
								<span
									className="h-3 w-3 rounded-sm border"
									style={{ backgroundColor: activeColor }}
								/>
								<svg
									className="h-2.5 w-2.5 text-muted-foreground"
									viewBox="0 0 10 6"
									fill="none"
								>
									<path
										d="M1 1L5 5L9 1"
										stroke="currentColor"
										strokeWidth="1.5"
										strokeLinecap="round"
									/>
								</svg>
							</button>

							{/* Ink options dropdown */}
							{showInkOptions && (
								<>
									{/* Backdrop to close dropdown */}
									<div
										className="fixed inset-0 z-40"
										onClick={() => setShowInkOptions(false)}
									/>
									<div className="absolute top-full left-0 mt-1 z-50 rounded-lg border bg-background shadow-lg p-2 w-44">
										{/* Colors */}
										<div className="space-y-0.5">
											{[
												...ANNOTATION_COLORS,
												{ name: "Black", value: "#000000" } as const,
											].map((c) => (
												<button
													key={c.value}
													type="button"
													className={cn(
														"flex items-center gap-2 w-full rounded px-2 py-1 text-xs hover:bg-muted transition-colors",
													)}
													onClick={() => {
														setActiveColor(c.value);
													}}
												>
													{activeColor === c.value && (
														<Check className="h-3 w-3 text-foreground shrink-0" />
													)}
													{activeColor !== c.value && (
														<span className="h-3 w-3 shrink-0" />
													)}
													<span
														className="h-3.5 w-3.5 rounded-sm border shrink-0"
														style={{ backgroundColor: c.value }}
													/>
													<span>{c.name}</span>
												</button>
											))}
										</div>

										<Separator className="my-1.5" />

										{/* Eraser */}
										<button
											type="button"
											className={cn(
												"flex items-center gap-2 w-full rounded px-2 py-1 text-xs hover:bg-muted transition-colors",
												inkEraserActive && "bg-muted",
											)}
											onClick={() => setInkEraserActive(!inkEraserActive)}
										>
											<svg
												className="h-3.5 w-3.5 shrink-0"
												viewBox="0 0 24 24"
												fill="none"
												stroke="currentColor"
												strokeWidth="2"
												strokeLinecap="round"
												strokeLinejoin="round"
											>
												<path d="m7 21-4.3-4.3c-1-1-1-2.5 0-3.4l9.6-9.6c1-1 2.5-1 3.4 0l5.6 5.6c1 1 1 2.5 0 3.4L13 21" />
												<path d="M22 21H7" />
												<path d="m5 11 9 9" />
											</svg>
											<span>{t("reader.eraser")}</span>
										</button>

										<Separator className="my-1.5" />

										{/* Stroke size */}
										<div className="flex items-center gap-2 px-2 py-1">
											<span className="text-[10px] text-muted-foreground whitespace-nowrap">
												{t("reader.inkSize")}:
											</span>
											<input
												type="range"
												min="1"
												max="8"
												step="0.5"
												value={inkStrokeWidth}
												onChange={(e) =>
													setInkStrokeWidth(Number.parseFloat(e.target.value))
												}
												className="flex-1 min-w-0 h-1 accent-primary"
											/>
											<span className="text-[10px] text-muted-foreground w-6 text-right shrink-0">
												{inkStrokeWidth.toFixed(1)}
											</span>
										</div>
									</div>
								</>
							)}
						</div>
					)}
				</>
			)}

			{/* Title (flexible space) */}
			<span className="flex-1 truncate text-sm font-medium text-center">
				{title}
			</span>

			{/* PDF/HTML toggle */}
			{!isFeedReaderMode && (
				<div className="flex items-center gap-1">
					{paper?.has_pdf && (
						<Button
							variant={readerMode === "pdf" ? "secondary" : "ghost"}
							size="icon"
							className="h-7 w-7"
							onClick={() => onToggleMode("pdf")}
							title="PDF"
						>
							<IconFileTypePdf className="h-4 w-4" stroke={1.5} />
						</Button>
					)}
					{paper?.has_html && (
						<Button
							variant={readerMode === "html" ? "secondary" : "ghost"}
							size="icon"
							className="h-7 w-7"
							onClick={() => onToggleMode("html")}
							title="HTML"
						>
							<Globe className="h-3.5 w-3.5" />
						</Button>
					)}
				</div>
			)}

			{/* Bilingual PDF toggle */}
			{readerMode === "pdf" &&
				!isFeedReaderMode &&
				(translationPdfs.length > 0 || isOpeningTranslation) && (
					<>
						<div className="h-5 w-px bg-border" />
						<div className="flex items-center gap-1">
							<Button
								variant={bilingualMode ? "secondary" : "ghost"}
								size="icon"
								className="h-7 w-7"
								onClick={onToggleBilingual}
								title={t("reader.bilingualPdf")}
							>
								<BookOpen className="h-3.5 w-3.5" />
							</Button>
							{bilingualMode && (
								<>
									<Button
										variant={bilingualSyncScroll ? "secondary" : "ghost"}
										size="sm"
										className="h-7 text-xs px-2"
										onClick={onToggleSyncScroll}
										title={
											bilingualSyncScroll
												? t("reader.syncScrollOn")
												: t("reader.syncScrollOff")
										}
									>
										<Link2
											className={cn(
												"h-3.5 w-3.5",
												!bilingualSyncScroll && "opacity-50",
											)}
										/>
									</Button>
									{translationPdfs.length > 1 && (
										<select
											className="h-7 rounded-md border bg-background px-1.5 text-xs outline-none"
											value={bilingualTranslationFile ?? ""}
											onChange={(e) => onSelectTranslationFile(e.target.value)}
										>
											{translationPdfs.map((a) => {
												// Extract the language part: "{stem}.{lang}.pdf" -> "{lang}"
												const match = a.filename.match(/\.(\w+)\.pdf$/i);
												const label = match ? match[1] : a.filename;
												return (
													<option key={a.filename} value={a.filename}>
														{label}
													</option>
												);
											})}
										</select>
									)}
								</>
							)}
						</div>
					</>
				)}

			{/* Translate (PDF & HTML) */}
			{!isFeedReaderMode && paper && (
				<TranslateToolbar
					paperId={paper.id}
					readerMode={readerMode}
					pdfFilename={pdfFilename}
					onHtmlChanged={onHtmlChanged}
				/>
			)}

			{isFeedReaderMode && (
				<Badge variant="outline" className="text-xs">
					{t("reader.streamingFromArxiv")}
				</Badge>
			)}

			{/* Search toggle */}
			{readerMode === "pdf" && (
				<>
					<div className="h-5 w-px bg-border" />
					{showSearch ? (
						<PdfSearchBar onClose={() => setShowSearch(false)} />
					) : (
						<button
							type="button"
							className="rounded p-1.5 text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
							onClick={() => setShowSearch(true)}
							title={t("reader.searchInPdf")}
						>
							<Search className="h-4 w-4" />
						</button>
					)}
				</>
			)}

			{/* HTML typography settings popover */}
			{readerMode === "html" && (
				<>
					<div className="h-5 w-px bg-border" />
					<div className="relative">
						<button
							type="button"
							className={cn(
								"rounded p-1.5 transition-colors",
								showTypography
									? "bg-muted text-foreground"
									: "text-muted-foreground hover:bg-muted hover:text-foreground",
							)}
							onClick={() => setShowTypography(!showTypography)}
							title={t("reader.typographySettings")}
						>
							<Type className="h-4 w-4" />
						</button>

						{showTypography && (
							<>
								{/* biome-ignore lint/a11y/useKeyWithClickEvents: backdrop overlay dismiss */}
								<div
									className="fixed inset-0 z-40"
									onClick={() => setShowTypography(false)}
								/>
								<div className="absolute top-full right-0 mt-1 z-50 rounded-lg border bg-background shadow-lg p-3 w-64 space-y-3">
									{/* Font Family */}
									<div>
										<label
											className="text-[11px] text-muted-foreground"
											htmlFor="tb-font-family"
										>
											{t("settings.fontFamily")}
										</label>
										<select
											id="tb-font-family"
											value={htmlReaderTypography.fontFamily}
											onChange={(e) =>
												setHtmlReaderTypography({
													fontFamily: e.target.value as HtmlReaderFontFamily,
												})
											}
											className="mt-0.5 h-7 w-full rounded-md border bg-transparent px-1.5 text-xs"
										>
											<option value="system">
												{t("settings.fontFamilySystem")}
											</option>
											<option value="serif">
												{t("settings.fontFamilySerif")}
											</option>
											<option value="sans-serif">
												{t("settings.fontFamilySansSerif")}
											</option>
											<option value="cjk">{t("settings.fontFamilyCjk")}</option>
											<option value="custom">
												{t("settings.fontFamilyCustom")}
											</option>
										</select>
									</div>

									{/* Custom font input */}
									{htmlReaderTypography.fontFamily === "custom" && (
										<div>
											<input
												type="text"
												value={htmlReaderTypography.customFontFamily}
												onChange={(e) =>
													setHtmlReaderTypography({
														customFontFamily: e.target.value,
													})
												}
												placeholder={t("settings.customFontFamilyPlaceholder")}
												className="h-7 w-full rounded-md border bg-transparent px-1.5 text-xs"
											/>
										</div>
									)}

									{/* Font Size */}
									<div>
										<div className="flex items-center justify-between">
											<label
												className="text-[11px] text-muted-foreground"
												htmlFor="tb-font-size"
											>
												{t("settings.fontSize")}
											</label>
											<span className="text-[11px] text-muted-foreground tabular-nums">
												{htmlReaderTypography.fontSize}px
											</span>
										</div>
										<input
											id="tb-font-size"
											type="range"
											min="12"
											max="24"
											step="1"
											value={htmlReaderTypography.fontSize}
											onChange={(e) =>
												setHtmlReaderTypography({
													fontSize: Number(e.target.value),
												})
											}
											className="w-full accent-primary mt-0.5"
										/>
									</div>

									{/* Line Height */}
									<div>
										<div className="flex items-center justify-between">
											<label
												className="text-[11px] text-muted-foreground"
												htmlFor="tb-line-height"
											>
												{t("settings.lineHeight")}
											</label>
											<span className="text-[11px] text-muted-foreground tabular-nums">
												{htmlReaderTypography.lineHeight.toFixed(1)}
											</span>
										</div>
										<input
											id="tb-line-height"
											type="range"
											min="1.2"
											max="2.4"
											step="0.1"
											value={htmlReaderTypography.lineHeight}
											onChange={(e) =>
												setHtmlReaderTypography({
													lineHeight: Number(e.target.value),
												})
											}
											className="w-full accent-primary mt-0.5"
										/>
									</div>

									{/* Font Weight */}
									<div>
										<div className="flex items-center justify-between">
											<label
												className="text-[11px] text-muted-foreground"
												htmlFor="tb-font-weight"
											>
												{t("settings.fontWeight")}
											</label>
											<span className="text-[11px] text-muted-foreground tabular-nums">
												{htmlReaderTypography.fontWeight <= 300
													? t("settings.fontWeightLight")
													: htmlReaderTypography.fontWeight >= 600
														? t("settings.fontWeightBold")
														: t("settings.fontWeightNormal")}
											</span>
										</div>
										<input
											id="tb-font-weight"
											type="range"
											min="300"
											max="700"
											step="100"
											value={htmlReaderTypography.fontWeight}
											onChange={(e) =>
												setHtmlReaderTypography({
													fontWeight: Number(e.target.value),
												})
											}
											className="w-full accent-primary mt-0.5"
										/>
									</div>

									{/* Max Content Width */}
									<div>
										<div className="flex items-center justify-between">
											<label
												className="text-[11px] text-muted-foreground"
												htmlFor="tb-max-width"
											>
												{t("settings.maxContentWidth")}
											</label>
											<span className="text-[11px] text-muted-foreground tabular-nums">
												{htmlReaderTypography.maxWidth === 0
													? t("settings.maxContentWidthUnlimited")
													: `${htmlReaderTypography.maxWidth}px`}
											</span>
										</div>
										<input
											id="tb-max-width"
											type="range"
											min="0"
											max="1200"
											step="50"
											value={htmlReaderTypography.maxWidth}
											onChange={(e) =>
												setHtmlReaderTypography({
													maxWidth: Number(e.target.value),
												})
											}
											className="w-full accent-primary mt-0.5"
										/>
									</div>

									{/* Reset */}
									<button
										type="button"
										className="flex items-center gap-1.5 w-full rounded-md px-2 py-1 text-xs text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
										onClick={() => {
											resetHtmlReaderTypography();
										}}
									>
										<RotateCcw className="h-3 w-3" />
										{t("reader.resetToDefault")}
									</button>
								</div>
							</>
						)}
					</div>
				</>
			)}

			<div className="h-5 w-px bg-border" />

			{/* Toggle right sidebar */}
			<button
				type="button"
				className="rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
				onClick={onToggleRightPanel}
				title={t("reader.toggleRightPanel")}
			>
				<PanelRight className="h-4 w-4" />
			</button>
		</header>
	);
}

/** Unified translate toolbar for both PDF and HTML modes */
function TranslateToolbar({
	paperId,
	readerMode,
	pdfFilename,
	onHtmlChanged,
}: {
	paperId: string;
	readerMode: "pdf" | "html";
	pdfFilename?: string;
	onHtmlChanged: () => void;
}) {
	const { t } = useTranslation();
	const openTab = useTabStore((s) => s.openTab);

	// PDF translation state from global store (survives spawn-and-return)
	const pdfTranslating = useTranslationStore((s) =>
		s.isPdfTranslating(paperId),
	);
	const setPdfTranslating = useTranslationStore((s) => s.setPdfTranslating);

	const startHtmlTranslation = useTranslationStore(
		(s) => s.startHtmlTranslation,
	);
	const htmlTranslating = useTranslationStore((s) =>
		s.isHtmlTranslating(paperId),
	);
	const progress = useTranslationStore((s) =>
		s.getHtmlTranslationProgress(paperId),
	);
	const registerComplete = useTranslationStore(
		(s) => s.registerHtmlTranslationComplete,
	);
	const unregisterComplete = useTranslationStore(
		(s) => s.unregisterHtmlTranslationComplete,
	);
	const syncActive = useTranslationStore((s) => s.syncActiveHtmlTranslations);

	useEffect(() => {
		syncActive();
	}, [syncActive]);

	useEffect(() => {
		registerComplete(paperId, onHtmlChanged);
		return () => unregisterComplete(paperId);
	}, [paperId, onHtmlChanged, registerComplete, unregisterComplete]);

	const handleTranslate = async () => {
		if (readerMode === "html") {
			startHtmlTranslation(paperId);
		} else {
			// Mark as translating immediately; the store state will be kept
			// alive by background-task events from Rust even after spawn returns.
			setPdfTranslating(paperId, true);
			try {
				await commands.translatePdf(paperId, pdfFilename);
			} catch (err) {
				const msg = String(err);
				console.error("Failed to start PDF translation:", msg);
				setPdfTranslating(paperId, false);
				// Detect BabelDOC / configuration issues and offer to open settings
				const isConfigIssue =
					msg.includes("not enabled") ||
					msg.includes("not configured") ||
					msg.includes("BabelDOC") ||
					msg.includes("babeldoc") ||
					msg.includes("Native language") ||
					msg.includes("API key") ||
					msg.includes("base URL") ||
					msg.includes("model");
				if (isConfigIssue) {
					const goToSettings = confirm(
						`${msg}\n\n${t("reader.openSettingsPrompt")}`,
					);
					if (goToSettings) {
						openTab({
							id: "settings",
							type: "settings",
							title: t("settings.title"),
						});
					}
				}
			}
		}
	};

	const translating = readerMode === "html" ? htmlTranslating : pdfTranslating;

	const translateTitle = (() => {
		if (!translating) {
			return readerMode === "html"
				? t("reader.translateBilingual")
				: t("reader.translatePdf");
		}
		const label = t("paper.translating");
		if (readerMode === "html" && progress && progress.total > 0) {
			return `${label} ${progress.done}/${progress.total}`;
		}
		return label;
	})();

	return (
		<>
			<div className="h-5 w-px bg-border" />
			<Button
				variant="ghost"
				size="icon"
				className="h-7 w-7"
				onClick={handleTranslate}
				disabled={translating}
				title={translateTitle}
			>
				{translating ? (
					<Loader2 className="h-3.5 w-3.5 animate-spin" />
				) : (
					<Languages className="h-3.5 w-3.5" />
				)}
			</Button>
		</>
	);
}

/** Build a CSS font-family string from the typography settings. */
function getTypographyFontFamily(typo: HtmlReaderTypography): string {
	switch (typo.fontFamily) {
		case "serif":
			return 'Georgia, "Times New Roman", serif';
		case "sans-serif":
			return '"Inter", "Segoe UI", system-ui, sans-serif';
		case "cjk":
			return '"Noto Sans SC", "PingFang SC", "Microsoft YaHei", sans-serif';
		case "custom":
			return typo.customFontFamily || "inherit";
		default:
			return "";
	}
}

/** Build the full typography CSS to inject into the HTML reader iframe. */
function buildTypographyCss(typo: HtmlReaderTypography): string {
	const rules: string[] = [];
	const ff = getTypographyFontFamily(typo);
	if (ff) {
		rules.push(`font-family: ${ff} !important;`);
	}
	if (typo.fontSize !== 16) {
		rules.push(`font-size: ${typo.fontSize}px !important;`);
	}
	if (typo.lineHeight !== 1.6) {
		rules.push(`line-height: ${typo.lineHeight} !important;`);
	}
	if (typo.fontWeight !== 400) {
		rules.push(`font-weight: ${typo.fontWeight} !important;`);
	}
	if (rules.length === 0 && typo.maxWidth === 800) return "";

	let css = "";
	if (rules.length > 0) {
		css += `body, body * { ${rules.join(" ")} }\n`;
		// Preserve monospace for code blocks
		css +=
			"pre, code, kbd, samp, .ltx_listing, pre *, code * { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace !important; }\n";
	}
	if (typo.maxWidth > 0) {
		css += `body > .ltx_page_main, body > main, body > article, body > .content, body { max-width: ${typo.maxWidth}px !important; margin-left: auto !important; margin-right: auto !important; }\n`;
	}
	return css;
}

function HtmlReader({
	paperId,
	version,
	isActive = true,
}: { paperId?: string; version?: number; isActive?: boolean }) {
	const { t } = useTranslation();
	const [htmlContent, setHtmlContent] = useState<string | null>(null);
	const [htmlBasePath, setHtmlBasePath] = useState<string | null>(null);
	const [error, setError] = useState<string | null>(null);
	const [loading, setLoading] = useState(false);
	const iframeRef = useRef<HTMLIFrameElement>(null);
	const iframeReadyRef = useRef(false);
	// Cache headings locally so we can re-push them to the global store
	// when this tab becomes active again.
	const localHtmlHeadingsRef = useRef<unknown[]>([]);
	const isDark = useIsDarkMode();
	const htmlReaderTypography = useUiStore((s) => s.htmlReaderTypography);
	const bilingualLayout = useUiStore((s) => s.bilingualLayout);
	const zoomLevel = useAnnotationStore((s) => s.zoomLevel);
	const annotations = useAnnotationStore((s) => s.annotations);
	const activeTool = useAnnotationStore((s) => s.activeTool);
	const activeColor = useAnnotationStore((s) => s.activeColor);
	const inkStrokeWidth = useAnnotationStore((s) => s.inkStrokeWidth);
	const inkEraserActive = useAnnotationStore((s) => s.inkEraserActive);
	const fetchAnnotations = useAnnotationStore((s) => s.fetchAnnotations);
	const addHtmlAnnotation = useAnnotationStore((s) => s.addHtmlAnnotation);
	const addHtmlInkAnnotation = useAnnotationStore(
		(s) => s.addHtmlInkAnnotation,
	);
	const deleteAnnotation = useAnnotationStore((s) => s.deleteAnnotation);
	const setScrollToHighlight = useAnnotationStore(
		(s) => s.setScrollToHighlight,
	);

	// Popup state for selection toolbar and highlight popup
	const [selectionPopup, setSelectionPopup] = useState<{
		x: number;
		y: number;
		position: unknown;
		selectedText: string;
	} | null>(null);
	const [highlightPopup, setHighlightPopup] = useState<{
		x: number;
		y: number;
		annotationId: string;
	} | null>(null);

	const closePopups = useCallback(() => {
		setSelectionPopup(null);
		setHighlightPopup(null);
	}, []);

	// Cmd/Ctrl+C support: the iframe selection is inaccessible from the parent
	// when focus is on the popup toolbar. Store selected text and intercept copy.
	const lastHtmlSelectedTextRef = useRef<string | null>(null);

	useEffect(() => {
		const handleKeyDown = (e: KeyboardEvent) => {
			if (!(e.metaKey || e.ctrlKey) || e.key !== "c") return;
			const target = e.target as HTMLElement | null;
			const inInput =
				target?.tagName === "INPUT" ||
				target?.tagName === "TEXTAREA" ||
				target?.isContentEditable;
			if (inInput) return;

			const text = lastHtmlSelectedTextRef.current;
			if (!text) return;
			e.preventDefault();
			navigator.clipboard.writeText(text);
		};
		document.addEventListener("keydown", handleKeyDown);
		return () => document.removeEventListener("keydown", handleKeyDown);
	}, []);

	const postToIframe = useCallback((message: Record<string, unknown>) => {
		iframeRef.current?.contentWindow?.postMessage(message, "*");
	}, []);

	// Close popups when tool changes
	useEffect(() => {
		closePopups();
	}, [activeTool, closePopups]);

	// When this tab becomes active, re-push the cached headings to the global store
	// so the outline panel shows the correct headings for this tab.
	useEffect(() => {
		if (isActive && localHtmlHeadingsRef.current.length > 0) {
			// biome-ignore format: import() type assertion must stay on one line for TS
			useAnnotationStore.getState().setHtmlHeadings(localHtmlHeadingsRef.current as import("@/stores/annotationStore").HtmlHeadingItem[]);
		}
	}, [isActive]);

	// Fetch annotations on mount and when this tab becomes active
	useEffect(() => {
		if (paperId && isActive) fetchAnnotations(paperId);
	}, [paperId, isActive, fetchAnnotations]);

	// Set scrollToHighlight for side panel navigation
	useEffect(() => {
		const scrollFn = (highlight: ZoroHighlight) => {
			postToIframe({ type: "zr-html-scroll-to", annotationId: highlight.id });
		};
		setScrollToHighlight(scrollFn);
		return () => setScrollToHighlight(null);
	}, [setScrollToHighlight, postToIframe]);

	// Set scrollToHtmlHeading for outline panel navigation
	const setScrollToHtmlHeading = useAnnotationStore(
		(s) => s.setScrollToHtmlHeading,
	);
	useEffect(() => {
		const scrollFn = (headingId: string) => {
			postToIframe({ type: "zr-html-scroll-to-heading", headingId });
		};
		setScrollToHtmlHeading(scrollFn);
		return () => setScrollToHtmlHeading(null);
	}, [setScrollToHtmlHeading, postToIframe]);

	// Handle pending HTML citation jump from notes panel
	const pendingHtmlCitationJump = useAnnotationStore(
		(s) => s.pendingHtmlCitationJump,
	);
	useEffect(() => {
		if (!pendingHtmlCitationJump || !iframeReadyRef.current) return;
		postToIframe({
			type: "zr-html-scroll-to-position",
			position: JSON.parse(pendingHtmlCitationJump),
		});
		useAnnotationStore.getState().setPendingHtmlCitationJump(null);
	}, [pendingHtmlCitationJump, postToIframe]);

	// Handle pending HTML annotation scroll (from side panel when switching from PDF to HTML mode)
	const pendingHtmlAnnotationScrollId = useAnnotationStore(
		(s) => s.pendingHtmlAnnotationScrollId,
	);
	useEffect(() => {
		if (!pendingHtmlAnnotationScrollId || !iframeReadyRef.current) return;
		setTimeout(() => {
			postToIframe({
				type: "zr-html-scroll-to",
				annotationId: pendingHtmlAnnotationScrollId,
			});
			useAnnotationStore.getState().setPendingHtmlAnnotationScrollId(null);
		}, 300);
	}, [pendingHtmlAnnotationScrollId, postToIframe]);

	// Listen for incremental translation insertions and inject into iframe
	useEffect(() => {
		if (!paperId) return;
		const unlistenPromise = listen<{
			paperId: string;
			tag: string;
			originalTextSnippet: string;
			translationHtml: string;
		}>("html-translation-insert", (event) => {
			if (event.payload.paperId !== paperId) return;
			postToIframe({
				type: "zr-html-insert-translation",
				tag: event.payload.tag,
				originalTextSnippet: event.payload.originalTextSnippet,
				translationHtml: event.payload.translationHtml,
			});
		});
		return () => {
			unlistenPromise.then((fn) => fn());
		};
	}, [paperId, postToIframe]);

	// Send tool state to iframe whenever it changes
	useEffect(() => {
		if (!iframeReadyRef.current) return;
		postToIframe({
			type: "zr-html-set-tool",
			tool: activeTool,
			color: activeColor,
			inkStrokeWidth,
			inkEraserActive,
		});
	}, [activeTool, activeColor, inkStrokeWidth, inkEraserActive, postToIframe]);

	// Sync display mode (original / bilingual / translated) to iframe
	const displayMode = useTranslationStore((s) => s.displayMode);
	useEffect(() => {
		if (!iframeReadyRef.current) return;
		postToIframe({ type: "zr-html-set-display-mode", mode: displayMode });
	}, [displayMode, postToIframe]);

	// Sync bilingual layout (interleaved / side-by-side) to iframe
	useEffect(() => {
		if (!iframeReadyRef.current) return;
		postToIframe({ type: "zr-html-set-bilingual-layout", layout: bilingualLayout });
	}, [bilingualLayout, postToIframe]);

	// Send annotations to iframe whenever they change
	useEffect(() => {
		if (!iframeReadyRef.current) return;
		const htmlAnns = annotations
			.filter((a) => {
				const pos = a.position as unknown as Record<string, unknown>;
				return pos?.format === "html";
			})
			.map((a) => ({
				id: a.id,
				type: a.type,
				color: a.color,
				comment: a.comment.text,
				position: a.position,
			}));
		postToIframe({
			type: "zr-html-restore-annotations",
			annotations: htmlAnns,
		});
	}, [annotations, postToIframe]);

	// Apply store zoomLevel as CSS zoom to iframe content
	useEffect(() => {
		const doc = iframeRef.current?.contentDocument;
		if (doc?.documentElement) {
			doc.documentElement.style.zoom = String(zoomLevel);
		}
	}, [zoomLevel]);

	const applyDarkModeToIframe = useCallback((dark: boolean) => {
		const doc = iframeRef.current?.contentDocument;
		if (!doc) return;

		let style = doc.getElementById("zr-dark-mode") as HTMLStyleElement | null;
		if (!style) {
			style = doc.createElement("style");
			style.id = "zr-dark-mode";
			(doc.head || doc.documentElement).appendChild(style);
		}

		if (!dark) {
			style.textContent = "";
			doc.documentElement.classList.remove("zr-dark");
			return;
		}

		doc.documentElement.classList.add("zr-dark");
		style.textContent = `
			:root, html {
				background-color: #1b1b1f !important;
				color-scheme: dark !important;
			}
			body {
				background-color: #1b1b1f !important;
				color: #d4d4d8 !important;
			}
			*, *::before, *::after {
				color: #d4d4d8 !important;
				border-color: #3f3f46 !important;
				text-decoration-color: #666 !important;
			}
			a, a:link, a:visited {
				color: #7dd3fc !important;
			}
			div, section, article, main, aside, nav, header, footer,
			figure, details, blockquote, ol, ul, dl, p,
			span, b, i, em, strong, small, label, legend,
			figcaption, caption, summary,
			h1, h2, h3, h4, h5, h6, li, dt, dd, table, tr, td {
				background-color: transparent !important;
			}
			pre, code, kbd, samp, .ltx_listing {
				background-color: #262630 !important;
			}
			th {
				background-color: #262630 !important;
			}
			hr {
				background-color: #3f3f46 !important;
			}
			input, textarea, select, button {
				background-color: #262630 !important;
			}
			/* MathJax v2 & v3 */
			.MathJax path, .MathJax rect, .MathJax line,
			.MathJax_SVG path, .MathJax_SVG rect,
			mjx-container path, mjx-container rect, mjx-container line {
				fill: currentColor !important;
			}
			/* Scrollbar */
			html {
				scrollbar-color: #555 #1b1b1f;
			}
			::-webkit-scrollbar { background: #1b1b1f; width: 8px; }
			::-webkit-scrollbar-thumb { background: #555; border-radius: 4px; }
			::-webkit-scrollbar-thumb:hover { background: #666; }
			/* Translation blocks */
			.zr-translation-block {
				border-color: #4b5563 !important;
			}
			.zr-translation-block,
			.zr-translation-block[data-zotero-translation] * {
				color: #c8cfd8 !important;
			}
			/* Ink SVG stays as-is */
			.zr-ink-svg, .zr-ink-svg * {
				color: initial !important;
			}
		`;
	}, []);

	const applyTypographyToIframe = useCallback((typo: HtmlReaderTypography) => {
		const doc = iframeRef.current?.contentDocument;
		if (!doc) return;

		let style = doc.getElementById("zr-typography") as HTMLStyleElement | null;
		if (!style) {
			style = doc.createElement("style");
			style.id = "zr-typography";
			(doc.head || doc.documentElement).appendChild(style);
		}

		style.textContent = buildTypographyCss(typo);
	}, []);

	useEffect(() => {
		applyDarkModeToIframe(isDark);
	}, [isDark, applyDarkModeToIframe]);

	useEffect(() => {
		applyTypographyToIframe(htmlReaderTypography);
	}, [htmlReaderTypography, applyTypographyToIframe]);

	const handleIframeLoad = useCallback(() => {
		const doc = iframeRef.current?.contentDocument;
		if (doc?.documentElement) {
			doc.documentElement.style.zoom = String(
				useAnnotationStore.getState().zoomLevel,
			);
		}
		applyDarkModeToIframe(isDark);
		applyTypographyToIframe(useUiStore.getState().htmlReaderTypography);
	}, [applyDarkModeToIframe, applyTypographyToIframe, isDark]);

	// Listen for messages from the iframe
	useEffect(() => {
		if (!paperId) return;
		const handler = async (event: MessageEvent) => {
			const data = event.data;
			if (!data?.type) return;

			switch (data.type) {
				case "zr-html-annotation-ready": {
					iframeReadyRef.current = true;
					// Send current tool state
					const state = useAnnotationStore.getState();
					postToIframe({
						type: "zr-html-set-tool",
						tool: state.activeTool,
						color: state.activeColor,
						inkStrokeWidth: state.inkStrokeWidth,
						inkEraserActive: state.inkEraserActive,
					});
					// Send existing annotations
					const htmlAnns = state.annotations
						.filter((a) => {
							const pos = a.position as unknown as Record<string, unknown>;
							return pos?.format === "html";
						})
						.map((a) => ({
							id: a.id,
							type: a.type,
							color: a.color,
							comment: a.comment.text,
							position: a.position,
						}));
					postToIframe({
						type: "zr-html-restore-annotations",
						annotations: htmlAnns,
					});
					// Consume pending annotation scroll (set when switching from PDF to HTML mode)
					const pendingScrollId = state.pendingHtmlAnnotationScrollId;
					if (pendingScrollId) {
						setTimeout(() => {
							postToIframe({
								type: "zr-html-scroll-to",
								annotationId: pendingScrollId,
							});
							useAnnotationStore
								.getState()
								.setPendingHtmlAnnotationScrollId(null);
						}, 500);
					}
					// Apply zoom and dark mode here because onLoad may not fire
					// reliably when using srcDoc (load event can fire synchronously
					// before React attaches the listener)
					const iframeDoc = iframeRef.current?.contentDocument;
					if (iframeDoc?.documentElement) {
						iframeDoc.documentElement.style.zoom = String(state.zoomLevel);
					}
					applyDarkModeToIframe(
						document.documentElement.classList.contains("dark"),
					);
					applyTypographyToIframe(useUiStore.getState().htmlReaderTypography);
					// Send current display mode so iframe hides/shows translation blocks
					postToIframe({
						type: "zr-html-set-display-mode",
						mode: useTranslationStore.getState().displayMode,
					});
					// Send current bilingual layout
					postToIframe({
						type: "zr-html-set-bilingual-layout",
						layout: useUiStore.getState().bilingualLayout,
					});
					break;
				}

				case "zr-html-selection": {
					closePopups();
					lastHtmlSelectedTextRef.current = data.selectedText ?? null;
					if (data.tool === "cursor") {
						// Cursor mode: show the annotation toolbar popup
						const cr = data.clientRect;
						if (cr) {
							setSelectionPopup({
								x: cr.left,
								y: cr.top + cr.height + 4,
								position: data.position,
								selectedText: data.selectedText,
							});
						}
					} else {
						// Highlight/underline tool: auto-apply
						const positionJson = JSON.stringify(data.position);
						const result = await addHtmlAnnotation(
							paperId,
							data.tool as AnnotationType,
							useAnnotationStore.getState().activeColor,
							positionJson,
							data.selectedText,
							null,
						);
						if (result) {
							postToIframe({
								type: "zr-html-add-highlight",
								annotation: {
									id: result.id,
									type: result.type,
									color: result.color,
									comment: result.comment.text,
									position: result.position,
								},
							});
						}
					}
					break;
				}

				case "zr-html-annotation-click": {
					closePopups();
					const cr = data.clientRect;
					if (cr && data.annotationId) {
						setHighlightPopup({
							x: cr.left,
							y: cr.top + cr.height + 4,
							annotationId: data.annotationId,
						});
					}
					break;
				}

				case "zr-html-click-empty": {
					closePopups();
					lastHtmlSelectedTextRef.current = null;
					break;
				}

				case "zr-html-note-request": {
					const positionJson = JSON.stringify(data.position);
					const result = await addHtmlAnnotation(
						paperId,
						"note",
						useAnnotationStore.getState().activeColor,
						positionJson,
						data.selectedText,
						null,
					);
					if (result) {
						postToIframe({
							type: "zr-html-add-highlight",
							annotation: {
								id: result.id,
								type: result.type,
								color: result.color,
								comment: result.comment.text,
								position: result.position,
							},
						});
					}
					break;
				}

				case "zr-html-ink-stroke": {
					await addHtmlInkAnnotation(paperId, data.color, {
						strokes: [data.stroke],
						boundingRect: data.boundingRect,
						contentHeight: data.contentHeight,
					});
					break;
				}

				case "zr-html-ink-erase": {
					await deleteAnnotation(data.annotationId, paperId);
					break;
				}

				case "zr-translation-edit": {
					if (
						typeof data.blockIndex === "number" &&
						typeof data.newText === "string"
					) {
						try {
							await commands.saveHtmlTranslationEdit(
								paperId,
								data.blockIndex,
								data.newText,
							);
						} catch (e) {
							console.error("Failed to save translation edit:", e);
						}
					}
					break;
				}

				case "zr-open-url": {
					if (typeof data.url === "string") {
						try {
							const { open } = await import("@tauri-apps/plugin-shell");
							await open(data.url);
						} catch (e) {
							console.error("Failed to open URL:", e);
						}
					}
					break;
				}

				case "zr-zoom-wheel": {
					if (typeof data.delta === "number") {
						const { zoomLevel: z, setZoomLevel: sz } =
							useAnnotationStore.getState();
						sz(z * (1 - data.delta * 0.002));
					}
					break;
				}

				case "zr-html-headings": {
					if (Array.isArray(data.headings)) {
						localHtmlHeadingsRef.current = data.headings;
						useAnnotationStore.getState().setHtmlHeadings(data.headings);
					}
					break;
				}
			}
		};
		window.addEventListener("message", handler);
		return () => {
			window.removeEventListener("message", handler);
			iframeReadyRef.current = false;
		};
	}, [
		paperId,
		postToIframe,
		addHtmlAnnotation,
		addHtmlInkAnnotation,
		deleteAnnotation,
		closePopups,
		applyDarkModeToIframe,
		applyTypographyToIframe,
	]);

	useEffect(() => {
		if (!paperId) return;
		let cancelled = false;
		setLoading(true);
		setError(null);
		setHtmlContent(null);

		(async () => {
			try {
				const htmlPath = await commands.getPaperHtmlPath(paperId);
				const data = await readFile(htmlPath);
				if (cancelled) return;
				const text = new TextDecoder().decode(data);
				// Extract directory path for resolving relative image paths
				const dirPath = htmlPath.substring(0, htmlPath.lastIndexOf("/") + 1);
				setHtmlBasePath(dirPath);
				setHtmlContent(text);
			} catch (e) {
				if (!cancelled) {
					setError(String(e));
				}
			} finally {
				if (!cancelled) {
					setLoading(false);
				}
			}
		})();

		return () => {
			cancelled = true;
		};
	}, [paperId, version]);

	if (loading) {
		return (
			<div className="flex h-full items-center justify-center text-muted-foreground">
				<Loader2 className="h-6 w-6 animate-spin" />
			</div>
		);
	}

	if (error) {
		return (
			<div className="flex h-full items-center justify-center text-muted-foreground">
				<div className="text-center">
					<Globe className="mx-auto mb-4 h-16 w-16" />
					<p className="text-sm">{error}</p>
				</div>
			</div>
		);
	}

	if (htmlContent) {
		const editScript = `
<script>
(function() {
  document.addEventListener('click', function(e) {
    var link = e.target.closest('a[href]');
    if (!link) return;
    var href = link.getAttribute('href') || '';
    if (href.startsWith('#')) {
      e.preventDefault();
      var id = href.slice(1);
      var target = document.getElementById(id) || document.querySelector('[name="' + id + '"]');
      if (target) target.scrollIntoView({ behavior: 'smooth' });
      return;
    }
    e.preventDefault();
    if (href.startsWith('http://') || href.startsWith('https://')) {
      window.parent.postMessage({ type: 'zr-open-url', url: href }, '*');
    }
  }, true);

  var inGesture = false;
  var pzoom = 1;
  var lockedWidth = 0;
  var anchorCx = 0, anchorCy = 0;
  var ptrX = 0, ptrY = 0;
  var _bilingualLayout = 'interleaved';

  function applyPinchZoom() {
    var b = document.body;
    if (pzoom === 1) {
      b.style.transform = '';
      b.style.transformOrigin = '';
      if (lockedWidth) { b.style.width = ''; lockedWidth = 0; }
      return;
    }
    if (!lockedWidth) {
      lockedWidth = b.offsetWidth;
      b.style.width = lockedWidth + 'px';
    }
    b.style.transformOrigin = '0 0';
    b.style.transform = 'scale(' + pzoom + ')';
  }

  document.addEventListener('wheel', function(e) {
    if (!e.ctrlKey && !e.metaKey) return;
    e.preventDefault();
    if (inGesture) return;
    window.parent.postMessage({ type: 'zr-zoom-wheel', delta: e.deltaY }, '*');
  }, { passive: false });
  var gBase = 1;
  document.addEventListener('gesturestart', function(e) {
    e.preventDefault();
    inGesture = true;
    gBase = pzoom;
    ptrX = e.clientX || 0;
    ptrY = e.clientY || 0;
    anchorCx = (ptrX + window.scrollX) / (pzoom || 1);
    anchorCy = (ptrY + window.scrollY) / (pzoom || 1);
  }, { passive: false });
  document.addEventListener('gesturechange', function(e) {
    e.preventDefault();
    if (e.clientX != null) { ptrX = e.clientX; ptrY = e.clientY; }
    pzoom = Math.max(0.25, Math.min(5, gBase * e.scale));
    applyPinchZoom();
    if (pzoom !== 1) {
      window.scrollTo(anchorCx * pzoom - ptrX, anchorCy * pzoom - ptrY);
    }
  }, { passive: false });
  document.addEventListener('gestureend', function(e) {
    e.preventDefault();
    setTimeout(function() { inGesture = false; }, 100);
  }, { passive: false });

  // Display mode control: show/hide translation blocks and original text
  window.addEventListener('message', function(e) {
    var data = e.data;
    if (!data || data.type !== 'zr-html-set-display-mode') return;
    var mode = data.mode; // 'original' | 'bilingual' | 'translated'
    var styleId = 'zr-display-mode-style';
    var style = document.getElementById(styleId);
    if (!style) {
      style = document.createElement('style');
      style.id = styleId;
      (document.head || document.documentElement).appendChild(style);
    }
    if (mode === 'original') {
      style.textContent = '.zr-translation-block { display: none !important; }';
    } else if (mode === 'translated') {
      style.textContent = '[data-zotero-translation="true"]:not(.zr-translation-block) { display: none !important; }';
    } else {
      style.textContent = '';
    }
  });

  window.addEventListener('message', function(e) {
    var data = e.data;
    if (!data || data.type !== 'zr-html-insert-translation') return;
    var tag = data.tag;
    var snippet = data.originalTextSnippet;
    var html = data.translationHtml;
    if (!document.getElementById('zotero-arxiv-translation-style')) {
      var style = document.createElement('style');
      style.id = 'zotero-arxiv-translation-style';
      style.textContent = '.zr-translation-block { border: 1.5px dashed #9ca3af; border-radius: 6px; padding: 8px 12px; margin-top: 6px; margin-bottom: 4px; } .zr-translation-block, .zr-translation-block[data-zotero-translation] * { color: #374151; } html.zr-dark .zr-translation-block { border-color: #4b5563; } html.zr-dark .zr-translation-block, html.zr-dark .zr-translation-block[data-zotero-translation] * { color: #c8cfd8 !important; }';
      (document.head || document.documentElement).appendChild(style);
    }
    var elements = document.querySelectorAll(tag);
    for (var i = 0; i < elements.length; i++) {
      var el = elements[i];
      if (el.getAttribute('data-zotero-translation') === 'true') continue;
      if (el.classList.contains('zr-translation-block')) continue;
      var text = (el.textContent || '').replace(/\\s+/g, ' ').trim();
      var norm = snippet.replace(/\\s+/g, ' ').trim();
      if (text.indexOf(norm) < 0) continue;
      el.setAttribute('data-zotero-translation', 'true');
      var temp = document.createElement('div');
      temp.innerHTML = html;
      var newEl = temp.firstElementChild;
      if (newEl && el.parentNode) {
        el.parentNode.insertBefore(newEl, el.nextSibling);
        if (_bilingualLayout === 'side-by-side') {
          _wrapPair(el, newEl);
        }
      }
      break;
    }
  });

  // Helper: wrap an original+translation pair in a flex container
  function _wrapPair(origEl, transEl) {
    if (origEl.parentNode && origEl.parentNode.classList && origEl.parentNode.classList.contains('zr-bilingual-pair')) return;
    var wrapper = document.createElement('div');
    wrapper.className = 'zr-bilingual-pair';
    origEl.parentNode.insertBefore(wrapper, origEl);
    wrapper.appendChild(origEl);
    wrapper.appendChild(transEl);
  }

  // Bilingual layout: side-by-side vs interleaved
  window.addEventListener('message', function(e) {
    var data = e.data;
    if (!data || data.type !== 'zr-html-set-bilingual-layout') return;
    var layout = data.layout;
    _bilingualLayout = layout;

    var styleId = 'zr-bilingual-layout-style';
    var style = document.getElementById(styleId);
    if (!style) {
      style = document.createElement('style');
      style.id = styleId;
      (document.head || document.documentElement).appendChild(style);
    }

    if (layout === 'side-by-side') {
      style.textContent = '.zr-bilingual-pair { display: flex; gap: 16px; align-items: flex-start; } .zr-bilingual-pair > * { flex: 1; min-width: 0; } .zr-bilingual-pair .zr-translation-block { margin-top: 0; }';
      // Wrap existing pairs
      var originals = document.querySelectorAll('[data-zotero-translation="true"]:not(.zr-translation-block)');
      for (var i = 0; i < originals.length; i++) {
        var orig = originals[i];
        if (orig.parentNode && orig.parentNode.classList && orig.parentNode.classList.contains('zr-bilingual-pair')) continue;
        var trans = orig.nextElementSibling;
        if (trans && trans.classList.contains('zr-translation-block')) {
          _wrapPair(orig, trans);
        }
      }
    } else {
      style.textContent = '';
      // Unwrap all pairs
      var pairs = document.querySelectorAll('.zr-bilingual-pair');
      for (var j = 0; j < pairs.length; j++) {
        var pair = pairs[j];
        var parent = pair.parentNode;
        while (pair.firstChild) {
          parent.insertBefore(pair.firstChild, pair);
        }
        parent.removeChild(pair);
      }
    }
  });

  document.addEventListener('dblclick', function(e) {
    var block = e.target.closest('.zr-translation-block');
    if (!block || block.querySelector('textarea')) return;
    var blocks = Array.from(document.querySelectorAll('.zr-translation-block'));
    var index = blocks.indexOf(block);
    if (index < 0) return;
    var original = block.textContent || '';
    block.textContent = '';
    var ta = document.createElement('textarea');
    ta.value = original;
    ta.style.cssText = 'width:100%;min-height:120px;box-sizing:border-box;font-size:0.95em;line-height:1.5;padding:6px;border:2px solid #4a6cf7;border-radius:4px;';
    block.appendChild(ta);
    ta.focus();
    ta.select();
    var saved = false;
    function commit() {
      if (saved) return;
      saved = true;
      var newText = ta.value.trim() || original;
      block.textContent = newText;
      window.parent.postMessage({ type: 'zr-translation-edit', blockIndex: index, newText: newText }, '*');
    }
    ta.addEventListener('keydown', function(ev) {
      if ((ev.ctrlKey || ev.metaKey) && ev.key === 'Enter') { ev.preventDefault(); commit(); }
      if (ev.key === 'Escape') { ev.preventDefault(); block.textContent = original; saved = true; }
    });
    ta.addEventListener('blur', function() { commit(); });
  }, true);
})();
</script>`;

		const annotationScript = getHtmlAnnotationScript();

		// Resolve relative image/source paths to local asset URLs so that
		// images that were not inlined as base64 during fetch can still load.
		let resolvedHtml = htmlContent;
		if (htmlBasePath) {
			resolvedHtml = resolvedHtml.replace(
				/(<(?:img|source)\b[^>]*\b(?:src|srcset))\s*=\s*(["'])(?!data:|https?:\/\/|\/\/|blob:)([^"']+)\2/gi,
				(_match, prefix, quote, relPath) => {
					const absPath = htmlBasePath + relPath;
					const assetUrl = convertFileSrc(absPath);
					return `${prefix}=${quote}${assetUrl}${quote}`;
				},
			);
		}

		const injected = resolvedHtml.replace(
			"</body>",
			`${editScript}${annotationScript}</body>`,
		);

		return (
			<div className="h-full w-full relative">
				<iframe
					ref={iframeRef}
					srcDoc={injected}
					sandbox="allow-same-origin allow-scripts allow-forms allow-modals"
					title={t("reader.htmlReader")}
					className="h-full w-full border-0"
					onLoad={handleIframeLoad}
				/>
				{selectionPopup && paperId && (
					<div
						className="absolute z-50"
						style={{ left: selectionPopup.x, top: selectionPopup.y }}
					>
						<AnnotationToolbar
							onConfirm={async (
								type: AnnotationType,
								color: string,
								comment: string,
							) => {
								const positionJson = JSON.stringify(selectionPopup.position);
								const result = await addHtmlAnnotation(
									paperId,
									type,
									color,
									positionJson,
									selectionPopup.selectedText,
									comment || null,
								);
								if (result) {
									postToIframe({
										type: "zr-html-add-highlight",
										annotation: {
											id: result.id,
											type: result.type,
											color: result.color,
											comment: result.comment.text,
											position: result.position,
										},
									});
								}
								setSelectionPopup(null);
							}}
							onCancel={() => setSelectionPopup(null)}
							selectedText={selectionPopup.selectedText}
							onCite={() => {
								useNoteStore.getState().setCitationClipboard({
									format: "html",
									selectedText: selectionPopup.selectedText,
									position: JSON.stringify(selectionPopup.position),
									pageNumber: 0,
								});
								setSelectionPopup(null);
							}}
						/>
					</div>
				)}
				{highlightPopup &&
					(() => {
						const ann = annotations.find(
							(a) => a.id === highlightPopup.annotationId,
						);
						if (!ann) return null;
						return (
							<div
								className="absolute z-50"
								style={{ left: highlightPopup.x, top: highlightPopup.y }}
							>
								<HighlightPopup
									highlight={ann}
									onClose={() => setHighlightPopup(null)}
									onCite={() => {
										useNoteStore.getState().setCitationClipboard({
											format: "html",
											selectedText: ann.selectedText ?? "",
											position: JSON.stringify(ann.position),
											pageNumber: 0,
										});
										setHighlightPopup(null);
									}}
								/>
							</div>
						);
					})()}
			</div>
		);
	}

	return (
		<div className="flex h-full items-center justify-center text-muted-foreground">
			Loading...
		</div>
	);
}

/** Right panel for feed reader: shows feed item metadata */
function FeedReaderMetadataPanel({ item }: { item: FeedItemResponse }) {
	const { t } = useTranslation();
	const addFeedItemToLibrary = useLibraryStore((s) => s.addFeedItemToLibrary);
	const [adding, setAdding] = useState(false);
	const [added, setAdded] = useState(false);

	const ensureTranslated = useTranslationStore((s) => s.ensureTranslated);
	const translatedTitle = useTranslatedText(
		"subscription_item",
		item.id,
		"title",
	);
	const translatedAiSummary = useTranslatedText(
		"subscription_item",
		item.id,
		"ai_summary",
	);
	const translatedAbstract = useTranslatedText(
		"subscription_item",
		item.id,
		"abstract_text",
	);
	const translationLoading = useTranslationLoading(
		"subscription_item",
		item.id,
	);

	useEffect(() => {
		const fields = ["title"];
		if (item.ai_summary) fields.push("ai_summary");
		if (item.abstract_text) fields.push("abstract_text");
		ensureTranslated("subscription_item", item.id, fields);
	}, [item.id, item.ai_summary, item.abstract_text, ensureTranslated]);

	const handleAdd = async () => {
		setAdding(true);
		try {
			await addFeedItemToLibrary(item.id);
			setAdded(true);
		} catch (e) {
			console.error("Failed to add to library:", e);
		}
		setAdding(false);
	};

	return (
		<div className="flex h-full flex-col">
			<div className="flex border-b px-4 py-2">
				<span className="text-xs font-medium">{t("reader.paperInfo")}</span>
			</div>
			<ScrollArea className="flex-1">
				<div className="p-4 space-y-3">
					{/* Title */}
					<BilingualText
						original={item.title}
						translated={translatedTitle}
						loading={translationLoading}
						variant="title"
						className="text-sm"
					/>

					{/* Authors */}
					{item.authors.length > 0 && (
						<p className="text-xs text-muted-foreground">
							{item.authors.map((a) => a.name).join(", ")}
						</p>
					)}

					{/* Display mode toggle (original / bilingual / translated) */}
					<div className="flex items-center gap-2">
						<DisplayModeToggle />
					</div>

					{/* Add to library button */}
					{!item.added_to_library && !added ? (
						<Button
							size="sm"
							variant="outline"
							className="w-full h-8 text-xs"
							onClick={handleAdd}
							disabled={adding}
						>
							{adding ? (
								<>
									<Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
									{t("common.adding")}
								</>
							) : (
								<>
									<BookOpen className="mr-1.5 h-3.5 w-3.5" />
									{t("reader.addToLibrary")}
								</>
							)}
						</Button>
					) : (
						<Button
							size="sm"
							variant="outline"
							className="w-full h-8 text-xs text-green-600"
							disabled
						>
							<Check className="mr-1.5 h-3.5 w-3.5" />
							{t("common.success")}
						</Button>
					)}

					<Separator />

					{/* Metadata grid */}
					<div className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1.5 text-xs">
						<span className="font-medium text-muted-foreground">ArXiv</span>
						<span className="truncate">{item.external_id}</span>
						{item.published_at && (
							<>
								<span className="font-medium text-muted-foreground">
									{t("paper.published")}
								</span>
								<span>{new Date(item.published_at).toLocaleDateString()}</span>
							</>
						)}
						{typeof item.upvotes === "number" && (
							<>
								<span className="font-medium text-muted-foreground">
									{t("reader.upvotes")}
								</span>
								<span>{item.upvotes}</span>
							</>
						)}
						{typeof item.github_stars === "number" && item.github_stars > 0 && (
							<>
								<span className="font-medium text-muted-foreground">
									{t("reader.githubStars")}
								</span>
								<span>{item.github_stars}</span>
							</>
						)}
					</div>

					{/* AI Keywords */}
					{item.ai_keywords && item.ai_keywords.length > 0 && (
						<>
							<Separator />
							<div>
								<h4 className="mb-1.5 text-xs font-medium text-muted-foreground">
									{t("reader.keywords")}
								</h4>
								<div className="flex flex-wrap gap-1">
									{item.ai_keywords.map((kw) => (
										<Badge key={kw} variant="secondary" className="text-[10px]">
											{kw}
										</Badge>
									))}
								</div>
							</div>
						</>
					)}

					{/* AI Summary */}
					{item.ai_summary && (
						<>
							<Separator />
							<div>
								<h4 className="mb-1 text-xs font-medium text-muted-foreground">
									{t("reader.aiSummary")}
								</h4>
								<BilingualText
									original={item.ai_summary}
									translated={translatedAiSummary}
									loading={translationLoading}
									variant="abstract"
									className="text-xs"
								/>
							</div>
						</>
					)}

					{/* Abstract (show separately if ai_summary also exists, or as fallback) */}
					{item.abstract_text && (
						<>
							<Separator />
							<div>
								<h4 className="mb-1 text-xs font-medium text-muted-foreground">
									{t("paper.abstract")}
								</h4>
								<BilingualText
									original={item.abstract_text}
									translated={translatedAbstract}
									loading={translationLoading}
									variant="abstract"
									className="text-xs"
								/>
							</div>
						</>
					)}

					{/* Links */}
					<Separator />
					<div className="space-y-1.5">
						{item.url && (
							<a
								href={item.url}
								target="_blank"
								rel="noopener noreferrer"
								className="flex items-center gap-1.5 text-xs text-primary hover:underline"
							>
								<FileText className="h-3 w-3" /> ArXiv Page
							</a>
						)}
						{item.github_repo && (
							<a
								href={item.github_repo}
								target="_blank"
								rel="noopener noreferrer"
								className="flex items-center gap-1.5 text-xs text-primary hover:underline"
							>
								GitHub Repository
							</a>
						)}
						{item.project_page && (
							<a
								href={item.project_page}
								target="_blank"
								rel="noopener noreferrer"
								className="flex items-center gap-1.5 text-xs text-primary hover:underline"
							>
								<Globe className="h-3 w-3" /> Project Page
							</a>
						)}
					</div>
				</div>
			</ScrollArea>
		</div>
	);
}
