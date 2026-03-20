// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { useUiStore } from "@/stores/uiStore";
import type { CitationPreviewMode } from "@/stores/uiStore";
import type { PDFDocumentProxy } from "pdfjs-dist";
import { useEffect, useRef, useState } from "react";

interface TooltipState {
	x: number;
	y: number;
	above: boolean;
	text: string | null;
	imageDataUrl: string | null;
}

interface CitationPreviewProps {
	pdfDocument: PDFDocumentProxy | null;
	containerEl: HTMLElement | null;
}

const LINK_SELECTOR = 'a[href^="#"]';
const SHOW_DELAY = 250;
const HIDE_DELAY = 150;
const MAX_TEXT_LENGTH = 600;
const MAX_LINES = 10;
const TOOLTIP_TEXT_MAX_W = 420;
const TOOLTIP_IMAGE_MAX_W = 480;

const IMAGE_RENDER_SCALE = 1.5;
const IMAGE_CROP_HEIGHT = 200;
const IMAGE_CROP_PADDING_TOP = 20;

// ─── Destination resolver ───

interface ResolvedDest {
	pageIndex: number;
	targetY: number;
}

async function resolveDestination(
	pdfDocument: PDFDocumentProxy,
	destHash: string,
): Promise<ResolvedDest | null> {
	const decoded = decodeURIComponent(destHash);

	let destArray: unknown[] | null = null;

	if (decoded.startsWith("[")) {
		try {
			destArray = JSON.parse(decoded);
		} catch {
			/* not a JSON array */
		}
	}

	if (!destArray) {
		destArray = await pdfDocument.getDestination(decoded);
	}

	if (!destArray || destArray.length === 0) return null;

	const pageRef = destArray[0] as { num: number; gen: number };
	const pageIndex = await pdfDocument.getPageIndex(pageRef);
	const page = await pdfDocument.getPage(pageIndex + 1);
	const viewport = page.getViewport({ scale: 1 });

	const destY =
		typeof destArray[3] === "number" ? (destArray[3] as number) : null;
	const targetY = destY !== null ? viewport.height - destY : 0;

	return { pageIndex, targetY };
}

// ─── Text extraction ───

async function extractDestinationText(
	pdfDocument: PDFDocumentProxy,
	dest: ResolvedDest,
): Promise<string | null> {
	const page = await pdfDocument.getPage(dest.pageIndex + 1);
	const textContent = await page.getTextContent();
	const viewport = page.getViewport({ scale: 1 });

	const items = textContent.items as Array<{
		str: string;
		transform: number[];
	}>;

	const lines: { y: number; text: string }[] = [];
	for (const item of items) {
		if (!item.str) continue;
		const itemY = viewport.height - item.transform[5];
		const existing = lines.find((l) => Math.abs(l.y - itemY) < 3);
		if (existing) {
			existing.text += item.str;
		} else {
			lines.push({ y: itemY, text: item.str });
		}
	}

	lines.sort((a, b) => a.y - b.y);

	// Find the first line at or just after the destination Y.
	// PDF destinations usually point to the top of the target element,
	// so we pick the first line whose Y >= targetY (with a small tolerance)
	// to avoid starting from the middle of a reference entry.
	let startIdx = 0;
	const Y_SNAP_TOLERANCE = 5;
	let found = false;
	for (let i = 0; i < lines.length; i++) {
		if (lines[i].y >= dest.targetY - Y_SNAP_TOLERANCE) {
			startIdx = i;
			found = true;
			break;
		}
	}
	if (!found) {
		// Fallback: use the last line if targetY is beyond all lines
		startIdx = lines.length - 1;
	}

	const collected: string[] = [];
	for (
		let i = startIdx;
		i < Math.min(startIdx + MAX_LINES, lines.length);
		i++
	) {
		const lineText = lines[i].text.trim();
		if (!lineText && collected.length > 0) break;
		if (lineText) collected.push(lineText);
	}

	let result = collected.join(" ").trim();
	if (result.length > MAX_TEXT_LENGTH) {
		result = `${result.slice(0, MAX_TEXT_LENGTH)}…`;
	}

	return result || null;
}

