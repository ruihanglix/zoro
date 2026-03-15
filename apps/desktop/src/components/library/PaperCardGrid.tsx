// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { BilingualCardAbstract } from "@/components/BilingualText";
import { HighlightedText } from "@/components/HighlightedText";
import { MultiSelectContextMenu } from "@/components/library/MultiSelectContextMenu";
import { PaperContextMenu } from "@/components/library/PaperContextMenu";
import { Badge } from "@/components/ui/badge";

import { ScrollArea } from "@/components/ui/scroll-area";
import type { PaperResponse } from "@/lib/commands";
import { startPaperDrag } from "@/lib/dragState";
import { cn } from "@/lib/utils";
import { useLibraryStore } from "@/stores/libraryStore";
import { useTabStore } from "@/stores/tabStore";
import {
	useTranslatedText,
	useTranslationStore,
} from "@/stores/translationStore";
import { CloudDownload, FileText, StickyNote } from "lucide-react";
import { useCallback, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

export function PaperCardGrid() {
	const papers = useLibraryStore((s) => s.papers);
	const selectedPaper = useLibraryStore((s) => s.selectedPaper);
	const setSelectedPaper = useLibraryStore((s) => s.setSelectedPaper);
	const selectedPaperIds = useLibraryStore((s) => s.selectedPaperIds);
	const toggleSelectPaper = useLibraryStore((s) => s.toggleSelectPaper);
	const selectAllPapers = useLibraryStore((s) => s.selectAllPapers);
	const clearSelection = useLibraryStore((s) => s.clearSelection);
	const selectPaperRange = useLibraryStore((s) => s.selectPaperRange);
	const openTab = useTabStore((s) => s.openTab);
	const ensureTranslatedBatch = useTranslationStore(
		(s) => s.ensureTranslatedBatch,
	);
	const displayMode = useTranslationStore((s) => s.displayMode);

	const hasSelection = selectedPaperIds.size > 0;

	// Keyboard shortcut: Ctrl/Cmd+A to select all, Esc to clear
	const gridRef = useRef<HTMLDivElement>(null);
	useEffect(() => {
		const handler = (e: KeyboardEvent) => {
			if ((e.metaKey || e.ctrlKey) && e.key === "a") {
				if (
					gridRef.current?.contains(document.activeElement) ||
					gridRef.current?.contains(e.target as Node)
				) {
					e.preventDefault();
					selectAllPapers();
				}
			}
			if (e.key === "Escape" && hasSelection) {
				clearSelection();
			}
		};
		document.addEventListener("keydown", handler);
		return () => document.removeEventListener("keydown", handler);
	}, [selectAllPapers, clearSelection, hasSelection]);

	const handleCardClick = useCallback(
		(paper: PaperResponse, e: React.MouseEvent) => {
			if (e.shiftKey) {
				e.preventDefault();
				selectPaperRange(paper.id);
			} else if (e.metaKey || e.ctrlKey) {
				e.preventDefault();
				toggleSelectPaper(paper.id);
			} else {
				if (hasSelection) {
					clearSelection();
				}
				setSelectedPaper(paper);
			}
		},
		[
			setSelectedPaper,
			toggleSelectPaper,
			selectPaperRange,
			clearSelection,
			hasSelection,
		],
	);

	// Batch-fetch and auto-translate titles for visible papers
	useEffect(() => {
		const nonNotes = papers.filter((p) => p.entry_type !== "note");
		if (nonNotes.length > 0) {
			ensureTranslatedBatch(
				"paper",
				nonNotes.map((p) => p.id),
				["title"],
			);
		}
	}, [papers, ensureTranslatedBatch, displayMode]);

	return (
		<ScrollArea className="h-full" ref={gridRef} tabIndex={-1}>
			<div className="grid grid-cols-[repeat(auto-fill,minmax(280px,1fr))] gap-3 p-4">
				{papers.map((paper) => {
					const isChecked = selectedPaperIds.has(paper.id);

					const cardContent = (
						<PaperCard
							paper={paper}
							selected={selectedPaper?.id === paper.id}
							checked={isChecked}
							onClick={(e) => handleCardClick(paper, e)}
							onDoubleClick={() => {
								if (paper.entry_type === "note") {
									openTab({
										type: "note",
										paperId: paper.id,
										title: paper.title,
									});
								} else {
									// Find the primary PDF: first local PDF attachment
									const primaryPdf = paper.attachments.find(
										(a) => a.file_type === "pdf" && a.is_local,
									);
									openTab({
										type: "reader",
										paperId: paper.id,
										readerMode: paper.has_pdf ? "pdf" : "html",
										pdfFilename: primaryPdf?.filename,
										title: paper.title,
									});
								}
							}}
						/>
					);

					// When multi-select is active, right-clicking any card (even unselected)
					// should show the multi-select menu and auto-include that card.
					const multiSelectIds = hasSelection
						? isChecked
							? Array.from(selectedPaperIds)
							: [...Array.from(selectedPaperIds), paper.id]
						: null;

					const wrappedCard = multiSelectIds ? (
						<MultiSelectContextMenu paperIds={multiSelectIds}>
							{cardContent}
						</MultiSelectContextMenu>
					) : (
						<PaperContextMenu paper={paper}>{cardContent}</PaperContextMenu>
					);

					return (
						<div
							key={paper.id}
							className="group"
							onMouseDown={(e) => startPaperDrag(e, paper.id, paper.title)}
						>
							{wrappedCard}
						</div>
					);
				})}
			</div>
		</ScrollArea>
	);
}

function PaperCard({
	paper,
	selected,
	checked,
	onClick,
	onDoubleClick,
}: {
	paper: PaperResponse;
	selected: boolean;
	checked: boolean;
	onClick: (e: React.MouseEvent) => void;
	onDoubleClick: () => void;
}) {
	const authors = paper.authors.map((a) => a.name).join(", ");
	const year = paper.published_date
		? new Date(paper.published_date).getFullYear()
		: null;
	const isNote = paper.entry_type === "note";
	const displayMode = useTranslationStore((s) => s.displayMode);
	const rawTranslatedTitle = useTranslatedText("paper", paper.id, "title");
	const rawTranslatedAbstract = useTranslatedText(
		"paper",
		paper.id,
		"abstract_text",
	);
	const translatedTitle = isNote ? null : rawTranslatedTitle;
	const translatedAbstract = isNote ? null : rawTranslatedAbstract;

	const showTitle =
		displayMode === "original" || !translatedTitle
			? paper.title
			: displayMode === "translated"
				? translatedTitle
				: translatedTitle;
	const showAbstract =
		displayMode === "original" || !translatedAbstract
			? paper.abstract_text
			: displayMode === "translated"
				? translatedAbstract
				: translatedAbstract;

	return (
		<div
			data-paper-id={paper.id}
			className={cn(
				"cursor-pointer rounded-lg border bg-card p-4 transition-all hover:shadow-md hover:border-primary/30 select-none",
				selected && "ring-2 ring-primary border-primary/50 shadow-md",
				checked && "ring-2 ring-primary/60 bg-primary/5",
				isNote && "border-primary/20",
			)}
			onClick={onClick}
			onDoubleClick={onDoubleClick}
		>
			{/* Title */}
			<h3
				className="font-medium text-sm leading-snug flex items-start gap-1.5"
				title={paper.title}
			>
				{isNote && (
					<StickyNote className="h-4 w-4 shrink-0 text-primary mt-0.5" />
				)}
				<HighlightedText text={showTitle} />
			</h3>
			{displayMode === "bilingual" && translatedTitle && (
				<p className="text-[11px] text-muted-foreground/60 mt-0.5">
					{paper.title}
				</p>
			)}

			{/* Authors + Year */}
			<div className="mt-1.5 flex items-baseline gap-1.5 text-xs text-muted-foreground">
				{authors && (
					<HighlightedText text={authors} className="truncate flex-1 min-w-0" />
				)}
				{year && <span className="shrink-0 tabular-nums">{year}</span>}
			</div>

			{/* Abstract snippet */}
			{displayMode === "bilingual" &&
			translatedAbstract &&
			paper.abstract_text ? (
				<BilingualCardAbstract
					original={paper.abstract_text}
					translated={translatedAbstract}
				/>
			) : (
				(showAbstract || paper.abstract_text) && (
					<p className="mt-2 text-xs text-muted-foreground leading-relaxed line-clamp-5">
						{showAbstract ?? paper.abstract_text}
					</p>
				)
			)}

			{/* Bottom row: badges */}
			<div className="mt-3 flex flex-wrap items-center gap-1.5">
				{paper.has_pdf && (
					<Badge variant="outline" className="text-[10px] px-1.5 py-0 gap-0.5">
						<FileText className="h-2.5 w-2.5" />
						PDF
						{!paper.pdf_downloaded && (
							<CloudDownload className="h-2.5 w-2.5 ml-0.5 text-blue-500" />
						)}
					</Badge>
				)}
				{paper.has_html && (
					<Badge variant="outline" className="text-[10px] px-1.5 py-0 gap-0.5">
						HTML
						{!paper.html_downloaded && (
							<CloudDownload className="h-2.5 w-2.5 ml-0.5 text-blue-500" />
						)}
					</Badge>
				)}
				{paper.read_status === "read" && (
					<Badge variant="secondary" className="text-[10px] px-1.5 py-0">
						<ReadStatusLabel status="read" />
					</Badge>
				)}
				{paper.read_status === "reading" && (
					<Badge variant="outline" className="text-[10px] px-1.5 py-0">
						<ReadStatusLabel status="reading" />
					</Badge>
				)}
			</div>
		</div>
	);
}

function ReadStatusLabel({ status }: { status: string }) {
	const { t } = useTranslation();
	return <>{status === "read" ? t("paper.read") : t("paper.reading")}</>;
}
