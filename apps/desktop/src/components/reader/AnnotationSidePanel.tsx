// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import { useAnnotationStore } from "@/stores/annotationStore";
import type { AnnotationType, ZoroHighlight } from "@/stores/annotationStore";
import { ANNOTATION_COLORS } from "@/stores/annotationStore";
import {
	ArrowUpDown,
	Check,
	Grid3X3,
	Highlighter,
	ImageIcon,
	List,
	MessageSquare,
	MoreHorizontal,
	Pen,
	Pencil,
	Search,
	StickyNote,
	Trash2,
	Underline,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { OutlinePanel } from "./OutlinePanel";
import { ThumbnailPanel } from "./ThumbnailPanel";

interface AnnotationSidePanelProps {
	paperId?: string | null;
	readerMode?: "pdf" | "html";
	bilingualMode?: boolean;
	translationFile?: string;
	translationAnnotations?: ZoroHighlight[];
	onDeleteTranslationAnnotation?: (id: string) => Promise<void>;
	onUpdateTranslationAnnotation?: (
		id: string,
		color?: string | null,
		comment?: string | null,
	) => Promise<void>;
	onUpdateTranslationAnnotationType?: (
		id: string,
		newType: AnnotationType,
	) => Promise<void>;
	/** Ref to the right-pane scroll-to-highlight function (bilingual mode) */
	scrollToTranslationHighlight?: React.MutableRefObject<
		((h: ZoroHighlight) => void) | null
	>;
	/** Called when an HTML annotation is clicked and the reader needs to switch to HTML mode */
	onNavigateToHtmlAnnotation?: (ann: ZoroHighlight) => void;
}

function isHtmlAnnotation(ann: ZoroHighlight): boolean {
	const pos = ann.position as unknown as Record<string, unknown>;
	return pos?.format === "html" || ann.pageNumber === 0;
}

function extractLangLabel(filename: string): string {
	const match = filename.match(/^paper\.(\w+)\.pdf$/);
	return match ? match[1].toUpperCase() : filename;
}

export function AnnotationSidePanel({
	paperId,
	readerMode,
	bilingualMode,
	translationFile,
	translationAnnotations,
	onDeleteTranslationAnnotation,
	onUpdateTranslationAnnotation,
	onUpdateTranslationAnnotationType,
	scrollToTranslationHighlight,
	onNavigateToHtmlAnnotation,
}: AnnotationSidePanelProps) {
	const readerPaperId = paperId ?? null;
	const annotations = useAnnotationStore((s) => s.annotations);
	const deleteAnnotation = useAnnotationStore((s) => s.deleteAnnotation);
	const updateAnnotation = useAnnotationStore((s) => s.updateAnnotation);
	const updateAnnotationType = useAnnotationStore(
		(s) => s.updateAnnotationType,
	);
	const scrollToHighlight = useAnnotationStore((s) => s.scrollToHighlight);
	const leftPanelView = useAnnotationStore((s) => s.leftPanelView);
	const setLeftPanelView = useAnnotationStore((s) => s.setLeftPanelView);
	const [searchQuery, setSearchQuery] = useState("");
	const { t } = useTranslation();

	// Context menu state
	const [contextMenuId, setContextMenuId] = useState<string | null>(null);

	// Inline comment editing state
	const [editingCommentId, setEditingCommentId] = useState<string | null>(null);
	const [commentDraft, setCommentDraft] = useState("");

	const translationIdSet = useMemo(
		() => new Set(translationAnnotations?.map((a) => a.id) ?? []),
		[translationAnnotations],
	);

	const isTranslation = (id: string) => translationIdSet.has(id);

	const translationLabel = useMemo(
		() => (translationFile ? extractLangLabel(translationFile) : "Trans"),
		[translationFile],
	);

	const handleClickAnnotation = (ann: ZoroHighlight) => {
		if (isTranslation(ann.id) && scrollToTranslationHighlight?.current) {
			scrollToTranslationHighlight.current(ann);
		} else if (isHtmlAnnotation(ann) && onNavigateToHtmlAnnotation) {
			onNavigateToHtmlAnnotation(ann);
		} else if (scrollToHighlight) {
			scrollToHighlight(ann);
		}
	};

	const handleDeleteAnnotation = async (ann: ZoroHighlight) => {
		if (isTranslation(ann.id)) {
			await onDeleteTranslationAnnotation?.(ann.id);
		} else if (readerPaperId) {
			await deleteAnnotation(ann.id, readerPaperId);
		}
		setContextMenuId(null);
	};

	const handleChangeColor = async (ann: ZoroHighlight, color: string) => {
		if (isTranslation(ann.id)) {
			await onUpdateTranslationAnnotation?.(ann.id, color);
		} else {
			await updateAnnotation(ann.id, color);
		}
		setContextMenuId(null);
	};

	const handleConvertType = async (ann: ZoroHighlight) => {
		const newType = ann.type === "highlight" ? "underline" : "highlight";
		if (isTranslation(ann.id)) {
			await onUpdateTranslationAnnotationType?.(ann.id, newType);
		} else {
			await updateAnnotationType(ann.id, newType);
		}
		setContextMenuId(null);
	};

	const handleStartEditComment = (ann: ZoroHighlight) => {
		setEditingCommentId(ann.id);
		setCommentDraft(ann.comment.text || "");
	};

	const handleSaveComment = async (annId: string) => {
		if (isTranslation(annId)) {
			await onUpdateTranslationAnnotation?.(annId, null, commentDraft.trim());
		} else {
			await updateAnnotation(annId, null, commentDraft.trim());
		}
		setEditingCommentId(null);
		setCommentDraft("");
	};

	const handleCancelEditComment = () => {
		setEditingCommentId(null);
		setCommentDraft("");
	};

	const annotationIcon = (type: string) => {
		switch (type) {
			case "underline":
				return <Underline className="h-3 w-3" />;
			case "area":
				return <ImageIcon className="h-3 w-3" />;
			case "note":
				return <StickyNote className="h-3 w-3" />;
			case "ink":
				return <Pen className="h-3 w-3" />;
			default:
				return <Highlighter className="h-3 w-3" />;
		}
	};

	// Merge original + translation annotations, filter, sort by page order
	const allAnnotations = useMemo(() => {
		const base = [...annotations];
		if (bilingualMode && translationAnnotations) {
			base.push(...translationAnnotations);
		}
		return base;
	}, [annotations, bilingualMode, translationAnnotations]);

	const filteredAnnotations = useMemo(() => {
		let filtered = allAnnotations;
		if (searchQuery.trim()) {
			const q = searchQuery.toLowerCase();
			filtered = allAnnotations.filter(
				(ann) =>
					(ann.selectedText && ann.selectedText.toLowerCase().includes(q)) ||
					(ann.comment.text && ann.comment.text.toLowerCase().includes(q)),
			);
		}
		return [...filtered].sort((a, b) => {
			if (a.pageNumber !== b.pageNumber) return a.pageNumber - b.pageNumber;
			const ay = a.position?.boundingRect?.y1 ?? 0;
			const by = b.position?.boundingRect?.y1 ?? 0;
			return ay - by;
		});
	}, [allAnnotations, searchQuery]);

	// Close context menu when clicking outside
	useEffect(() => {
		if (!contextMenuId) return;
		const handleClickOutside = () => setContextMenuId(null);
		document.addEventListener("click", handleClickOutside);
		return () => document.removeEventListener("click", handleClickOutside);
	}, [contextMenuId]);

	return (
		<div className="flex h-full flex-col border-r">
			{/* Top bar: view switcher icons */}
			<div className="flex items-center gap-1 border-b px-2 py-1.5">
				{readerMode !== "html" && (
					<button
						type="button"
						className={cn(
							"rounded p-1.5 transition-colors",
							leftPanelView === "thumbnails"
								? "bg-muted text-foreground"
								: "text-muted-foreground hover:bg-muted hover:text-foreground",
						)}
						onClick={() => setLeftPanelView("thumbnails")}
						title={t("reader.pageThumbnails")}
					>
						<Grid3X3 className="h-4 w-4" />
					</button>
				)}
				<button
					type="button"
					className={cn(
						"rounded p-1.5 transition-colors",
						leftPanelView === "outline"
							? "bg-muted text-foreground"
							: "text-muted-foreground hover:bg-muted hover:text-foreground",
					)}
					onClick={() => setLeftPanelView("outline")}
					title={t("reader.documentOutline")}
				>
					<List className="h-4 w-4" />
				</button>
				<button
					type="button"
					className={cn(
						"rounded p-1.5 transition-colors",
						leftPanelView === "annotations"
							? "bg-muted text-foreground"
							: "text-muted-foreground hover:bg-muted hover:text-foreground",
					)}
					onClick={() => setLeftPanelView("annotations")}
					title={t("reader.annotations")}
				>
					<Highlighter className="h-4 w-4" />
				</button>

				{/* Search bar (only in annotations view) */}
				{leftPanelView === "annotations" && (
					<div className="ml-auto flex items-center gap-1 rounded border px-1.5 py-0.5 text-xs flex-1 max-w-[160px]">
						<Search className="h-3 w-3 text-muted-foreground shrink-0" />
						<input
							type="text"
							value={searchQuery}
							onChange={(e) => setSearchQuery(e.target.value)}
							placeholder={t("reader.searchAnnotations")}
							className="w-full bg-transparent text-xs outline-none placeholder:text-muted-foreground"
						/>
					</div>
				)}
			</div>

			{/* View content */}
			{leftPanelView === "thumbnails" && readerMode !== "html" && (
				<ThumbnailPanel />
			)}
			{(leftPanelView === "outline" ||
				(leftPanelView === "thumbnails" && readerMode === "html")) && (
				<OutlinePanel readerMode={readerMode} />
			)}
			{leftPanelView === "annotations" && (
				<ScrollArea className="flex-1">
					<div className="p-3 space-y-2">
						{filteredAnnotations.length === 0 ? (
							<div className="text-[11px] text-muted-foreground text-center py-8">
								<Highlighter className="mx-auto mb-2 h-8 w-8 opacity-50" />
								{searchQuery.trim() ? (
									<p>{t("reader.noMatchingAnnotations")}</p>
								) : (
									<>
										<p>{t("reader.noAnnotationsYet")}</p>
										<p className="mt-1">{t("reader.selectTextToHighlight")}</p>
									</>
								)}
							</div>
						) : (
							filteredAnnotations.map((ann) => (
								<div
									key={ann.id}
									className="group relative rounded-md border transition-colors hover:bg-muted/50"
								>
									{/* Clickable card body */}
									<button
										type="button"
										className="w-full p-2 text-xs text-left"
										onClick={() => handleClickAnnotation(ann)}
									>
										{/* Header: type icon + page + source label + color dot + menu */}
										<div className="flex items-center gap-1.5 mb-1">
											<span style={{ color: ann.color }}>
												{annotationIcon(ann.type)}
											</span>
											<span className="text-[10px] text-muted-foreground">
												{ann.pageNumber === 0 ? "HTML" : `p.${ann.pageNumber}`}
											</span>
											{bilingualMode && (
												<span
													className={cn(
														"text-[9px] font-medium px-1 py-px rounded",
														isTranslation(ann.id)
															? "bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-300"
															: "bg-amber-100 text-amber-700 dark:bg-amber-900/40 dark:text-amber-300",
													)}
												>
													{isTranslation(ann.id) ? translationLabel : "PDF"}
												</span>
											)}
											<span
												className="ml-auto h-2.5 w-2.5 rounded-full shrink-0"
												style={{ backgroundColor: ann.color }}
											/>
											<button
												type="button"
												className="hidden group-hover:block p-0.5 rounded text-muted-foreground hover:text-foreground"
												onClick={(e) => {
													e.stopPropagation();
													setContextMenuId(
														contextMenuId === ann.id ? null : ann.id,
													);
												}}
											>
												<MoreHorizontal className="h-3 w-3" />
											</button>
										</div>

										{/* Selected text */}
										{ann.selectedText && (
											<p
												className="text-[11px] text-foreground/80 line-clamp-3 border-l-2 pl-1.5"
												style={{ borderColor: ann.color }}
											>
												{ann.selectedText}
											</p>
										)}

										{/* Area highlight thumbnail */}
										{ann.type === "area" && ann.imageData && (
											<img
												src={ann.imageData}
												alt="Area highlight"
												className="mt-1 rounded border max-h-20 w-full object-contain"
											/>
										)}

										{/* Ink annotation label */}
										{ann.type === "ink" && !ann.selectedText && (
											<p className="text-[11px] text-muted-foreground italic mt-0.5">
												Ink drawing
											</p>
										)}

										{/* Comment */}
										{ann.comment.text && editingCommentId !== ann.id && (
											<div
												className="mt-1 flex items-start gap-1 text-[11px] text-foreground/70 group/comment cursor-pointer"
												onClick={(e) => {
													e.stopPropagation();
													handleStartEditComment(ann);
												}}
											>
												<MessageSquare className="h-3 w-3 shrink-0 mt-0.5" />
												<span className="line-clamp-2 flex-1">
													{ann.comment.text}
												</span>
												<Pencil className="h-3 w-3 shrink-0 mt-0.5 opacity-0 group-hover/comment:opacity-50" />
											</div>
										)}
									</button>

									{/* Inline comment editor */}
									{editingCommentId === ann.id && (
										<div className="px-2 pb-2 border-t">
											<textarea
												autoFocus
												value={commentDraft}
												onChange={(e) => setCommentDraft(e.target.value)}
												onKeyDown={(e) => {
													if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
														e.preventDefault();
														handleSaveComment(ann.id);
													}
													if (e.key === "Escape") {
														handleCancelEditComment();
													}
												}}
												placeholder={t("reader.addCommentPlaceholder")}
												className="w-full mt-1.5 rounded border bg-muted/30 px-1.5 py-1 text-[11px] outline-none resize-none placeholder:text-muted-foreground focus:ring-1 focus:ring-ring"
												rows={2}
											/>
											<div className="flex items-center justify-end gap-1 mt-1">
												<button
													type="button"
													className="rounded px-2 py-0.5 text-[10px] text-muted-foreground hover:bg-muted transition-colors"
													onClick={() => handleCancelEditComment()}
												>
													{t("common.cancel")}
												</button>
												<button
													type="button"
													className="rounded bg-primary px-2 py-0.5 text-[10px] text-primary-foreground hover:bg-primary/90 transition-colors"
													onClick={() => handleSaveComment(ann.id)}
												>
													{t("common.save")}
												</button>
											</div>
										</div>
									)}

									{/* Add Comment link */}
									{!ann.comment.text && editingCommentId !== ann.id && (
										<button
											type="button"
											className="w-full px-2 py-1.5 text-left text-[10px] text-muted-foreground hover:text-foreground border-t transition-colors"
											onClick={(e) => {
												e.stopPropagation();
												handleStartEditComment(ann);
											}}
										>
											Add Comment
										</button>
									)}

									{/* Context menu */}
									{contextMenuId === ann.id && (
										<AnnotationContextMenu
											annotation={ann}
											onChangeColor={(color) => handleChangeColor(ann, color)}
											onConvertType={() => handleConvertType(ann)}
											onDelete={() => handleDeleteAnnotation(ann)}
										/>
									)}
								</div>
							))
						)}
					</div>
				</ScrollArea>
			)}
		</div>
	);
}

