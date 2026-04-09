// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import type {
	AuthorSearchResultResponse,
	WatchListResultResponse,
} from "@/lib/commands";
import { cn } from "@/lib/utils";
import { useUiStore } from "@/stores/uiStore";
import { useWatchListStore } from "@/stores/watchListStore";
import {
	BookOpen,
	Check,
	ExternalLink,
	Eye,
	Loader2,
	Plus,
	RefreshCw,
	Search,
	Settings2,
	Trash2,
	User,
	X,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

export function WatchList() {
	const { t } = useTranslation();
	const activeWatchListId = useUiStore((s) => s.activeWatchListId);
	const setActiveWatchListId = useUiStore((s) => s.setActiveWatchListId);

	const watchLists = useWatchListStore((s) => s.watchLists);
	const items = useWatchListStore((s) => s.items);
	const results = useWatchListStore((s) => s.results);
	const resultsLoading = useWatchListStore((s) => s.resultsLoading);
	const refreshing = useWatchListStore((s) => s.refreshing);
	const authorSearchResults = useWatchListStore((s) => s.authorSearchResults);
	const searchingAuthors = useWatchListStore((s) => s.searchingAuthors);

	const createWatchList = useWatchListStore((s) => s.createWatchList);
	const deleteWatchList = useWatchListStore((s) => s.deleteWatchList);
	const fetchItems = useWatchListStore((s) => s.fetchItems);
	const addItem = useWatchListStore((s) => s.addItem);
	const deleteItem = useWatchListStore((s) => s.deleteItem);
	const fetchResults = useWatchListStore((s) => s.fetchResults);
	const addResultToLibrary = useWatchListStore((s) => s.addResultToLibrary);
	const refreshWatchList = useWatchListStore((s) => s.refreshWatchList);
	const searchAuthors = useWatchListStore((s) => s.searchAuthors);
	const clearAuthorSearch = useWatchListStore((s) => s.clearAuthorSearch);

	const [showCreateDialog, setShowCreateDialog] = useState(false);
	const [newListName, setNewListName] = useState("");
	const [showManagePanel, setShowManagePanel] = useState(false);
	const [addItemType, setAddItemType] = useState<"author" | "seed-paper">(
		"author",
	);
	const [authorQuery, setAuthorQuery] = useState("");
	const [seedPaperDoi, setSeedPaperDoi] = useState("");
	const [seedPaperTitle, setSeedPaperTitle] = useState("");
	const [addingResult, setAddingResult] = useState<string | null>(null);
	const searchTimeoutRef = useRef<ReturnType<typeof setTimeout>>(undefined);

	const activeList = watchLists.find((wl) => wl.id === activeWatchListId);

	// Fetch results when active list changes
	useEffect(() => {
		fetchResults(activeWatchListId);
		if (activeWatchListId) {
			fetchItems(activeWatchListId);
		}
	}, [activeWatchListId, fetchResults, fetchItems]);

	// Debounced author search
	const handleAuthorSearch = useCallback(
		(query: string) => {
			setAuthorQuery(query);
			if (searchTimeoutRef.current) clearTimeout(searchTimeoutRef.current);
			searchTimeoutRef.current = setTimeout(() => {
				searchAuthors(query);
			}, 300);
		},
		[searchAuthors],
	);

	const handleCreateList = async () => {
		if (!newListName.trim()) return;
		const created = await createWatchList(newListName.trim());
		setNewListName("");
		setShowCreateDialog(false);
		setActiveWatchListId(created.id);
	};

	const handleDeleteList = async (id: string) => {
		if (!confirm(t("watchList.confirmDeleteList"))) return;
		await deleteWatchList(id);
		if (activeWatchListId === id) {
			setActiveWatchListId(null);
		}
	};

	const handleAddAuthor = async (author: AuthorSearchResultResponse) => {
		if (!activeWatchListId) return;
		await addItem({
			listId: activeWatchListId,
			itemType: "author",
			externalId: author.external_id,
			source: author.source,
			displayName: author.name,
		});
		clearAuthorSearch();
		setAuthorQuery("");
	};

	const handleAddSeedPaper = async () => {
		if (!activeWatchListId || !seedPaperDoi.trim()) return;
		await addItem({
			listId: activeWatchListId,
			itemType: "seed-paper",
			externalId: seedPaperDoi.trim(),
			source: "doi",
			displayName: seedPaperTitle.trim() || seedPaperDoi.trim(),
		});
		setSeedPaperDoi("");
		setSeedPaperTitle("");
	};

	const handleAddToLibrary = async (resultId: string) => {
		setAddingResult(resultId);
		try {
			await addResultToLibrary(resultId);
		} catch (e) {
			console.error("Failed to add to library:", e);
		}
		setAddingResult(null);
	};

	const handleRefresh = async () => {
		if (!activeWatchListId) return;
		await refreshWatchList(activeWatchListId);
	};

	return (
		<div className="flex h-full flex-col">
			{/* Header */}
			<div className="flex items-center justify-between border-b px-4 py-3">
				<div className="flex items-center gap-2">
					<Eye className="h-5 w-5 text-primary" />
					<h1 className="text-lg font-semibold">
						{activeList ? activeList.name : t("watchList.title")}
					</h1>
					{activeList && activeList.new_result_count > 0 && (
						<span className="rounded-full bg-primary/10 px-2 py-0.5 text-xs font-medium text-primary">
							{activeList.new_result_count} {t("watchList.newResults")}
						</span>
					)}
				</div>
				<div className="flex items-center gap-2">
					{activeWatchListId && (
						<>
							<Button
								variant="outline"
								size="sm"
								onClick={() => setShowManagePanel(!showManagePanel)}
							>
								<Settings2 className="mr-1 h-3.5 w-3.5" />
								{t("watchList.manage")}
							</Button>
							<Button
								variant="outline"
								size="sm"
								onClick={handleRefresh}
								disabled={refreshing}
							>
								<RefreshCw
									className={cn(
										"mr-1 h-3.5 w-3.5",
										refreshing && "animate-spin",
									)}
								/>
								{t("watchList.refresh")}
							</Button>
						</>
					)}
					<Button
						variant="outline"
						size="sm"
						onClick={() => setShowCreateDialog(true)}
					>
						<Plus className="mr-1 h-3.5 w-3.5" />
						{t("watchList.newList")}
					</Button>
				</div>
			</div>

			{/* Create dialog */}
			{showCreateDialog && (
				<div className="border-b px-4 py-3 bg-muted/30">
					<div className="flex items-center gap-2">
						<Input
							placeholder={t("watchList.listNamePlaceholder")}
							value={newListName}
							onChange={(e) => setNewListName(e.target.value)}
							onKeyDown={(e) => e.key === "Enter" && handleCreateList()}
							autoFocus
							className="max-w-xs"
						/>
						<Button size="sm" onClick={handleCreateList}>
							{t("common.create")}
						</Button>
						<Button
							size="sm"
							variant="ghost"
							onClick={() => setShowCreateDialog(false)}
						>
							<X className="h-4 w-4" />
						</Button>
					</div>
				</div>
			)}

			<div className="flex flex-1 overflow-hidden">
				{/* Main content */}
				<ScrollArea className="flex-1">
					<div className="p-4">
						{/* No active list — show overview */}
						{!activeWatchListId && (
							<div>
								{watchLists.length === 0 ? (
									<div className="flex flex-col items-center justify-center py-20 text-muted-foreground">
										<Eye className="h-12 w-12 mb-4 opacity-30" />
										<p className="text-lg font-medium mb-2">
											{t("watchList.emptyTitle")}
										</p>
										<p className="text-sm mb-4">
											{t("watchList.emptyDescription")}
										</p>
										<Button onClick={() => setShowCreateDialog(true)}>
											<Plus className="mr-1 h-4 w-4" />
											{t("watchList.createFirst")}
										</Button>
									</div>
								) : (
									<div>
										<p className="text-sm text-muted-foreground mb-4">
											{t("watchList.allResultsDescription")}
										</p>
										<ResultsList
											results={results}
											loading={resultsLoading}
											addingResult={addingResult}
											onAddToLibrary={handleAddToLibrary}
											t={t}
										/>
									</div>
								)}
							</div>
						)}

						{/* Active list — show results */}
						{activeWatchListId && (
							<ResultsList
								results={results}
								loading={resultsLoading}
								addingResult={addingResult}
								onAddToLibrary={handleAddToLibrary}
								t={t}
							/>
						)}
					</div>
				</ScrollArea>

				{/* Manage panel (right side) */}
				{showManagePanel && activeWatchListId && (
					<div className="w-80 border-l bg-muted/20 flex flex-col">
						<div className="flex items-center justify-between border-b px-3 py-2">
							<span className="text-sm font-medium">
								{t("watchList.manageItems")}
							</span>
							<Button
								variant="ghost"
								size="sm"
								onClick={() => setShowManagePanel(false)}
							>
								<X className="h-4 w-4" />
							</Button>
						</div>

						<ScrollArea className="flex-1">
							<div className="p-3 space-y-4">
								{/* Current items */}
								<div>
									<p className="text-xs font-medium text-muted-foreground mb-2 uppercase">
										{t("watchList.currentItems")}
									</p>
									{items.length === 0 && (
										<p className="text-xs text-muted-foreground">
											{t("watchList.noItems")}
										</p>
									)}
									{items.map((item) => (
										<div
											key={item.id}
											className="flex items-center gap-2 py-1.5 group"
										>
											{item.item_type === "author" ? (
												<User className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
											) : (
												<BookOpen className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
											)}
											<span className="text-sm truncate flex-1">
												{item.display_name}
											</span>
											<span className="text-[10px] text-muted-foreground">
												{item.source}
											</span>
											<button
												type="button"
												className="opacity-0 group-hover:opacity-100 text-muted-foreground hover:text-destructive transition-opacity"
												onClick={() => deleteItem(item.id, activeWatchListId)}
											>
												<Trash2 className="h-3.5 w-3.5" />
											</button>
										</div>
									))}
								</div>

								{/* Add item */}
								<div>
									<p className="text-xs font-medium text-muted-foreground mb-2 uppercase">
										{t("watchList.addItem")}
									</p>

									{/* Type toggle */}
									<div className="flex gap-1 mb-2">
										<Button
											variant={addItemType === "author" ? "default" : "outline"}
											size="sm"
											className="flex-1 text-xs"
											onClick={() => setAddItemType("author")}
										>
											<User className="mr-1 h-3 w-3" />
											{t("watchList.author")}
										</Button>
										<Button
											variant={
												addItemType === "seed-paper" ? "default" : "outline"
											}
											size="sm"
											className="flex-1 text-xs"
											onClick={() => setAddItemType("seed-paper")}
										>
											<BookOpen className="mr-1 h-3 w-3" />
											{t("watchList.seedPaper")}
										</Button>
									</div>

									{addItemType === "author" && (
										<div>
											<div className="relative">
												<Search className="absolute left-2 top-2 h-3.5 w-3.5 text-muted-foreground" />
												<Input
													placeholder={t("watchList.searchAuthorPlaceholder")}
													value={authorQuery}
													onChange={(e) => handleAuthorSearch(e.target.value)}
													className="pl-7 text-sm h-8"
												/>
											</div>
											{searchingAuthors && (
												<div className="flex items-center gap-1 py-2 text-xs text-muted-foreground">
													<Loader2 className="h-3 w-3 animate-spin" />
													{t("common.loading")}
												</div>
											)}
											{authorSearchResults.length > 0 && (
												<div className="mt-1 max-h-48 overflow-y-auto border rounded-md">
													{authorSearchResults.map((author, i) => (
														<button
															key={`${author.source}-${author.external_id}-${i}`}
															type="button"
															className="flex w-full items-start gap-2 px-2 py-1.5 text-left hover:bg-accent/50 text-sm border-b last:border-b-0"
															onClick={() => handleAddAuthor(author)}
														>
															<User className="h-3.5 w-3.5 mt-0.5 shrink-0 text-muted-foreground" />
															<div className="min-w-0 flex-1">
																<p className="truncate font-medium text-xs">
																	{author.name}
																</p>
																{author.notes && (
																	<p className="truncate text-[10px] text-muted-foreground">
																		{author.notes}
																	</p>
																)}
																<p className="text-[10px] text-muted-foreground">
																	{author.source}
																	{author.paper_count != null &&
																		` · ${author.paper_count} papers`}
																</p>
															</div>
														</button>
													))}
												</div>
											)}
										</div>
									)}

									{addItemType === "seed-paper" && (
										<div className="space-y-2">
											<Input
												placeholder={t("watchList.doiPlaceholder")}
												value={seedPaperDoi}
												onChange={(e) => setSeedPaperDoi(e.target.value)}
												className="text-sm h-8"
											/>
											<Input
												placeholder={t("watchList.titlePlaceholder")}
												value={seedPaperTitle}
												onChange={(e) => setSeedPaperTitle(e.target.value)}
												className="text-sm h-8"
											/>
											<Button
												size="sm"
												className="w-full"
												onClick={handleAddSeedPaper}
												disabled={!seedPaperDoi.trim()}
											>
												<Plus className="mr-1 h-3 w-3" />
												{t("watchList.addSeedPaper")}
											</Button>
										</div>
									)}
								</div>

								{/* Danger zone */}
								<div className="pt-2 border-t">
									<Button
										variant="destructive"
										size="sm"
										className="w-full"
										onClick={() => handleDeleteList(activeWatchListId)}
									>
										<Trash2 className="mr-1 h-3.5 w-3.5" />
										{t("watchList.deleteList")}
									</Button>
								</div>
							</div>
						</ScrollArea>
					</div>
				)}
			</div>
		</div>
	);
}

// ── Results list component ──────────────────────────────────────────────────

function ResultsList({
	results,
	loading,
	addingResult,
	onAddToLibrary,
	t,
}: {
	results: WatchListResultResponse[];
	loading: boolean;
	addingResult: string | null;
	onAddToLibrary: (id: string) => void;
	t: (key: string) => string;
}) {
	if (loading) {
		return (
			<div className="flex items-center justify-center py-20">
				<Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
			</div>
		);
	}

	if (results.length === 0) {
		return (
			<div className="flex flex-col items-center justify-center py-20 text-muted-foreground">
				<Eye className="h-10 w-10 mb-3 opacity-30" />
				<p className="text-sm">{t("watchList.noResults")}</p>
				<p className="text-xs mt-1">{t("watchList.noResultsHint")}</p>
			</div>
		);
	}

	return (
		<div className="space-y-2">
			{results.map((result) => (
				<div
					key={result.id}
					className={cn(
						"rounded-lg border p-3 transition-colors",
						result.added_to_library && "opacity-60",
					)}
				>
					<div className="flex items-start gap-3">
						<div className="flex-1 min-w-0">
							<h3 className="text-sm font-medium leading-snug">
								{result.title}
							</h3>
							{result.authors.length > 0 && (
								<p className="text-xs text-muted-foreground mt-0.5 truncate">
									{result.authors.map((a) => a.name).join(", ")}
								</p>
							)}
							{result.abstract_text && (
								<p className="text-xs text-muted-foreground mt-1 line-clamp-2">
									{result.abstract_text}
								</p>
							)}
							<div className="flex items-center gap-2 mt-1.5 text-[10px] text-muted-foreground">
								{result.published_date && (
									<span>{result.published_date.slice(0, 10)}</span>
								)}
								{result.source_display_name && (
									<>
										<span>·</span>
										<span>{result.source_display_name}</span>
									</>
								)}
								{result.item_type && (
									<>
										<span>·</span>
										<span className="capitalize">{result.item_type}</span>
									</>
								)}
							</div>
						</div>
						<div className="flex items-center gap-1 shrink-0">
							{result.url && (
								<a
									href={result.url}
									target="_blank"
									rel="noopener noreferrer"
									className="p-1 rounded hover:bg-accent/50 text-muted-foreground"
								>
									<ExternalLink className="h-3.5 w-3.5" />
								</a>
							)}
							{result.added_to_library ? (
								<span className="flex items-center gap-0.5 text-xs text-green-600">
									<Check className="h-3.5 w-3.5" />
								</span>
							) : (
								<Button
									variant="outline"
									size="sm"
									className="h-7 text-xs"
									onClick={() => onAddToLibrary(result.id)}
									disabled={addingResult === result.id}
								>
									{addingResult === result.id ? (
										<Loader2 className="h-3 w-3 animate-spin" />
									) : (
										<>
											<Plus className="mr-0.5 h-3 w-3" />
											{t("watchList.addToLibrary")}
										</>
									)}
								</Button>
							)}
						</div>
					</div>
				</div>
			))}
		</div>
	);
}
