// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { useAnnotationStore } from "@/stores/annotationStore";
import type {
	InkAnnotationData,
	InkPoint,
	InkStroke,
	ZoroHighlight,
} from "@/stores/annotationStore";
import { useCallback, useEffect, useRef, useState } from "react";

/**
 * Convert an array of points to an SVG path `d` attribute string.
 * Uses quadratic bezier curves for smooth lines.
 */
export function pointsToSvgPath(points: InkPoint[]): string {
	if (points.length === 0) return "";
	if (points.length === 1) {
		return `M ${points[0].x} ${points[0].y} L ${points[0].x} ${points[0].y}`;
	}
	if (points.length === 2) {
		return `M ${points[0].x} ${points[0].y} L ${points[1].x} ${points[1].y}`;
	}

	let d = `M ${points[0].x} ${points[0].y}`;
	for (let i = 1; i < points.length - 1; i++) {
		const midX = (points[i].x + points[i + 1].x) / 2;
		const midY = (points[i].y + points[i + 1].y) / 2;
		d += ` Q ${points[i].x} ${points[i].y} ${midX} ${midY}`;
	}
	const last = points[points.length - 1];
	d += ` L ${last.x} ${last.y}`;
	return d;
}

/**
 * Get the page element and its dimensions for a given point in the viewer.
 * Returns the page number, the page element, and the point relative to the page.
 */
function getPageAtPoint(
	clientX: number,
	clientY: number,
): {
	pageNumber: number;
	pageEl: HTMLElement;
	relX: number;
	relY: number;
} | null {
	const pages = document.querySelectorAll<HTMLElement>(
		".page[data-page-number]",
	);
	for (const page of pages) {
		const rect = page.getBoundingClientRect();
		if (
			clientX >= rect.left &&
			clientX <= rect.right &&
			clientY >= rect.top &&
			clientY <= rect.bottom
		) {
			const pageNumber = Number.parseInt(page.dataset.pageNumber ?? "0", 10);
			// Return coordinates as percentage of page dimensions (0-100 scale)
			const relX = ((clientX - rect.left) / rect.width) * 100;
			const relY = ((clientY - rect.top) / rect.height) * 100;
			return { pageNumber, pageEl: page, relX, relY };
		}
	}
	return null;
}

/**
 * Check if a point is near an ink stroke (for eraser).
 */
function isPointNearStroke(
	px: number,
	py: number,
	stroke: InkStroke,
	threshold: number,
): boolean {
	for (const pt of stroke.points) {
		const dx = px - pt.x;
		const dy = py - pt.y;
		if (dx * dx + dy * dy < threshold * threshold) {
			return true;
		}
	}
	return false;
}

/** Parse ink strokes from an annotation's position data */
function getInkStrokes(ann: ZoroHighlight): InkStroke[] {
	try {
		// position is a parsed object (ScaledPosition + extra inkStrokes field)
		// The inkStrokes field survives JSON.parse but is not in the ScaledPosition type
		const pos = ann.position as unknown as Record<string, unknown>;
		const strokes = pos.inkStrokes;
		if (Array.isArray(strokes)) {
			return strokes as InkStroke[];
		}
		return [];
	} catch {
		return [];
	}
}

/**
 * Group ink annotations by page number.
 */
function groupInkByPage(
	annotations: ZoroHighlight[],
): Record<number, Array<{ ann: ZoroHighlight; strokes: InkStroke[] }>> {
	const byPage: Record<
		number,
		Array<{ ann: ZoroHighlight; strokes: InkStroke[] }>
	> = {};
	for (const ann of annotations) {
		if (ann.type !== "ink") continue;
		const strokes = getInkStrokes(ann);
		if (strokes.length > 0) {
			const page = ann.pageNumber;
			if (!byPage[page]) byPage[page] = [];
			byPage[page].push({ ann, strokes });
		}
	}
	return byPage;
}

/**
 * Render ink SVG paths into a page's DOM element.
 * Creates or updates an SVG element inside the page div.
 */
