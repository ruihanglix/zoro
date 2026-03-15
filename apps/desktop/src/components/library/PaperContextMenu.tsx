// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import {
	ContextMenu,
	ContextMenuContent,
	ContextMenuItem,
	ContextMenuSeparator,
	ContextMenuSub,
	ContextMenuSubContent,
	ContextMenuSubTrigger,
	ContextMenuTrigger,
} from "@/components/ui/context-menu";
import type { PaperResponse } from "@/lib/commands";
import * as commands from "@/lib/commands";
import { useLibraryStore } from "@/stores/libraryStore";
import { useTabStore } from "@/stores/tabStore";
import { useUiStore } from "@/stores/uiStore";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
	BookCheck,
	BookMarked,
	BookOpen,
	CloudDownload,
	Copy,
	Database,
	Download,
	ExternalLink,
	FileText,
	FolderOpen,
	FolderPlus,
	Globe,
	Languages,
	Loader2,
	RefreshCw,
	Search,
	StickyNote,
	Trash2,
} from "lucide-react";
import { useState } from "react";
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

interface PaperContextMenuProps {
	paper: PaperResponse;
	children: React.ReactNode;
}

export function PaperContextMenu({ paper, children }: PaperContextMenuProps) {
	const { t } = useTranslation();
	const deletePaper = useLibraryStore((s) => s.deletePaper);
	const updatePaperStatus = useLibraryStore((s) => s.updatePaperStatus);
	const collections = useLibraryStore((s) => s.collections);
	const fetchPapers = useLibraryStore((s) => s.fetchPapers);
	const fetchCollections = useLibraryStore((s) => s.fetchCollections);
	const openTab = useTabStore((s) => s.openTab);
	const openMetadataSearch = useUiStore((s) => s.openMetadataSearchDialog);
	const [copiedLabel, setCopiedLabel] = useState<string | null>(null);
	const [downloading, setDownloading] = useState<string | null>(null);
	const [enriching, setEnriching] = useState(false);

	const handleAutoEnrich = async () => {
		setEnriching(true);
		try {
			await commands.enrichPaperMetadata(paper.id);
			await fetchPapers();
		} catch (err) {
			console.error("Failed to enrich metadata:", err);
		}
		setEnriching(false);
	};

	const handleManualSearch = () => {
		openMetadataSearch(paper.id);
	};

	const handleOpenPdf = () => {
		openTab({
			type: "reader",
			paperId: paper.id,
			readerMode: "pdf",
			title: paper.title,
		});
	};

	const handleOpenHtml = () => {
		openTab({
			type: "reader",
			paperId: paper.id,
			readerMode: "html",
			title: paper.title,
		});
	};

	const handleCopyCitation = async (style: string) => {
		try {
			const result =
				style === "bibtex"
					? await commands.getPaperBibtex(paper.id)
					: await commands.getFormattedCitation(paper.id, style);
			await writeText(result.text);
			setCopiedLabel(style);
			setTimeout(() => setCopiedLabel(null), 2000);
		} catch (err) {
			console.error("Failed to copy citation:", err);
		}
	};

	const handleCopyTitle = async () => {
		try {
			await writeText(paper.title);
		} catch (err) {
			console.error("Failed to copy title:", err);
		}
	};

	const handleSetStatus = (status: string) => {
		updatePaperStatus(paper.id, status);
	};

	const handleAddToCollection = async (collectionId: string) => {
		try {
			await commands.addPaperToCollection(paper.id, collectionId);
			await fetchPapers();
			await fetchCollections();
		} catch (err) {
			console.error("Failed to add to collection:", err);
		}
	};

	const confirmBeforeDelete = useUiStore((s) => s.confirmBeforeDelete);

	const handleDelete = () => {
		setTimeout(async () => {
			if (
				confirmBeforeDelete &&
				!confirm(t("contextMenu.deletePaperConfirm"))
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

	const handleTranslatePdf = async () => {
		try {
			await commands.translatePdf(paper.id);
		} catch (err) {
			console.error("Failed to start PDF translation:", err);
		}
	};

	const handleExportAnnotatedPdf = async () => {
		try {
			await commands.exportAnnotatedPdf(paper.id);
		} catch (err) {
			console.error("Failed to export annotated PDF:", err);
		}
	};

	const handleExportAnnotatedHtml = async () => {
		try {
			await commands.exportAnnotatedHtml(paper.id);
		} catch (err) {
			console.error("Failed to export annotated HTML:", err);
		}
	};

	const handleDownloadFile = async (fileType: string) => {
		setDownloading(fileType);
		try {
			await commands.downloadPaperFile(paper.id, fileType);
			await fetchPapers();
		} catch (err) {
			console.error(`Failed to download ${fileType}:`, err);
		} finally {
			setDownloading(null);
		}
	};

	const nextStatus =
		paper.read_status === "unread"
			? "reading"
			: paper.read_status === "reading"
				? "read"
				: "unread";

	const statusIcon =
		nextStatus === "read" ? (
			<BookCheck className="mr-2 h-4 w-4" />
		) : nextStatus === "reading" ? (
			<BookMarked className="mr-2 h-4 w-4" />
		) : (
			<BookOpen className="mr-2 h-4 w-4" />
		);

	const isNote = paper.entry_type === "note";

	const handleOpenNote = () => {
		openTab({
			type: "note",
			paperId: paper.id,
			title: paper.title,
		});
	};

	return (
		<ContextMenu
			onOpenChange={(open) =>
				console.log("[PaperContextMenu] onOpenChange:", open, paper.title)
			}
		>
			<ContextMenuTrigger asChild>
				<div
					onContextMenu={() =>
						console.log("[PaperContextMenu] onContextMenu fired:", paper.title)
					}
				>
					{children}
				</div>
			</ContextMenuTrigger>
			<ContextMenuContent className="w-56">
				{/* Open actions */}
				{isNote && (
					<ContextMenuItem onSelect={handleOpenNote}>
						<StickyNote className="mr-2 h-4 w-4" />
						{t("contextMenu.openNote")}
					</ContextMenuItem>
				)}
				{!isNote && paper.has_pdf && (
					<ContextMenuItem onSelect={handleOpenPdf}>
						<FileText className="mr-2 h-4 w-4" />
						{t("contextMenu.openPdf")}
						{!paper.pdf_downloaded && (
							<span className="ml-auto text-[10px] text-blue-500">☁</span>
						)}
					</ContextMenuItem>
				)}
				{!isNote && paper.has_html && (
					<ContextMenuItem onSelect={handleOpenHtml}>
						<Globe className="mr-2 h-4 w-4" />
						{t("contextMenu.openHtml")}
						{!paper.html_downloaded && (
							<span className="ml-auto text-[10px] text-blue-500">☁</span>
						)}
					</ContextMenuItem>
				)}

				{/* Download actions for cloud-only files */}
				{paper.has_pdf && !paper.pdf_downloaded && (
					<ContextMenuItem onSelect={() => handleDownloadFile("pdf")}>
						{downloading === "pdf" ? (
							<Loader2 className="mr-2 h-4 w-4 animate-spin" />
						) : (
							<CloudDownload className="mr-2 h-4 w-4" />
						)}
						Download PDF
					</ContextMenuItem>
				)}
				{paper.has_html && !paper.html_downloaded && (
					<ContextMenuItem onSelect={() => handleDownloadFile("html")}>
						{downloading === "html" ? (
							<Loader2 className="mr-2 h-4 w-4 animate-spin" />
						) : (
							<CloudDownload className="mr-2 h-4 w-4" />
						)}
						Download HTML
					</ContextMenuItem>
				)}
				{paper.url && (
					<ContextMenuItem onSelect={() => window.open(paper.url!, "_blank")}>
						<ExternalLink className="mr-2 h-4 w-4" />
						{t("contextMenu.openUrl")}
					</ContextMenuItem>
				)}

				{paper.has_pdf && (
					<ContextMenuItem onSelect={handleTranslatePdf}>
						<Languages className="mr-2 h-4 w-4" />
						{t("paper.translate")} PDF
					</ContextMenuItem>
				)}

				{(paper.has_pdf || paper.has_html) && (
					<ContextMenuSub>
						<ContextMenuSubTrigger>
							<Download className="mr-2 h-4 w-4" />
							{t("contextMenu.export")}
						</ContextMenuSubTrigger>
						<ContextMenuSubContent className="w-52">
							{paper.has_pdf && (
								<ContextMenuItem
									onSelect={async () => {
										try {
											await commands.exportPdf(paper.id);
										} catch (err) {
											console.error("Failed to export PDF:", err);
										}
									}}
								>
									<FileText className="mr-2 h-4 w-4" />
									PDF
								</ContextMenuItem>
							)}
							{paper.has_html && (
								<ContextMenuItem
									onSelect={async () => {
										try {
											await commands.exportHtml(paper.id);
										} catch (err) {
											console.error("Failed to export HTML:", err);
										}
									}}
								>
									<Globe className="mr-2 h-4 w-4" />
									HTML
								</ContextMenuItem>
							)}
							<ContextMenuSeparator />
							{paper.has_pdf && (
								<ContextMenuItem onSelect={handleExportAnnotatedPdf}>
									<FileText className="mr-2 h-4 w-4" />
									{t("paper.pdfWithAnnotations")}
								</ContextMenuItem>
							)}
							{paper.has_html && (
								<ContextMenuItem onSelect={handleExportAnnotatedHtml}>
									<Globe className="mr-2 h-4 w-4" />
									{t("paper.htmlWithAnnotations")}
								</ContextMenuItem>
							)}
						</ContextMenuSubContent>
					</ContextMenuSub>
				)}

				{(paper.has_pdf || paper.has_html || paper.url) && (
					<ContextMenuSeparator />
				)}

				{/* Copy actions */}
				<ContextMenuItem onSelect={handleCopyTitle}>
					<Copy className="mr-2 h-4 w-4" />
					{t("common.copy")} {t("paper.title")}
				</ContextMenuItem>

				<ContextMenuSub>
					<ContextMenuSubTrigger>
						<Copy className="mr-2 h-4 w-4" />
						{t("contextMenu.copyCitation")}
					</ContextMenuSubTrigger>
					<ContextMenuSubContent className="w-40">
						{CITATION_STYLES.map((style) => (
							<ContextMenuItem
								key={style.id}
								onSelect={() => handleCopyCitation(style.id)}
							>
								{copiedLabel === style.id ? t("common.copied") : style.label}
							</ContextMenuItem>
						))}
					</ContextMenuSubContent>
				</ContextMenuSub>

				{/* Metadata submenu */}
				<ContextMenuSub>
					<ContextMenuSubTrigger>
						<Database className="mr-2 h-4 w-4" />
						{t("contextMenu.metadata")}
					</ContextMenuSubTrigger>
					<ContextMenuSubContent className="w-52">
						<ContextMenuItem onSelect={handleAutoEnrich} disabled={enriching}>
							{enriching ? (
								<Loader2 className="mr-2 h-4 w-4 animate-spin" />
							) : (
								<RefreshCw className="mr-2 h-4 w-4" />
							)}
							{t("contextMenu.autoFetchMetadata")}
						</ContextMenuItem>
						<ContextMenuItem onSelect={handleManualSearch}>
							<Search className="mr-2 h-4 w-4" />
							{t("contextMenu.manualSearchMetadata")}
						</ContextMenuItem>
					</ContextMenuSubContent>
				</ContextMenuSub>

				<ContextMenuSeparator />

				{/* Status */}
				<ContextMenuItem onSelect={() => handleSetStatus(nextStatus)}>
					{statusIcon}
					{nextStatus === "read"
						? t("contextMenu.markAsRead")
						: nextStatus === "reading"
							? t("contextMenu.markAsReading")
							: t("contextMenu.markAsUnread")}
				</ContextMenuItem>

				{/* Add to collection */}
				{collections.length > 0 && (
					<ContextMenuSub>
						<ContextMenuSubTrigger>
							<FolderPlus className="mr-2 h-4 w-4" />
							{t("contextMenu.addToCollection")}
						</ContextMenuSubTrigger>
						<ContextMenuSubContent className="w-48">
							{collections.map((col) => (
								<ContextMenuItem
									key={col.id}
									onSelect={() => handleAddToCollection(col.id)}
								>
									{col.name}
								</ContextMenuItem>
							))}
						</ContextMenuSubContent>
					</ContextMenuSub>
				)}

				{/* Open containing folder */}
				<ContextMenuItem
					onSelect={async () => {
						try {
							await commands.showPaperFolder(paper.id);
						} catch (err) {
							console.error("Failed to open paper folder:", err);
						}
					}}
				>
					<FolderOpen className="mr-2 h-4 w-4" />
					{t("contextMenu.openContainingFolder")}
				</ContextMenuItem>

				<ContextMenuSeparator />

				{/* Delete */}
				<ContextMenuItem
					onSelect={handleDelete}
					className="text-destructive focus:text-destructive"
				>
					<Trash2 className="mr-2 h-4 w-4" />
					{t("common.delete")}
				</ContextMenuItem>
			</ContextMenuContent>
		</ContextMenu>
	);
}
