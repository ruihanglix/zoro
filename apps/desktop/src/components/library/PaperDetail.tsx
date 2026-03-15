// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { BilingualText } from "@/components/BilingualText";
import { EditableField } from "@/components/library/EditableField";
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
import type {
	CitationResponse,
	HttpDebugInfo,
	PaperResponse,
	UpdatePaperInput,
} from "@/lib/commands";
import * as commands from "@/lib/commands";
import { useLibraryStore } from "@/stores/libraryStore";
import { useTabStore } from "@/stores/tabStore";
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
	ChevronRight,
	Copy,
	Download,
	ExternalLink,
	FileText,
	Globe,
	Languages,
	Loader2,
	MoreHorizontal,
	RefreshCw,
	Trash2,
	X,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

const CITATION_STYLES = [
	{ id: "bibtex", label: "BibTeX" },
	{ id: "apa", label: "APA" },
	{ id: "ieee", label: "IEEE" },
	{ id: "mla", label: "MLA" },
	{ id: "chicago", label: "Chicago" },
	{ id: "vancouver", label: "Vancouver" },
	{ id: "ris", label: "RIS" },
];

export function PaperDetail({ paper }: { paper: PaperResponse }) {
	const { t } = useTranslation();
	const deletePaper = useLibraryStore((s) => s.deletePaper);
	const updatePaperStatus = useLibraryStore((s) => s.updatePaperStatus);
	const updatePaper = useLibraryStore((s) => s.updatePaper);
	const updatePaperAuthors = useLibraryStore((s) => s.updatePaperAuthors);
	const setSelectedPaper = useLibraryStore((s) => s.setSelectedPaper);
	const fetchPaper = useLibraryStore((s) => s.fetchPaper);
	const openTab = useTabStore((s) => s.openTab);

	// Translation
	const ensureTranslated = useTranslationStore((s) => s.ensureTranslated);
	const translateFields = useTranslationStore((s) => s.translateFields);

	const translatedTitle = useTranslatedText("paper", paper.id, "title");
	const translatedAbstract = useTranslatedText(
		"paper",
		paper.id,
		"abstract_text",
	);
	const translationLoading = useTranslationLoading("paper", paper.id);

	// Auto-translate on view
	useEffect(() => {
		const fields = ["title"];
		if (paper.abstract_text) fields.push("abstract_text");
		ensureTranslated("paper", paper.id, fields);
	}, [paper.id, paper.abstract_text, ensureTranslated]);

	const handleTranslate = () => {
		const fields = ["title"];
		if (paper.abstract_text) fields.push("abstract_text");
		translateFields("paper", paper.id, fields);
	};

	const handleRetranslate = () => {
		handleTranslate();
	};

	// Parse labels from extra_json (read-only metadata labels from Zotero, arXiv, etc.)
	const labels: string[] = (() => {
		if (!paper.extra_json) return [];
		try {
			const extra = JSON.parse(paper.extra_json);
			return Array.isArray(extra.labels) ? extra.labels : [];
		} catch {
			return [];
		}
	})();

	const [copiedStyle, setCopiedStyle] = useState<string | null>(null);
	const [enriching, setEnriching] = useState(false);

	const debugMode = useUiStore((s) => s.debugMode);

	const [citationResult, setCitationResult] = useState<CitationResponse | null>(
		null,
	);
	const [citationStyle, setCitationStyle] = useState("bibtex");
	const [citationLoading, setCitationLoading] = useState(false);
	const [citationError, setCitationError] = useState<string | null>(null);
	const [citationCopied, setCitationCopied] = useState(false);
	const [httpDebugOpen, setHttpDebugOpen] = useState(false);

	const confirmBeforeDelete = useUiStore((s) => s.confirmBeforeDelete);
	const openMetadataSearch = useUiStore((s) => s.openMetadataSearchDialog);

	const handleDelete = () => {
		setTimeout(async () => {
			if (confirmBeforeDelete && !confirm(t("paper.deleteConfirm"))) {
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
		if (paper.has_html && !confirm(t("paper.redownloadHtmlConfirm"))) {
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

	const handleFetchCitation = async (style: string) => {
		setCitationStyle(style);
		setCitationLoading(true);
		setCitationError(null);
		setCitationResult(null);
		setCitationCopied(false);
		try {
			const result =
				style === "bibtex"
					? await commands.getPaperBibtex(paper.id)
					: await commands.getFormattedCitation(paper.id, style);
			setCitationResult(result);
		} catch (err) {
			setCitationError(String(err));
		} finally {
			setCitationLoading(false);
		}
	};

	const handleCopyCitationFromPreview = async () => {
		if (!citationResult) return;
		try {
			await writeText(citationResult.text);
			setCitationCopied(true);
			setTimeout(() => setCitationCopied(false), 2000);
		} catch (err) {
			console.error("Failed to copy citation:", err);
		}
	};

	// Save a single metadata field
	const saveField = useCallback(
		(field: keyof UpdatePaperInput) => (value: string | null) => {
			updatePaper(paper.id, { [field]: value });
		},
		[paper.id, updatePaper],
	);

	// Save authors (comma-separated string -> array of names)
	const saveAuthors = useCallback(
		(value: string | null) => {
			if (value === null) {
				updatePaperAuthors(paper.id, []);
			} else {
				const names = value
					.split(",")
					.map((n) => n.trim())
					.filter((n) => n.length > 0);
				updatePaperAuthors(paper.id, names);
			}
		},
		[paper.id, updatePaperAuthors],
	);

	const statusIcon =
		paper.read_status === "read" ? (
			<BookCheck className="mr-1.5 h-3.5 w-3.5" />
		) : paper.read_status === "reading" ? (
			<BookMarked className="mr-1.5 h-3.5 w-3.5" />
		) : (
			<BookOpen className="mr-1.5 h-3.5 w-3.5" />
		);

	const authorsText =
		paper.authors.length > 0
			? paper.authors.map((a) => a.name).join(", ")
			: null;

	return (
		<ScrollArea className="h-full">
			<div className="p-5">
				{/* Header: Title + close */}
				<div className="flex items-start gap-3">
					<div className="flex-1">
						<BilingualText
							original={paper.title}
							translated={translatedTitle}
							loading={translationLoading}
							onRequestTranslation={handleTranslate}
							variant="title"
						/>
					</div>
					<Button
						variant="ghost"
						size="icon"
						className="h-7 w-7 shrink-0"
						onClick={() => setSelectedPaper(null)}
					>
						<X className="h-4 w-4" />
					</Button>
				</div>

				{/* Action buttons */}
				<div className="mt-3 flex flex-wrap gap-1.5">
					{paper.has_pdf && (
						<Button
							size="sm"
							variant="outline"
							className="h-7 text-xs"
							onClick={() =>
								openTab({
									type: "reader",
									paperId: paper.id,
									readerMode: "pdf",
									title: paper.title,
								})
							}
						>
							<FileText className="mr-1.5 h-3.5 w-3.5" />
							PDF
						</Button>
					)}
					{paper.has_html && (
						<Button
							size="sm"
							variant="outline"
							className="h-7 text-xs"
							onClick={() =>
								openTab({
									type: "reader",
									paperId: paper.id,
									readerMode: "html",
									title: paper.title,
								})
							}
						>
							<Globe className="mr-1.5 h-3.5 w-3.5" />
							HTML
						</Button>
					)}

					{paper.url && (
						<Button size="sm" variant="outline" className="h-7 text-xs" asChild>
							<a href={paper.url} target="_blank" rel="noopener noreferrer">
								<ExternalLink className="mr-1.5 h-3.5 w-3.5" />
								URL
							</a>
						</Button>
					)}

					<Button
						size="sm"
						variant="ghost"
						className="h-7 text-xs capitalize"
						onClick={cycleReadStatus}
					>
						{statusIcon}
						{paper.read_status === "read"
							? t("paper.read")
							: paper.read_status === "reading"
								? t("paper.reading")
								: t("paper.unread")}
					</Button>

					{/* Overflow menu */}
					<DropdownMenu>
						<DropdownMenuTrigger asChild>
							<Button size="sm" variant="ghost" className="h-7 w-7 p-0">
								<MoreHorizontal className="h-4 w-4" />
							</Button>
						</DropdownMenuTrigger>
						<DropdownMenuContent align="start">
							{paper.arxiv_id && (
								<DropdownMenuItem onClick={handleFetchArxivHtml}>
									<Download className="h-4 w-4" />
									{paper.has_html
										? t("paper.refetchHtml")
										: t("paper.arxivHtml")}
								</DropdownMenuItem>
							)}
							<DropdownMenuItem
								onClick={() => {
									const q = encodeURIComponent(paper.title);
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
									{copiedStyle ? t("common.copied") : t("paper.cite")}
								</DropdownMenuSubTrigger>
								<DropdownMenuSubContent>
									{CITATION_STYLES.map((s) => (
										<DropdownMenuItem
											key={s.id}
											onClick={() => handleCopyCitation(s.id)}
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
														await commands.exportPdf(paper.id);
													} catch (e) {
														console.error("Export PDF failed:", e);
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
														await commands.exportHtml(paper.id);
													} catch (e) {
														console.error("Export HTML failed:", e);
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
														await commands.exportAnnotatedPdf(paper.id);
													} catch (e) {
														console.error("Export annotated PDF failed:", e);
													}
												}}
											>
												<FileText className="h-4 w-4" />
												{t("paper.pdfWithAnnotations")}
											</DropdownMenuItem>
										)}
										{paper.has_html && (
											<DropdownMenuItem
												onClick={async () => {
													try {
														await commands.exportAnnotatedHtml(paper.id);
													} catch (e) {
														console.error("Export annotated HTML failed:", e);
													}
												}}
											>
												<Globe className="h-4 w-4" />
												{t("paper.htmlWithAnnotations")}
											</DropdownMenuItem>
										)}
									</DropdownMenuSubContent>
								</DropdownMenuSub>
							)}

							<DropdownMenuSub>
								<DropdownMenuSubTrigger>
									<RefreshCw
										className={enriching ? "h-4 w-4 animate-spin" : "h-4 w-4"}
									/>
									{t("contextMenu.metadata")}
								</DropdownMenuSubTrigger>
								<DropdownMenuSubContent>
									<DropdownMenuItem onClick={handleEnrich} disabled={enriching}>
										{enriching
											? t("paper.enriching")
											: t("contextMenu.autoFetchMetadata")}
									</DropdownMenuItem>
									<DropdownMenuItem
										onClick={() => openMetadataSearch(paper.id)}
									>
										{t("contextMenu.manualSearchMetadata")}
									</DropdownMenuItem>
								</DropdownMenuSubContent>
							</DropdownMenuSub>

							<DropdownMenuItem
								onClick={translatedTitle ? handleRetranslate : handleTranslate}
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

				{/* Abstract */}
				<Separator className="my-3" />
				<div>
					<h3 className="mb-1.5 text-xs font-medium text-muted-foreground">
						{t("paper.abstract")}
					</h3>
					{paper.abstract_text ? (
						<BilingualText
							original={paper.abstract_text}
							translated={translatedAbstract}
							loading={translationLoading}
							onRequestTranslation={handleTranslate}
							variant="abstract"
						/>
					) : (
						<EditableField
							value={null}
							onSave={saveField("abstract_text")}
							placeholder={t("paper.clickToAddAbstract")}
							multiline
						/>
					)}
				</div>

				<Separator className="my-3" />

				{/* Metadata grid — Zotero-style 2-column key/value, all fields visible */}
				<div className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1.5 items-baseline text-xs">
					{/* Entry Type */}
					<span className="font-medium text-muted-foreground">
						{t("paper.type")}
					</span>
					<EditableField
						value={paper.entry_type}
						onSave={saveField("entry_type")}
						placeholder={t("paper.entryTypePlaceholder")}
					/>

					{/* Title (editable in grid) */}
					<span className="font-medium text-muted-foreground">
						{t("paper.title")}
					</span>
					<EditableField
						value={paper.title}
						onSave={saveField("title")}
						placeholder={t("paper.titlePlaceholder")}
					/>

					{/* Short Title */}
					<span className="font-medium text-muted-foreground">
						{t("paper.shortTitle")}
					</span>
					<EditableField
						value={paper.short_title}
						onSave={saveField("short_title")}
						placeholder={t("paper.shortTitlePlaceholder")}
					/>

					{/* Authors (comma-separated) */}
					<span className="font-medium text-muted-foreground self-start">
						{t("paper.authors")}
					</span>
					<EditableField
						value={authorsText}
						onSave={saveAuthors}
						placeholder={t("paper.authorPlaceholder")}
					/>

					{/* DOI */}
					<span className="font-medium text-muted-foreground">
						{t("paper.doi")}
					</span>
					<EditableField
						value={paper.doi}
						onSave={saveField("doi")}
						placeholder={t("paper.doiPlaceholder")}
					/>

					{/* ArXiv ID */}
					<span className="font-medium text-muted-foreground">
						{t("paper.arxiv")}
					</span>
					<EditableField
						value={paper.arxiv_id}
						onSave={saveField("arxiv_id")}
						placeholder={t("paper.arxivPlaceholder")}
					/>

					{/* Journal */}
					<span className="font-medium text-muted-foreground">
						{t("paper.journal")}
					</span>
					<EditableField
						value={paper.journal}
						onSave={saveField("journal")}
						placeholder={t("paper.journalPlaceholder")}
					/>

					{/* Volume */}
					<span className="font-medium text-muted-foreground">
						{t("paper.volume")}
					</span>
					<EditableField
						value={paper.volume}
						onSave={saveField("volume")}
						placeholder={t("paper.volumePlaceholder")}
					/>

					{/* Issue */}
					<span className="font-medium text-muted-foreground">
						{t("paper.issue")}
					</span>
					<EditableField
						value={paper.issue}
						onSave={saveField("issue")}
						placeholder={t("paper.issuePlaceholder")}
					/>

					{/* Pages */}
					<span className="font-medium text-muted-foreground">
						{t("paper.pages")}
					</span>
					<EditableField
						value={paper.pages}
						onSave={saveField("pages")}
						placeholder={t("paper.pagesPlaceholder")}
					/>

					{/* Publisher */}
					<span className="font-medium text-muted-foreground">
						{t("paper.publisher")}
					</span>
					<EditableField
						value={paper.publisher}
						onSave={saveField("publisher")}
						placeholder={t("paper.publisherPlaceholder")}
					/>

					{/* Published Date */}
					<span className="font-medium text-muted-foreground">
						{t("paper.published")}
					</span>
					<EditableField
						value={paper.published_date}
						onSave={saveField("published_date")}
						placeholder={t("paper.publishedDatePlaceholder")}
					/>

					{/* Added Date (read-only) */}
					<span className="font-medium text-muted-foreground">
						{t("paper.added")}
					</span>
					<EditableField
						value={new Date(paper.added_date).toLocaleString(undefined, {
							year: "numeric",
							month: "short",
							day: "numeric",
							hour: "2-digit",
							minute: "2-digit",
						})}
						onSave={() => {}}
						readOnly
					/>

					{/* Source */}
					<span className="font-medium text-muted-foreground">
						{t("paper.source")}
					</span>
					<EditableField
						value={paper.source}
						onSave={saveField("source")}
						placeholder={t("paper.sourcePlaceholder")}
					/>

					{/* URL */}
					<span className="font-medium text-muted-foreground">
						{t("paper.url")}
					</span>
					<EditableField
						value={paper.url}
						onSave={saveField("url")}
						placeholder={t("paper.urlPlaceholder")}
					/>

					{/* ISSN */}
					<span className="font-medium text-muted-foreground">
						{t("paper.issn")}
					</span>
					<EditableField
						value={paper.issn}
						onSave={saveField("issn")}
						placeholder={t("paper.issnPlaceholder")}
					/>

					{/* ISBN */}
					<span className="font-medium text-muted-foreground">
						{t("paper.isbn")}
					</span>
					<EditableField
						value={paper.isbn}
						onSave={saveField("isbn")}
						placeholder={t("paper.isbnPlaceholder")}
					/>
				</div>

				{/* Tags */}
				{paper.tags.length > 0 && (
					<>
						<Separator className="my-3" />
						<div>
							<h3 className="mb-1.5 text-xs font-medium text-muted-foreground">
								{t("paper.tags")}
							</h3>
							<div className="flex flex-wrap gap-1">
								{paper.tags.map((tag) => (
									<Badge key={tag.id} variant="secondary" className="text-xs">
										{tag.name}
									</Badge>
								))}
							</div>
						</div>
					</>
				)}

				{/* Labels (read-only metadata from Zotero, arXiv, etc.) */}
				{labels.length > 0 && (
					<>
						<Separator className="my-3" />
						<div>
							<h3 className="mb-1.5 text-xs font-medium text-muted-foreground">
								{t("paper.labels")}
							</h3>
							<div className="flex flex-wrap gap-1">
								{labels.map((label) => (
									<Badge key={label} variant="outline" className="text-xs">
										{label}
									</Badge>
								))}
							</div>
						</div>
					</>
				)}

				{/* Attachments */}
				{paper.attachments.length > 0 && (
					<>
						<Separator className="my-3" />
						<div>
							<h3 className="mb-1.5 text-xs font-medium text-muted-foreground">
								{t("paper.attachments")}
							</h3>
							<div className="space-y-1">
								{paper.attachments.map((att) => {
									const isOpenable =
										att.is_local &&
										(att.file_type === "pdf" || att.file_type === "html");
									const handleOpen = () => {
										if (att.file_type === "pdf") {
											openTab({
												type: "reader",
												paperId: paper.id,
												readerMode: "pdf",
												pdfFilename: att.filename,
												title: `${att.filename} - ${paper.title}`,
											});
										} else if (att.file_type === "html") {
											openTab({
												type: "reader",
												paperId: paper.id,
												readerMode: "html",
												title: paper.title,
											});
										}
									};
									return (
										<div
											key={att.id}
											className={`flex items-center gap-2 text-xs text-muted-foreground ${isOpenable ? "cursor-pointer hover:text-foreground" : ""}`}
											onClick={isOpenable ? handleOpen : undefined}
										>
											<FileText className="h-3 w-3 shrink-0" />
											<span className="truncate">{att.filename}</span>
											{att.file_size != null && att.file_size > 0 && (
												<span className="shrink-0 text-[10px]">
													({(att.file_size / 1024).toFixed(0)} KB)
												</span>
											)}
										</div>
									);
								})}
							</div>
						</div>
					</>
				)}
				{/* Citation Preview */}
				<Separator className="my-3" />
				<div>
					<h3 className="mb-1.5 text-xs font-medium text-muted-foreground">
						{t("paper.citation")}
					</h3>
					<div className="flex flex-wrap gap-1 mb-2">
						{CITATION_STYLES.map((s) => (
							<Button
								key={s.id}
								size="sm"
								variant={
									citationStyle === s.id && citationResult
										? "default"
										: "outline"
								}
								className="h-6 text-[11px] px-2"
								onClick={() => handleFetchCitation(s.id)}
								disabled={citationLoading}
							>
								{citationLoading && citationStyle === s.id ? (
									<Loader2 className="mr-1 h-3 w-3 animate-spin" />
								) : null}
								{s.label}
							</Button>
						))}
					</div>

					{citationError && (
						<p className="text-xs text-destructive mb-2">{citationError}</p>
					)}

					{citationResult && (
						<div className="space-y-2">
							<div className="relative">
								<pre className="text-xs bg-muted/50 rounded-md p-2.5 whitespace-pre-wrap break-all font-mono leading-relaxed max-h-48 overflow-y-auto">
									{citationResult.text}
								</pre>
								<Button
									size="sm"
									variant="ghost"
									className="absolute top-1 right-1 h-6 w-6 p-0"
									onClick={handleCopyCitationFromPreview}
								>
									{citationCopied ? (
										<Check className="h-3.5 w-3.5 text-green-500" />
									) : (
										<Copy className="h-3.5 w-3.5" />
									)}
								</Button>
							</div>

							{debugMode && (
								<>
									<div className="text-[10px] text-muted-foreground space-y-0.5 border-l-2 border-muted pl-2">
										<div>
											<span className="font-medium">Provider:</span>{" "}
											{citationResult.source.provider}
										</div>
										{citationResult.source.doi && (
											<div>
												<span className="font-medium">DOI:</span>{" "}
												{citationResult.source.doi}
											</div>
										)}
										{citationResult.source.request_url && (
											<div>
												<span className="font-medium">URL:</span>{" "}
												{citationResult.source.request_url}
											</div>
										)}
										{citationResult.source.accept_header && (
											<div>
												<span className="font-medium">Accept:</span>{" "}
												<code className="bg-muted px-1 rounded">
													{citationResult.source.accept_header}
												</code>
											</div>
										)}
										<div>
											<span className="font-medium">
												{citationResult.cached ? "Cached" : "Fetched"}:
											</span>{" "}
											{citationResult.fetched_date
												? new Date(citationResult.fetched_date).toLocaleString()
												: "just now"}
										</div>
									</div>

									{citationResult.http_debug && (
										<HttpDebugPanel
											debug={citationResult.http_debug}
											open={httpDebugOpen}
											onToggle={() => setHttpDebugOpen((v) => !v)}
										/>
									)}
									{citationResult.cached && !citationResult.http_debug && (
										<p className="text-[10px] text-muted-foreground italic">
											{t("reader.cachedNoRequest")}
										</p>
									)}
								</>
							)}
						</div>
					)}

					{!citationResult && !citationError && !citationLoading && (
						<p className="text-xs text-muted-foreground">
							{t("paper.selectFormatToPreview")}
						</p>
					)}
				</div>
			</div>
		</ScrollArea>
	);
}

function HttpDebugPanel({
	debug,
	open,
	onToggle,
}: { debug: HttpDebugInfo; open: boolean; onToggle: () => void }) {
	const { t } = useTranslation();
	const requestLine = `${debug.method} ${debug.request_url}`;
	const requestHeaders = Object.entries(debug.request_headers)
		.map(([k, v]) => `${k}: ${v}`)
		.join("\n");
	const responseLine = `HTTP ${debug.status_code} (final: ${debug.final_url})`;
	const responseHeaders = Object.entries(debug.response_headers)
		.map(([k, v]) => `${k}: ${v}`)
		.join("\n");

	const raw = [
		`> ${requestLine}`,
		...requestHeaders.split("\n").map((h) => `> ${h}`),
		"",
		`< ${responseLine}`,
		...responseHeaders.split("\n").map((h) => `< ${h}`),
		"",
		debug.body,
	].join("\n");

	return (
		<div className="border border-muted rounded-md overflow-hidden">
			<button
				type="button"
				className="flex items-center gap-1 w-full px-2 py-1 text-[10px] font-medium text-muted-foreground hover:bg-muted/50 transition-colors"
				onClick={onToggle}
			>
				<ChevronRight
					className={`h-3 w-3 transition-transform ${open ? "rotate-90" : ""}`}
				/>
				{t("reader.rawHttp")}
			</button>
			{open && (
				<pre className="text-[10px] font-mono leading-relaxed bg-muted/30 p-2 overflow-x-auto max-h-64 overflow-y-auto whitespace-pre-wrap break-all border-t border-muted">
					{raw}
				</pre>
			)}
		</div>
	);
}
