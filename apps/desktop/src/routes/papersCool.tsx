// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import {
	BilingualCardAbstract,
	ParagraphedCardText,
} from "@/components/BilingualText";
import { CollapsibleAuthors } from "@/components/CollapsibleAuthors";
import { DisplayModeToggle } from "@/components/DisplayModeToggle";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { PapersCoolPaperResponse } from "@/lib/commands";
import * as commands from "@/lib/commands";
import { cn } from "@/lib/utils";
import { usePapersCoolStore } from "@/stores/papersCoolStore";
import { useTabStore } from "@/stores/tabStore";
import {
	useTranslatedText,
	useTranslationStore,
} from "@/stores/translationStore";
import {
	BookOpen,
	Calendar,
	ChevronDown,
	ChevronLeft,
	ChevronRight,
	ExternalLink,
	Eye,
	FileText,
	Loader2,
	Plus,
	RefreshCw,
	Search,
	Star,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

// ── Helpers ─────────────────────────────────────────────────────────────────

function todayStr(): string {
	const d = new Date();
	const y = d.getFullYear();
	const m = String(d.getMonth() + 1).padStart(2, "0");
	const day = String(d.getDate()).padStart(2, "0");
	return `${y}-${m}-${day}`;
}

function shiftDate(dateStr: string, delta: number): string {
	const d = new Date(`${dateStr}T00:00:00`);
	d.setDate(d.getDate() + delta);
	const y = d.getFullYear();
	const m = String(d.getMonth() + 1).padStart(2, "0");
	const day = String(d.getDate()).padStart(2, "0");
	return `${y}-${m}-${day}`;
}

function formatDateLabel(dateStr: string): string {
	const d = new Date(`${dateStr}T00:00:00`);
	return d.toLocaleDateString(undefined, {
		weekday: "short",
		year: "numeric",
		month: "short",
		day: "numeric",
	});
}

// ── Main Component ──────────────────────────────────────────────────────────

export function PapersCool() {
	const { t } = useTranslation();
	const index = usePapersCoolStore((s) => s.index);
	const indexLoading = usePapersCoolStore((s) => s.indexLoading);
	const fetchIndex = usePapersCoolStore((s) => s.fetchIndex);
	const mode = usePapersCoolStore((s) => s.mode);
	const page = usePapersCoolStore((s) => s.page);
	const loading = usePapersCoolStore((s) => s.loading);
	const error = usePapersCoolStore((s) => s.error);
	const selectedCategory = usePapersCoolStore((s) => s.selectedCategory);
	const selectedVenue = usePapersCoolStore((s) => s.selectedVenue);
	const selectedGroup = usePapersCoolStore((s) => s.selectedGroup);
	const currentDate = usePapersCoolStore((s) => s.currentDate);
	const browseArxiv = usePapersCoolStore((s) => s.browseArxiv);
	const browseVenue = usePapersCoolStore((s) => s.browseVenue);
	const searchAction = usePapersCoolStore((s) => s.search);
	const setDate = usePapersCoolStore((s) => s.setDate);
	const bookmarks = usePapersCoolStore((s) => s.bookmarks);
	const addBookmark = usePapersCoolStore((s) => s.addBookmark);
	const removeBookmark = usePapersCoolStore((s) => s.removeBookmark);

	const [searchInput, setSearchInput] = useState("");
	const ensureTranslatedBatch = useTranslationStore(
		(s) => s.ensureTranslatedBatch,
	);
	const displayMode = useTranslationStore((s) => s.displayMode);

	useEffect(() => {
		if (page && page.papers.length > 0) {
			ensureTranslatedBatch(
				"papers_cool_paper",
				page.papers.map((p) => p.external_id),
				["title", "abstract_text"],
			);
		}
	}, [page, ensureTranslatedBatch, displayMode]);

	const sortedArxivGroups = useMemo(() => {
		if (!index) return [];
		const groups = [...index.arxiv_groups];
		groups.sort((a, b) => {
			if (a.name === "Computer Science") return -1;
			if (b.name === "Computer Science") return 1;
			return 0;
		});
		return groups;
	}, [index]);

	useEffect(() => {
		if (!index && !indexLoading) {
			fetchIndex();
		}
	}, [index, indexLoading, fetchIndex]);

	const date = currentDate ?? todayStr();
	const isToday = date === todayStr();

	const handleSearch = () => {
		if (searchInput.trim()) {
			searchAction(searchInput.trim());
		}
	};

	const handleRefresh = () => {
		if (mode === "arxiv" && selectedCategory) {
			browseArxiv(selectedCategory, date, true);
		} else if (mode === "venue" && selectedVenue) {
			browseVenue(selectedVenue, selectedGroup ?? undefined, true);
		} else if (mode === "search") {
			const q = usePapersCoolStore.getState().searchQuery;
			if (q) searchAction(q, true);
		}
	};

	const headerTitle = page?.title ?? "Papers.cool";
	const headerSubtitle =
		mode === "arxiv"
			? formatDateLabel(date)
			: mode === "venue" && selectedGroup
				? selectedGroup
				: null;

	return (
		<div className="flex h-full w-full">
			{/* Inner sidebar */}
			<nav className="w-56 shrink-0 border-r bg-muted/30 flex flex-col select-none">
				<div className="p-3 border-b">
					<div className="relative">
						<Search className="absolute left-2 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground" />
						<input
							type="text"
							className="w-full rounded-md border bg-background px-8 py-1.5 text-xs placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
							placeholder={t("papersCool.searchPapers")}
							value={searchInput}
							onChange={(e) => setSearchInput(e.target.value)}
							onKeyDown={(e) => {
								if (e.key === "Enter") handleSearch();
							}}
						/>
					</div>
				</div>

				<ScrollArea className="flex-1">
					<div className="p-2 space-y-0.5">
						{indexLoading && !index && (
							<div className="flex items-center gap-2 px-2 py-4 text-xs text-muted-foreground">
								<Loader2 className="h-3.5 w-3.5 animate-spin" />
								{t("common.loading")}
							</div>
						)}

						{/* Bookmarks */}
						{bookmarks.length > 0 && (
							<>
								<div className="px-2 pt-1 pb-0.5 text-[11px] font-semibold text-muted-foreground/70 uppercase tracking-wider flex items-center gap-1">
									<Star className="h-3 w-3 fill-yellow-400 text-yellow-400" />
									{t("papersCool.bookmarks")}
								</div>
								{bookmarks.map((bm) => {
									const isActive =
										(bm.type === "arxiv" &&
											mode === "arxiv" &&
											selectedCategory === bm.key) ||
										(bm.type === "venue" &&
											mode === "venue" &&
											selectedVenue === bm.key &&
											(selectedGroup ?? "") === (bm.venueGroup ?? ""));
									return (
										<div
											key={`bm-${bm.type}-${bm.key}-${bm.venueGroup ?? ""}`}
											className="group flex items-center"
										>
											<button
												type="button"
												className={cn(
													"flex flex-1 items-center gap-1.5 rounded px-2 py-1 text-xs hover:bg-accent/50 transition-colors text-left min-w-0",
													isActive &&
														"bg-accent text-accent-foreground font-medium",
												)}
												onClick={() => {
													if (bm.type === "arxiv") browseArxiv(bm.key);
													else browseVenue(bm.key, bm.venueGroup);
												}}
											>
												{bm.type === "arxiv" ? (
													<FileText className="h-3 w-3 shrink-0 text-muted-foreground" />
												) : (
													<BookOpen className="h-3 w-3 shrink-0 text-muted-foreground" />
												)}
												<span className="truncate">{bm.label}</span>
											</button>
											<button
												type="button"
												className="p-0.5 opacity-0 group-hover:opacity-100 transition-opacity shrink-0 mr-1"
												onClick={() =>
													removeBookmark(bm.type, bm.key, bm.venueGroup)
												}
												title={t("papersCool.removeBookmark")}
											>
												<Star className="h-3 w-3 fill-yellow-400 text-yellow-400" />
											</button>
										</div>
									);
								})}
								<div className="my-2 border-t" />
							</>
						)}

						{index && (
							<>
								{/* arXiv section heading */}
								<div className="px-2 pt-1 pb-0.5 text-[11px] font-semibold text-muted-foreground/70 uppercase tracking-wider">
									arXiv
								</div>

								{sortedArxivGroups.map((group) => (
									<CollapsibleGroup
										key={group.name}
										title={group.name}
										icon={<FileText className="h-3 w-3" />}
										defaultOpen={false}
									>
										{group.categories.map((cat) => (
											<SidebarItem
												key={cat.code}
												active={
													mode === "arxiv" && selectedCategory === cat.code
												}
												onClick={() => browseArxiv(cat.code)}
												title={cat.name}
												bookmarked={usePapersCoolStore
													.getState()
													.isBookmarked("arxiv", cat.code)}
												onToggleBookmark={() => {
													if (
														usePapersCoolStore
															.getState()
															.isBookmarked("arxiv", cat.code)
													) {
														removeBookmark("arxiv", cat.code);
													} else {
														addBookmark({
															type: "arxiv",
															key: cat.code,
															label: `${cat.code} ${cat.name}`,
														});
													}
												}}
											>
												<span className="text-muted-foreground font-mono text-[10px] shrink-0 w-14">
													{cat.code}
												</span>
												<span className="truncate">{cat.name}</span>
											</SidebarItem>
										))}
									</CollapsibleGroup>
								))}

								{/* Separator */}
								<div className="my-2 border-t" />

								{/* Venues section heading */}
								<div className="px-2 pt-1 pb-0.5 text-[11px] font-semibold text-muted-foreground/70 uppercase tracking-wider">
									{t("papersCool.venues")}
								</div>

								{index.venues.map((venue) => (
									<CollapsibleGroup
										key={venue.name}
										title={venue.name}
										icon={<BookOpen className="h-3 w-3" />}
									>
										{venue.editions.map((edition) =>
											edition.groups.length > 0 ? (
												<CollapsibleGroup
													key={edition.key}
													title={edition.year}
													depth={1}
												>
													<SidebarItem
														active={
															mode === "venue" &&
															selectedVenue === edition.key &&
															!selectedGroup
														}
														onClick={() => browseVenue(edition.key)}
														bookmarked={usePapersCoolStore
															.getState()
															.isBookmarked("venue", edition.key)}
														onToggleBookmark={() => {
															if (
																usePapersCoolStore
																	.getState()
																	.isBookmarked("venue", edition.key)
															) {
																removeBookmark("venue", edition.key);
															} else {
																addBookmark({
																	type: "venue",
																	key: edition.key,
																	label: `${edition.key} — All`,
																});
															}
														}}
													>
														<span className="truncate">
															{t("papersCool.allPapers")}
														</span>
													</SidebarItem>
													{edition.groups.map((g) => (
														<SidebarItem
															key={g.query}
															active={
																mode === "venue" &&
																selectedVenue === edition.key &&
																selectedGroup === g.name
															}
															onClick={() => browseVenue(edition.key, g.name)}
															bookmarked={usePapersCoolStore
																.getState()
																.isBookmarked("venue", edition.key, g.name)}
															onToggleBookmark={() => {
																if (
																	usePapersCoolStore
																		.getState()
																		.isBookmarked("venue", edition.key, g.name)
																) {
																	removeBookmark("venue", edition.key, g.name);
																} else {
																	addBookmark({
																		type: "venue",
																		key: edition.key,
																		label: `${edition.key} — ${g.name}`,
																		venueGroup: g.name,
																	});
																}
															}}
														>
															<span className="truncate">{g.name}</span>
														</SidebarItem>
													))}
												</CollapsibleGroup>
											) : (
												<SidebarItem
													key={edition.key}
													active={
														mode === "venue" && selectedVenue === edition.key
													}
													onClick={() => browseVenue(edition.key)}
													bookmarked={usePapersCoolStore
														.getState()
														.isBookmarked("venue", edition.key)}
													onToggleBookmark={() => {
														if (
															usePapersCoolStore
																.getState()
																.isBookmarked("venue", edition.key)
														) {
															removeBookmark("venue", edition.key);
														} else {
															addBookmark({
																type: "venue",
																key: edition.key,
																label: edition.key,
															});
														}
													}}
												>
													<span className="truncate">{edition.year}</span>
												</SidebarItem>
											),
										)}
									</CollapsibleGroup>
								))}
							</>
						)}
					</div>
				</ScrollArea>
			</nav>

			{/* Content area */}
			<div className="flex-1 min-w-0 flex flex-col">
				{/* Header */}
				<div className="flex items-center justify-between border-b px-6 py-3 select-none shrink-0">
					<div>
						<h2 className="text-lg font-semibold">{headerTitle}</h2>
						{headerSubtitle && (
							<p className="text-sm text-muted-foreground">{headerSubtitle}</p>
						)}
					</div>
					<div className="flex items-center gap-2">
						{page && (
							<span className="text-xs text-muted-foreground">
								{page.papers.length} of {page.total} papers
							</span>
						)}
						<DisplayModeToggle />
						<Button
							variant="outline"
							size="sm"
							onClick={handleRefresh}
							disabled={loading}
						>
							<RefreshCw
								className={cn("mr-1.5 h-3.5 w-3.5", loading && "animate-spin")}
							/>
							Refresh
						</Button>
					</div>
				</div>

				{/* Date navigation (arXiv mode only) */}
				{mode === "arxiv" && selectedCategory && (
					<div className="flex items-center justify-center gap-2 border-b px-6 py-2 bg-muted/30 select-none shrink-0">
						<Button
							variant="ghost"
							size="icon"
							className="h-8 w-8"
							onClick={() => setDate(shiftDate(date, -1))}
						>
							<ChevronLeft className="h-4 w-4" />
						</Button>
						<div className="flex items-center gap-2">
							<Calendar className="h-4 w-4 text-muted-foreground" />
							<span className="text-sm font-medium min-w-[180px] text-center">
								{formatDateLabel(date)}
							</span>
						</div>
						<Button
							variant="ghost"
							size="icon"
							className="h-8 w-8"
							onClick={() => setDate(shiftDate(date, 1))}
							disabled={isToday}
						>
							<ChevronRight className="h-4 w-4" />
						</Button>
						{!isToday && (
							<Button
								variant="outline"
								size="sm"
								className="ml-2 h-7 text-xs"
								onClick={() => setDate(todayStr())}
							>
								{t("papersCool.today")}
							</Button>
						)}
					</div>
				)}

				{/* Paper list */}
				{loading ? (
					<div className="flex flex-1 items-center justify-center text-muted-foreground">
						<Loader2 className="mr-2 h-5 w-5 animate-spin" />
						{t("papersCool.loadingPapers")}
					</div>
				) : error ? (
					<div className="flex flex-1 items-center justify-center">
						<p className="text-sm text-destructive">{error}</p>
					</div>
				) : !page || page.papers.length === 0 ? (
					<div className="flex flex-1 flex-col items-center justify-center gap-2 text-muted-foreground">
						<p className="text-sm">
							{selectedCategory || selectedVenue
								? t("papersCool.noPapersFound")
								: t("papersCool.selectCategoryOrVenue")}
						</p>
					</div>
				) : (
					<ScrollArea className="flex-1">
						<div className="grid grid-cols-[repeat(auto-fill,minmax(300px,1fr))] gap-4 p-4">
							{page.papers.map((paper) => (
								<PaperCard key={paper.external_id} paper={paper} />
							))}
						</div>
					</ScrollArea>
				)}
			</div>
		</div>
	);
}

// ── Sidebar Item (with bookmark star) ────────────────────────────────────────

function SidebarItem({
	active,
	onClick,
	title,
	children,
	bookmarked,
	onToggleBookmark,
}: {
	active: boolean;
	onClick: () => void;
	title?: string;
	children: React.ReactNode;
	bookmarked: boolean;
	onToggleBookmark: () => void;
}) {
	const { t } = useTranslation();
	return (
		<div
			className={cn(
				"group flex items-center gap-1 rounded px-2 py-1 text-xs hover:bg-accent/50 transition-colors cursor-pointer",
				active && "bg-accent text-accent-foreground font-medium",
			)}
			onClick={onClick}
			title={title}
		>
			<button
				type="button"
				className={cn(
					"shrink-0 transition-opacity",
					bookmarked ? "opacity-100" : "opacity-0 group-hover:opacity-100",
				)}
				onClick={(e) => {
					e.stopPropagation();
					onToggleBookmark();
				}}
				title={
					bookmarked
						? t("papersCool.removeBookmark")
						: t("papersCool.addBookmark")
				}
			>
				<Star
					className={cn(
						"h-3 w-3",
						bookmarked
							? "fill-yellow-400 text-yellow-400"
							: "text-muted-foreground hover:text-yellow-400",
					)}
				/>
			</button>
			<div className="flex flex-1 items-center gap-1.5 text-left min-w-0">
				{children}
			</div>
		</div>
	);
}

// ── Collapsible Group ───────────────────────────────────────────────────────

function CollapsibleGroup({
	title,
	icon,
	children,
	defaultOpen = false,
	depth = 0,
}: {
	title: string;
	icon?: React.ReactNode;
	children: React.ReactNode;
	defaultOpen?: boolean;
	depth?: number;
}) {
	const [open, setOpen] = useState(defaultOpen);

	return (
		<div>
			<button
				type="button"
				className="flex w-full items-center gap-1 rounded px-1.5 py-1 text-xs font-medium text-muted-foreground hover:bg-accent/30 transition-colors"
				style={{ paddingLeft: `${depth * 12 + 6}px` }}
				onClick={() => setOpen(!open)}
			>
				<ChevronDown
					className={cn(
						"h-3 w-3 shrink-0 transition-transform",
						!open && "-rotate-90",
					)}
				/>
				{icon && <span className="shrink-0">{icon}</span>}
				<span className="truncate">{title}</span>
			</button>
			{open && <div>{children}</div>}
		</div>
	);
}

// ── Paper Card ──────────────────────────────────────────────────────────────

function PaperCard({ paper }: { paper: PapersCoolPaperResponse }) {
	const { t } = useTranslation();
	const openTab = useTabStore((s) => s.openTab);
	const [adding, setAdding] = useState(false);
	const [added, setAdded] = useState(false);

	const displayMode = useTranslationStore((s) => s.displayMode);
	const translatedTitle = useTranslatedText(
		"papers_cool_paper",
		paper.external_id,
		"title",
	);
	const translatedAbstract = useTranslatedText(
		"papers_cool_paper",
		paper.external_id,
		"abstract_text",
	);

	const showTitle =
		displayMode === "original" || !translatedTitle
			? paper.title
			: translatedTitle;

	const showAbstract =
		displayMode === "original" || !translatedAbstract
			? paper.abstract_text
			: translatedAbstract;

	const asFeedItem = useCallback(
		(): commands.FeedItemResponse => ({
			id: paper.external_id,
			external_id: paper.external_id,
			title: paper.title,
			authors: paper.authors.map((name) => ({ name, affiliation: null })),
			abstract_text: paper.abstract_text,
			url: paper.abs_url,
			pdf_url: paper.pdf_url,
			html_url: null,
			upvotes: null,
			published_at: paper.published_date,
			fetched_date: "",
			added_to_library: false,
			thumbnail_url: null,
			ai_summary: null,
			ai_keywords: paper.keywords.length > 0 ? paper.keywords : null,
			project_page: null,
			github_repo: null,
			github_stars: null,
			num_comments: null,
			media_urls: [],
			cached_thumbnail_path: null,
			organization: null,
		}),
		[paper],
	);

	const handleOpenWebview = useCallback(() => {
		openTab({
			type: "webview",
			url: paper.papers_cool_url,
			feedItem: asFeedItem(),
			title: paper.title,
		});
	}, [paper, openTab, asFeedItem]);

	const handleOpenPdf = useCallback(() => {
		if (paper.pdf_url) {
			openTab({
				type: "reader",
				feedItem: asFeedItem(),
				readerMode: "pdf",
				title: paper.title,
			});
		}
	}, [paper, openTab, asFeedItem]);

	// Delay single-click so that double-click has time to register
	const clickTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
	const handleClick = useCallback(() => {
		if (clickTimer.current) clearTimeout(clickTimer.current);
		clickTimer.current = setTimeout(() => {
			handleOpenWebview();
			clickTimer.current = null;
		}, 300);
	}, [handleOpenWebview]);
	const handleDoubleClick = useCallback(() => {
		if (clickTimer.current) {
			clearTimeout(clickTimer.current);
			clickTimer.current = null;
		}
		handleOpenPdf();
	}, [handleOpenPdf]);

	const handleAddToLibrary = async () => {
		setAdding(true);
		try {
			const input: commands.AddPaperInput = {
				title: paper.title,
				authors: paper.authors.map((name) => ({ name })),
				abstract_text: paper.abstract_text ?? undefined,
				url: paper.abs_url ?? undefined,
				pdf_url: paper.pdf_url ?? undefined,
				arxiv_id: paper.external_id.includes("@")
					? undefined
					: paper.external_id,
			};
			await commands.addPaper(input);
			setAdded(true);
		} catch (e) {
			console.error("Failed to add paper:", e);
		}
		setAdding(false);
	};

	return (
		<div
			className="flex flex-col rounded-lg border bg-card overflow-hidden transition-all hover:shadow-md hover:border-primary/30 cursor-pointer select-none"
			onClick={handleClick}
			onDoubleClick={handleDoubleClick}
		>
			<div className="flex-1 p-4 pb-0">
				{/* Title */}
				<h3
					className={cn(
						"font-medium text-sm leading-snug",
						displayMode === "original" && "line-clamp-2",
					)}
					title={paper.title}
				>
					{showTitle}
				</h3>
				{displayMode === "bilingual" && translatedTitle && (
					<p className="text-[11px] text-muted-foreground/60 mt-0.5">
						{paper.title}
					</p>
				)}

				{/* Authors */}
				{paper.authors.length > 0 && (
					<CollapsibleAuthors authors={paper.authors} className="mt-1" />
				)}

				{/* Abstract */}
				{displayMode === "bilingual" &&
				translatedAbstract &&
				paper.abstract_text ? (
					<BilingualCardAbstract
						original={paper.abstract_text}
						translated={translatedAbstract}
					/>
				) : (
					(showAbstract || paper.abstract_text) && (
						<ParagraphedCardText
							text={showAbstract ?? paper.abstract_text ?? ""}
						/>
					)
				)}

				{/* Categories */}
				{paper.categories.length > 0 && (
					<div className="mt-1.5 flex flex-wrap gap-1">
						{paper.categories.slice(0, 4).map((cat) => (
							<Badge
								key={cat.code}
								variant="secondary"
								className="text-[10px] px-1.5 py-0"
							>
								{cat.code}
							</Badge>
						))}
						{paper.categories.length > 4 && (
							<span className="text-[10px] text-muted-foreground">
								+{paper.categories.length - 4}
							</span>
						)}
					</div>
				)}
			</div>

			{/* Bottom zone */}
			<div className="px-4 pb-3 pt-2">
				{/* Metrics */}
				<div className="flex items-center gap-2 text-xs text-muted-foreground">
					<span className="truncate font-mono">{paper.external_id}</span>
					{paper.pdf_opens > 0 && (
						<span
							className="inline-flex items-center gap-0.5 shrink-0"
							title={t("papersCool.pdfOpens")}
						>
							<Eye className="h-3 w-3" />
							{paper.pdf_opens}
						</span>
					)}
					{paper.kimi_opens > 0 && (
						<Badge variant="outline" className="text-[10px] px-1 py-0 shrink-0">
							Kimi {paper.kimi_opens}
						</Badge>
					)}
					{paper.published_date && (
						<span className="shrink-0">
							{paper.published_date.slice(0, 10)}
						</span>
					)}
				</div>

				{/* Actions — stopPropagation so clicks here don't open webview */}
				{/* biome-ignore lint: click handler is for event interception only */}
				<div
					className="mt-2 flex items-center justify-between"
					onClick={(e) => e.stopPropagation()}
				>
					<div className="flex items-center gap-1">
						{paper.abs_url && (
							<Button variant="ghost" size="icon" className="h-7 w-7" asChild>
								<a
									href={paper.abs_url}
									target="_blank"
									rel="noopener noreferrer"
									title={t("papersCool.openArxiv")}
								>
									<ExternalLink className="h-3.5 w-3.5" />
								</a>
							</Button>
						)}
						<Button variant="ghost" size="icon" className="h-7 w-7" asChild>
							<a
								href={paper.papers_cool_url}
								target="_blank"
								rel="noopener noreferrer"
								title={t("papersCool.openPapersCool")}
							>
								<BookOpen className="h-3.5 w-3.5" />
							</a>
						</Button>
					</div>
					{added ? (
						<Button variant="ghost" size="sm" disabled className="h-7 text-xs">
							{t("feed.added")}
						</Button>
					) : (
						<Button
							variant="outline"
							size="sm"
							className="h-7 text-xs"
							onClick={handleAddToLibrary}
							disabled={adding}
						>
							<Plus className="mr-1 h-3.5 w-3.5" />
							{adding ? t("papersCool.adding") : t("feed.add")}
						</Button>
					)}
				</div>
			</div>
		</div>
	);
}
