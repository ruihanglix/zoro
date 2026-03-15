// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import type {
	AnnotationType,
	InkAnnotationData,
	ZoroHighlight,
} from "@/stores/annotationStore";
import { useEffect, useMemo, useRef } from "react";
import type { Content, ScaledPosition } from "react-pdf-highlighter";
import { PdfAnnotationViewer } from "./PdfAnnotationViewer";

interface TranslationAnns {
	annotations: ZoroHighlight[];
	fetchAnnotations: () => Promise<void>;
	addAnnotation: (
		type: AnnotationType,
		color: string,
		position: ScaledPosition,
		content: Content,
	) => Promise<ZoroHighlight | null>;
	addInkAnnotation: (
		color: string,
		pageNumber: number,
		inkData: InkAnnotationData,
	) => Promise<ZoroHighlight | null>;
	updateAnnotation: (
		id: string,
		color?: string | null,
		comment?: string | null,
	) => Promise<void>;
	deleteAnnotation: (id: string) => Promise<void>;
}

interface BilingualPdfViewerProps {
	paperId: string | null;
	pdfFilename?: string;
	translationFile: string;
	syncScroll: boolean;
	translationAnns: TranslationAnns;
	/** Whether this viewer's tab is currently active */
	isActive?: boolean;
	/** Ref to receive the right-pane scroll-to-highlight function */
	translationScrollRef?: React.MutableRefObject<
		((h: ZoroHighlight) => void) | null
	>;
}

/**
 * Side-by-side bilingual PDF viewer.
 * Left pane: original PDF (paper.pdf) — uses the global annotationStore.
 * Right pane: translated PDF (e.g. paper.zh.pdf) — uses local annotation state.
 * Both panes support full annotation (highlight, underline, note, ink).
 */
