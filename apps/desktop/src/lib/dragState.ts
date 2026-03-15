// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

// Custom pointer-event based drag system for paper items.
// HTML5 drag/drop does not work in Tauri v2 macOS WKWebView because
// Tauri's native drag handler intercepts all drag events.
// This module implements dragging via mousedown/mousemove/mouseup instead.

// --- Drag state ---
let draggingPaperId: string | null = null;
let draggingPaperTitle: string | null = null;

export function getDraggingPaper(): string | null {
	return draggingPaperId;
}

// --- Drop target registry ---
export interface DropTarget {
	id: string;
	type: "collection" | "tag";
	label: string;
	element: HTMLElement;
	onDrop: (paperId: string) => void;
}

const dropTargets = new Map<string, DropTarget>();

export function registerDropTarget(target: DropTarget) {
	dropTargets.set(target.id, target);
}

export function unregisterDropTarget(id: string) {
	dropTargets.delete(id);
}

// --- Ghost element ---
let ghostEl: HTMLDivElement | null = null;

function createGhost(title: string) {
	const el = document.createElement("div");
	el.style.cssText = `
        position: fixed;
        z-index: 99999;
        pointer-events: none;
        padding: 6px 12px;
        border-radius: 6px;
        background: hsl(var(--primary));
        color: hsl(var(--primary-foreground));
        font-size: 12px;
        font-weight: 500;
        max-width: 260px;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
        box-shadow: 0 4px 12px rgba(0,0,0,0.15);
        opacity: 0.92;
        transform: translate(-50%, -120%);
    `;
	el.textContent = title;
	document.body.appendChild(el);
	return el;
}

function removeGhost() {
	if (ghostEl) {
		ghostEl.remove();
		ghostEl = null;
	}
}

// --- Hit testing ---
let currentHoveredTarget: DropTarget | null = null;

function hitTest(x: number, y: number): DropTarget | null {
	for (const target of dropTargets.values()) {
		const rect = target.element.getBoundingClientRect();
		if (
			x >= rect.left &&
			x <= rect.right &&
			y >= rect.top &&
			y <= rect.bottom
		) {
			return target;
		}
	}
	return null;
}

// Highlight CSS class name
const DRAG_OVER_CLASS = "paper-drag-over";

function setHighlight(target: DropTarget | null) {
	if (currentHoveredTarget === target) return;
	// Remove old highlight
	if (currentHoveredTarget) {
		currentHoveredTarget.element.classList.remove(DRAG_OVER_CLASS);
	}
	currentHoveredTarget = target;
	// Add new highlight
	if (currentHoveredTarget) {
		currentHoveredTarget.element.classList.add(DRAG_OVER_CLASS);
	}
}

// --- Drag threshold ---
const DRAG_THRESHOLD = 5; // px – ignore small accidental moves

// --- Global drag handlers ---
let isDragging = false;
let startX = 0;
let startY = 0;
let pendingPaperId: string | null = null;
let pendingPaperTitle: string | null = null;

function onMouseMove(e: MouseEvent) {
	if (!pendingPaperId) return;

	if (!isDragging) {
		const dx = e.clientX - startX;
		const dy = e.clientY - startY;
		if (Math.sqrt(dx * dx + dy * dy) < DRAG_THRESHOLD) return;
		// Threshold exceeded – start real drag
		isDragging = true;
		draggingPaperId = pendingPaperId;
		draggingPaperTitle = pendingPaperTitle;
		ghostEl = createGhost(draggingPaperTitle ?? "");
		console.log("[DragStart] Paper:", draggingPaperId, draggingPaperTitle);
	}

	// Move ghost
	if (ghostEl) {
		ghostEl.style.left = `${e.clientX}px`;
		ghostEl.style.top = `${e.clientY}px`;
	}

	// Hit test
	const target = hitTest(e.clientX, e.clientY);
	setHighlight(target);
}

function onMouseUp(e: MouseEvent) {
	if (isDragging && draggingPaperId) {
		const target = hitTest(e.clientX, e.clientY);
		if (target) {
			console.log(
				"[Drop] Paper:",
				draggingPaperId,
				"→",
				target.type,
				target.label,
			);
			target.onDrop(draggingPaperId);
		} else {
			console.log("[DragCancel] No target at", e.clientX, e.clientY);
		}
	}

	// Cleanup
	setHighlight(null);
	removeGhost();
	isDragging = false;
	draggingPaperId = null;
	draggingPaperTitle = null;
	pendingPaperId = null;
	pendingPaperTitle = null;
	document.removeEventListener("mousemove", onMouseMove);
	document.removeEventListener("mouseup", onMouseUp);
	document.body.style.userSelect = "";
}

/**
 * Call this from onMouseDown on a paper row/card.
 * It sets up global mouse listeners to track the drag.
 */
export function startPaperDrag(
	e: React.MouseEvent,
	paperId: string,
	paperTitle: string,
) {
	// Only respond to left button
	if (e.button !== 0) return;

	pendingPaperId = paperId;
	pendingPaperTitle = paperTitle;
	startX = e.clientX;
	startY = e.clientY;
	isDragging = false;

	document.body.style.userSelect = "none";
	document.addEventListener("mousemove", onMouseMove);
	document.addEventListener("mouseup", onMouseUp);
}

// --- Inject global CSS for drop target highlighting ---
const styleId = "paper-drag-styles";
if (typeof document !== "undefined" && !document.getElementById(styleId)) {
	const style = document.createElement("style");
	style.id = styleId;
	style.textContent = `
        .paper-drag-over {
            background-color: hsl(var(--primary) / 0.15) !important;
            outline: 2px solid hsl(var(--primary) / 0.5);
            outline-offset: -1px;
            border-radius: 4px;
            transition: background-color 0.15s, outline 0.15s;
        }
    `;
	document.head.appendChild(style);
}