// ─── Image crop rendering ───
// Render the full page to a temporary canvas, then crop the region around the
// destination with drawImage. Using ctx.translate() before page.render() breaks
// because pdf.js applies its own viewport transform internally, and the
// resulting transparent pixels turn black in the JPEG output.

async function renderDestinationImage(
	pdfDocument: PDFDocumentProxy,
	dest: ResolvedDest,
): Promise<string | null> {
	const page = await pdfDocument.getPage(dest.pageIndex + 1);
	const viewport = page.getViewport({ scale: IMAGE_RENDER_SCALE });

	const fullCanvas = document.createElement("canvas");
	fullCanvas.width = viewport.width;
	fullCanvas.height = viewport.height;
	const fullCtx = fullCanvas.getContext("2d");
	if (!fullCtx) return null;

	fullCtx.fillStyle = "#ffffff";
	fullCtx.fillRect(0, 0, fullCanvas.width, fullCanvas.height);

	await page.render({
		canvasContext: fullCtx,
		viewport,
	}).promise;

	const destYPx = dest.targetY * IMAGE_RENDER_SCALE;
	const paddingTopPx = IMAGE_CROP_PADDING_TOP * IMAGE_RENDER_SCALE;
	const cropTopPx = Math.max(0, destYPx - paddingTopPx);
	const cropHeightPx = Math.min(
		IMAGE_CROP_HEIGHT * IMAGE_RENDER_SCALE,
		viewport.height - cropTopPx,
	);

	if (cropHeightPx <= 0) return null;

	const cropCanvas = document.createElement("canvas");
	cropCanvas.width = viewport.width;
	cropCanvas.height = cropHeightPx;
	const cropCtx = cropCanvas.getContext("2d");
	if (!cropCtx) return null;

	cropCtx.drawImage(
		fullCanvas,
		0,
		cropTopPx,
		viewport.width,
		cropHeightPx,
		0,
		0,
		viewport.width,
		cropHeightPx,
	);

	return cropCanvas.toDataURL("image/jpeg", 0.85);
}

// ─── Component ───

