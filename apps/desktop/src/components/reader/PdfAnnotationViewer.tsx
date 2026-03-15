// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import {
	AreaHighlight,
	Highlight,
	PdfHighlighter,
	PdfLoader,
	Popup,
} from "react-pdf-highlighter";
import "react-pdf-highlighter/dist/style.css";
import * as commands from "@/lib/commands";
import { useAnnotationStore } from "@/stores/annotationStore";
import type { AnnotationType, ZoroHighlight } from "@/stores/annotationStore";
import { useNoteStore } from "@/stores/noteStore";
import { useIsDarkMode } from "@/stores/uiStore";
import { readFile } from "@tauri-apps/plugin-fs";
import { FileText, Loader2 } from "lucide-react";
import type { PDFDocumentProxy } from "pdfjs-dist";
import pdfjsWorkerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { ScaledPosition } from "react-pdf-highlighter";
import type { T_ViewportHighlight } from "react-pdf-highlighter/dist/components/PdfHighlighter";
import { AnnotationToolbar } from "./AnnotationToolbar";
import { CitationPreview } from "./CitationPreview";
import { HighlightPopup } from "./HighlightPopup";
import { InkCanvas } from "./InkCanvas";
import { StickyNoteIcon } from "./StickyNoteIcon";

// Create a blob URL from Uint8Array for PdfLoader (which requires a URL string)
function createBlobUrl(data: Uint8Array): string {
	// Cast needed due to TS 5.6 ArrayBuffer/SharedArrayBuffer strictness
	const blob = new Blob([data as unknown as BlobPart], {
		type: "application/pdf",
	});
	const url = URL.createObjectURL(blob);
	return url;
}

interface AnnotationOverrides {
	annotations: ZoroHighlight[];
	fetchAnnotations: () => Promise<void>;
	addAnnotation: (
		type: AnnotationType,
		color: string,
		position: ScaledPosition,
		content: { text?: string; image?: string },
	) => Promise<ZoroHighlight | null>;
	addInkAnnotation: (
		color: string,
		pageNumber: number,
		inkData: {
			strokes: { points: { x: number; y: number }[]; strokeWidth: number }[];
			boundingRect: { x1: number; y1: number; x2: number; y2: number };
		},
	) => Promise<ZoroHighlight | null>;
	updateAnnotation: (
		id: string,
		color?: string | null,
		comment?: string | null,
	) => Promise<void>;
	deleteAnnotation: (id: string) => Promise<void>;
}

interface PdfAnnotationViewerProps {
	paperId: string | null;
	pdfUrl?: string | null;
	pdfFilename?: string;
	/** When true, viewer manages its own state without writing to annotationStore */
	isolated?: boolean;
	/** When true, this viewer is the active tab and should sync state to the global store.
	 *  Only relevant when isolated is false. Defaults to true for backwards compatibility. */
	isActive?: boolean;
	/** Override annotation CRUD (for bilingual mode) */
	annotationOverrides?: AnnotationOverrides;
	/** Ref to receive the scroll-to-highlight function (for isolated viewers) */
	scrollToHighlightRef?: React.MutableRefObject<
		((h: ZoroHighlight) => void) | null
	>;
}

/**
 * Find the Highlight__part elements for a given annotation and flash them.
 * We locate them by matching the annotation's position rects against the
 * rendered Highlight__part elements on the target page.
 */
function flashHighlightParts(
	highlight: ZoroHighlight,
	scope: Element | Document = document,
) {
	const pageNumber = highlight.position.pageNumber;
	const pageDiv = scope.querySelector(
		`.page[data-page-number="${pageNumber}"]`,
	);
	if (!pageDiv) {
		console.warn("[flash] pageDiv not found for page", pageNumber);
		return;
	}

	// Find Highlight__part elements on this page
	const highlightLayer = pageDiv.querySelector(
		".PdfHighlighter__highlight-layer",
	);
	if (!highlightLayer) {
		console.warn("[flash] highlight layer not found on page", pageNumber);
		return;
	}

	// Match parts by checking proximity to the annotation's bounding rect.
	// Scaled coords: x1/y1 are viewport px at creation time, width/height are
	// the viewport dimensions at that time. Convert to current viewport px:
	//   currentPx = pageSize * scaled.coord / scaled.refSize
	const pageHeight = (pageDiv as HTMLElement).clientHeight;
	const pageWidth = (pageDiv as HTMLElement).clientWidth;
	const br = highlight.position.boundingRect;
	const expectedTop = (pageHeight * br.y1) / br.height;
	const expectedLeft = (pageWidth * br.x1) / br.width;
	const tolerance = 10; // px

	const allParts =
		highlightLayer.querySelectorAll<HTMLElement>(".Highlight__part");

	// Collect parts that belong to this annotation.
	// Strategy: find the first part near the expected position, then take all
	// consecutive parts that are part of the same Highlight container.
	let targetHighlightDiv: Element | null = null;
	for (const part of allParts) {
		const partTop = Number.parseFloat(part.style.top) || 0;
		const partLeft = Number.parseFloat(part.style.left) || 0;
		if (
			Math.abs(partTop - expectedTop) < tolerance &&
			Math.abs(partLeft - expectedLeft) < tolerance
		) {
			// Found a matching part — get its parent Highlight container
			targetHighlightDiv = part.closest(".Highlight");
			break;
		}
	}

	if (!targetHighlightDiv) {
		// Fallback: flash all parts near the expected top position
		const nearbyParts: HTMLElement[] = [];
		for (const part of allParts) {
			const partTop = Number.parseFloat(part.style.top) || 0;
			if (Math.abs(partTop - expectedTop) < tolerance * 5) {
				nearbyParts.push(part);
			}
		}
		if (nearbyParts.length > 0) {
			runFlashSequence(nearbyParts, highlight.type === "underline");
		}
		return;
	}

	const parts = Array.from(
		targetHighlightDiv.querySelectorAll<HTMLElement>(".Highlight__part"),
	);
	const isUnderline = targetHighlightDiv.closest(".pdf-underline") !== null;
	runFlashSequence(parts, isUnderline);
}