function renderInkOnPage(
	pageEl: HTMLElement,
	pageNum: number,
	inkAnnotations: Array<{ ann: ZoroHighlight; strokes: InkStroke[] }>,
) {
	const svgId = `ink-layer-${pageNum}`;
	let svg = pageEl.querySelector<SVGSVGElement>(`#${svgId}`);

	if (inkAnnotations.length === 0) {
		svg?.remove();
		return;
	}

	if (!svg) {
		svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
		svg.id = svgId;
		svg.style.position = "absolute";
		svg.style.top = "0";
		svg.style.left = "0";
		svg.style.width = "100%";
		svg.style.height = "100%";
		svg.style.pointerEvents = "none";
		svg.style.zIndex = "5";
		svg.setAttribute("viewBox", "0 0 100 100");
		svg.setAttribute("preserveAspectRatio", "none");
		pageEl.style.position = "relative";
		pageEl.appendChild(svg);
	}

	// Clear existing paths and re-render
	svg.innerHTML = "";

	for (const { ann, strokes } of inkAnnotations) {
		for (const stroke of strokes) {
			const path = document.createElementNS(
				"http://www.w3.org/2000/svg",
				"path",
			);
			path.setAttribute("d", pointsToSvgPath(stroke.points));
			path.setAttribute("fill", "none");
			path.setAttribute("stroke", ann.color);
			path.setAttribute("stroke-width", String(stroke.strokeWidth));
			path.setAttribute("stroke-linecap", "round");
			path.setAttribute("stroke-linejoin", "round");
			path.setAttribute("vector-effect", "non-scaling-stroke");
			path.dataset.annotationId = ann.id;
			svg.appendChild(path);
		}
	}
}

/**
 * Render all ink annotations across all visible pages.
 * This is the core rendering function called by effects.
 * When scope is provided, DOM queries are scoped to that container.
 */
function renderAllInk(
	annotations: ZoroHighlight[],
	scope?: HTMLElement | null,
) {
	const root: ParentNode = scope ?? document;
	const grouped = groupInkByPage(annotations);
	const pagesWithInk = new Set<number>();

	for (const [pageNumStr, inkAnns] of Object.entries(grouped)) {
		const pageNum = Number.parseInt(pageNumStr, 10);
		pagesWithInk.add(pageNum);
		const pageEl = root.querySelector<HTMLElement>(
			`.page[data-page-number="${pageNum}"]`,
		);
		if (pageEl) {
			renderInkOnPage(pageEl, pageNum, inkAnns);
		}
	}

	const allPages = root.querySelectorAll<HTMLElement>(
		".page[data-page-number]",
	);
	for (const page of allPages) {
		const pageNum = Number.parseInt(page.dataset.pageNumber ?? "0", 10);
		if (!pagesWithInk.has(pageNum)) {
			const svg = page.querySelector(`#ink-layer-${pageNum}`);
			svg?.remove();
		}
	}
}

interface InkCanvasProps {
	paperId: string | null;
	sourceFile?: string;
	/** Override annotations (for isolated/bilingual mode) */
	overrideAnnotations?: ZoroHighlight[];
	/** Override addInkAnnotation (for isolated/bilingual mode) */
	overrideAddInk?: (
		color: string,
		pageNumber: number,
		inkData: InkAnnotationData,
	) => Promise<ZoroHighlight | null>;
	/** Override deleteAnnotation (for isolated/bilingual mode) */
	overrideDelete?: (id: string) => Promise<void>;
	/** Scope DOM queries to this container (for bilingual mode) */
	containerEl?: HTMLElement | null;
}

/**
 * InkCanvas handles freehand drawing on PDF pages AND rendering saved ink
 * annotations.
 *
 * Rendering approach: Ink annotations are rendered by injecting SVG elements
 * directly into each PDF page's DOM element. This ensures ink strokes scroll
 * naturally with the pages without any positioning/overlay complexity.
 *
 * Drawing: A transparent overlay captures pointer events when the ink tool
 * is active. The overlay is removed when switching to other tools.
 *
 * IMPORTANT: All hooks must be called unconditionally (before any early return)
 * to satisfy React's rules of hooks. The early return at the bottom only
 * affects the JSX overlay — the rendering effects always run.
 */
