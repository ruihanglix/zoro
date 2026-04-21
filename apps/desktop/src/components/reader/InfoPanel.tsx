// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { BilingualText } from "@/components/BilingualText";
import { CollapsibleAuthors } from "@/components/CollapsibleAuthors";
import { DisplayModeToggle } from "@/components/DisplayModeToggle";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuSeparator,
	DropdownMenuSub,
	DropdownMenuSubContent,
	DropdownMenuSubTrigger,
	DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import * as commands from "@/lib/commands";
import type { PaperResponse, TagResponse } from "@/lib/commands";
import { cn, confirmAction } from "@/lib/utils";
import { useLibraryStore } from "@/stores/libraryStore";
import {
	useTranslatedText,
	useTranslationLoading,
	useTranslationStore,
} from "@/stores/translationStore";
import { useUiStore } from "@/stores/uiStore";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
	BookCheck,
	BookMarked,
	BookOpen,
	Check,
	Copy,
	Download,
	ExternalLink,
	FileText,
	Globe,
	Languages,
	Loader2,
	MoreHorizontal,
	Plus,
	RefreshCw,
	Trash2,
	X,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

interface InfoPanelProps {
	paper: PaperResponse;
}

export function InfoPanel({ paper }: InfoPanelProps) {
	const { t } = useTranslation();
	const updatePaperStatus = useLibraryStore((s) => s.updatePaperStatus);
	const deletePaper = useLibraryStore((s) => s.deletePaper);
	const fetchPaper = useLibraryStore((s) => s.fetchPaper);
	const addTagToPaper = useLibraryStore((s) => s.addTagToPaper);
	const removeTagFromPaper = useLibraryStore((s) => s.removeTagFromPaper);
	const fetchPapers = useLibraryStore((s) => s.fetchPapers);
	const confirmBeforeDelete = useUiStore((s) => s.confirmBeforeDelete);
	const openMetadataSearch = useUiStore((s) => s.openMetadataSearchDialog);
	const translateFields = useTranslationStore((s) => s.translateFields);

	const ensureTranslated = useTranslationStore((s) => s.ensureTranslated);
	const translatedTitle = useTranslatedText("paper", paper.id, "title");
	const translatedAbstract = useTranslatedText(
		"paper",
		paper.id,
		"abstract_text",
	);
	const translationLoading = useTranslationLoading("paper", paper.id);

	useEffect(() => {
		const fields = ["title"];
		if (paper.abstract_text) fields.push("abstract_text");
		ensureTranslated("paper", paper.id, fields);
	}, [paper.id, paper.abstract_text, ensureTranslated]);

	// Parse labels from extra_json
	const labels: string[] = (() => {
		if (!paper.extra_json) return [];
		try {
			const extra = JSON.parse(paper.extra_json);
			return Array.isArray(extra.labels) ? extra.labels : [];
		} catch {
			return [];
		}
	})();

	const CITATION_STYLES = [
		{ id: "bibtex", label: "BibTeX" },
		{ id: "apa", label: "APA" },
		{ id: "ieee", label: "IEEE" },
		{ id: "mla", label: "MLA" },
		{ id: "chicago", label: "Chicago" },
		{ id: "vancouver", label: "Vancouver" },
		{ id: "ris", label: "RIS" },
	];

	const [copiedStyle, setCopiedStyle] = useState<string | null>(null);
	const [enriching, setEnriching] = useState(false);

	const handleCopyCitation = async (style: string) => {
		try {
			const result =
				style === "bibtex"
					? await commands.getPaperBibtex(paper.id)
					: await commands.getFormattedCitation(paper.id, style);
			await writeText(result.text);
			setCopiedStyle(style);
			setTimeout(() => setCopiedStyle(null), 2000);
		} catch (err) {
			console.error("Failed to copy citation:", err);
		}
	};

	const handleFetchArxivHtml = async () => {
		if (
			paper.has_html &&
			!(await confirmAction(t("paper.redownloadHtmlConfirm")))
		) {
			return;
		}
		try {
			await commands.fetchArxivHtml(paper.id);
		} catch (e) {
			console.error("Failed to fetch arXiv HTML:", e);
		}
	};

	const handleEnrich = async () => {
		setEnriching(true);
		try {
			await commands.enrichPaperMetadata(paper.id);
			await fetchPaper(paper.id);
		} catch (err) {
			console.error("Enrichment failed:", err);
		}
		setEnriching(false);
	};

	const handleTranslate = () => {
		const fields = ["title"];
		if (paper.abstract_text) fields.push("abstract_text");
		translateFields("paper", paper.id, fields);
	};

	const handleDelete = () => {
		setTimeout(async () => {
			if (
				confirmBeforeDelete &&
				!(await confirmAction(t("paper.deleteConfirm")))
			) {
				return;
			}
			try {
				await deletePaper(paper.id);
			} catch (err) {
				console.error("Failed to delete paper:", err);
			}
		}, 0);
	};

	const cycleReadStatus = () => {
		const next =
			paper.read_status === "unread"
				? "reading"
				: paper.read_status === "reading"
					? "read"
					: "unread";
		updatePaperStatus(paper.id, next);
	};

	const statusIcon =
		paper.read_status === "read" ? (
			<BookCheck className="h-3.5 w-3.5" />
		) : paper.read_status === "reading" ? (
			<BookMarked className="h-3.5 w-3.5" />
		) : (
			<BookOpen className="h-3.5 w-3.5" />
		);

	return (
		<ScrollArea className="flex-1">
			<div className="p-4">
				<div className="space-y-3">
					{/* Title */}
					<BilingualText
						original={paper.title}
						translated={translatedTitle}
						loading={translationLoading}
						variant="title"
						className="text-sm"
					/>

					{/* Authors */}
					{paper.authors.length > 0 && (
						<CollapsibleAuthors
							authors={paper.authors.map((a) => a.name)}
						/>
					)}

					{/* Read status + display mode + overflow menu */}
					<div className="flex items-center gap-2">
						<Button
							size="sm"
							variant="outline"
							className="h-7 text-xs capitalize"
							onClick={cycleReadStatus}
						>
							{statusIcon}
							<span className="ml-1.5">{paper.read_status}</span>
						</Button>
						<DisplayModeToggle />

						{/* Overflow menu */}
						<DropdownMenu>
							<DropdownMenuTrigger asChild>
								<Button
									size="sm"
									variant="ghost"
									className="h-7 w-7 p-0"
								>
									<MoreHorizontal className="h-4 w-4" />
								</Button>
							</DropdownMenuTrigger>
							<DropdownMenuContent align="start">
								{paper.arxiv_id && (
									<DropdownMenuItem
										onClick={handleFetchArxivHtml}
									>
										<Download className="h-4 w-4" />
										{paper.has_html
											? t("paper.refetchHtml")
											: t("paper.arxivHtml")}
									</DropdownMenuItem>
								)}
								<DropdownMenuItem
									onClick={() => {
										const q = encodeURIComponent(
											paper.title,
										);
										window.open(
											`https://scholar.google.com/scholar?q=${q}`,
											"_blank",
										);
									}}
								>
									<ExternalLink className="h-4 w-4" />
									{t("paper.scholar")}
								</DropdownMenuItem>

								{/* Cite sub-menu */}
								<DropdownMenuSub>
									<DropdownMenuSubTrigger>
										{copiedStyle ? (
											<Check className="h-4 w-4 text-green-500" />
										) : (
											<Copy className="h-4 w-4" />
										)}
										{copiedStyle
											? t("common.copied")
											: t("paper.cite")}
									</DropdownMenuSubTrigger>
									<DropdownMenuSubContent>
										{CITATION_STYLES.map((s) => (
											<DropdownMenuItem
												key={s.id}
												onClick={() =>
													handleCopyCitation(s.id)
												}
											>
												{copiedStyle === s.id ? (
													<Check className="h-4 w-4 text-green-500" />
												) : (
													<Copy className="h-4 w-4 opacity-50" />
												)}
												{s.label}
											</DropdownMenuItem>
										))}
									</DropdownMenuSubContent>
								</DropdownMenuSub>

								{/* Export sub-menu */}
								{(paper.has_pdf || paper.has_html) && (
									<DropdownMenuSub>
										<DropdownMenuSubTrigger>
											<Download className="h-4 w-4" />
											{t("paper.export")}
										</DropdownMenuSubTrigger>
										<DropdownMenuSubContent>
											{paper.has_pdf && (
												<DropdownMenuItem
													onClick={async () => {
														try {
															await commands.exportPdf(
																paper.id,
															);
														} catch (e) {
															console.error(
																"Export PDF failed:",
																e,
															);
														}
													}}
												>
													<FileText className="h-4 w-4" />
													PDF
												</DropdownMenuItem>
											)}
											{paper.has_html && (
												<DropdownMenuItem
													onClick={async () => {
														try {
															await commands.exportHtml(
																paper.id,
															);
														} catch (e) {
															console.error(
																"Export HTML failed:",
																e,
															);
														}
													}}
												>
													<Globe className="h-4 w-4" />
													HTML
												</DropdownMenuItem>
											)}
											<DropdownMenuSeparator />
											{paper.has_pdf && (
												<DropdownMenuItem
													onClick={async () => {
														try {
															await commands.exportAnnotatedPdf(
																paper.id,
															);
														} catch (e) {
															console.error(
																"Export annotated PDF failed:",
																e,
															);
														}
													}}
												>
													<FileText className="h-4 w-4" />
													{t(
														"paper.pdfWithAnnotations",
													)}
												</DropdownMenuItem>
											)}
											{paper.has_html && (
												<DropdownMenuItem
													onClick={async () => {
														try {
															await commands.exportAnnotatedHtml(
																paper.id,
															);
														} catch (e) {
															console.error(
																"Export annotated HTML failed:",
																e,
															);
														}
													}}
												>
													<Globe className="h-4 w-4" />
													{t(
														"paper.htmlWithAnnotations",
													)}
												</DropdownMenuItem>
											)}
										</DropdownMenuSubContent>
									</DropdownMenuSub>
								)}

								<DropdownMenuSub>
									<DropdownMenuSubTrigger>
										<RefreshCw
											className={
												enriching
													? "h-4 w-4 animate-spin"
													: "h-4 w-4"
											}
										/>
										{t("contextMenu.metadata")}
									</DropdownMenuSubTrigger>
									<DropdownMenuSubContent>
										<DropdownMenuItem
											onClick={handleEnrich}
											disabled={enriching}
										>
											{enriching
												? t("paper.enriching")
												: t(
														"contextMenu.autoFetchMetadata",
													)}
										</DropdownMenuItem>
										<DropdownMenuItem
											onClick={() =>
												openMetadataSearch(paper.id)
											}
										>
											{t(
												"contextMenu.manualSearchMetadata",
											)}
										</DropdownMenuItem>
									</DropdownMenuSubContent>
								</DropdownMenuSub>

								<DropdownMenuItem
									onClick={handleTranslate}
									disabled={translationLoading}
								>
									{translationLoading ? (
										<Loader2 className="h-4 w-4 animate-spin" />
									) : (
										<Languages className="h-4 w-4" />
									)}
									{translationLoading
										? t("paper.translating")
										: translatedTitle
											? t("paper.retranslate")
											: t("paper.translate")}
								</DropdownMenuItem>

								<DropdownMenuSeparator />
								<DropdownMenuItem
									className="text-destructive focus:text-destructive"
									onSelect={handleDelete}
								>
									<Trash2 className="h-4 w-4" />
									{t("common.delete")}
								</DropdownMenuItem>
							</DropdownMenuContent>
						</DropdownMenu>
					</div>

					<Separator />

					{/* Metadata grid */}
					<div className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1.5 text-xs">
						{paper.doi && (
							<>
								<span className="font-medium text-muted-foreground">
									DOI
								</span>
								<span className="truncate">{paper.doi}</span>
							</>
						)}
						{paper.arxiv_id && (
							<>
								<span className="font-medium text-muted-foreground">
									ArXiv
								</span>
								<span className="truncate">
									{paper.arxiv_id}
								</span>
							</>
						)}
						{paper.published_date && (
							<>
								<span className="font-medium text-muted-foreground">
									{t("paper.published")}
								</span>
								<span>{paper.published_date}</span>
							</>
						)}
						<span className="font-medium text-muted-foreground">
							{t("paper.added")}
						</span>
						<span>
							{new Date(paper.added_date).toLocaleString(
								undefined,
								{
									year: "numeric",
									month: "short",
									day: "numeric",
									hour: "2-digit",
									minute: "2-digit",
								},
							)}
						</span>
						{paper.source && (
							<>
								<span className="font-medium text-muted-foreground">
									{t("paper.source")}
								</span>
								<span>{paper.source}</span>
							</>
						)}
					</div>

					{/* Abstract */}
					{paper.abstract_text && (
						<>
							<Separator />
							<div>
								<h4 className="mb-1 text-xs font-medium text-muted-foreground">
									{t("paper.abstract")}
								</h4>
								<BilingualText
									original={paper.abstract_text}
									translated={translatedAbstract}
									loading={translationLoading}
									variant="abstract"
									className="text-xs"
								/>
							</div>
						</>
					)}

					{/* Tags */}
					<Separator />
					<div className="space-y-2">
						<h4 className="text-xs font-medium text-muted-foreground">
							{t("paper.tags")}
						</h4>
						<div className="flex flex-wrap gap-1.5">
							{paper.tags.map((tag) => (
								<Badge
									key={tag.id}
									variant="secondary"
									className="text-xs group/tag"
								>
									{tag.name}
									<button
										type="button"
										className="ml-0.5 opacity-0 group-hover/tag:opacity-100 transition-opacity"
										onClick={async () => {
											await removeTagFromPaper(
												paper.id,
												tag.name,
											);
											await fetchPapers();
										}}
									>
										<X className="h-3 w-3" />
									</button>
								</Badge>
							))}
							<TagAdder
								onAdd={async (name) => {
									await addTagToPaper(paper.id, name);
									await fetchPapers();
								}}
							/>
						</div>
						{labels.length > 0 && (
							<>
								<h4 className="text-xs font-medium text-muted-foreground">
									Labels
								</h4>
								<div className="flex flex-wrap gap-1.5">
									{labels.map((label) => (
										<Badge
											key={label}
											variant="outline"
											className="text-xs"
										>
											{label}
										</Badge>
									))}
								</div>
							</>
						)}
					</div>
				</div>
			</div>
		</ScrollArea>
	);
}