function runFlashSequence(parts: HTMLElement[], isUnderline: boolean) {
	const flashColor = "rgba(59, 130, 246, 0.5)";
	const flashColorDim = "rgba(59, 130, 246, 0.15)";

	// Save originals
	const originals = parts.map((p) => ({
		background: p.style.background,
		borderBottom: p.style.borderBottom,
		transition: p.style.transition,
	}));

	const applyFlash = (on: boolean) => {
		parts.forEach((p) => {
			p.dataset.zcFlashing = "true";
			p.style.setProperty("transition", "none", "important");
			if (isUnderline) {
				p.style.setProperty("background", "transparent", "important");
				p.style.setProperty(
					"border-bottom",
					on
						? "3px solid rgb(59, 130, 246)"
						: "2px solid rgba(59, 130, 246, 0.15)",
					"important",
				);
			} else {
				p.style.setProperty(
					"background",
					on ? flashColor : flashColorDim,
					"important",
				);
			}
		});
	};

	const restore = () => {
		parts.forEach((p, i) => {
			delete p.dataset.zcFlashing;
			p.style.setProperty("transition", "background 0.3s", "important");
			p.style.setProperty("background", originals[i].background, "important");
			p.style.setProperty(
				"border-bottom",
				originals[i].borderBottom,
				"important",
			);
			setTimeout(() => {
				p.style.setProperty("transition", originals[i].transition);
			}, 350);
		});
	};

	// Flash: on -> off -> on -> restore
	applyFlash(true);
	setTimeout(() => applyFlash(false), 200);
	setTimeout(() => applyFlash(true), 400);
	setTimeout(restore, 800);
}

