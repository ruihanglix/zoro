// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { cn } from "@/lib/utils";
import { useAnnotationStore } from "@/stores/annotationStore";
import type { HtmlHeadingItem } from "@/stores/annotationStore";
import { ChevronRight, FileText } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

interface OutlineItem {
	title: string;
	bold: boolean;
	italic: boolean;
	dest: unknown;
	items: OutlineItem[];
}

/**
 * Renders the document outline (table of contents / bookmarks).
 * Supports both PDF (via pdfDocument.getOutline()) and HTML (via htmlHeadings from store).
 *
 * Uses the explicit `readerMode` prop to decide which branch to render,
 * so the outline updates correctly when switching between PDF / HTML tabs.
 */
export function OutlinePanel({ readerMode }: { readerMode?: "pdf" | "html" }) {
	const { t } = useTranslation();
	const pdfDocument = useAnnotationStore((s) => s.pdfDocument);
	const htmlHeadings = useAnnotationStore((s) => s.htmlHeadings);
	const [outline, setOutline] = useState<OutlineItem[] | null>(null);
	const [loading, setLoading] = useState(true);

	const isHtmlMode = readerMode === "html";

	// PDF outline loading
	useEffect(() => {
		if (!pdfDocument) {
			setOutline(null);
			setLoading(false);
			return;
		}

		let cancelled = false;
		setLoading(true);

		pdfDocument
			.getOutline()
			.then((result) => {
				if (!cancelled) {
					setOutline(result as OutlineItem[] | null);
					setLoading(false);
				}
			})
			.catch(() => {
				if (!cancelled) {
					setOutline(null);
					setLoading(false);
				}
			});

		return () => {
			cancelled = true;
		};
	}, [pdfDocument]);

	// --- HTML mode: render headings from store ---
	if (isHtmlMode) {
		if (htmlHeadings.length > 0) {
			return (
				<div className="overflow-y-auto h-full p-2">
					{htmlHeadings.map((item) => (
						<HtmlHeadingNode key={item.id} item={item} depth={0} />
					))}
				</div>
			);
		}
		return (
			<div className="flex h-full flex-col items-center justify-center gap-2 text-muted-foreground">
				<FileText className="h-8 w-8 opacity-50" />
				<p className="text-xs">{t("reader.noOutline")}</p>
				<p className="text-[10px]">{t("reader.noTableOfContentsHtml")}</p>
			</div>
		);
	}

	// --- PDF mode ---
	if (loading) {
		return (
			<div className="flex h-full items-center justify-center text-xs text-muted-foreground">
				{t("reader.loadingOutline")}
			</div>
		);
	}

	// No outline available (PDF without TOC)
	if (!outline || outline.length === 0) {
		return (
			<div className="flex h-full flex-col items-center justify-center gap-2 text-muted-foreground">
				<FileText className="h-8 w-8 opacity-50" />
				<p className="text-xs">{t("reader.noOutline")}</p>
				<p className="text-[10px]">{t("reader.noTableOfContents")}</p>
			</div>
		);
	}

	return (
		<div className="overflow-y-auto h-full p-2">
			{outline.map((item, i) => (
				<OutlineNode key={`${item.title}-${i}`} item={item} depth={0} />
			))}
		</div>
	);
}

// ═══════════════════ PDF Outline Node ═══════════════════

function OutlineNode({ item, depth }: { item: OutlineItem; depth: number }) {
	const pdfDocument = useAnnotationStore((s) => s.pdfDocument);
	const navigateToPage = useAnnotationStore((s) => s.navigateToPage);
	const [expanded, setExpanded] = useState(depth < 1);
	const hasChildren = item.items && item.items.length > 0;

	const handleClick = useCallback(async () => {
		if (!pdfDocument || !item.dest) return;

		try {
			// dest can be a string (named destination) or an array (explicit destination)
			let dest: unknown = item.dest;
			if (typeof dest === "string") {
				dest = await pdfDocument.getDestination(dest);
			}
			if (!Array.isArray(dest) || dest.length === 0) return;

			// dest[0] is a page reference object
			const pageIndex = await pdfDocument.getPageIndex(dest[0] as never);
			navigateToPage(pageIndex + 1); // pageIndex is 0-based
		} catch (e) {
			console.error("Failed to navigate to outline destination:", e);
		}
	}, [pdfDocument, item.dest, navigateToPage]);

	return (
		<div>
			<button
				type="button"
				className={cn(
					"flex w-full items-center gap-1 rounded px-1.5 py-1 text-left text-xs hover:bg-muted/50 transition-colors",
					item.bold && "font-semibold",
					item.italic && "italic",
				)}
				style={{ paddingLeft: `${depth * 12 + 6}px` }}
				onClick={() => {
					handleClick();
					if (hasChildren) setExpanded(!expanded);
				}}
			>
				{hasChildren && (
					<ChevronRight
						className={cn(
							"h-3 w-3 shrink-0 transition-transform",
							expanded && "rotate-90",
						)}
					/>
				)}
				{!hasChildren && <span className="w-3 shrink-0" />}
				<span className="truncate">{item.title}</span>
			</button>
			{hasChildren && expanded && (
				<div>
					{item.items.map((child, i) => (
						<OutlineNode
							key={`${child.title}-${i}`}
							item={child}
							depth={depth + 1}
						/>
					))}
				</div>
			)}
		</div>
	);
}

// ═══════════════════ HTML Heading Node ═══════════════════

function HtmlHeadingNode({
	item,
	depth,
}: { item: HtmlHeadingItem; depth: number }) {
	const scrollToHtmlHeading = useAnnotationStore((s) => s.scrollToHtmlHeading);
	const [expanded, setExpanded] = useState(depth < 2);
	const hasChildren = item.children && item.children.length > 0;

	const handleClick = useCallback(() => {
		scrollToHtmlHeading?.(item.id);
	}, [scrollToHtmlHeading, item.id]);

	return (
		<div>
			<button
				type="button"
				className={cn(
					"flex w-full items-center gap-1 rounded px-1.5 py-1 text-left text-xs hover:bg-muted/50 transition-colors",
					item.level <= 2 && "font-semibold",
				)}
				style={{ paddingLeft: `${depth * 12 + 6}px` }}
				onClick={() => {
					handleClick();
					if (hasChildren) setExpanded(!expanded);
				}}
			>
				{hasChildren && (
					<ChevronRight
						className={cn(
							"h-3 w-3 shrink-0 transition-transform",
							expanded && "rotate-90",
						)}
					/>
				)}
				{!hasChildren && <span className="w-3 shrink-0" />}
				<span className="truncate">{item.text}</span>
			</button>
			{hasChildren && expanded && (
				<div>
					{item.children.map((child) => (
						<HtmlHeadingNode key={child.id} item={child} depth={depth + 1} />
					))}
				</div>
			)}
		</div>
	);
}