/** Context menu for annotation actions (Zotero-style) */
function AnnotationContextMenu({
	annotation,
	onChangeColor,
	onConvertType,
	onDelete,
}: {
	annotation: ZoroHighlight;
	onChangeColor: (color: string) => void;
	onConvertType: () => void;
	onDelete: () => void;
}) {
	return (
		<div
			className="absolute right-0 top-8 z-50 min-w-[180px] rounded-md border bg-popover p-1 shadow-lg"
			onClick={(e) => e.stopPropagation()}
		>
			{/* Color options */}
			<div className="px-2 py-1.5">
				<div className="flex flex-wrap gap-1.5">
					{ANNOTATION_COLORS.map((c) => (
						<button
							key={c.value}
							type="button"
							className={cn(
								"flex items-center gap-1.5 rounded px-1.5 py-0.5 text-[11px] hover:bg-muted transition-colors w-full",
								annotation.color === c.value && "bg-muted",
							)}
							onClick={() => onChangeColor(c.value)}
						>
							<span
								className="h-3.5 w-3.5 rounded-sm shrink-0"
								style={{ backgroundColor: c.value }}
							/>
							<span>{c.name}</span>
							{annotation.color === c.value && (
								<Check className="h-3 w-3 ml-auto" />
							)}
						</button>
					))}
				</div>
			</div>

			<div className="my-1 h-px bg-border" />

			{/* Convert type (highlight <-> underline) */}
			{(annotation.type === "highlight" || annotation.type === "underline") && (
				<button
					type="button"
					className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-[11px] hover:bg-muted transition-colors"
					onClick={onConvertType}
				>
					<ArrowUpDown className="h-3 w-3" />
					{annotation.type === "highlight"
						? "Convert to underline"
						: "Convert to highlight"}
				</button>
			)}

			{/* Delete */}
			<button
				type="button"
				className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-[11px] text-destructive hover:bg-destructive/10 transition-colors"
				onClick={onDelete}
			>
				<Trash2 className="h-3 w-3" />
				Delete
			</button>
		</div>
	);
}