export function PdfAnnotationViewer({
	paperId,
	pdfUrl,
	pdfFilename,
	isolated,
	isActive = true,
	annotationOverrides,
	scrollToHighlightRef,
}: PdfAnnotationViewerProps) {
	const [pdfBlobUrl, setPdfBlobUrl] = useState<string | null>(null);
	const [loading, setLoading] = useState(true);
	const [error, setError] = useState<string | null>(null);
	const blobUrlRef = useRef<string | null>(null);
	const containerRef = useRef<HTMLDivElement>(null);
	const pdfHighlighterRef = useRef<PdfHighlighter<ZoroHighlight>>(null);

	const sourceFile = pdfFilename || "paper.pdf";

	// In isolated mode, use overrides; otherwise use the global store
	const storeAnnotations = useAnnotationStore((s) => s.annotations);
	const storeFetchAnnotations = useAnnotationStore((s) => s.fetchAnnotations);
	const storeAddAnnotation = useAnnotationStore((s) => s.addAnnotation);
	const { t } = useTranslation();
	const activeTool = useAnnotationStore((s) => s.activeTool);
	const storeSetScrollToHighlight = useAnnotationStore(
		(s) => s.setScrollToHighlight,
	);
	const storeSetScrollToPage = useAnnotationStore((s) => s.setScrollToPage);
	const storeSetPdfDocument = useAnnotationStore((s) => s.setPdfDocument);
	const storeSetPdfViewer = useAnnotationStore((s) => s.setPdfViewer);
	const storeSetCurrentPage = useAnnotationStore((s) => s.setCurrentPage);
	const storeFetchReaderState = useAnnotationStore((s) => s.fetchReaderState);
	const storeSaveReaderState = useAnnotationStore((s) => s.saveReaderState);
	const zoomLevel = useAnnotationStore((s) => s.zoomLevel);
	const storePdfDocument = useAnnotationStore((s) => s.pdfDocument);

	const isDark = useIsDarkMode();
	const annotations = annotationOverrides?.annotations ?? storeAnnotations;

	// Workaround for react-pdf-highlighter async init() race condition:
	// The library's init() is async, so textlayerrendered events can fire before
	// the event listener is registered. When that happens, renderHighlightLayers()
	// is never called and highlights don't appear.
	// Fix: bump a counter when text layers appear in the DOM, which creates a new
	// highlights array reference and forces PdfHighlighter.componentDidUpdate to
	// call renderHighlightLayers().
	const [highlightBump, setHighlightBump] = useState(0);
	useEffect(() => {
		// Watch for text layer divs appearing (signals pdf.js has rendered pages)
		// Debounced: rapid page loads during fast scrolling only trigger one bump
		let debounceTimer: ReturnType<typeof setTimeout> | null = null;
		const debouncedBump = () => {
			if (debounceTimer) clearTimeout(debounceTimer);
			debounceTimer = setTimeout(() => {
				setHighlightBump((n) => n + 1);
			}, 200);
		};

		const observer = new MutationObserver((mutations) => {
			for (const m of mutations) {
				for (const node of m.addedNodes) {
					if (node instanceof HTMLElement) {
						if (
							node.classList?.contains("textLayer") ||
							node.querySelector?.(".textLayer")
						) {
							debouncedBump();
							return;
						}
					}
				}
			}
		});
		// Scope to the component container instead of document.body to reduce noise
		const target = containerRef.current ?? document.body;
		observer.observe(target, { childList: true, subtree: true });
		return () => {
			if (debounceTimer) clearTimeout(debounceTimer);
			observer.disconnect();
		};
	}, []);

	// Workaround for highlight misalignment on initial load:
	// react-pdf-highlighter renders highlights via createRoot().render() which is
	// async in React 18. When the library calls scaledPositionToViewport during
	// the async render, the pdf.js viewport may not yet reflect the final "auto"
	// scale (pdf.js transitions from DEFAULT_SCALE to "auto" after pagesinit).
	// The page div dimensions update via CSS var(--scale-factor), but the viewport
	// object used for coordinate conversion can be stale. After scrolling, a
	// re-render uses the stabilized viewport and positions are correct.
	// Fix: observe the .pdfViewer container for size changes (triggered by
	// --scale-factor updates) and schedule a delayed re-render. Also do a
	// one-time delayed bump after the PDF loads to catch any missed resize.
	useEffect(() => {
		if (!pdfBlobUrl) return;
		const el = containerRef.current;
		if (!el) return;

		let stabilizeTimer: ReturnType<typeof setTimeout> | null = null;

		const scheduleBump = () => {
			if (stabilizeTimer) clearTimeout(stabilizeTimer);
			stabilizeTimer = setTimeout(() => {
				setHighlightBump((n) => n + 1);
			}, 150);
		};

		const resizeObserver = new ResizeObserver(() => {
			scheduleBump();
		});

		const pdfViewer = el.querySelector(".pdfViewer");
		if (pdfViewer) {
			resizeObserver.observe(pdfViewer);
		} else {
			const mo = new MutationObserver(() => {
				const pv = el.querySelector(".pdfViewer");
				if (pv) {
					resizeObserver.observe(pv);
					mo.disconnect();
				}
			});
			mo.observe(el, { childList: true, subtree: true });
		}

		const fallbackTimer = setTimeout(() => {
			setHighlightBump((n) => n + 1);
		}, 800);

		return () => {
			if (stabilizeTimer) clearTimeout(stabilizeTimer);
			clearTimeout(fallbackTimer);
			resizeObserver.disconnect();
		};
	}, [pdfBlobUrl]);

	// Create a new array reference when bump changes, forcing PdfHighlighter to
	// re-run renderHighlightLayers via componentDidUpdate.
	// Filter out ink annotations — they are rendered by InkCanvas via direct DOM
	// injection, not by PdfHighlighter's highlight layer system. Including them
	// would create empty Highlight components and corrupt AnnotationColorStyles'
	// sequential index mapping.
	const highlightsForViewer = useMemo(
		() => annotations.filter((a) => a.type !== "ink"),
		// eslint-disable-next-line react-hooks/exhaustive-deps
		[annotations, highlightBump],
	);

	// --- DEBUG: log highlight counts for each viewer ---
	useEffect(() => {
		console.log(
			`[PDFViewer:${sourceFile}] annotations=${annotations.length}, ` +
				`highlightsForViewer=${highlightsForViewer.length}, ` +
				`isolated=${!!isolated}, bump=${highlightBump}`,
		);
	}, [annotations, highlightsForViewer, isolated, highlightBump, sourceFile]);

	// Register a DOM-based scroll-to-highlight function.
	useEffect(() => {
		if (isolated && !scrollToHighlightRef) return;

		const scrollFn = (highlight: ZoroHighlight) => {
			const scope = containerRef.current ?? document;
			const pageNumber = highlight.position.pageNumber;
			const boundingRect = highlight.position.boundingRect;
			const isPdfCoords = !!(
				highlight.position as unknown as Record<string, unknown>
			).usePdfCoordinates;

			// For usePdfCoordinates annotations (Zotero imports), delegate to
			// react-pdf-highlighter's built-in scrollTo which correctly converts
			// PDF coordinates to viewport coordinates.
			if (isPdfCoords) {
				const hlInstance = pdfHighlighterRef.current as unknown as {
					scrollTo?: (h: ZoroHighlight) => void;
				} | null;
				if (hlInstance?.scrollTo) {
					hlInstance.scrollTo(highlight);
				} else {
					// Fallback: DOM-based page scroll
					const container = scope.querySelector(
						"[class*='_container_']",
					) as HTMLElement | null;
					const pageDiv = scope.querySelector(
						`.page[data-page-number="${pageNumber}"]`,
					) as HTMLElement | null;
					if (container && pageDiv) {
						container.scrollTo({ top: pageDiv.offsetTop, behavior: "smooth" });
					}
				}
			} else {
				// Normal (scaled) annotations: calculate precise scroll offset
				const container = scope.querySelector(
					"[class*='_container_']",
				) as HTMLElement | null;
				const pageDiv = scope.querySelector(
					`.page[data-page-number="${pageNumber}"]`,
				) as HTMLElement | null;
				if (!container || !pageDiv) return;

				const pageHeight = pageDiv.clientHeight;
				const scrollMargin = 50;
				const targetY =
					pageDiv.offsetTop +
					(pageHeight * boundingRect.y1) / boundingRect.height -
					scrollMargin;
				container.scrollTo({ top: Math.max(0, targetY), behavior: "smooth" });
			}

			// Flash the highlight parts after scroll animation completes
			setTimeout(() => {
				const flashPageDiv = scope.querySelector(
					`.page[data-page-number="${pageNumber}"]`,
				) as HTMLElement | null;
				if (highlight.type === "note") {
					const noteEl = flashPageDiv?.querySelector(
						`.zr-pdf-note[data-annotation-id="${highlight.id}"]`,
					) as HTMLElement | null;
					if (noteEl) {
						noteEl.style.transition = "transform 0.2s";
						noteEl.style.transform = "scale(1.5)";
						setTimeout(() => {
							noteEl.style.transform = "scale(1)";
						}, 800);
					}
				} else {
					flashHighlightParts(highlight, scope);
				}
			}, 400);
		};

		if (isolated && scrollToHighlightRef) {
			scrollToHighlightRef.current = scrollFn;
			return () => {
				scrollToHighlightRef.current = null;
			};
		}

		storeSetScrollToHighlight(scrollFn);
		return () => {
			storeSetScrollToHighlight(null);
		};
	}, [isolated, storeSetScrollToHighlight, scrollToHighlightRef]);

	// Register scroll-to-page function
	useEffect(() => {
		if (isolated) return;

		const scrollToPage = (pageNumber: number) => {
			// Prefer the pdf.js viewer's native API for reliable page navigation.
			// It works even when target page DOM elements haven't been rendered yet
			// (e.g. lazy-rendered pages in feed reader / paper.cool mode).
			const viewer = viewerInstanceRef.current;
			if (
				viewer &&
				typeof viewer.currentPageNumber === "number" &&
				pageNumber >= 1
			) {
				try {
					viewer.currentPageNumber = pageNumber;
					return;
				} catch {
					// Fall through to DOM-based approach
				}
			}

			// Fallback: DOM-based scroll
			const scope = containerRef.current ?? document;
			const pageDiv = scope.querySelector(
				`.page[data-page-number="${pageNumber}"]`,
			) as HTMLElement | null;
			const container = scope.querySelector(
				"[class*='_container_']",
			) as HTMLElement | null;

			if (!container || !pageDiv) return;
			container.scrollTo({ top: pageDiv.offsetTop, behavior: "smooth" });
		};

		storeSetScrollToPage(scrollToPage);
		return () => storeSetScrollToPage(null);
	}, [isolated, storeSetScrollToPage]);

	// Track current page from scroll position.
	// We depend on storePdfDocument so this effect re-runs once the PDF has
	// been fully loaded and PdfHighlighter has rendered the scroll container.
	useEffect(() => {
		if (!storePdfDocument) return;

		const scope = containerRef.current ?? document;

		// The scroll container rendered by react-pdf-highlighter may not be in
		// the DOM immediately after storePdfDocument is set. Retry a few times
		// with short delays before giving up.
		let cancelled = false;
		let cleanupScroll: (() => void) | null = null;

		const attachScrollListener = () => {
			const container = scope.querySelector(
				"[class*='_container_']",
			) as HTMLElement | null;
			if (!container) return false;

			// Throttled scroll handler: compute current page at most every
			// 200ms to reduce DOM queries in bilingual mode.
			let scrollThrottleTimer: ReturnType<typeof setTimeout> | null = null;
			const computeCurrentPage = () => {
				scrollThrottleTimer = null;
				const pages = container.querySelectorAll<HTMLElement>(
					".page[data-page-number]",
				);
				const containerTop = container.scrollTop;
				const containerMiddle = containerTop + container.clientHeight / 3;

				let closestPage = 1;
				let closestDist = Number.POSITIVE_INFINITY;

				for (const page of pages) {
					const pageTop = page.offsetTop;
					const pageBottom = pageTop + page.clientHeight;
					const pageMid = (pageTop + pageBottom) / 2;
					const dist = Math.abs(pageMid - containerMiddle);
					if (dist < closestDist) {
						closestDist = dist;
						closestPage = Number(page.dataset.pageNumber) || 1;
					}
				}

				if (!isolated && isActive) {
					storeSetCurrentPage(closestPage);
				}
			};

			const handlePageScroll = () => {
				if (scrollThrottleTimer === null) {
					scrollThrottleTimer = setTimeout(computeCurrentPage, 200);
				}
			};

			container.addEventListener("scroll", handlePageScroll, { passive: true });
			// Run once immediately so the page number is correct on load
			computeCurrentPage();
			cleanupScroll = () => {
				container.removeEventListener("scroll", handlePageScroll);
				if (scrollThrottleTimer !== null) clearTimeout(scrollThrottleTimer);
			};
			return true;
		};

		// Try immediately, then retry with increasing delays
		if (!attachScrollListener()) {
			const retryDelays = [100, 300, 600, 1000];
			let retryIdx = 0;
			const tryAgain = () => {
				if (cancelled || retryIdx >= retryDelays.length) return;
				setTimeout(() => {
					if (cancelled) return;
					if (!attachScrollListener() && retryIdx < retryDelays.length) {
						retryIdx++;
						tryAgain();
					}
				}, retryDelays[retryIdx]);
				retryIdx++;
			};
			tryAgain();
		}

		return () => {
			cancelled = true;
			cleanupScroll?.();
		};
	}, [isolated, isActive, storeSetCurrentPage, pdfBlobUrl, storePdfDocument]);

	// Load PDF data
	useEffect(() => {
		const hasPdfUrl = !!pdfUrl;
		const hasPaperId = !!paperId;
		if (!hasPdfUrl && !hasPaperId) {
			return;
		}

		let cancelled = false;
		setLoading(true);
		setError(null);
		setPdfBlobUrl(null);

		(async () => {
			try {
				let data: Uint8Array;
				if (hasPdfUrl) {
					const tempPath = await commands.fetchRemotePdf(pdfUrl!);
					console.log("[PdfViewer] Loading remote PDF:", tempPath);
					data = await readFile(tempPath);
				} else if (pdfFilename) {
					const filePath = await commands.getPaperFilePath(
						paperId!,
						pdfFilename,
					);
					console.log(
						"[PdfViewer] Loading PDF by filename:",
						pdfFilename,
						"->",
						filePath,
					);
					data = await readFile(filePath);
				} else {
					const pdfPath = await commands.getPaperPdfPath(paperId!);
					console.log("[PdfViewer] Loading default PDF:", pdfPath);
					data = await readFile(pdfPath);
				}
				if (!cancelled) {
					const url = createBlobUrl(data);
					blobUrlRef.current = url;
					setPdfBlobUrl(url);
				}
			} catch (e) {
				console.error("[PdfViewer] Failed to load PDF data:", e);
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
			if (blobUrlRef.current) {
				URL.revokeObjectURL(blobUrlRef.current);
				blobUrlRef.current = null;
			}
		};
	}, [paperId, pdfUrl, pdfFilename]);

	// Load annotations scoped to the specific PDF file
	useEffect(() => {
		if (!paperId) return;
		if (!isolated && !isActive) return; // Only fetch when this tab is active
		if (annotationOverrides) {
			annotationOverrides.fetchAnnotations();
		} else {
			storeFetchAnnotations(paperId, sourceFile);
		}
		if (!isolated) {
			storeFetchReaderState(paperId);
		}
	}, [
		paperId,
		sourceFile,
		annotationOverrides,
		isolated,
		isActive,
		storeFetchAnnotations,
		storeFetchReaderState,
	]);

	const scrollSaveTimeout = useRef<ReturnType<typeof setTimeout> | null>(null);
	const handleScroll = useCallback(() => {
		if (!paperId || isolated) return;
		if (scrollSaveTimeout.current) {
			clearTimeout(scrollSaveTimeout.current);
		}
		scrollSaveTimeout.current = setTimeout(() => {
			const scope = containerRef.current ?? document;
			const container = scope.querySelector(
				".PdfHighlighter .pdfViewer",
			) as HTMLElement | null;
			if (container) {
				const scrollTop =
					container.closest("[style*='overflow']")?.scrollTop ?? 0;
				const scrollHeight = container.scrollHeight || 1;
				const scrollPct = (scrollTop / scrollHeight) * 100;
				storeSaveReaderState(paperId, scrollPct);
			}
		}, 1000);
	}, [paperId, isolated, storeSaveReaderState]);

	// Cleanup timeouts on unmount
	useEffect(() => {
		return () => {
			if (scrollSaveTimeout.current) {
				clearTimeout(scrollSaveTimeout.current);
			}
		};
	}, []);

	// Click-to-place note when note tool is active
	useEffect(() => {
		const container = containerRef.current;
		if (!container) return;

		const handleNoteClick = async (e: MouseEvent) => {
			const currentTool = useAnnotationStore.getState().activeTool;
			if (currentTool !== "note") return;
			if (!paperId) return;

			const sel = window.getSelection();
			if (sel && !sel.isCollapsed) return;

			const target = e.target as HTMLElement;
			if (target.closest(".zr-pdf-note")) return;

			const pageDiv = target.closest(
				".page[data-page-number]",
			) as HTMLElement | null;
			if (!pageDiv) return;

			const pageNum = Number(pageDiv.dataset.pageNumber);
			if (!pageNum) return;

			const pageRect = pageDiv.getBoundingClientRect();
			const clickX = e.clientX - pageRect.left;
			const clickY = e.clientY - pageRect.top;
			const pageWidth = pageDiv.clientWidth;
			const pageHeight = pageDiv.clientHeight;

			const position: ScaledPosition = {
				boundingRect: {
					x1: clickX,
					y1: clickY,
					x2: clickX + 24,
					y2: clickY + 24,
					width: pageWidth,
					height: pageHeight,
					pageNumber: pageNum,
				},
				rects: [],
				pageNumber: pageNum,
			};

			const currentColor = useAnnotationStore.getState().activeColor;
			if (annotationOverrides) {
				await annotationOverrides.addAnnotation(
					"note",
					currentColor,
					position,
					{},
				);
			} else {
				await storeAddAnnotation(
					paperId,
					"note",
					currentColor,
					position,
					{},
					sourceFile,
				);
			}
		};

		container.addEventListener("click", handleNoteClick);
		return () => container.removeEventListener("click", handleNoteClick);
	}, [paperId, sourceFile, annotationOverrides, storeAddAnnotation]);

	// Pinch-to-zoom (trackpad) and Ctrl/Cmd+wheel zoom.
	// Two-phase approach for smooth performance:
	//   1. Visual phase: apply GPU-composited transform: scale() during gesture
	//   2. Commit phase: apply CSS zoom via store update when gesture ends
	useEffect(() => {
		const el = containerRef.current;
		if (!el) return;

		let targetZoom = useAnnotationStore.getState().zoomLevel;
		let commitTimer: ReturnType<typeof setTimeout> | null = null;

		const getPdfViewer = () =>
			el.querySelector(".pdfViewer") as HTMLElement | null;

		const clampZoom = (z: number) => Math.max(0.25, Math.min(5, z));

		const applyVisualZoom = (newTarget: number) => {
			targetZoom = clampZoom(newTarget);
			const viewer = getPdfViewer();
			if (!viewer) return;
			const storeZoom = useAnnotationStore.getState().zoomLevel;
			const scaleFactor = targetZoom / storeZoom;
			viewer.style.transformOrigin = "0 0";
			viewer.style.transform = `scale(${scaleFactor})`;
		};

		const commitZoom = () => {
			commitTimer = null;
			const viewer = getPdfViewer();
			if (viewer) {
				viewer.style.transform = "";
				viewer.style.transformOrigin = "";
			}
			useAnnotationStore.getState().setZoomLevel(targetZoom);
		};

		const scheduleCommit = () => {
			if (commitTimer) clearTimeout(commitTimer);
			commitTimer = setTimeout(commitZoom, 100);
		};

		const handleWheel = (e: WheelEvent) => {
			if (!e.ctrlKey && !e.metaKey) return;
			e.preventDefault();
			applyVisualZoom(targetZoom * (1 - e.deltaY * 0.01));
			scheduleCommit();
		};

		let gestureBaseZoom = 1;

		const handleGestureStart = (e: Event) => {
			e.preventDefault();
			if (commitTimer) clearTimeout(commitTimer);
			gestureBaseZoom = targetZoom;
		};

		const handleGestureChange = (e: Event) => {
			e.preventDefault();
			const s = (e as unknown as { scale: number }).scale;
			applyVisualZoom(gestureBaseZoom * s);
		};

		const handleGestureEnd = (e: Event) => {
			e.preventDefault();
			commitZoom();
		};

		el.addEventListener("wheel", handleWheel, { passive: false });
		el.addEventListener("gesturestart", handleGestureStart, { passive: false });
		el.addEventListener("gesturechange", handleGestureChange, {
			passive: false,
		});
		el.addEventListener("gestureend", handleGestureEnd, { passive: false });

		return () => {
			if (commitTimer) clearTimeout(commitTimer);
			el.removeEventListener("wheel", handleWheel);
			el.removeEventListener("gesturestart", handleGestureStart);
			el.removeEventListener("gesturechange", handleGestureChange);
			el.removeEventListener("gestureend", handleGestureEnd);
		};
	}, [pdfBlobUrl]);

	// Intercept clicks on internal PDF links (Table/Figure/Citation references)
	// so setCurrentPage can use debounced jump detection for smooth scrolling.
	// Only marks the source page; all history logic lives in setCurrentPage.
	useEffect(() => {
		if (isolated) return;
		const container = containerRef.current;
		if (!container) return;

		const handleLinkClick = (e: MouseEvent) => {
			const link = (e.target as HTMLElement).closest(
				'.annotationLayer a, a[href^="#"]',
			);
			if (!link) return;
			useAnnotationStore.getState().markInternalLinkJump();
		};

		container.addEventListener("click", handleLinkClick, true);
		return () => container.removeEventListener("click", handleLinkClick, true);
	}, [isolated, pdfBlobUrl]);

	// Keep a local ref to the loaded pdfDocument so we can re-sync to the
	// global store when this tab becomes active.
	const localPdfDocRef = useRef<PDFDocumentProxy | null>(null);

	const handlePdfDocument = useCallback(
		(pdfDocument: PDFDocumentProxy) => {
			localPdfDocRef.current = pdfDocument;
			if (!isolated && isActive) {
				storeSetPdfDocument(pdfDocument);
			}
		},
		[isolated, isActive, storeSetPdfDocument],
	);

	// When this tab becomes active, re-sync the local pdfDocument to the
	// global store so that totalPages / pdfDocument are correct.
	useEffect(() => {
		if (!isolated && isActive && localPdfDocRef.current) {
			storeSetPdfDocument(localPdfDocRef.current);
		}
	}, [isolated, isActive, storeSetPdfDocument]);

	useEffect(() => {
		if (isolated) return;
		return () => {
			storeSetPdfDocument(null);
			storeSetPdfViewer(null);
		};
	}, [isolated, storeSetPdfDocument, storeSetPdfViewer]);

	// ── Aggressive pre-rendering & buffer boost ──
	// pdf.js default: buffer=10 pages, pre-render 1 page ahead, idle cleanup at 30s.
	// This causes: (1) next pages not rendered when flipping, (2) going back re-renders.
	// Fix: enlarge buffer to 30+, patch rendering queue to pre-render 5 pages ahead,
	// disable idle cleanup, enable HWA, and actively drive multi-page pre-rendering.
	const viewerInstanceRef = useRef<Record<string, unknown> | null>(null);
	useEffect(() => {
		if (!pdfBlobUrl) return;
		let cancelled = false;
		let timer: ReturnType<typeof setTimeout> | null = null;
		let preRenderInterval: ReturnType<typeof setInterval> | null = null;

		const tryBoostAndPatch = () => {
			const hlRef = pdfHighlighterRef.current as unknown as Record<
				string,
				unknown
			> | null;
			const viewer = hlRef?.viewer as Record<string, unknown> | undefined;
			if (!viewer) {
				if (!cancelled) timer = setTimeout(tryBoostAndPatch, 200);
				return;
			}
			viewerInstanceRef.current = viewer;

			// ── 1. Enlarge page buffer (keep more rendered pages in memory) ──
			try {
				const buf = (viewer as Record<string, unknown>)._buffer as
					| { resize?: (n: number, ids?: Set<number> | null) => void }
					| undefined;
				if (buf?.resize) {
					// Bilingual mode: 24 pages per side; single: 30 pages
					const targetSize = isolated ? 24 : 30;
					buf.resize(targetSize, null);
				}
			} catch {
				// Buffer internals may differ across pdf.js versions
			}

			// Cap maxCanvasPixels in bilingual mode to balance memory
			if (isolated && typeof viewer.maxCanvasPixels === "number") {
				viewer.maxCanvasPixels = 4_194_304; // ~4MP instead of default ~16MP
			}

			// ── 2. Patch rendering queue: deeper pre-render (5 pages ahead) ──
			// Original getHighestPriority only looks 1 page beyond visible range.
			// We monkey-patch it to scan up to PRE_RENDER_DEPTH pages ahead/behind.
			const PRE_RENDER_DEPTH = 5;
			try {
				const renderingQueue = viewer.renderingQueue as
					| Record<string, unknown>
					| undefined;
				if (
					renderingQueue &&
					typeof renderingQueue.getHighestPriority === "function"
				) {
					const origGetHP =
						renderingQueue.getHighestPriority.bind(renderingQueue);
					renderingQueue.getHighestPriority = (
						visible: {
							views: { view: Record<string, unknown> }[];
							first?: { id: number };
							last?: { id: number };
							ids?: Set<number>;
						},
						views: Record<string, unknown>[],
						scrolledDown: boolean,
						preRenderExtra = false,
					): Record<string, unknown> | null => {
						// First try the original logic (render visible + fill holes + 1 ahead)
						const orig = origGetHP(
							visible,
							views,
							scrolledDown,
							preRenderExtra,
						);
						if (orig) return orig;

						// Extend: scan up to PRE_RENDER_DEPTH pages beyond visible range
						const firstId = visible.first?.id ?? 1;
						const lastId = visible.last?.id ?? 1;
						const isFinished = (v: Record<string, unknown>) =>
							v && (v.renderingState as number) === 3; // RenderingStates.FINISHED

						if (scrolledDown) {
							// Pre-render pages after the last visible
							for (let i = 1; i <= PRE_RENDER_DEPTH; i++) {
								const idx = lastId - 1 + i;
								if (idx >= 0 && idx < views.length && !isFinished(views[idx])) {
									return views[idx];
								}
							}
							// Also pre-render a couple behind (for scroll-back)
							for (let i = 1; i <= 2; i++) {
								const idx = firstId - 1 - i;
								if (idx >= 0 && idx < views.length && !isFinished(views[idx])) {
									return views[idx];
								}
							}
						} else {
							// Scrolling up: pre-render pages before the first visible
							for (let i = 1; i <= PRE_RENDER_DEPTH; i++) {
								const idx = firstId - 1 - i;
								if (idx >= 0 && idx < views.length && !isFinished(views[idx])) {
									return views[idx];
								}
							}
							// Also pre-render a couple ahead (for scroll-down)
							for (let i = 1; i <= 2; i++) {
								const idx = lastId - 1 + i;
								if (idx >= 0 && idx < views.length && !isFinished(views[idx])) {
									return views[idx];
								}
							}
						}
						return null;
					};
				}

				// ── 3. Disable idle cleanup (prevent destroying rendered pages) ──
				// PDFRenderingQueue sets an idle timeout that calls cleanup() after 30s.
				// Override onIdle to no-op so pages are never destroyed while viewing.
				if (renderingQueue) {
					renderingQueue.onIdle = null;
					// Also clear any existing idle timeout
					if (renderingQueue.idleTimeout) {
						clearTimeout(
							renderingQueue.idleTimeout as ReturnType<typeof setTimeout>,
						);
						renderingQueue.idleTimeout = null;
					}
				}
			} catch {
				// Rendering queue patch failed — non-critical
			}

			// ── 4. Enable Hardware-Accelerated Canvas (HWA) on page views ──
			// pdf.js PDFPageView can use willReadFrequently:false + HWA canvas
			// for faster GPU compositing. Set _enableHWA on existing pages.
			try {
				const pages = viewer._pages as Record<string, unknown>[] | undefined;
				if (pages) {
					for (const page of pages) {
						if (page && typeof page === "object") {
							// Enable HWA flag for future renders
							(page as Record<string, boolean>)._enableHWA = true;
						}
					}
				}
			} catch {
				// Non-critical
			}

			// ── 5. Active pre-render pump ──
			// After viewer init, periodically call forceRendering() to drive the
			// rendering queue until all nearby pages are done. This is critical
			// because pdf.js only renders ONE page per forceRendering() call.
			// We pump it repeatedly so the full pre-render depth gets processed.
			let pumpCount = 0;
			const maxPumps = 50; // Stop after enough pumps to avoid infinite loop
			const pumpRendering = () => {
				if (cancelled || pumpCount > maxPumps) {
					if (preRenderInterval) {
						clearInterval(preRenderInterval);
						preRenderInterval = null;
					}
					return;
				}
				pumpCount++;
				try {
					const didRender = (viewer.forceRendering as () => boolean)?.();
					if (!didRender) {
						// All nearby pages are rendered — stop pumping
						if (preRenderInterval) {
							clearInterval(preRenderInterval);
							preRenderInterval = null;
						}
					}
				} catch {
					// Ignore
				}
			};
			// Pump every 100ms to render ~10 pages/sec
			preRenderInterval = setInterval(pumpRendering, 100);

			// ── 6. Re-pump on scroll to pre-render newly approaching pages ──
			const container = viewer.container as HTMLElement | undefined;
			if (container) {
				let scrollPumpTimer: ReturnType<typeof setTimeout> | null = null;
				const onScrollPump = () => {
					if (scrollPumpTimer) return;
					scrollPumpTimer = setTimeout(() => {
						scrollPumpTimer = null;
						// Reset pump and start again
						pumpCount = 0;
						if (!preRenderInterval) {
							preRenderInterval = setInterval(pumpRendering, 100);
						}
					}, 150);
				};
				container.addEventListener("scroll", onScrollPump, { passive: true });
				// Store cleanup ref
				(viewer as Record<string, unknown>).__zcScrollPumpCleanup = () => {
					container.removeEventListener("scroll", onScrollPump);
					if (scrollPumpTimer) clearTimeout(scrollPumpTimer);
				};
			}
		};

		timer = setTimeout(tryBoostAndPatch, 300);
		return () => {
			cancelled = true;
			if (timer) clearTimeout(timer);
			if (preRenderInterval) clearInterval(preRenderInterval);
			// Cleanup scroll pump listener
			const viewer = viewerInstanceRef.current;
			if (viewer) {
				const cleanup = (viewer as Record<string, unknown>)
					.__zcScrollPumpCleanup as (() => void) | undefined;
				cleanup?.();
			}
		};
	}, [pdfBlobUrl, isolated]);

	// Track last selected text for Cmd+C copy support.
	// react-pdf-highlighter shows an annotation toolbar popup on selection, which
	// steals focus and prevents the native copy event from firing. We listen for
	// Cmd/Ctrl+C at the document level and write the stored text to clipboard.
	const lastSelectedTextRef = useRef<string | null>(null);

	useEffect(() => {
		const handleKeyDown = (e: KeyboardEvent) => {
			if (!(e.metaKey || e.ctrlKey) || e.key !== "c") return;

			const target = e.target as HTMLElement | null;
			const inInput =
				target?.tagName === "INPUT" ||
				target?.tagName === "TEXTAREA" ||
				target?.isContentEditable;
			if (inInput) return;

			const nativeSel = window.getSelection()?.toString()?.trim();
			const text = nativeSel || lastSelectedTextRef.current;
			if (!text) return;

			e.preventDefault();
			navigator.clipboard.writeText(text);
		};

		document.addEventListener("keydown", handleKeyDown);
		return () => document.removeEventListener("keydown", handleKeyDown);
	}, []);

	const handleSelectionFinished = useCallback(
		(
			position: ScaledPosition,
			content: { text?: string; image?: string },
			hideTipAndSelection: () => void,
			_transformSelection: () => void,
		) => {
			lastSelectedTextRef.current = content.text ?? null;

			const currentTool = useAnnotationStore.getState().activeTool;
			const currentColor = useAnnotationStore.getState().activeColor;

			const doAdd = async (
				type: AnnotationType,
				color: string,
				pos: ScaledPosition,
				cnt: { text?: string; image?: string },
			) => {
				if (!paperId) return null;
				if (annotationOverrides) {
					return annotationOverrides.addAnnotation(type, color, pos, cnt);
				}
				return storeAddAnnotation(paperId, type, color, pos, cnt, sourceFile);
			};

			if (currentTool === "highlight" || currentTool === "underline") {
				(async () => {
					await doAdd(currentTool, currentColor, position, content);
					hideTipAndSelection();
				})();
				return null;
			}

			if (currentTool === "note") {
				hideTipAndSelection();
				return null;
			}

			const dismiss = () => {
				lastSelectedTextRef.current = null;
				hideTipAndSelection();
			};

			return (
				<AnnotationToolbar
					onConfirm={async (
						type: AnnotationType,
						color: string,
						comment: string,
					) => {
						const result = await doAdd(type, color, position, content);
						if (result && comment) {
							if (annotationOverrides) {
								await annotationOverrides.updateAnnotation(
									result.id,
									undefined,
									comment,
								);
							} else {
								const { updateAnnotation } = useAnnotationStore.getState();
								await updateAnnotation(result.id, undefined, comment);
							}
						}
						dismiss();
					}}
					onCancel={dismiss}
					selectedText={content.text}
					onCite={() => {
						useNoteStore.getState().setCitationClipboard({
							format: "pdf",
							selectedText: content.text ?? "",
							position: JSON.stringify(position),
							pageNumber: position.pageNumber,
						});
						dismiss();
					}}
				/>
			);
		},
		[paperId, sourceFile, annotationOverrides, storeAddAnnotation],
	);

	// Worker URL imported statically via Vite's ?url suffix — guaranteed correct path
	const workerUrl = pdfjsWorkerUrl;

	const pdfScaleValue = "auto";

	if (loading) {
		return (
			<div className="flex h-full items-center justify-center text-muted-foreground">
				<Loader2 className="h-8 w-8 animate-spin" />
				<span className="ml-2 text-sm">{t("reader.loadingPdf")}</span>
			</div>
		);
	}

	if (error) {
		return (
			<div className="flex h-full items-center justify-center text-muted-foreground">
				<div className="text-center">
					<FileText className="mx-auto mb-4 h-16 w-16" />
					<p className="text-sm">{t("reader.failedToLoadPdf")}</p>
					<p className="text-xs mt-2 max-w-md text-destructive">{error}</p>
				</div>
			</div>
		);
	}

	if (!pdfBlobUrl) {
		return null;
	}

	return (
		<div ref={containerRef} className="h-full w-full relative">
			{/* key forces full unmount/remount so PdfHighlighter.init() is never
			   called twice on the same viewer (React StrictMode double-mount fix) */}
			<PdfLoader
				key={pdfBlobUrl}
				url={pdfBlobUrl}
				workerSrc={workerUrl}
				beforeLoad={
					<div className="flex h-full items-center justify-center">
						<Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
					</div>
				}
			>
				{(pdfDocument) => {
					// Store the pdfDocument reference for thumbnails/outline/search
					handlePdfDocument(pdfDocument);

					return (
						<PdfHighlighter<ZoroHighlight>
							ref={
								pdfHighlighterRef as React.Ref<PdfHighlighter<ZoroHighlight>>
							}
							pdfDocument={pdfDocument}
							pdfScaleValue={pdfScaleValue}
							enableAreaSelection={(event) => event.altKey}
							onScrollChange={handleScroll}
							scrollRef={() => {
								// We bypass the library's scrollRef mechanism (see useEffect above).
								// This callback is required by PdfHighlighter but we don't use it.
							}}
							highlights={highlightsForViewer}
							onSelectionFinished={handleSelectionFinished}
							highlightTransform={(
								highlight: T_ViewportHighlight<ZoroHighlight>,
								index: number,
								setTip,
								hideTip,
								_viewportToScaled,
								_screenshot,
								isScrolledTo,
							) => {
								const isGhostHighlight = !highlight.id;
								const isAreaHighlight = !!(highlight as ZoroHighlight)
									.imageData;
								const annotationType =
									(highlight as ZoroHighlight).type || "highlight";

								// --- DEBUG ---
								if (index === 0) {
									console.log(
										`[PDFViewer:${sourceFile}] highlightTransform called, ` +
											`total highlights on this page, first id=${highlight.id}, ` +
											`type=${annotationType}`,
									);
								}

								const makeCiteHandler = () => {
									const h = highlight as unknown as ZoroHighlight;
									useNoteStore.getState().setCitationClipboard({
										format: "pdf",
										selectedText: h.selectedText ?? "",
										position: JSON.stringify(h.position),
										pageNumber: h.pageNumber,
									});
									hideTip();
								};

								if (annotationType === "note") {
									const vp = highlight.position.boundingRect;
									return (
										<Popup
											popupContent={
												<HighlightPopup
													highlight={highlight as unknown as ZoroHighlight}
													onClose={hideTip}
													onCite={makeCiteHandler}
													onUpdateAnnotation={
														annotationOverrides?.updateAnnotation
													}
													onDeleteAnnotation={
														annotationOverrides?.deleteAnnotation
													}
												/>
											}
											onMouseOver={(popupContent) =>
												setTip(highlight, () => popupContent)
											}
											onMouseOut={hideTip}
											key={index}
										>
											<div
												className="zr-pdf-note"
												data-annotation-id={highlight.id}
												style={{
													position: "absolute",
													left: `${vp.left}px`,
													top: `${vp.top}px`,
													zIndex: 10,
												}}
											>
												<StickyNoteIcon
													color={(highlight as ZoroHighlight).color}
													size={24}
												/>
											</div>
										</Popup>
									);
								}

								const component = isAreaHighlight ? (
									<AreaHighlight
										isScrolledTo={isScrolledTo}
										highlight={highlight}
										onChange={() => {
											// Area resize not supported for persisted annotations
										}}
									/>
								) : (
									<div
										className={
											annotationType === "underline"
												? "pdf-underline"
												: "pdf-highlight"
										}
									>
										<Highlight
											isScrolledTo={isScrolledTo}
											position={highlight.position}
											comment={highlight.comment}
										/>
									</div>
								);

								return (
									<div
										key={index}
										className={isGhostHighlight ? "ghost-highlight" : undefined}
										style={{ cursor: "pointer" }}
										onPointerDown={(e) => {
											console.log(
												`[PDFViewer:${sourceFile}] onPointerDown id=${highlight.id}`,
											);
											e.stopPropagation();
										}}
										onMouseDown={(e) => {
											console.log(
												`[PDFViewer:${sourceFile}] onMouseDown id=${highlight.id}`,
											);
											e.stopPropagation();
											e.preventDefault();
										}}
										onClick={() => {
											console.log(
												`[PDFViewer:${sourceFile}] onClick id=${highlight.id}, ` +
													`type=${annotationType}, hasRef=${!!pdfHighlighterRef.current}`,
											);

											const popupContent = (
												<HighlightPopup
													highlight={highlight as unknown as ZoroHighlight}
													onClose={hideTip}
													onCite={makeCiteHandler}
													onUpdateAnnotation={
														annotationOverrides?.updateAnnotation
													}
													onDeleteAnnotation={
														annotationOverrides?.deleteAnnotation
													}
												/>
											);

											setTip(highlight, () => popupContent);

											const h = pdfHighlighterRef.current;
											if (h) {
												const state = (
													h as unknown as {
														state: Record<string, unknown>;
													}
												).state;
												console.log(
													`[PDFViewer:${sourceFile}] PdfHighlighter state: ` +
														`tipPos=${!!state.tipPosition}, ` +
														`isCollapsed=${state.isCollapsed}, ` +
														`ghost=${!!state.ghostHighlight}, ` +
														`areaSel=${state.isAreaSelectionInProgress}`,
												);
												(
													h as unknown as {
														setState: (s: object) => void;
													}
												).setState({
													tipPosition: highlight.position,
													tipChildren: popupContent,
												});
												console.log(
													`[PDFViewer:${sourceFile}] force-set tip via ref OK`,
												);
											} else {
												console.warn(`[PDFViewer:${sourceFile}] ref is NULL`);
											}
										}}
									>
										{component}
									</div>
								);
							}}
						/>
					);
				}}
			</PdfLoader>
			{/* Custom styles for zoom, annotation colors, underlines, and ghost highlights */}
			<style>{`
        .pdfViewer {
          zoom: ${zoomLevel};
        }
        ${
					activeTool === "note"
						? `
        .textLayer { cursor: crosshair !important; }
        .PdfHighlighter { cursor: crosshair; }
        `
						: ""
				}
        .Highlight__part {
          background: var(--highlight-color, rgba(255, 226, 143, 0.5)) !important;
        }
        .pdf-underline .Highlight__part {
          background: transparent !important;
          border-bottom: 2px solid var(--highlight-color, #ffe28f) !important;
        }
        .ghost-highlight .Highlight__part {
          background: rgba(0, 0, 0, 0.2) !important;
        }
        .PdfHighlighter__highlight-layer {
          z-index: 4 !important;
          pointer-events: auto !important;
        }
        .Highlight__part {
          cursor: pointer !important;
          pointer-events: auto !important;
        }
        /* ── Compositing optimisations (light + dark) ── */
        .page {
          contain: layout style paint;
          content-visibility: auto;
          contain-intrinsic-size: auto 800px;
        }
        .page canvas {
          will-change: transform;
        }
        [class*='_container_'] {
          will-change: scroll-position;
          overflow-anchor: none;
        }
        ${
					isDark
						? `
        .page canvas {
          filter: invert(0.88) hue-rotate(180deg);
          will-change: filter, transform;
        }
        .pdfViewer {
          background-color: #1a1a1a !important;
        }
        .page {
          background-color: #1a1a1a !important;
          box-shadow: 0 0 8px rgba(0,0,0,0.5) !important;
          contain: layout style paint;
        }
        [class*='_container_'] {
          background-color: #1a1a1a !important;
        }
        /* Dark-mode highlight: counter-invert the highlight layer so that
           colours appear vivid on the inverted canvas, matching Zotero. */
        .PdfHighlighter__highlight-layer {
          filter: invert(0.88) hue-rotate(180deg);
        }
        .pdf-underline .Highlight__part {
          border-bottom-width: 3px !important;
        }
        `
						: ""
				}
      `}</style>
			{/* Apply per-annotation colors via dynamic CSS */}
			<AnnotationColorStyles
				annotations={annotations.filter(
					(a) => a.type !== "ink" && a.type !== "note",
				)}
				containerEl={containerRef.current}
			/>
			{/* Ink drawing canvas overlay */}
			<InkCanvas
				paperId={paperId ?? null}
				sourceFile={sourceFile}
				overrideAnnotations={annotationOverrides?.annotations}
				overrideAddInk={annotationOverrides?.addInkAnnotation}
				overrideDelete={annotationOverrides?.deleteAnnotation}
				containerEl={containerRef.current}
			/>
			{/* Citation reference hover preview */}
			{!isolated && (
				<CitationPreview
					pdfDocument={storePdfDocument}
					containerEl={containerRef.current}
				/>
			)}
		</div>
	);
}

/** Inject per-annotation color CSS variables */
function AnnotationColorStyles({
	annotations,
	containerEl,
}: {
	annotations: ZoroHighlight[];
	containerEl?: HTMLElement | null;
}) {
	useEffect(() => {
		if (annotations.length === 0) return;

		const applyColors = () => {
			const root: ParentNode = containerEl ?? document;
			const allParts = root.querySelectorAll(".Highlight__part");
			if (allParts.length === 0) return;

			let currentIdx = 0;
			for (const ann of annotations) {
				const isUnderline = ann.type === "underline";
				const rects = ann.position.rects || [];
				const numRects = Math.max(rects.length, 1);

				for (let i = 0; i < numRects && currentIdx < allParts.length; i++) {
					const part = allParts[currentIdx] as HTMLElement;
					if (part) {
						// Skip parts currently being flashed by the scroll-to animation
						if (!part.dataset.zcFlashing) {
							if (isUnderline) {
								part.style.setProperty(
									"background",
									"transparent",
									"important",
								);
								part.style.setProperty(
									"border-bottom",
									`2px solid ${ann.color}`,
									"important",
								);
							} else {
								part.style.setProperty(
									"background",
									hexToRgba(ann.color, 0.4),
									"important",
								);
							}
						}
					}
					currentIdx++;
				}
			}
		};

		// Retry applyColors at increasing intervals to handle the case where
		// Highlight__part elements don't exist yet (PDF still rendering).
		const timers = [100, 300, 600, 1200, 2500].map((delay) =>
			setTimeout(applyColors, delay),
		);

		const observeTarget = containerEl ?? document.body;
		let debounceTimer: ReturnType<typeof setTimeout> | null = null;
		const observer = new MutationObserver(() => {
			// Debounce: coalesce rapid DOM mutations (e.g. fast scrolling) into
			// a single applyColors call instead of firing on every mutation.
			if (debounceTimer) clearTimeout(debounceTimer);
			debounceTimer = setTimeout(applyColors, 150);
		});
		observer.observe(observeTarget, { childList: true, subtree: true });

		return () => {
			if (debounceTimer) clearTimeout(debounceTimer);
			for (const t of timers) clearTimeout(t);
			observer.disconnect();
		};
	}, [annotations, containerEl]);
	return null;
}

function hexToRgba(hex: string, alpha: number): string {
	const r = Number.parseInt(hex.slice(1, 3), 16);
	const g = Number.parseInt(hex.slice(3, 5), 16);
	const b = Number.parseInt(hex.slice(5, 7), 16);
	return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}
