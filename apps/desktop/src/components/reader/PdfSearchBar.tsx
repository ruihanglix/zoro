// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { cn } from "@/lib/utils";
import { ChevronDown, ChevronUp, Search, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

interface FindController {
	executeCommand: (cmd: string, state: Record<string, unknown>) => void;
}

interface PdfViewerInstance {
	eventBus: {
		dispatch: (eventName: string, data: Record<string, unknown>) => void;
		on: (eventName: string, callback: (...args: unknown[]) => void) => void;
		off: (eventName: string, callback: (...args: unknown[]) => void) => void;
	};
	findController: FindController | null;
}

/**
 * Find the PdfHighlighter's internal PDFViewer instance by walking the React
 * fiber tree from the container DOM node. The PdfHighlighter is a class
 * component that stores `this.viewer` (a PDFViewer instance).
 */
function findPdfViewer(): PdfViewerInstance | null {
	// The PdfHighlighter renders a container div with a specific CSS module class.
	// We look for the div that contains the .pdfViewer child.
	const pdfViewerDiv = document.querySelector(".pdfViewer");
	if (!pdfViewerDiv) return null;

	// The container is the parent of .pdfViewer
	const container = pdfViewerDiv.parentElement;
	if (!container) return null;

	// Walk the React fiber tree to find the PdfHighlighter class component
	const fiberKey = Object.keys(container).find((k) =>
		k.startsWith("__reactFiber$"),
	);
	if (!fiberKey) return null;

	let fiber = (container as unknown as Record<string, unknown>)[
		fiberKey
	] as Record<string, unknown> | null;
	let attempts = 0;
	while (fiber && attempts < 30) {
		const stateNode = fiber.stateNode as Record<string, unknown> | null;
		if (
			stateNode &&
			typeof stateNode === "object" &&
			"viewer" in stateNode &&
			stateNode.viewer &&
			typeof stateNode.viewer === "object"
		) {
			const viewer = stateNode.viewer as PdfViewerInstance;
			if ("eventBus" in viewer) {
				return viewer;
			}
		}
		fiber = fiber.return as Record<string, unknown> | null;
		attempts++;
	}
	return null;
}

/**
 * Ensure the PDFViewer has a PDFFindController attached.
 * react-pdf-highlighter doesn't create one, so we create it ourselves.
 *
 * The PDFViewer passes findController to each page's TextHighlighter via a
 * getter in #layerProperties. However, TextHighlighter instances are lazily
 * created and cached via shadow(). If a page's TextHighlighter was already
 * created before we attach the find controller, it will have
 * findController: null and search highlights won't render on that page.
 *
 * To fix this, after creating the find controller we also patch it into
 * every existing TextHighlighter instance on already-rendered pages.
 */
async function ensureFindController(viewer: PdfViewerInstance): Promise<void> {
	if (viewer.findController) return;

	// Cast to unknown first to access internal pdf.js properties not in our interface
	const v = viewer as unknown as Record<string, unknown>;

	try {
		const pdfjs = await import("pdfjs-dist/web/pdf_viewer.mjs");

		const findController = new pdfjs.PDFFindController({
			linkService: (v.linkService ?? { eventBus: viewer.eventBus }) as never,
			eventBus: viewer.eventBus as never,
		});

		// Attach to the viewer (the #layerProperties getter reads this dynamically)
		v.findController = findController;

		// Set onIsPageVisible (same as PDFViewer constructor does)
		findController.onIsPageVisible = (pageNumber: number) => {
			const getVisiblePages = v._getVisiblePages as
				| (() => { ids: Set<number> })
				| undefined;
			return getVisiblePages?.().ids.has(pageNumber) ?? true;
		};

		// Patch findController into existing TextHighlighter instances on
		// already-rendered pages so they can render search highlights.
		const pages = v._pages as Array<Record<string, unknown>> | undefined;
		if (pages) {
			for (const pageView of pages) {
				// _textHighlighter is created lazily via shadow() — only patch if
				// it already exists (checking the own property avoids triggering
				// the lazy getter which would create a new one with null).
				const desc = Object.getOwnPropertyDescriptor(
					pageView,
					"_textHighlighter",
				);
				if (desc && desc.value) {
					(desc.value as Record<string, unknown>).findController =
						findController;
				}
			}
		}

		// Set the PDF document so the find controller can extract text
		const pdfDocument = v._pdfDocument ?? v.pdfDocument ?? null;
		if (pdfDocument) {
			findController.setDocument(pdfDocument as never);
		}
	} catch (e) {
		console.error("[PdfSearchBar] Failed to create PDFFindController:", e);
	}
}

/**
 * PDF text search bar using pdf.js built-in find controller.
 * Dispatches find events to the pdf.js EventBus.
 */
function SearchToggle({
	active,
	onClick,
	title,
	children,
}: {
	active: boolean;
	onClick: () => void;
	title: string;
	children: React.ReactNode;
}) {
	return (
		<button
			type="button"
			className={cn(
				"rounded px-1 py-0.5 text-[11px] font-medium transition-colors border",
				active
					? "bg-primary text-primary-foreground border-primary"
					: "text-muted-foreground hover:bg-muted hover:text-foreground border-transparent",
			)}
			onClick={onClick}
			title={title}
		>
			{children}
		</button>
	);
}

export function PdfSearchBar({
	onClose,
}: {
	onClose: () => void;
}) {
	const { t } = useTranslation();
	const [query, setQuery] = useState("");
	const [matchCount, setMatchCount] = useState(0);
	const [currentMatch, setCurrentMatch] = useState(0);
	const [ready, setReady] = useState(false);
	const [caseSensitive, setCaseSensitive] = useState(false);
	const [entireWord, setEntireWord] = useState(false);
	const [highlightAll, setHighlightAll] = useState(true);
	const inputRef = useRef<HTMLInputElement>(null);
	const viewerRef = useRef<PdfViewerInstance | null>(null);

	useEffect(() => {
		inputRef.current?.focus();
	}, []);

	useEffect(() => {
		let cancelled = false;

		const init = async () => {
			for (let i = 0; i < 10; i++) {
				const viewer = findPdfViewer();
				if (viewer) {
					await ensureFindController(viewer);
					if (!cancelled) {
						viewerRef.current = viewer;
						setReady(true);
					}
					return;
				}
				await new Promise((r) => setTimeout(r, 200));
			}
		};

		init();
		return () => {
			cancelled = true;
		};
	}, []);

	useEffect(() => {
		const viewer = viewerRef.current;
		if (!viewer || !ready) return;

		const handleMatchesCount = (...args: unknown[]) => {
			const data = args[0] as
				| { matchesCount?: { current: number; total: number } }
				| undefined;
			if (data?.matchesCount) {
				setMatchCount(data.matchesCount.total);
				setCurrentMatch(data.matchesCount.current);
			}
		};

		const handleControlState = (...args: unknown[]) => {
			const data = args[0] as
				| { matchesCount?: { current: number; total: number } }
				| undefined;
			if (data?.matchesCount) {
				setMatchCount(data.matchesCount.total);
				setCurrentMatch(data.matchesCount.current);
			}
		};

		viewer.eventBus.on("updatefindmatchescount", handleMatchesCount);
		viewer.eventBus.on("updatefindcontrolstate", handleControlState);

		return () => {
			viewer.eventBus.off("updatefindmatchescount", handleMatchesCount);
			viewer.eventBus.off("updatefindcontrolstate", handleControlState);
		};
	}, [ready]);

	const dispatchFind = useCallback(
		(findPrevious = false, isAgain = false) => {
			const viewer = viewerRef.current;
			if (!viewer) return;

			viewer.eventBus.dispatch("find", {
				source: "PdfSearchBar",
				type: isAgain ? "again" : "",
				query,
				caseSensitive,
				entireWord,
				highlightAll,
				findPrevious,
			});
		},
		[query, caseSensitive, entireWord, highlightAll],
	);

	useEffect(() => {
		if (!ready) return;

		if (query.trim()) {
			dispatchFind(false, false);
		} else {
			const viewer = viewerRef.current;
			if (viewer) {
				viewer.eventBus.dispatch("find", {
					source: "PdfSearchBar",
					type: "",
					query: "",
					caseSensitive: false,
					entireWord: false,
					highlightAll: true,
					findPrevious: false,
				});
			}
			setMatchCount(0);
			setCurrentMatch(0);
		}
	}, [query, caseSensitive, entireWord, highlightAll, ready, dispatchFind]);

	const handleNext = () => dispatchFind(false, true);
	const handlePrev = () => dispatchFind(true, true);

	const handleClose = () => {
		const viewer = viewerRef.current;
		if (viewer) {
			viewer.eventBus.dispatch("find", {
				source: "PdfSearchBar",
				type: "",
				query: "",
				caseSensitive: false,
				entireWord: false,
				highlightAll: true,
				findPrevious: false,
			});
		}
		onClose();
	};

	return (
		<div className="flex items-center gap-1.5 rounded-md border bg-background px-2 py-1 shadow-sm">
			<Search className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
			<input
				ref={inputRef}
				type="text"
				value={query}
				onChange={(e) => setQuery(e.target.value)}
				onKeyDown={(e) => {
					if (e.key === "Enter") {
						if (e.shiftKey) {
							handlePrev();
						} else {
							handleNext();
						}
					}
					if (e.key === "Escape") {
						handleClose();
					}
				}}
				placeholder={t("reader.searchInPdfPlaceholder")}
				className="w-32 bg-transparent text-xs outline-none placeholder:text-muted-foreground"
			/>

			<div className="flex items-center gap-0.5">
				<SearchToggle
					active={caseSensitive}
					onClick={() => setCaseSensitive(!caseSensitive)}
					title={t("reader.matchCase")}
				>
					Aa
				</SearchToggle>
				<SearchToggle
					active={entireWord}
					onClick={() => setEntireWord(!entireWord)}
					title={t("reader.wholeWord")}
				>
					<span className="underline underline-offset-2">ab</span>
				</SearchToggle>
				<SearchToggle
					active={highlightAll}
					onClick={() => setHighlightAll(!highlightAll)}
					title={t("reader.highlightAll")}
				>
					<span className="bg-yellow-300/60 dark:bg-yellow-500/40 px-0.5 rounded-sm">
						H
					</span>
				</SearchToggle>
			</div>

			{query.trim() && (
				<span className="text-[10px] text-muted-foreground whitespace-nowrap">
					{matchCount > 0
						? `${currentMatch} / ${matchCount}`
						: t("reader.noResults")}
				</span>
			)}
			<div className="flex items-center gap-0.5">
				<button
					type="button"
					className="rounded p-0.5 text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-30 transition-colors"
					onClick={handlePrev}
					disabled={matchCount === 0}
					title={t("reader.previousMatch")}
				>
					<ChevronUp className="h-3.5 w-3.5" />
				</button>
				<button
					type="button"
					className="rounded p-0.5 text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-30 transition-colors"
					onClick={handleNext}
					disabled={matchCount === 0}
					title={t("reader.nextMatch")}
				>
					<ChevronDown className="h-3.5 w-3.5" />
				</button>
			</div>
			<button
				type="button"
				className="rounded p-0.5 text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
				onClick={handleClose}
				title={t("reader.closeSearch")}
			>
				<X className="h-3.5 w-3.5" />
			</button>
		</div>
	);
}
