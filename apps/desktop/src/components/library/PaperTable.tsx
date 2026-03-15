// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { HighlightedText } from "@/components/HighlightedText";
import { MultiSelectContextMenu } from "@/components/library/MultiSelectContextMenu";
import { PaperContextMenu } from "@/components/library/PaperContextMenu";
import { Badge } from "@/components/ui/badge";

import {
	ContextMenu,
	ContextMenuCheckboxItem,
	ContextMenuContent,
	ContextMenuItem,
	ContextMenuSeparator,
	ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import { COLUMN_DEFS, COLUMN_DEF_MAP } from "@/lib/columnConfig";
import type { AttachmentResponse, PaperResponse } from "@/lib/commands";
import * as commands from "@/lib/commands";
import { startPaperDrag } from "@/lib/dragState";
import { cn } from "@/lib/utils";
import { useLibraryStore } from "@/stores/libraryStore";
import { useTabStore } from "@/stores/tabStore";
import {
	useTranslatedText,
	useTranslationStore,
} from "@/stores/translationStore";
import { useUiStore } from "@/stores/uiStore";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
	ArrowDown,
	ArrowUp,
	Camera,
	ChevronDown,
	ChevronRight,
	Copy,
	Download,
	File,
	FileText,
	FolderOpen,
	Globe,
	Star,
	StickyNote,
} from "lucide-react";
import {
	type PointerEvent as ReactPointerEvent,
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { useTranslation } from "react-i18next";

// ============================================================
// Main PaperTable component
// ============================================================

export function PaperTable() {
	const { t } = useTranslation();
	const papers = useLibraryStore((s) => s.papers);
	const selectedPaper = useLibraryStore((s) => s.selectedPaper);
	const setSelectedPaper = useLibraryStore((s) => s.setSelectedPaper);
	const selectedPaperIds = useLibraryStore((s) => s.selectedPaperIds);
	const toggleSelectPaper = useLibraryStore((s) => s.toggleSelectPaper);
	const selectAllPapers = useLibraryStore((s) => s.selectAllPapers);
	const clearSelection = useLibraryStore((s) => s.clearSelection);
	const selectPaperRange = useLibraryStore((s) => s.selectPaperRange);
	const sortBy = useLibraryStore((s) => s.sortBy);
	const setSortBy = useLibraryStore((s) => s.setSortBy);
	const sortOrder = useLibraryStore((s) => s.sortOrder);
	const setSortOrder = useLibraryStore((s) => s.setSortOrder);
	const openTab = useTabStore((s) => s.openTab);

	const hasSelection = selectedPaperIds.size > 0;

	// Keyboard shortcut: Ctrl/Cmd+A to select all
	const tableRef = useRef<HTMLDivElement>(null);
	useEffect(() => {
		const handler = (e: KeyboardEvent) => {
			if ((e.metaKey || e.ctrlKey) && e.key === "a") {
				// Only handle when focus is within the table
				if (
					tableRef.current?.contains(document.activeElement) ||
					tableRef.current?.contains(e.target as Node)
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

	// Row click handler with multi-select support
	const handleRowClick = useCallback(
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
	const ensureTranslatedBatch = useTranslationStore(
		(s) => s.ensureTranslatedBatch,
	);
	const displayMode = useTranslationStore((s) => s.displayMode);

	// Column state from uiStore
	const columns = useUiStore((s) => s.columns);
	const toggleColumnVisibility = useUiStore((s) => s.toggleColumnVisibility);
	const reorderColumns = useUiStore((s) => s.reorderColumns);
	const resizeColumn = useUiStore((s) => s.resizeColumn);
	const resetColumns = useUiStore((s) => s.resetColumns);

	// Visible columns (filtered + ordered)
	const visibleColumns = useMemo(
		() => columns.filter((col) => col.visible),
		[columns],
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

	// Expand/collapse state
	const [expandedPapers, setExpandedPapers] = useState<Set<string>>(new Set());
	const papersRef = useRef(papers);
	useEffect(() => {
		if (papersRef.current !== papers) {
			papersRef.current = papers;
			setExpandedPapers(new Set());
		}
	}, [papers]);

	const toggleExpand = useCallback((paperId: string) => {
		setExpandedPapers((prev) => {
			const next = new Set(prev);
			if (next.has(paperId)) {
				next.delete(paperId);
			} else {
				next.add(paperId);
			}
			return next;
		});
	}, []);

	// Sorting
	const toggleSort = (field: string) => {
		if (sortBy === field) {
			setSortOrder(sortOrder === "asc" ? "desc" : "asc");
		} else {
			setSortBy(field);
			setSortOrder("desc");
		}
	};

	// --- Column drag reorder state ---
	const [dragColumnId, setDragColumnId] = useState<string | null>(null);
	const [dragOverIndex, setDragOverIndex] = useState<number | null>(null);
	const dragOverIndexRef = useRef<number | null>(null);
	const dragStartX = useRef(0);
	const dragStarted = useRef(false);

	// Keep ref in sync with state
	useEffect(() => {
		dragOverIndexRef.current = dragOverIndex;
	}, [dragOverIndex]);

	// Keep refs for columns/visibleColumns to avoid stale closures
	const columnsRef = useRef(columns);
	const visibleColumnsRef = useRef(visibleColumns);
	useEffect(() => {
		columnsRef.current = columns;
	}, [columns]);
	useEffect(() => {
		visibleColumnsRef.current = visibleColumns;
	}, [visibleColumns]);

	const handleColumnDragStart = useCallback(
		(e: ReactPointerEvent, columnId: string) => {
			// Only left button
			if (e.button !== 0) return;
			// Don't start drag on resize handle
			if ((e.target as HTMLElement).dataset.resizeHandle) return;

			dragStartX.current = e.clientX;
			dragStarted.current = false;

			const onMove = (me: globalThis.PointerEvent) => {
				const dx = Math.abs(me.clientX - dragStartX.current);
				if (!dragStarted.current && dx > 5) {
					dragStarted.current = true;
					setDragColumnId(columnId);
				}
				if (dragStarted.current) {
					// Find which header cell we're over
					const headerCells = document.querySelectorAll("[data-column-header]");
					for (let i = 0; i < headerCells.length; i++) {
						const rect = headerCells[i].getBoundingClientRect();
						if (me.clientX >= rect.left && me.clientX <= rect.right) {
							dragOverIndexRef.current = i;
							setDragOverIndex(i);
							break;
						}
					}
				}
			};

			const onUp = () => {
				document.removeEventListener("pointermove", onMove);
				document.removeEventListener("pointerup", onUp);

				const overIdx = dragOverIndexRef.current;
				const cols = columnsRef.current;
				const visCols = visibleColumnsRef.current;

				if (dragStarted.current && overIdx !== null) {
					const fromFullIndex = cols.findIndex((c) => c.id === columnId);
					const targetVisibleCol = visCols[overIdx];
					if (targetVisibleCol) {
						const toFullIndex = cols.findIndex(
							(c) => c.id === targetVisibleCol.id,
						);
						if (
							fromFullIndex !== -1 &&
							toFullIndex !== -1 &&
							fromFullIndex !== toFullIndex
						) {
							reorderColumns(fromFullIndex, toFullIndex);
						}
					}
				}

				setDragColumnId(null);
				setDragOverIndex(null);
				dragOverIndexRef.current = null;
				dragStarted.current = false;
			};

			document.addEventListener("pointermove", onMove);
			document.addEventListener("pointerup", onUp);
		},
		[reorderColumns],
	);

	// --- Column resize state ---
	const [resizingColumnId, setResizingColumnId] = useState<string | null>(null);
	const resizeStartX = useRef(0);
	const resizeStartWidth = useRef(0);

	const handleResizeStart = useCallback(
		(e: ReactPointerEvent, columnId: string, currentWidth: number) => {
			e.preventDefault();
			e.stopPropagation();
			setResizingColumnId(columnId);
			resizeStartX.current = e.clientX;
			resizeStartWidth.current = currentWidth;

			const onMove = (me: globalThis.PointerEvent) => {
				const dx = me.clientX - resizeStartX.current;
				const newWidth = resizeStartWidth.current + dx;
				resizeColumn(columnId, newWidth);
			};

			const onUp = () => {
				document.removeEventListener("pointermove", onMove);
				document.removeEventListener("pointerup", onUp);
				setResizingColumnId(null);
			};

			document.addEventListener("pointermove", onMove);
			document.addEventListener("pointerup", onUp);
		},
		[resizeColumn],
	);

	// Compute total fixed width for attachment sub-row spacer
	const totalFixedWidth = useMemo(() => {
		let total = 0;
		for (const col of visibleColumns) {
			if (col.id !== "title" && col.width > 0) {
				total += col.width;
			}
		}
		return total;
	}, [visibleColumns]);

	return (
		<div ref={tableRef} className="flex h-full flex-col" tabIndex={-1}>
			<ScrollArea className="flex-1">
				<div className="w-max min-w-full">
					{/* Column header with right-click context menu */}
					<ContextMenu>
						<ContextMenuTrigger asChild>
							<div
								className="flex items-center border-b bg-muted text-xs font-medium text-muted-foreground select-none sticky top-0 z-10"
								style={{
									cursor: resizingColumnId ? "col-resize" : undefined,
								}}
							>
								{/* Expand arrow placeholder */}
								<div className="w-6 shrink-0" />

								{visibleColumns.map((col, visIdx) => {
									const def = COLUMN_DEF_MAP[col.id];
									if (!def) return null;
									const isFlex = col.width === -1;
									const isSortable = !!def.sortField;
									const isDragTarget =
										dragColumnId !== null &&
										dragColumnId !== col.id &&
										dragOverIndex === visIdx;

									return (
										<div
											key={col.id}
											data-column-header={col.id}
											className={cn(
												"relative px-3 py-2 flex items-center overflow-hidden",
												!isFlex && "shrink-0",
												isFlex && "min-w-0",
												isSortable && "cursor-pointer hover:text-foreground",
												dragColumnId === col.id && "opacity-40",
												isDragTarget && "border-l-2 border-primary",
											)}
											style={
												isFlex
													? {
															flex: "1 1 0%",
															minWidth: `${def.minWidth}px`,
														}
													: {
															width: `${col.width}px`,
														}
											}
											onClick={() => {
												if (
													isSortable &&
													def.sortField &&
													!dragStarted.current
												) {
													toggleSort(def.sortField);
												}
											}}
											onPointerDown={(e) => handleColumnDragStart(e, col.id)}
										>
											<span className="truncate">{def.label}</span>
											{isSortable && def.sortField && (
												<SortIcon
													field={def.sortField}
													sortBy={sortBy}
													sortOrder={sortOrder}
												/>
											)}

											{/* Resize handle */}
											<div
												data-resize-handle="true"
												className="absolute right-0 top-0 bottom-0 w-[4px] cursor-col-resize hover:bg-primary/30 z-20"
												onPointerDown={(e) => {
													const actualWidth = isFlex
														? (
																e.currentTarget.parentElement as HTMLElement
															).getBoundingClientRect().width
														: col.width;
													handleResizeStart(e, col.id, actualWidth);
												}}
											/>
										</div>
									);
								})}
							</div>
						</ContextMenuTrigger>

						{/* Right-click menu for column visibility */}
						<ContextMenuContent className="w-48 max-h-[70vh] overflow-y-auto">
							{COLUMN_DEFS.map((def) => {
								const colState = columns.find((c) => c.id === def.id);
								return (
									<ContextMenuCheckboxItem
										key={def.id}
										checked={colState?.visible ?? false}
										disabled={def.pinned}
										onCheckedChange={() => toggleColumnVisibility(def.id)}
										onSelect={(e) => e.preventDefault()}
									>
										{def.label}
									</ContextMenuCheckboxItem>
								);
							})}
							<ContextMenuSeparator />
							<ContextMenuItem onSelect={() => resetColumns()}>
								{t("library.resetColumns")}
							</ContextMenuItem>
						</ContextMenuContent>
					</ContextMenu>

					{/* Table body */}
					{papers.map((paper) => {
						const isExpanded = expandedPapers.has(paper.id);
						const isChecked = selectedPaperIds.has(paper.id);

						const rowContent = (
							<PaperTableRow
								paper={paper}
								selected={selectedPaper?.id === paper.id}
								checked={isChecked}
								expanded={isExpanded}
								visibleColumns={visibleColumns}
								onToggleExpand={() => toggleExpand(paper.id)}
								onClick={(e) => handleRowClick(paper, e)}
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

						// Wrap with multi-select or single context menu
						// When multi-select is active, right-clicking any row (even unselected)
						// should show the multi-select menu and auto-include that row.
						const multiSelectIds = hasSelection
							? isChecked
								? Array.from(selectedPaperIds)
								: [...Array.from(selectedPaperIds), paper.id]
							: null;

						const wrappedRow = multiSelectIds ? (
							<MultiSelectContextMenu paperIds={multiSelectIds}>
								{rowContent}
							</MultiSelectContextMenu>
						) : (
							<PaperContextMenu paper={paper}>{rowContent}</PaperContextMenu>
						);

						return (
							<div
								key={paper.id}
								onMouseDown={(e) => startPaperDrag(e, paper.id, paper.title)}
							>
								{wrappedRow}
								{/* Attachment sub-rows */}
								{isExpanded &&
									paper.attachments.map((att) => (
										<AttachmentRow
											key={att.id}
											attachment={att}
											paperId={paper.id}
											totalFixedWidth={totalFixedWidth}
											onDoubleClick={() => {
												if (
													(att.file_type === "pdf" ||
														att.file_type === "html") &&
													att.is_local
												) {
													openTab({
														type: "reader",
														paperId: paper.id,
														readerMode: att.file_type,
														pdfFilename:
															att.file_type === "pdf"
																? att.filename
																: undefined,
														title: `${att.filename} - ${paper.title}`,
													});
												}
											}}
										/>
									))}
							</div>
						);
					})}
				</div>
			</ScrollArea>
		</div>
	);
}

// ============================================================
// Sort icon helper
// ============================================================

function SortIcon({
	field,
	sortBy,
	sortOrder,
}: {
	field: string;
	sortBy: string;
	sortOrder: string;
}) {
	if (sortBy !== field) return null;
	return sortOrder === "asc" ? (
		<ArrowUp className="ml-1 h-3 w-3 shrink-0" />
	) : (
		<ArrowDown className="ml-1 h-3 w-3 shrink-0" />
	);
}

// ============================================================
// Attachment type icon helper
// ============================================================

function AttachmentIcon({
	fileType,
	isLocal,
	className,
}: {
	fileType: string;
	isLocal: boolean;
	className?: string;
}) {
	const colorClass = isLocal ? "" : "text-muted-foreground opacity-50";

	switch (fileType) {
		case "pdf":
			return (
				<FileText
					className={cn(
						"h-3.5 w-3.5",
						isLocal ? "text-orange-500" : colorClass,
						className,
					)}
				/>
			);
		case "html":
			return <Globe className={cn("h-3.5 w-3.5", colorClass, className)} />;
		case "snapshot":
			return <Camera className={cn("h-3.5 w-3.5", colorClass, className)} />;
		default:
			return <File className={cn("h-3.5 w-3.5", colorClass, className)} />;
	}
}

// ============================================================
// Attachment sub-row
// ============================================================

function AttachmentRow({
	attachment,
	paperId,
	totalFixedWidth,
	onDoubleClick,
}: {
	attachment: AttachmentResponse;
	paperId: string;
	totalFixedWidth: number;
	onDoubleClick: () => void;
}) {
	const { t } = useTranslation();
	const openTab = useTabStore((s) => s.openTab);
	const createdDate = new Date(attachment.created_date).toLocaleDateString(
		undefined,
		{ month: "short", day: "numeric" },
	);

	const isOpenable =
		attachment.is_local &&
		(attachment.file_type === "pdf" || attachment.file_type === "html");

	const handleOpen = () => {
		if (attachment.file_type === "pdf") {
			openTab({
				type: "reader",
				paperId,
				readerMode: "pdf",
				pdfFilename: attachment.filename,
				title: attachment.filename,
			});
		} else if (attachment.file_type === "html") {
			openTab({
				type: "reader",
				paperId,
				readerMode: "html",
				title: attachment.filename,
			});
		}
	};

	const handleExport = async () => {
		try {
			if (attachment.file_type === "pdf") {
				await commands.exportPdf(paperId, attachment.filename);
			} else if (attachment.file_type === "html") {
				await commands.exportHtml(paperId);
			}
		} catch (err) {
			console.error("Failed to export attachment:", err);
		}
	};

	const handleCopyFilename = async () => {
		try {
			await writeText(attachment.filename);
		} catch (err) {
			console.error("Failed to copy filename:", err);
		}
	};

	const handleShowInFolder = async () => {
		try {
			await commands.showAttachmentInFolder(paperId, attachment.filename);
		} catch (err) {
			console.error("Failed to show in folder:", err);
		}
	};

	const isMac = navigator.platform.toUpperCase().includes("MAC");
	const showInFolderLabel = isMac
		? t("paperTable.showInFinder")
		: t("paperTable.showInFileExplorer");

	return (
		<ContextMenu>
			<ContextMenuTrigger asChild>
				<div
					className="flex items-center border-b border-border/30 text-xs text-muted-foreground hover:bg-accent/30 cursor-default transition-colors select-none"
					onDoubleClick={onDoubleClick}
				>
					{/* Indent: expand arrow width */}
					<div className="w-6 shrink-0" />
					<div className="flex items-center gap-2 flex-1 min-w-0 px-3 py-1 pl-4">
						<AttachmentIcon
							fileType={attachment.file_type}
							isLocal={attachment.is_local}
						/>
						<span
							className={cn(
								"truncate text-[12px]",
								!attachment.is_local && "opacity-60",
							)}
						>
							{attachment.filename}
						</span>
					</div>
					{/* Spacer to fill remaining columns */}
					<div
						className="shrink-0 px-3 py-1 text-xs text-right"
						style={{ width: `${totalFixedWidth}px` }}
					>
						{createdDate}
					</div>
				</div>
			</ContextMenuTrigger>
			<ContextMenuContent className="w-48">
				{isOpenable && (
					<ContextMenuItem onSelect={handleOpen}>
						<FileText className="mr-2 h-4 w-4" />
						{t("common.open")}
					</ContextMenuItem>
				)}
				{attachment.is_local && (
					<ContextMenuItem onSelect={handleExport}>
						<Download className="mr-2 h-4 w-4" />
						{t("common.export")}
					</ContextMenuItem>
				)}
				{attachment.is_local && (
					<ContextMenuItem onSelect={handleShowInFolder}>
						<FolderOpen className="mr-2 h-4 w-4" />
						{showInFolderLabel}
					</ContextMenuItem>
				)}
				{(isOpenable || attachment.is_local) && <ContextMenuSeparator />}
				<ContextMenuItem onSelect={handleCopyFilename}>
					<Copy className="mr-2 h-4 w-4" />
					{t("contextMenu.copyFilename")}
				</ContextMenuItem>
			</ContextMenuContent>
		</ContextMenu>
	);
}

// ============================================================
// Paper table row — data-driven cell rendering
// ============================================================

interface PaperTableRowProps {
	paper: PaperResponse;
	selected: boolean;
	checked: boolean;
	expanded: boolean;
	visibleColumns: { id: string; visible: boolean; width: number }[];
	onToggleExpand: () => void;
	onClick: (e: React.MouseEvent) => void;
	onDoubleClick: () => void;
}

function PaperTableRow({
	paper,
	selected,
	checked,
	expanded,
	visibleColumns,
	onToggleExpand,
	onClick,
	onDoubleClick,
}: PaperTableRowProps) {
	const hasAttachments = paper.attachments.length > 0;

	return (
		<div
			data-paper-id={paper.id}
			className={cn(
				"flex items-center border-b border-border/50 text-sm cursor-pointer hover:bg-accent/50 transition-colors select-none",
				selected && "bg-accent",
				checked && "bg-primary/10",
			)}
			onClick={onClick}
			onDoubleClick={onDoubleClick}
		>
			{/* Expand/collapse arrow */}
			<div
				className="w-6 shrink-0 flex items-center justify-center"
				onClick={(e) => {
					if (hasAttachments) {
						e.stopPropagation();
						onToggleExpand();
					}
				}}
			>
				{hasAttachments ? (
					expanded ? (
						<ChevronDown className="h-3.5 w-3.5 text-muted-foreground hover:text-foreground" />
					) : (
						<ChevronRight className="h-3.5 w-3.5 text-muted-foreground hover:text-foreground" />
					)
				) : null}
			</div>

			{/* Data cells */}
			{visibleColumns.map((col) => {
				const def = COLUMN_DEF_MAP[col.id];
				if (!def) return null;
				const isFlex = col.width === -1;

				return (
					<div
						key={col.id}
						className={cn(
							"px-3 py-1.5 min-w-0 overflow-hidden",
							!isFlex && "shrink-0",
						)}
						style={
							isFlex
								? {
										flex: "1 1 0%",
										minWidth: `${def.minWidth}px`,
									}
								: { width: `${col.width}px` }
						}
					>
						<CellRenderer columnId={col.id} paper={paper} />
					</div>
				);
			})}
		</div>
	);
}

// ============================================================
// Cell renderer — renders the appropriate content for each column
// ============================================================

function CellRenderer({
	columnId,
	paper,
}: {
	columnId: string;
	paper: PaperResponse;
}) {
	switch (columnId) {
		case "title":
			return <TitleCell paper={paper} />;
		case "shortTitle":
			return <ShortTitleCell paper={paper} />;
		case "authors":
			return <AuthorsCell paper={paper} />;
		case "year":
			return <YearCell paper={paper} />;
		case "source":
			return <TextCell value={paper.source ?? ""} />;
		case "readStatus":
			return <StatusCell paper={paper} />;
		case "addedDate":
			return <DateCell date={paper.added_date} />;
		case "modifiedDate":
			return <DateCell date={paper.modified_date} />;
		case "doi":
			return <TextCell value={paper.doi ?? ""} />;
		case "arxivId":
			return <TextCell value={paper.arxiv_id ?? ""} />;
		case "journal":
			return <TextCell value={paper.journal ?? ""} />;
		case "volume":
			return <TextCell value={paper.volume ?? ""} />;
		case "issue":
			return <TextCell value={paper.issue ?? ""} />;
		case "pages":
			return <TextCell value={paper.pages ?? ""} />;
		case "publisher":
			return <TextCell value={paper.publisher ?? ""} />;
		case "entryType":
			return <TextCell value={paper.entry_type ?? ""} />;
		case "rating":
			return <RatingCell rating={paper.rating} />;
		case "tags":
			return <TagsCell paper={paper} />;
		case "attachments":
			return <AttachmentsCell paper={paper} />;
		case "pdfStatus":
			return <PdfStatusCell paper={paper} />;
		case "abstract":
			return <TextCell value={paper.abstract_text ?? ""} />;
		case "url":
			return <TextCell value={paper.url ?? ""} />;
		default:
			return null;
	}
}

// --- Individual cell components ---

function TitleCell({ paper }: { paper: PaperResponse }) {
	const displayMode = useTranslationStore((s) => s.displayMode);
	const rawTranslatedTitle = useTranslatedText("paper", paper.id, "title");
	const isNote = paper.entry_type === "note";
	const translatedTitle = isNote ? null : rawTranslatedTitle;
	const showTitle =
		displayMode === "original" || !translatedTitle
			? paper.title
			: translatedTitle;

	return (
		<div className="flex items-center gap-1.5 min-w-0">
			{isNote && <StickyNote className="h-3.5 w-3.5 shrink-0 text-primary" />}
			<div className="min-w-0 flex-1">
				<HighlightedText
					text={showTitle}
					className="truncate block font-medium text-[13px] leading-tight"
				/>
				{displayMode === "bilingual" && translatedTitle && (
					<span className="truncate block text-[11px] text-muted-foreground/60 leading-tight mt-0.5">
						{paper.title}
					</span>
				)}
			</div>
		</div>
	);
}

function ShortTitleCell({ paper }: { paper: PaperResponse }) {
	const shortTitle =
		paper.short_title ??
		(paper.title.includes(":") ? paper.title.split(":")[0].trim() : "");
	return (
		<span
			className="text-muted-foreground text-xs truncate block font-medium"
			title={shortTitle}
		>
			{shortTitle}
		</span>
	);
}

function AuthorsCell({ paper }: { paper: PaperResponse }) {
	const firstAuthor = paper.authors[0]?.name ?? "";
	const authorCount = paper.authors.length;
	const authorText = authorCount > 1 ? `${firstAuthor} et al.` : firstAuthor;

	return (
		<HighlightedText
			text={authorText}
			className="text-muted-foreground text-xs truncate block"
		/>
	);
}

function YearCell({ paper }: { paper: PaperResponse }) {
	const year = paper.published_date
		? new Date(paper.published_date).getFullYear()
		: "";
	return <span className="text-muted-foreground text-xs">{year}</span>;
}

function TextCell({ value }: { value: string }) {
	return (
		<span
			className="text-muted-foreground text-xs truncate block"
			title={value}
		>
			{value}
		</span>
	);
}

function StatusCell({ paper }: { paper: PaperResponse }) {
	const { t } = useTranslation();
	if (paper.read_status === "read") {
		return (
			<Badge variant="secondary" className="text-[10px] px-1.5 py-0">
				{t("paper.read")}
			</Badge>
		);
	}
	if (paper.read_status === "reading") {
		return (
			<Badge variant="outline" className="text-[10px] px-1.5 py-0">
				{t("paper.reading")}
			</Badge>
		);
	}
	return null;
}

function DateCell({ date }: { date: string }) {
	const formatted = new Date(date).toLocaleString(undefined, {
		year: "numeric",
		month: "short",
		day: "numeric",
		hour: "2-digit",
		minute: "2-digit",
	});
	return <span className="text-muted-foreground text-xs">{formatted}</span>;
}

function RatingCell({ rating }: { rating: number | null }) {
	if (rating == null) return null;
	return (
		<div className="flex items-center gap-0.5">
			{Array.from({ length: 5 }, (_, i) => (
				<Star
					key={i}
					className={cn(
						"h-3 w-3",
						i < rating
							? "fill-yellow-400 text-yellow-400"
							: "text-muted-foreground/30",
					)}
				/>
			))}
		</div>
	);
}

function TagsCell({ paper }: { paper: PaperResponse }) {
	if (paper.tags.length === 0) return null;
	return (
		<div className="flex items-center gap-1 overflow-hidden">
			{paper.tags.map((tag) => (
				<Badge
					key={tag.id}
					variant="outline"
					className="text-[10px] px-1 py-0 shrink-0"
					style={
						tag.color
							? {
									borderColor: tag.color,
									color: tag.color,
								}
							: undefined
					}
				>
					{tag.name}
				</Badge>
			))}
		</div>
	);
}

function AttachmentsCell({ paper }: { paper: PaperResponse }) {
	const count = paper.attachments.length;
	if (count === 0) return null;
	return <span className="text-muted-foreground text-xs">{count}</span>;
}

function PdfStatusCell({ paper }: { paper: PaperResponse }) {
	const pdfAttachment = paper.attachments.find((a) => a.file_type === "pdf");
	if (!pdfAttachment) return null;
	return (
		<FileText
			className={cn(
				"h-3.5 w-3.5",
				pdfAttachment.is_local
					? "text-orange-500"
					: "text-muted-foreground opacity-50",
			)}
		/>
	);
}