/** Inline tag adder with autocomplete */
function TagAdder({ onAdd }: { onAdd: (name: string) => Promise<void> }) {
	const { t } = useTranslation();
	const [editing, setEditing] = useState(false);
	const [value, setValue] = useState("");
	const [suggestions, setSuggestions] = useState<TagResponse[]>([]);
	const [selectedIdx, setSelectedIdx] = useState(-1);
	const inputRef = useRef<HTMLInputElement>(null);

	useEffect(() => {
		if (editing) inputRef.current?.focus();
	}, [editing]);

	useEffect(() => {
		if (!value.trim()) {
			setSuggestions([]);
			setSelectedIdx(-1);
			return;
		}
		let cancelled = false;
		commands.searchTags(value.trim(), 8).then((tags) => {
			if (!cancelled) {
				setSuggestions(tags);
				setSelectedIdx(-1);
			}
		});
		return () => {
			cancelled = true;
		};
	}, [value]);

	const submit = async (name: string) => {
		const trimmed = name.trim();
		if (!trimmed) return;
		await onAdd(trimmed);
		setValue("");
		setSuggestions([]);
		setEditing(false);
	};

	const handleKeyDown = (e: React.KeyboardEvent) => {
		if (e.key === "Escape") {
			setEditing(false);
			setValue("");
			setSuggestions([]);
		} else if (e.key === "Enter") {
			e.preventDefault();
			if (selectedIdx >= 0 && selectedIdx < suggestions.length) {
				submit(suggestions[selectedIdx].name);
			} else {
				submit(value);
			}
		} else if (e.key === "ArrowDown") {
			e.preventDefault();
			setSelectedIdx((i) => Math.min(i + 1, suggestions.length - 1));
		} else if (e.key === "ArrowUp") {
			e.preventDefault();
			setSelectedIdx((i) => Math.max(i - 1, 0));
		}
	};

	if (!editing) {
		return (
			<button
				type="button"
				className="inline-flex items-center gap-0.5 rounded-md border border-dashed px-2 py-0.5 text-xs text-muted-foreground hover:text-foreground hover:border-foreground transition-colors"
				onClick={() => setEditing(true)}
			>
				<Plus className="h-3 w-3" />
			</button>
		);
	}

	return (
		<div className="relative">
			<input
				ref={inputRef}
				type="text"
				className="h-6 w-28 rounded-md border bg-background px-2 text-xs outline-none focus:ring-1 focus:ring-primary"
				placeholder={t("paper.addTag")}
				value={value}
				onChange={(e) => setValue(e.target.value)}
				onKeyDown={handleKeyDown}
				onBlur={() => {
					setTimeout(() => {
						setEditing(false);
						setValue("");
						setSuggestions([]);
					}, 150);
				}}
			/>
			{suggestions.length > 0 && (
				<div className="absolute top-full left-0 z-50 mt-1 w-40 rounded-md border bg-popover shadow-md">
					{suggestions.map((tag, i) => (
						<button
							key={tag.id}
							type="button"
							className={cn(
								"w-full px-2 py-1 text-left text-xs hover:bg-muted transition-colors",
								i === selectedIdx && "bg-muted",
							)}
							onMouseDown={(e) => {
								e.preventDefault();
								submit(tag.name);
							}}
						>
							{tag.name}
						</button>
					))}
				</div>
			)}
		</div>
	);
}
