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
import * as commands from "@/lib/commands";
import { useLibraryStore } from "@/stores/libraryStore";
import { useUiStore } from "@/stores/uiStore";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
	BookCheck,
	BookMarked,
	BookOpen,
	Copy,
	Download,
	FileText,
	FolderPlus,
	Trash2,
	X,
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

interface MultiSelectContextMenuProps {
	paperIds: string[];
	children: React.ReactNode;
}

export function MultiSelectContextMenu({
	paperIds,
	children,
}: MultiSelectContextMenuProps) {
	const { t } = useTranslation();
	const deletePaper = useLibraryStore((s) => s.deletePaper);
	const updatePaperStatus = useLibraryStore((s) => s.updatePaperStatus);
	const collections = useLibraryStore((s) => s.collections);
	const fetchPapers = useLibraryStore((s) => s.fetchPapers);
	const fetchCollections = useLibraryStore((s) => s.fetchCollections);
	const clearSelection = useLibraryStore((s) => s.clearSelection);
	const confirmBeforeDelete = useUiStore((s) => s.confirmBeforeDelete);
	const [copiedLabel, setCopiedLabel] = useState<string | null>(null);

	const count = paperIds.length;

	// Batch copy citation for all selected papers
	const handleCopyCitation = async (style: string) => {
		console.log(
			"[MultiSelectContextMenu] handleCopyCitation called, style:",
			style,
			"paperIds:",
			paperIds,
		);
		try {
			const promises = paperIds.map((id) =>
				style === "bibtex"
					? commands.getPaperBibtex(id)
					: commands.getFormattedCitation(id, style),
			);
			const settled = await Promise.allSettled(promises);
			const results: string[] = [];
			for (const r of settled) {
				if (r.status === "fulfilled" && r.value.text) {
					results.push(r.value.text);
				}
			}
			console.log(
				"[MultiSelectContextMenu] citation results count:",
				results.length,
			);
			await writeText(results.join("\n\n"));
			setCopiedLabel(style);
			setTimeout(() => setCopiedLabel(null), 2000);
		} catch (err) {
			console.error("Failed to copy citations:", err);
		}
	};

	// Batch export BibTeX
	const handleExportBibtex = async () => {
		try {
			const content = await commands.exportBibtex(paperIds);
			await writeText(content);
		} catch (err) {
			console.error("Failed to export BibTeX:", err);
		}
	};

	// Batch export RIS
	const handleExportRis = async () => {
		try {
			const content = await commands.exportRis(paperIds);
			await writeText(content);
		} catch (err) {
			console.error("Failed to export RIS:", err);
		}
	};

	// Batch set status
	const handleSetStatus = async (status: string) => {
		try {
			for (const id of paperIds) {
				await updatePaperStatus(id, status);
			}
		} catch (err) {
			console.error("Failed to update status:", err);
		}
	};

	// Batch add to collection
	const handleAddToCollection = async (collectionId: string) => {
		try {
			for (const id of paperIds) {
				await commands.addPaperToCollection(id, collectionId);
			}
			await fetchPapers();
			await fetchCollections();
		} catch (err) {
			console.error("Failed to add to collection:", err);
		}
	};

	// Batch delete
	const handleDelete = () => {
		setTimeout(async () => {
			if (
				confirmBeforeDelete &&
				!confirm(t("contextMenu.deleteMultipleConfirm", { count }))
			) {
				return;
			}
			try {
				for (const id of paperIds) {
					await deletePaper(id);
				}
				clearSelection();
			} catch (err) {
				console.error("Failed to delete papers:", err);
			}
		}, 0);
	};

	return (
		<ContextMenu>
			<ContextMenuTrigger asChild>
				<div onContextMenu={(e) => e.stopPropagation()}>{children}</div>
			</ContextMenuTrigger>
			<ContextMenuContent className="w-56">
				{/* Header showing count */}
				<div className="px-2 py-1.5 text-xs text-muted-foreground font-medium">
					{count} {t("common.selected")}
				</div>
				<ContextMenuSeparator />

				{/* Copy Citations */}
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

				{/* Export */}
				<ContextMenuSub>
					<ContextMenuSubTrigger>
						<Download className="mr-2 h-4 w-4" />
						{t("contextMenu.export")}
					</ContextMenuSubTrigger>
					<ContextMenuSubContent className="w-48">
						<ContextMenuItem onSelect={handleExportBibtex}>
							<FileText className="mr-2 h-4 w-4" />
							BibTeX (copy to clipboard)
						</ContextMenuItem>
						<ContextMenuItem onSelect={handleExportRis}>
							<FileText className="mr-2 h-4 w-4" />
							RIS (copy to clipboard)
						</ContextMenuItem>
					</ContextMenuSubContent>
				</ContextMenuSub>

				<ContextMenuSeparator />

				{/* Set Status */}
				<ContextMenuSub>
					<ContextMenuSubTrigger>
						<BookOpen className="mr-2 h-4 w-4" />
						{t("common.status")}
					</ContextMenuSubTrigger>
					<ContextMenuSubContent className="w-40">
						<ContextMenuItem onSelect={() => handleSetStatus("unread")}>
							<BookOpen className="mr-2 h-4 w-4" />
							{t("paper.unread")}
						</ContextMenuItem>
						<ContextMenuItem onSelect={() => handleSetStatus("reading")}>
							<BookMarked className="mr-2 h-4 w-4" />
							{t("paper.reading")}
						</ContextMenuItem>
						<ContextMenuItem onSelect={() => handleSetStatus("read")}>
							<BookCheck className="mr-2 h-4 w-4" />
							{t("paper.read")}
						</ContextMenuItem>
					</ContextMenuSubContent>
				</ContextMenuSub>

				{/* Add to Collection */}
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

				<ContextMenuSeparator />

				{/* Clear Selection */}
				<ContextMenuItem onSelect={() => clearSelection()}>
					<X className="mr-2 h-4 w-4" />
					{t("common.clear")}
				</ContextMenuItem>

				{/* Delete */}
				<ContextMenuItem
					onSelect={handleDelete}
					className="text-destructive focus:text-destructive"
				>
					<Trash2 className="mr-2 h-4 w-4" />
					{t("common.delete")} {count}
				</ContextMenuItem>
			</ContextMenuContent>
		</ContextMenu>
	);
}
