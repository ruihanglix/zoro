// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { DisplayModeToggle } from "@/components/DisplayModeToggle";
import { Button } from "@/components/ui/button";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import {
	Tooltip,
	TooltipContent,
	TooltipProvider,
	TooltipTrigger,
} from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import { useLibraryStore } from "@/stores/libraryStore";
import { useTabStore } from "@/stores/tabStore";
import { useUiStore } from "@/stores/uiStore";
import {
	ArrowUpDown,
	CheckSquare,
	FileText,
	LayoutGrid,
	List,
	StickyNote,
	X,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { PaperCardGrid } from "./PaperCardGrid";
import { PaperTable } from "./PaperTable";

export function PaperList() {
	const { t } = useTranslation();
	const papers = useLibraryStore((s) => s.papers);
	const loading = useLibraryStore((s) => s.loading);
	const sortBy = useLibraryStore((s) => s.sortBy);
	const setSortBy = useLibraryStore((s) => s.setSortBy);
	const sortOrder = useLibraryStore((s) => s.sortOrder);
	const setSortOrder = useLibraryStore((s) => s.setSortOrder);
	const currentCollectionId = useLibraryStore((s) => s.currentCollectionId);
	const createStandaloneNote = useLibraryStore((s) => s.createStandaloneNote);
	const selectedPaperIds = useLibraryStore((s) => s.selectedPaperIds);
	const selectAllPapers = useLibraryStore((s) => s.selectAllPapers);
	const clearSelection = useLibraryStore((s) => s.clearSelection);
	const openTab = useTabStore((s) => s.openTab);
	const listMode = useUiStore((s) => s.listMode);
	const setListMode = useUiStore((s) => s.setListMode);

	const selectionCount = selectedPaperIds.size;

	const handleNewNote = async () => {
		const paper = await createStandaloneNote(currentCollectionId ?? undefined);
		if (paper) {
			openTab({
				type: "note",
				paperId: paper.id,
				title: paper.title,
			});
		}
	};

	const toggleSort = (field: string) => {
		if (sortBy === field) {
			setSortOrder(sortOrder === "asc" ? "desc" : "asc");
		} else {
			setSortBy(field);
			setSortOrder("desc");
		}
	};

	if (loading) {
		return (
			<div className="flex h-full items-center justify-center text-muted-foreground">
				{t("library.loadingPapers")}
			</div>
		);
	}

	if (papers.length === 0) {
		return (
			<div className="flex h-full flex-col items-center justify-center gap-2 text-muted-foreground">
				<FileText className="h-12 w-12" />
				<p>{t("library.noPapersYet")}</p>
				<p className="text-sm">{t("library.noPapersDescription")}</p>
			</div>
		);
	}

	return (
		<div className="flex h-full flex-col">
			{/* Toolbar */}
			<div className="flex items-center gap-2 border-b px-3 py-1.5 select-none">
				<span className="text-xs text-muted-foreground tabular-nums">
					{papers.length} {t("library.papers")}
				</span>

				{/* Selection info */}
				{selectionCount > 0 && (
					<span className="flex items-center gap-1 text-xs text-primary font-medium">
						<CheckSquare className="h-3 w-3" />
						{selectionCount} {t("common.selected")}
						<Button
							variant="ghost"
							size="sm"
							className="h-5 w-5 p-0 ml-0.5"
							onClick={clearSelection}
							title={t("topBar.clearSearch")}
						>
							<X className="h-3 w-3" />
						</Button>
						{selectionCount < papers.length && (
							<Button
								variant="ghost"
								size="sm"
								className="h-5 px-1.5 text-[10px]"
								onClick={selectAllPapers}
							>
								{t("common.selectAll")}
							</Button>
						)}
					</span>
				)}

				<Button
					variant="ghost"
					size="sm"
					className="h-7 px-2 text-xs gap-1"
					onClick={handleNewNote}
					title={t("library.newNote")}
				>
					<StickyNote className="h-3.5 w-3.5" />
					{t("library.note")}
				</Button>

				<div className="ml-auto flex items-center gap-1">
					{/* Display mode toggle (original / bilingual / translated) */}
					<DisplayModeToggle />

					{/* Sort controls — only shown in card view since table has its own column headers */}
					{listMode === "card" && (
						<>
							<Button
								variant="ghost"
								size="sm"
								className={cn(
									"h-7 px-2 text-xs",
									sortBy === "added_date" && "text-primary",
								)}
								onClick={() => toggleSort("added_date")}
							>
								<ArrowUpDown className="mr-1 h-3 w-3" />
								{t("library.date")}
							</Button>
							<Button
								variant="ghost"
								size="sm"
								className={cn(
									"h-7 px-2 text-xs",
									sortBy === "title" && "text-primary",
								)}
								onClick={() => toggleSort("title")}
							>
								<ArrowUpDown className="mr-1 h-3 w-3" />
								{t("library.title")}
							</Button>
						</>
					)}

					{/* View toggle */}
					<TooltipProvider delayDuration={300}>
						<ToggleGroup
							type="single"
							size="sm"
							value={listMode}
							onValueChange={(value) => {
								if (value) setListMode(value as "list" | "card");
							}}
							className="ml-1"
						>
							<Tooltip>
								<TooltipTrigger asChild>
									<ToggleGroupItem
										value="list"
										aria-label={t("library.tableView")}
										className="h-7 w-7 p-0"
									>
										<List className="h-4 w-4" />
									</ToggleGroupItem>
								</TooltipTrigger>
								<TooltipContent>{t("library.tableView")}</TooltipContent>
							</Tooltip>
							<Tooltip>
								<TooltipTrigger asChild>
									<ToggleGroupItem
										value="card"
										aria-label={t("library.cardView")}
										className="h-7 w-7 p-0"
									>
										<LayoutGrid className="h-4 w-4" />
									</ToggleGroupItem>
								</TooltipTrigger>
								<TooltipContent>{t("library.cardView")}</TooltipContent>
							</Tooltip>
						</ToggleGroup>
					</TooltipProvider>
				</div>
			</div>

			{/* Content */}
			<div className="flex-1 overflow-hidden">
				{listMode === "list" ? <PaperTable /> : <PaperCardGrid />}
			</div>
		</div>
	);
}