export function BilingualPdfViewer({
	paperId,
	pdfFilename,
	translationFile,
	syncScroll,
	translationAnns,
	isActive = true,
	translationScrollRef,
}: BilingualPdfViewerProps) {
	const rootRef = useRef<HTMLDivElement>(null);
	const syncScrollRef = useRef(syncScroll);
	syncScrollRef.current = syncScroll;

	const annotationOverrides = useMemo(
		() => ({
			annotations: translationAnns.annotations,
			fetchAnnotations: translationAnns.fetchAnnotations,
			addAnnotation: translationAnns.addAnnotation,
			addInkAnnotation: translationAnns.addInkAnnotation,
			updateAnnotation: translationAnns.updateAnnotation,
			deleteAnnotation: translationAnns.deleteAnnotation,
		}),
		[
			translationAnns.annotations,
			translationAnns.fetchAnnotations,
			translationAnns.addAnnotation,
			translationAnns.addInkAnnotation,
			translationAnns.updateAnnotation,
			translationAnns.deleteAnnotation,
		],
	);

	useEffect(() => {
		const root = rootRef.current;
		if (!root) return;

		let leftContainer: HTMLElement | null = null;
		let rightContainer: HTMLElement | null = null;
		let cleanupListeners: (() => void) | null = null;

		const findContainers = (): boolean => {
			const panes = root.querySelectorAll<HTMLElement>(":scope > div");
			if (panes.length < 2) return false;
			leftContainer = panes[0].querySelector("[class*='_container_']");
			rightContainer = panes[1].querySelector("[class*='_container_']");
			return !!(leftContainer && rightContainer);
		};

		const attachListeners = () => {
			if (!leftContainer || !rightContainer) return;
			const l = leftContainer;
			const r = rightContainer;

			// ── Active-side tracking ──
			// Only the side the user is physically interacting with drives sync.
			let activeSide: "left" | "right" | null = null;
			let activeSideTimer: ReturnType<typeof setTimeout> | null = null;

			const setActiveSide = (side: "left" | "right") => {
				activeSide = side;
				if (activeSideTimer) clearTimeout(activeSideTimer);
				activeSideTimer = setTimeout(() => {
					activeSide = null;
				}, 200);
			};

			const onLeftPointerEnter = () => setActiveSide("left");
			const onRightPointerEnter = () => setActiveSide("right");
			const onLeftWheel = () => setActiveSide("left");
			const onRightWheel = () => setActiveSide("right");
			const onLeftTouchStart = () => setActiveSide("left");
			const onRightTouchStart = () => setActiveSide("right");

			// ── Page layout cache ──
			// Cache page offsetTop/height to avoid DOM reads during scroll sync.
			// The cache is invalidated lazily via MutationObserver (new pages) and
			// ResizeObserver (zoom / resize). The rAF hot path never touches the DOM.
			interface PageEntry {
				offsetTop: number;
				height: number;
				pageNumber: string;
			}
			interface PageIndex {
				byNumber: Map<string, { offsetTop: number; height: number }>;
			}
			let leftCache: PageEntry[] = [];
			let rightCache: PageEntry[] = [];
			let leftIndex: PageIndex = { byNumber: new Map() };
			let rightIndex: PageIndex = { byNumber: new Map() };
			let leftDirty = true;
			let rightDirty = true;

			const buildCache = (
				container: HTMLElement,
			): { entries: PageEntry[]; index: PageIndex } => {
				const pages = container.querySelectorAll<HTMLElement>(
					".page[data-page-number]",
				);
				const entries: PageEntry[] = new Array(pages.length);
				const byNumber = new Map<
					string,
					{ offsetTop: number; height: number }
				>();
				for (let i = 0; i < pages.length; i++) {
					const p = pages[i];
					const num = p.dataset.pageNumber || String(i + 1);
					const entry: PageEntry = {
						offsetTop: p.offsetTop,
						height: p.clientHeight,
						pageNumber: num,
					};
					entries[i] = entry;
					byNumber.set(num, {
						offsetTop: entry.offsetTop,
						height: entry.height,
					});
				}
				return { entries, index: { byNumber } };
			};

			/** Rebuild only the side(s) that have been marked dirty. */
			const ensureCaches = () => {
				if (leftDirty) {
					const c = buildCache(l);
					leftCache = c.entries;
					leftIndex = c.index;
					leftDirty = false;
				}
				if (rightDirty) {
					const c = buildCache(r);
					rightCache = c.entries;
					rightIndex = c.index;
					rightDirty = false;
				}
			};

			// Initial build
			ensureCaches();

			// ── Observers: mark cache dirty on DOM / layout changes ──
			// MutationObserver detects new/removed .page elements (lazy render).
			// Debounce so rapid page additions don't rebuild multiple times.
			let mutDebounce: ReturnType<typeof setTimeout> | null = null;
			const markBothDirty = () => {
				leftDirty = true;
				rightDirty = true;
			};
			const mutationObs = new MutationObserver(() => {
				if (mutDebounce) clearTimeout(mutDebounce);
				mutDebounce = setTimeout(markBothDirty, 100);
			});
			// Observe the .pdfViewer inside each container for page additions
			const lViewer = l.querySelector(".pdfViewer");
			const rViewer = r.querySelector(".pdfViewer");
			if (lViewer)
				mutationObs.observe(lViewer, { childList: true, subtree: true });
			if (rViewer)
				mutationObs.observe(rViewer, { childList: true, subtree: true });

			// ResizeObserver detects zoom / window-resize layout shifts that change
			// page offsetTop/height without adding/removing pages.
			const resizeObs = new ResizeObserver(() => {
				markBothDirty();
			});
			if (lViewer) resizeObs.observe(lViewer);
			if (rViewer) resizeObs.observe(rViewer);

			// ── Binary search for current page ──
			const findCurrentPage = (
				entries: PageEntry[],
				scrollTop: number,
			): PageEntry | null => {
				if (entries.length === 0) return null;
				let lo = 0;
				let hi = entries.length - 1;
				let result = 0;
				while (lo <= hi) {
					const mid = (lo + hi) >>> 1;
					if (entries[mid].offsetTop <= scrollTop + 1) {
						result = mid;
						lo = mid + 1;
					} else {
						hi = mid - 1;
					}
				}
				return entries[result];
			};

			// ── Core sync (hot path – pure memory when cache is clean) ──
			const syncByPage = (
				srcEntries: PageEntry[],
				tgtIndex: PageIndex,
				source: HTMLElement,
				target: HTMLElement,
			) => {
				const scrollTop = source.scrollTop;
				const page = findCurrentPage(srcEntries, scrollTop);
				if (!page) return;

				const intraPageOffset =
					(scrollTop - page.offsetTop) / Math.max(page.height, 1);

				const tgt = tgtIndex.byNumber.get(page.pageNumber);
				if (tgt) {
					target.scrollTop = tgt.offsetTop + intraPageOffset * tgt.height;
				} else {
					// Fallback: ratio-based
					const ratio =
						scrollTop / Math.max(source.scrollHeight - source.clientHeight, 1);
					target.scrollTop =
						ratio * Math.max(target.scrollHeight - target.clientHeight, 1);
				}
			};

			// ── Helper: get internal PDFViewer instance from a pane ──
			// PdfHighlighter (class component) stores its viewer at `this.viewer`.
			// We traverse the React fiber tree to find it, then call
			// `forceRendering()` after syncing scrollTop so that the passive side
			// pre-renders upcoming pages instead of waiting for its own scroll
			// event to catch up.
			const getPdfViewerFromPane = (
				pane: HTMLElement,
			): Record<string, unknown> | null => {
				try {
					const fiberKey = Object.keys(pane).find((k) =>
						k.startsWith("__reactFiber$"),
					);
					if (!fiberKey) return null;
					let fiber = (pane as unknown as Record<string, unknown>)[
						fiberKey
					] as Record<string, unknown> | null;
					for (let i = 0; i < 30 && fiber; i++) {
						const inst = fiber.stateNode as Record<string, unknown> | null;
						if (inst?.viewer) return inst.viewer as Record<string, unknown>;
						fiber = (fiber.return ?? fiber.child ?? null) as Record<
							string,
							unknown
						> | null;
					}
				} catch {
					/* ignore */
				}
				return null;
			};

			// Cache viewer instances – they don't change once mounted.
			let leftViewer: Record<string, unknown> | null = null;
			let rightViewer: Record<string, unknown> | null = null;

			// Pump-style forceRendering for passive side.
			// pdf.js only renders ONE page per forceRendering() call.
			// We pump it repeatedly (up to 10 times) so nearby pages get rendered.
			let leftPumpInterval: ReturnType<typeof setInterval> | null = null;
			let rightPumpInterval: ReturnType<typeof setInterval> | null = null;

			const triggerForceRendering = (
				viewer: Record<string, unknown> | null,
				side: "left" | "right",
			) => {
				if (!viewer || typeof viewer.forceRendering !== "function") return;
				// Don't start a new pump if one is already running
				if (side === "left" && leftPumpInterval) return;
				if (side === "right" && rightPumpInterval) return;

				let pumpCount = 0;
				const maxPumps = 10;
				const pump = () => {
					pumpCount++;
					try {
						const didRender = (viewer.forceRendering as () => boolean)();
						if (!didRender || pumpCount >= maxPumps) {
							// All nearby pages rendered or max pumps reached
							if (side === "left") {
								if (leftPumpInterval) clearInterval(leftPumpInterval);
								leftPumpInterval = null;
							} else {
								if (rightPumpInterval) clearInterval(rightPumpInterval);
								rightPumpInterval = null;
							}
						}
					} catch {
						if (side === "left") {
							if (leftPumpInterval) clearInterval(leftPumpInterval);
							leftPumpInterval = null;
						} else {
							if (rightPumpInterval) clearInterval(rightPumpInterval);
							rightPumpInterval = null;
						}
					}
				};
				const interval = setInterval(pump, 80);
				if (side === "left") leftPumpInterval = interval;
				else rightPumpInterval = interval;
				// Also run immediately
				pump();
			};

			// ── rAF-throttled scroll handlers ──
			// At most one sync per animation frame per side.
			// ensureCaches() is called here but it's a no-op when dirty flags are
			// false, so 99% of frames do zero DOM work.
			let leftRaf = 0;
			let rightRaf = 0;

			const onLeftScroll = () => {
				if (!syncScrollRef.current) return;
				if (activeSide === "right") return;
				if (leftRaf) return;
				leftRaf = requestAnimationFrame(() => {
					leftRaf = 0;
					ensureCaches();
					syncByPage(leftCache, rightIndex, l, r);
					// Trigger pre-rendering on the passive (right) side
					if (!rightViewer) rightViewer = getPdfViewerFromPane(r);
					triggerForceRendering(rightViewer, "right");
				});
			};

			const onRightScroll = () => {
				if (!syncScrollRef.current) return;
				if (activeSide === "left") return;
				if (rightRaf) return;
				rightRaf = requestAnimationFrame(() => {
					rightRaf = 0;
					ensureCaches();
					syncByPage(rightCache, leftIndex, r, l);
					// Trigger pre-rendering on the passive (left) side
					if (!leftViewer) leftViewer = getPdfViewerFromPane(l);
					triggerForceRendering(leftViewer, "left");
				});
			};

			// ── Attach listeners ──
			l.addEventListener("mouseenter", onLeftPointerEnter);
			r.addEventListener("mouseenter", onRightPointerEnter);
			l.addEventListener("wheel", onLeftWheel, { passive: true });
			r.addEventListener("wheel", onRightWheel, { passive: true });
			l.addEventListener("touchstart", onLeftTouchStart, { passive: true });
			r.addEventListener("touchstart", onRightTouchStart, { passive: true });
			l.addEventListener("scroll", onLeftScroll, { passive: true });
			r.addEventListener("scroll", onRightScroll, { passive: true });

			cleanupListeners = () => {
				l.removeEventListener("mouseenter", onLeftPointerEnter);
				r.removeEventListener("mouseenter", onRightPointerEnter);
				l.removeEventListener("wheel", onLeftWheel);
				r.removeEventListener("wheel", onRightWheel);
				l.removeEventListener("touchstart", onLeftTouchStart);
				r.removeEventListener("touchstart", onRightTouchStart);
				l.removeEventListener("scroll", onLeftScroll);
				r.removeEventListener("scroll", onRightScroll);
				if (activeSideTimer) clearTimeout(activeSideTimer);
				if (leftRaf) cancelAnimationFrame(leftRaf);
				if (rightRaf) cancelAnimationFrame(rightRaf);
				if (leftPumpInterval) clearInterval(leftPumpInterval);
				if (rightPumpInterval) clearInterval(rightPumpInterval);
				if (mutDebounce) clearTimeout(mutDebounce);
				mutationObs.disconnect();
				resizeObs.disconnect();
			};
		};

		const pollInterval = setInterval(() => {
			if (findContainers()) {
				clearInterval(pollInterval);
				attachListeners();
			}
		}, 300);

		if (findContainers()) {
			clearInterval(pollInterval);
			attachListeners();
		}

		return () => {
			clearInterval(pollInterval);
			cleanupListeners?.();
		};
	}, []);

	return (
		<div ref={rootRef} className="flex h-full w-full">
			{/* Original PDF */}
			<div className="flex-1 h-full min-w-0 border-r">
				<PdfAnnotationViewer
					paperId={paperId}
					pdfFilename={pdfFilename}
					isActive={isActive}
				/>
			</div>

			{/* Translated PDF */}
			<div className="flex-1 h-full min-w-0">
				<PdfAnnotationViewer
					paperId={paperId}
					pdfFilename={translationFile}
					isolated
					annotationOverrides={annotationOverrides}
					scrollToHighlightRef={translationScrollRef}
				/>
			</div>
		</div>
	);
}