export function InkCanvas({
	paperId,
	sourceFile,
	overrideAnnotations,
	overrideAddInk,
	overrideDelete,
	containerEl,
}: InkCanvasProps) {
	const activeTool = useAnnotationStore((s) => s.activeTool);
	const activeColor = useAnnotationStore((s) => s.activeColor);
	const inkStrokeWidth = useAnnotationStore((s) => s.inkStrokeWidth);
	const inkEraserActive = useAnnotationStore((s) => s.inkEraserActive);
	const storeAnnotations = useAnnotationStore((s) => s.annotations);
	const storeAddInk = useAnnotationStore((s) => s.addInkAnnotation);
	const storeDelete = useAnnotationStore((s) => s.deleteAnnotation);

	const annotations = overrideAnnotations ?? storeAnnotations;

	const [currentStroke, setCurrentStroke] = useState<InkPoint[]>([]);
	const [currentPage, setCurrentPage] = useState<number>(0);
	const [currentPageEl, setCurrentPageEl] = useState<HTMLElement | null>(null);
	const isDrawing = useRef(false);
	// Track the latest annotations in a ref so the MutationObserver callback
	// always has access to the current value without re-subscribing.
	const annotationsRef = useRef(annotations);
	annotationsRef.current = annotations;

	const isInkMode = activeTool === "ink";

	const containerElRef = useRef(containerEl);
	containerElRef.current = containerEl;

	// ─── EFFECT 1: Render saved ink annotations into page DOM ───
	useEffect(() => {
		const rafId = requestAnimationFrame(() => {
			renderAllInk(annotations, containerElRef.current);
		});
		return () => cancelAnimationFrame(rafId);
	}, [annotations]);

	// ─── EFFECT 2: Re-render when pages are added/removed (lazy loading) ───
	useEffect(() => {
		let debounceTimer: ReturnType<typeof setTimeout> | null = null;

		const observer = new MutationObserver((mutations) => {
			let pageAdded = false;
			for (const m of mutations) {
				for (const node of m.addedNodes) {
					if (
						node instanceof HTMLElement &&
						(node.classList?.contains("page") || node.querySelector?.(".page"))
					) {
						pageAdded = true;
						break;
					}
				}
				if (pageAdded) break;
			}
			if (pageAdded) {
				// debounce：快速翻页时合并多次渲染
				if (debounceTimer) clearTimeout(debounceTimer);
				debounceTimer = setTimeout(() => {
					renderAllInk(annotationsRef.current, containerElRef.current);
				}, 150);
			}
		});

		const root: ParentNode = containerElRef.current ?? document;
		const pdfViewer = root.querySelector(".pdfViewer");
		if (pdfViewer) {
			observer.observe(pdfViewer, { childList: true, subtree: true });
		}

		return () => {
			if (debounceTimer) clearTimeout(debounceTimer);
			observer.disconnect();
		};
	}, []);

	// ─── EFFECT 3: Safety net — 利用空闲时间检查 ink SVG 是否存在 ───
	useEffect(() => {
		let disposed = false;
		let timeoutId: ReturnType<typeof setTimeout> | null = null;

		const checkInkSvgs = () => {
			if (disposed) return;

			const doCheck = () => {
				if (disposed) return;
				const anns = annotationsRef.current;
				const inkAnns = anns.filter((a) => a.type === "ink");
				if (inkAnns.length === 0) {
					// 没有 ink 标注时延长检查间隔到 5 秒
					timeoutId = setTimeout(checkInkSvgs, 5000);
					return;
				}

				const root: ParentNode = containerElRef.current ?? document;
				const grouped = groupInkByPage(anns);
				let missing = false;
				for (const pageNumStr of Object.keys(grouped)) {
					const pageNum = Number.parseInt(pageNumStr, 10);
					const pageEl = root.querySelector<HTMLElement>(
						`.page[data-page-number="${pageNum}"]`,
					);
					if (pageEl && !pageEl.querySelector(`#ink-layer-${pageNum}`)) {
						missing = true;
						break;
					}
				}
				if (missing) {
					renderAllInk(anns, containerElRef.current);
				}
				// 正常间隔 3 秒
				timeoutId = setTimeout(checkInkSvgs, 3000);
			};

			// 优先在浏览器空闲时执行，避免阻塞滚动/渲染
			if (typeof requestIdleCallback === "function") {
				requestIdleCallback(() => doCheck(), { timeout: 3000 });
			} else {
				doCheck();
			}
		};

		// 首次延迟 2 秒后开始检查
		timeoutId = setTimeout(checkInkSvgs, 2000);

		return () => {
			disposed = true;
			if (timeoutId) clearTimeout(timeoutId);
		};
	}, []);

	// ─── Drawing handlers ───

	const handlePointerDown = useCallback(
		(e: React.PointerEvent) => {
			if (!isInkMode) return;

			const pageInfo = getPageAtPoint(e.clientX, e.clientY);
			if (!pageInfo) return;

			if (inkEraserActive) {
				const inkAnnotations = annotations.filter(
					(a) => a.type === "ink" && a.pageNumber === pageInfo.pageNumber,
				);
				for (const ann of inkAnnotations) {
					const strokes = getInkStrokes(ann);
					for (const stroke of strokes) {
						if (isPointNearStroke(pageInfo.relX, pageInfo.relY, stroke, 3)) {
							if (overrideDelete) {
								overrideDelete(ann.id);
							} else if (paperId) {
								storeDelete(ann.id, paperId);
							}
							break;
						}
					}
				}
				return;
			}

			e.preventDefault();
			e.stopPropagation();
			(e.target as HTMLElement).setPointerCapture(e.pointerId);
			isDrawing.current = true;
			setCurrentPage(pageInfo.pageNumber);
			setCurrentPageEl(pageInfo.pageEl);
			setCurrentStroke([{ x: pageInfo.relX, y: pageInfo.relY }]);
		},
		[
			isInkMode,
			inkEraserActive,
			annotations,
			paperId,
			overrideDelete,
			storeDelete,
		],
	);

	const handlePointerMove = useCallback(
		(e: React.PointerEvent) => {
			if (!isDrawing.current || !isInkMode) return;

			const pageInfo = getPageAtPoint(e.clientX, e.clientY);
			if (!pageInfo || pageInfo.pageNumber !== currentPage) return;

			e.preventDefault();
			setCurrentStroke((prev) => [
				...prev,
				{ x: pageInfo.relX, y: pageInfo.relY },
			]);
		},
		[isInkMode, currentPage],
	);

	const handlePointerUp = useCallback(
		async (e: React.PointerEvent) => {
			if (!isDrawing.current || !isInkMode) return;
			isDrawing.current = false;
			(e.target as HTMLElement).releasePointerCapture(e.pointerId);

			if (currentStroke.length < 2 || !paperId) {
				setCurrentStroke([]);
				setCurrentPageEl(null);
				return;
			}

			// Save a reference to the page element and stroke data before clearing state
			const savedPageEl = currentPageEl;
			const savedPageNum = currentPage;
			const savedStroke = currentStroke;
			const savedColor = activeColor;
			const savedWidth = inkStrokeWidth;

			// Calculate bounding rect
			let x1 = Number.POSITIVE_INFINITY;
			let y1 = Number.POSITIVE_INFINITY;
			let x2 = Number.NEGATIVE_INFINITY;
			let y2 = Number.NEGATIVE_INFINITY;
			for (const pt of savedStroke) {
				x1 = Math.min(x1, pt.x);
				y1 = Math.min(y1, pt.y);
				x2 = Math.max(x2, pt.x);
				y2 = Math.max(y2, pt.y);
			}

			const inkData: InkAnnotationData = {
				strokes: [{ points: savedStroke, strokeWidth: savedWidth }],
				boundingRect: { x1, y1, x2, y2 },
			};

			// Clear the current stroke state (removes the temporary drawing SVG)
			setCurrentStroke([]);
			setCurrentPageEl(null);

			// Immediately render the stroke as a "preview" on the page while we
			// wait for the backend to save. This prevents the visual gap between
			// the temporary drawing SVG being removed and the permanent one appearing.
			if (savedPageEl) {
				const previewSvgId = `ink-preview-${savedPageNum}`;
				let previewSvg = savedPageEl.querySelector<SVGSVGElement>(
					`#${previewSvgId}`,
				);
				if (!previewSvg) {
					previewSvg = document.createElementNS(
						"http://www.w3.org/2000/svg",
						"svg",
					);
					previewSvg.id = previewSvgId;
					previewSvg.style.position = "absolute";
					previewSvg.style.top = "0";
					previewSvg.style.left = "0";
					previewSvg.style.width = "100%";
					previewSvg.style.height = "100%";
					previewSvg.style.pointerEvents = "none";
					previewSvg.style.zIndex = "6";
					previewSvg.setAttribute("viewBox", "0 0 100 100");
					previewSvg.setAttribute("preserveAspectRatio", "none");
					savedPageEl.appendChild(previewSvg);
				}
				const path = document.createElementNS(
					"http://www.w3.org/2000/svg",
					"path",
				);
				path.setAttribute("d", pointsToSvgPath(savedStroke));
				path.setAttribute("fill", "none");
				path.setAttribute("stroke", savedColor);
				path.setAttribute("stroke-width", String(savedWidth));
				path.setAttribute("stroke-linecap", "round");
				path.setAttribute("stroke-linejoin", "round");
				path.setAttribute("vector-effect", "non-scaling-stroke");
				previewSvg.appendChild(path);
			}

			if (overrideAddInk) {
				await overrideAddInk(savedColor, savedPageNum, inkData);
			} else {
				await storeAddInk(
					paperId,
					savedColor,
					savedPageNum,
					inkData,
					sourceFile,
				);
			}

			// Remove the preview SVG now that the permanent layer is rendered
			if (savedPageEl) {
				const previewSvg = savedPageEl.querySelector(
					`#ink-preview-${savedPageNum}`,
				);
				previewSvg?.remove();
			}
		},
		[
			isInkMode,
			currentStroke,
			currentPage,
			currentPageEl,
			paperId,
			sourceFile,
			activeColor,
			inkStrokeWidth,
			overrideAddInk,
			storeAddInk,
		],
	);

	// ─── EFFECT 4: Render current drawing stroke as temporary SVG ───
	useEffect(() => {
		if (!currentPageEl || currentStroke.length < 2) return;

		const pageNum = Number.parseInt(
			currentPageEl.dataset.pageNumber ?? "0",
			10,
		);
		const svgId = `ink-drawing-${pageNum}`;
		let svg = currentPageEl.querySelector<SVGSVGElement>(`#${svgId}`);

		if (!svg) {
			svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
			svg.id = svgId;
			svg.style.position = "absolute";
			svg.style.top = "0";
			svg.style.left = "0";
			svg.style.width = "100%";
			svg.style.height = "100%";
			svg.style.pointerEvents = "none";
			svg.style.zIndex = "6";
			svg.setAttribute("viewBox", "0 0 100 100");
			svg.setAttribute("preserveAspectRatio", "none");
			currentPageEl.appendChild(svg);
		}

		svg.innerHTML = "";
		const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
		path.setAttribute("d", pointsToSvgPath(currentStroke));
		path.setAttribute("fill", "none");
		path.setAttribute("stroke", activeColor);
		path.setAttribute("stroke-width", String(inkStrokeWidth));
		path.setAttribute("stroke-linecap", "round");
		path.setAttribute("stroke-linejoin", "round");
		path.setAttribute("vector-effect", "non-scaling-stroke");
		svg.appendChild(path);

		return () => {
			// Clean up temporary drawing SVG when stroke is finished
			if (!isDrawing.current) {
				svg?.remove();
			}
		};
	}, [currentStroke, currentPageEl, activeColor, inkStrokeWidth]);

	// ─── EFFECT 5: Clean up drawing SVG when stroke is cleared ───
	useEffect(() => {
		if (currentStroke.length === 0 && currentPageEl) {
			const pageNum = Number.parseInt(
				currentPageEl.dataset.pageNumber ?? "0",
				10,
			);
			const svg = currentPageEl.querySelector(`#ink-drawing-${pageNum}`);
			svg?.remove();
		}
	}, [currentStroke, currentPageEl]);

	// Only render the input overlay when ink tool is active.
	// All hooks above run unconditionally regardless of this return.
	if (!isInkMode) return null;

	return (
		<div
			className="absolute inset-0 z-10"
			style={{
				pointerEvents: "auto",
				cursor: inkEraserActive ? "crosshair" : "crosshair",
			}}
			onPointerDown={handlePointerDown}
			onPointerMove={handlePointerMove}
			onPointerUp={handlePointerUp}
		/>
	);
}
