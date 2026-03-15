// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { cn } from "@/lib/utils";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

interface CollapsibleAuthorsProps {
	/** Author names to display */
	authors: string[];
	/** Maximum visible lines before collapsing (default: 3) */
	maxLines?: number;
	/** Additional CSS class names */
	className?: string;
}

/**
 * Displays a list of author names, collapsing them when they exceed
 * a given number of lines. When collapsed, shows the first few and
 * last few authors with a clickable "…" in between.
 */
export function CollapsibleAuthors({
	authors,
	maxLines = 3,
	className,
}: CollapsibleAuthorsProps) {
	const { t } = useTranslation();
	const [expanded, setExpanded] = useState(false);
	const [needsCollapse, setNeedsCollapse] = useState(false);
	const [visibleHead, setVisibleHead] = useState(authors.length);
	const [visibleTail, setVisibleTail] = useState(0);

	const measureRef = useRef<HTMLParagraphElement>(null);

	const fullText = authors.join(", ");

	// Reset state when authors change
	useEffect(() => {
		setExpanded(false);
	}, [authors]);

	// Measure after mount to determine if collapse is needed, and compute
	// how many head/tail authors to keep
	useEffect(() => {
		const el = measureRef.current;
		if (!el || authors.length === 0) {
			setNeedsCollapse(false);
			return;
		}

		// Compute single-line height
		const style = window.getComputedStyle(el);
		const lh = Number.parseFloat(style.lineHeight);
		const lineHeight = Number.isNaN(lh)
			? Number.parseFloat(style.fontSize) * 1.5
			: lh;

		const maxHeight = lineHeight * maxLines;

		if (el.scrollHeight <= maxHeight + 2) {
			setNeedsCollapse(false);
			setVisibleHead(authors.length);
			setVisibleTail(0);
			return;
		}

		// Needs collapse — find how many authors to keep
		setNeedsCollapse(true);
		const totalAuthors = authors.length;
		const tailCount = 1;

		// Use a hidden clone to measure subsets
		const clone = el.cloneNode(false) as HTMLParagraphElement;
		clone.style.cssText = `
      position: absolute; visibility: hidden; height: auto;
      max-height: none; overflow: visible; white-space: normal;
      width: ${el.offsetWidth}px;
    `;
		// Copy computed font styles
		clone.style.fontSize = style.fontSize;
		clone.style.fontFamily = style.fontFamily;
		clone.style.lineHeight = style.lineHeight;
		clone.style.letterSpacing = style.letterSpacing;
		el.parentElement?.appendChild(clone);

		const separator = ", ";
		const ellipsis = ", … ";
		const tailText = authors.slice(totalAuthors - tailCount).join(separator);

		let bestHead = 1;
		for (let h = totalAuthors - tailCount; h >= 1; h--) {
			const headText = authors.slice(0, h).join(separator);
			clone.textContent = `${headText}${ellipsis}${tailText}`;
			if (clone.scrollHeight <= maxHeight + 2) {
				bestHead = h;
				break;
			}
		}

		clone.remove();
		setVisibleHead(bestHead);
		setVisibleTail(tailCount);
	}, [authors, maxLines, fullText]);

	const handleToggle = useCallback((e: React.MouseEvent) => {
		e.stopPropagation();
		setExpanded((v) => !v);
	}, []);

	if (authors.length === 0) return null;

	// Expanded state
	if (expanded) {
		return (
			<p className={cn("text-xs text-muted-foreground", className)}>
				{fullText}
				<button
					type="button"
					className="ml-1 text-primary/70 hover:text-primary transition-colors cursor-pointer text-[10px]"
					onClick={handleToggle}
					title={t("common.collapse")}
				>
					▲
				</button>
			</p>
		);
	}

	// Collapsed or measuring state
	if (!needsCollapse) {
		// Render full text with ref for measurement
		return (
			<p
				ref={measureRef}
				className={cn("text-xs text-muted-foreground", className)}
			>
				{fullText}
			</p>
		);
	}

	// Collapsed view: head authors + "…N more" + tail authors
	const headText = authors.slice(0, visibleHead).join(", ");
	const tailText =
		visibleTail > 0
			? authors.slice(authors.length - visibleTail).join(", ")
			: "";
	const hiddenCount = authors.length - visibleHead - visibleTail;

	return (
		<>
			{/* Hidden full-text element for re-measurement on resize */}
			<p
				ref={measureRef}
				className={cn("text-xs text-muted-foreground", className)}
				style={{
					position: "absolute",
					visibility: "hidden",
					height: "auto",
					maxHeight: "none",
					overflow: "visible",
					pointerEvents: "none",
				}}
				aria-hidden
			>
				{fullText}
			</p>

			{/* Visible collapsed content */}
			<p className={cn("text-xs text-muted-foreground", className)}>
				{headText}
				{", "}
				<button
					type="button"
					className="text-primary/70 hover:text-primary transition-colors cursor-pointer font-medium"
					onClick={handleToggle}
					title={t("common.showAllAuthors", { count: hiddenCount })}
				>
					…{hiddenCount > 0 && ` ${hiddenCount} ${t("common.more")}`}
				</button>
				{tailText && (
					<>
						{", "}
						{tailText}
					</>
				)}
			</p>
		</>
	);
}