export function CitationPreview({
	pdfDocument,
	containerEl,
}: CitationPreviewProps) {
	const [tooltip, setTooltip] = useState<TooltipState | null>(null);
	const cacheRef = useRef<
		Map<string, { text: string | null; image: string | null }>
	>(new Map());
	const hideTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
	const showTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
	const activeDestRef = useRef<string | null>(null);

	const mode = useUiStore((s) => s.citationPreviewMode);

	const prevPdfDocRef = useRef(pdfDocument);
	if (prevPdfDocRef.current !== pdfDocument) {
		prevPdfDocRef.current = pdfDocument;
		cacheRef.current.clear();
	}

	useEffect(() => {
		if (!containerEl || !pdfDocument || mode === "off") return;

		async function resolvePreview(
			destHash: string,
			previewMode: CitationPreviewMode,
		): Promise<{ text: string | null; image: string | null } | null> {
			const cached = cacheRef.current.get(destHash);
			if (cached) {
				if (previewMode === "text" && cached.text !== undefined) return cached;
				if (previewMode === "image" && cached.image !== undefined)
					return cached;
			}

			try {
				const doc = pdfDocument as PDFDocumentProxy;
				const dest = await resolveDestination(doc, destHash);
				if (!dest) {
					cacheRef.current.set(destHash, { text: null, image: null });
					return null;
				}

				const entry = cached ?? { text: null, image: null };

				if (previewMode === "text" && entry.text === null) {
					entry.text = await extractDestinationText(doc, dest);
				}
				if (previewMode === "image" && entry.image === null) {
					entry.image = await renderDestinationImage(doc, dest);
				}

				cacheRef.current.set(destHash, entry);
				return entry;
			} catch {
				cacheRef.current.set(destHash, { text: null, image: null });
				return null;
			}
		}

		const handleMouseOver = (e: MouseEvent) => {
			const target = e.target as HTMLElement;
			const link = target.closest?.(LINK_SELECTOR) as HTMLAnchorElement | null;
			if (!link?.closest(".annotationLayer")) return;

			const href = link.getAttribute("href");
			if (!href?.startsWith("#") || href.length < 2) return;

			const destHash = href.slice(1);

			if (hideTimeoutRef.current) {
				clearTimeout(hideTimeoutRef.current);
				hideTimeoutRef.current = null;
			}

			if (activeDestRef.current === destHash) return;

			if (showTimeoutRef.current) {
				clearTimeout(showTimeoutRef.current);
			}

			activeDestRef.current = destHash;

			const currentMode = useUiStore.getState().citationPreviewMode;

			showTimeoutRef.current = setTimeout(async () => {
				const result = await resolvePreview(destHash, currentMode);
				if (!result || activeDestRef.current !== destHash) return;

				const hasContent =
					currentMode === "text" ? !!result.text : !!result.image;
				if (!hasContent) return;

				const maxW =
					currentMode === "image" ? TOOLTIP_IMAGE_MAX_W : TOOLTIP_TEXT_MAX_W;

				const linkRect = link.getBoundingClientRect();
				let x = linkRect.left;
				let y = linkRect.bottom + 6;

				if (x + maxW + 10 > window.innerWidth) {
					x = window.innerWidth - maxW - 20;
				}
				if (x < 4) x = 4;

				const tooltipEstHeight = currentMode === "image" ? 320 : 150;
				const above = y + tooltipEstHeight > window.innerHeight;
				if (above) {
					y = linkRect.top - 6;
				}

				setTooltip({
					x,
					y,
					above,
					text: currentMode === "text" ? result.text : null,
					imageDataUrl: currentMode === "image" ? result.image : null,
				});
			}, SHOW_DELAY);
		};

		const handleMouseOut = (e: MouseEvent) => {
			const target = e.target as HTMLElement;
			if (!target.closest?.(LINK_SELECTOR)?.closest(".annotationLayer")) {
				return;
			}

			if (showTimeoutRef.current) {
				clearTimeout(showTimeoutRef.current);
				showTimeoutRef.current = null;
			}

			hideTimeoutRef.current = setTimeout(() => {
				setTooltip(null);
				activeDestRef.current = null;
			}, HIDE_DELAY);
		};

		const scrollContainer = containerEl.querySelector("[class*='_container_']");
		const handleScroll = () => {
			setTooltip(null);
			activeDestRef.current = null;
			if (showTimeoutRef.current) {
				clearTimeout(showTimeoutRef.current);
				showTimeoutRef.current = null;
			}
		};

		containerEl.addEventListener("mouseover", handleMouseOver);
		containerEl.addEventListener("mouseout", handleMouseOut);
		scrollContainer?.addEventListener("scroll", handleScroll, {
			passive: true,
		});

		return () => {
			containerEl.removeEventListener("mouseover", handleMouseOver);
			containerEl.removeEventListener("mouseout", handleMouseOut);
			scrollContainer?.removeEventListener("scroll", handleScroll);
			if (hideTimeoutRef.current) clearTimeout(hideTimeoutRef.current);
			if (showTimeoutRef.current) clearTimeout(showTimeoutRef.current);
		};
	}, [containerEl, pdfDocument, mode]);

	if (!tooltip) return null;

	const isImage = !!tooltip.imageDataUrl;
	const maxW = isImage ? TOOLTIP_IMAGE_MAX_W : TOOLTIP_TEXT_MAX_W;

	return (
		<div
			className="fixed z-[9999] rounded-lg border bg-popover/95 backdrop-blur-sm shadow-lg animate-in fade-in-0 zoom-in-95 duration-150 overflow-hidden"
			style={{
				left: tooltip.x,
				top: tooltip.y,
				maxWidth: maxW,
				transform: tooltip.above ? "translateY(-100%)" : undefined,
			}}
			onMouseEnter={() => {
				if (hideTimeoutRef.current) {
					clearTimeout(hideTimeoutRef.current);
					hideTimeoutRef.current = null;
				}
			}}
			onMouseLeave={() => {
				setTooltip(null);
				activeDestRef.current = null;
			}}
		>
			{tooltip.text && (
				<p className="px-3 py-2.5 text-xs leading-relaxed text-popover-foreground">
					{tooltip.text}
				</p>
			)}
			{tooltip.imageDataUrl && (
				<img
					src={tooltip.imageDataUrl}
					alt="Citation preview"
					className="block w-full"
					draggable={false}
				/>
			)}
		</div>
	);
}
