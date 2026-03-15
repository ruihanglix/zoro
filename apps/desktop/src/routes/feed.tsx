// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import {
	BilingualCardAbstract,
	ParagraphedCardText,
} from "@/components/BilingualText";
import { DisplayModeToggle } from "@/components/DisplayModeToggle";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import {
	Tooltip,
	TooltipContent,
	TooltipProvider,
	TooltipTrigger,
} from "@/components/ui/tooltip";
import type { FeedItemResponse } from "@/lib/commands";
import { useLibraryStore } from "@/stores/libraryStore";
import { useTabStore } from "@/stores/tabStore";
import {
	useTranslatedText,
	useTranslationStore,
} from "@/stores/translationStore";
import { useUiStore } from "@/stores/uiStore";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
	Building2,
	Calendar,
	Check,
	ChevronLeft,
	ChevronRight,
	ExternalLink,
	Github,
	Globe,
	LayoutGrid,
	List,
	MessageSquare,
	Plus,
	RefreshCw,
	Star,
	ThumbsUp,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

/** Format a Date to YYYY-MM-DD string in UTC.
 *  HuggingFace publishes Daily Papers on a UTC clock, so all date
 *  calculations must use UTC to stay in sync. */
function toDateString(d: Date): string {
	const y = d.getUTCFullYear();
	const m = String(d.getUTCMonth() + 1).padStart(2, "0");
	const day = String(d.getUTCDate()).padStart(2, "0");
	return `${y}-${m}-${day}`;
}

/** Format a date string to a human-readable label. */
function formatDateLabel(dateStr: string): string {
	const d = new Date(`${dateStr}T00:00:00`);
	return d.toLocaleDateString(undefined, {
		weekday: "short",
		year: "numeric",
		month: "short",
		day: "numeric",
	});
}

/** Get today's date as YYYY-MM-DD in UTC. */
function todayStr(): string {
	return toDateString(new Date());
}

/** Shift a YYYY-MM-DD date by `delta` days, skipping weekends (Sat/Sun)
 *  since HuggingFace Daily Papers does not publish on weekends.
 *  Uses UTC to stay consistent with HuggingFace's clock. */
function shiftDate(dateStr: string, delta: number): string {
	const step = delta > 0 ? 1 : -1;
	let remaining = Math.abs(delta);
	const d = new Date(`${dateStr}T00:00:00Z`);
	while (remaining > 0) {
		d.setUTCDate(d.getUTCDate() + step);
		const day = d.getUTCDay(); // 0=Sun, 6=Sat
		if (day !== 0 && day !== 6) {
			remaining--;
		}
	}
	return toDateString(d);
}

const VIDEO_EXTS = new Set(["mp4", "webm", "mov", "qt"]);
const ANIM_EXTS = new Set(["gif", "webp"]);

function urlExtension(url: string): string {
	const path = url.split("?")[0];
	const dot = path.lastIndexOf(".");
	return dot >= 0 ? path.slice(dot + 1).toLowerCase() : "";
}

interface MediaInfo {
	src: string;
	type: "image" | "video" | "gif";
}

/** Pick the best media to display for a feed item.
 *  Priority: mediaUrls (first entry) > cached thumbnail > remote thumbnail. */
function resolveMedia(item: FeedItemResponse): MediaInfo | null {
	if (item.media_urls.length > 0) {
		const url = item.media_urls[0];
		const ext = urlExtension(url);
		if (VIDEO_EXTS.has(ext)) return { src: url, type: "video" };
		if (ANIM_EXTS.has(ext)) return { src: url, type: "gif" };
		return { src: url, type: "image" };
	}
	if (item.cached_thumbnail_path) {
		return { src: convertFileSrc(item.cached_thumbnail_path), type: "image" };
	}
	if (item.thumbnail_url) {
		return { src: item.thumbnail_url, type: "image" };
	}
	return null;
}

function FeedMedia({
	media,
	className,
}: { media: MediaInfo; className?: string }) {
	if (media.type === "video") {
		return (
			<video
				src={media.src}
				className={className}
				muted
				loop
				playsInline
				autoPlay
			/>
		);
	}
	return <img src={media.src} alt="" className={className} loading="lazy" />;
}

export function Feed() {
	const { t } = useTranslation();
	const subscriptions = useLibraryStore((s) => s.subscriptions);
	const feedItems = useLibraryStore((s) => s.feedItems);
	const feedLoading = useLibraryStore((s) => s.feedLoading);
	const feedDate = useLibraryStore((s) => s.feedDate);
	const latestFeedDate = useLibraryStore((s) => s.latestFeedDate);
	const error = useLibraryStore((s) => s.error);
	const fetchFeedByDate = useLibraryStore((s) => s.fetchFeedByDate);
	const setFeedDate = useLibraryStore((s) => s.setFeedDate);
	const fetchLatestFeedDate = useLibraryStore((s) => s.fetchLatestFeedDate);
	const fetchSubscriptions = useLibraryStore((s) => s.fetchSubscriptions);
	const refreshSubscription = useLibraryStore((s) => s.refreshSubscription);
	const addFeedItemToLibrary = useLibraryStore((s) => s.addFeedItemToLibrary);
	const feedListMode = useUiStore((s) => s.feedListMode);
	const setFeedListMode = useUiStore((s) => s.setFeedListMode);
	const openTab = useTabStore((s) => s.openTab);
	const [refreshing, setRefreshing] = useState(false);
	const ensureTranslatedBatch = useTranslationStore(
		(s) => s.ensureTranslatedBatch,
	);
	const displayMode = useTranslationStore((s) => s.displayMode);

	// On first mount, always refresh the latest date in the background.
	// If we already have a cached value (from localStorage), it will be used
	// immediately so the user sees content right away; once the API responds
	// with a newer date the view will seamlessly switch over.
	const hasFetchedRef = useRef(false);
	useEffect(() => {
		if (!hasFetchedRef.current) {
			hasFetchedRef.current = true;
			fetchLatestFeedDate();
		}
	}, [fetchLatestFeedDate]);

	// Batch-fetch and auto-translate titles + summaries for visible feed items
	useEffect(() => {
		if (feedItems.length > 0) {
			ensureTranslatedBatch(
				"subscription_item",
				feedItems.map((item) => item.id),
				["title", "ai_summary"],
			);
		}
	}, [feedItems, ensureTranslatedBatch, displayMode]);

	// Use cached/fetched date; fall back to todayStr() only when nothing is available
	const effectiveLatest = latestFeedDate ?? todayStr();
	const currentDate = feedDate ?? effectiveLatest;
	const isLatest = currentDate != null && currentDate === effectiveLatest;
	const isFuture =
		currentDate != null &&
		effectiveLatest != null &&
		currentDate > effectiveLatest;

	const activeSub = subscriptions.find(
		(s) => s.source_type === "huggingface-daily",
	);
	const activeSubId = activeSub?.id;

	// Fetch items whenever activeSubId or date changes
	// Wait until we have a resolved date (don't fetch with null)
	useEffect(() => {
		if (!activeSubId || !currentDate) return;
		fetchFeedByDate(activeSubId, currentDate);
	}, [activeSubId, currentDate, fetchFeedByDate]);

	useEffect(() => {
		const unlisten = listen<string>("subscription-updated", () => {
			if (activeSubId && currentDate) {
				fetchFeedByDate(activeSubId, currentDate);
			}
			fetchSubscriptions();
		});
		return () => {
			unlisten.then((f) => f());
		};
	}, [activeSubId, currentDate, fetchFeedByDate, fetchSubscriptions]);

	const handleRefresh = async () => {
		if (!activeSub) return;
		setRefreshing(true);
		console.log("[Feed] handleRefresh start", {
			isLatest,
			feedDate,
			currentDate,
			effectiveLatest,
			latestFeedDate,
		});
		try {
			if (isLatest || !feedDate) {
				// refresh_subscription now internally uses fetch_by_date with the
				// latest date, so it stores exactly the same data that
				// fetch_feed_items_by_date would return.
				const count = await refreshSubscription(activeSub.id);
				console.log(`[Feed] refreshSubscription returned ${count} new items`);
				// Re-fetch the latest date and reload the view
				const freshDate = await fetchLatestFeedDate();
				console.log("[Feed] fetchLatestFeedDate returned:", freshDate);
				const dateToFetch = freshDate ?? effectiveLatest ?? todayStr();
				console.log("[Feed] will fetchFeedByDate with date:", dateToFetch);
				await fetchFeedByDate(activeSub.id, dateToFetch);
			} else if (currentDate) {
				// For historical dates, just re-fetch from the API
				console.log("[Feed] re-fetching historical date:", currentDate);
				await fetchFeedByDate(activeSub.id, currentDate, true);
			}
		} catch (err) {
			console.error("[Feed] Failed to refresh:", err);
		}
		console.log("[Feed] handleRefresh done");
		setRefreshing(false);
	};

	const handleAddToLibrary = async (itemId: string) => {
		try {
			await addFeedItemToLibrary(itemId);
			if (activeSub && currentDate) {
				await fetchFeedByDate(activeSub.id, currentDate);
			}
		} catch (err) {
			console.error("Failed to add to library:", err);
		}
	};

	const handleOpenPdf = useCallback(
		(item: FeedItemResponse) => {
			openTab({
				type: "reader",
				feedItem: item,
				readerMode: "pdf",
				title: item.title,
			});
		},
		[openTab],
	);

	const handleOpenWebview = useCallback(
		(item: FeedItemResponse) => {
			openTab({
				type: "webview",
				url: `https://huggingface.co/papers/${item.external_id}`,
				feedItem: item,
				title: item.title,
			});
		},
		[openTab],
	);

	const handlePrevDay = () => {
		setFeedDate(shiftDate(currentDate, -1));
	};

	const handleNextDay = () => {
		if (!isFuture) {
			setFeedDate(shiftDate(currentDate, 1));
		}
	};

	const handleLatest = () => {
		setFeedDate(null);
	};

	return (
		<div className="flex h-full flex-col">
			{/* Header */}
			<div className="flex items-center justify-between border-b px-6 py-3 select-none">
				<div>
					<h2 className="text-lg font-semibold">{t("feed.title")}</h2>
					<p className="text-sm text-muted-foreground">
						{activeSub?.last_polled
							? `${t("feed.lastUpdated")}: ${new Date(activeSub.last_polled).toLocaleString()}`
							: t("feed.neverRefreshed")}
					</p>
				</div>
				<div className="flex items-center gap-2">
					<DisplayModeToggle />
					<TooltipProvider delayDuration={300}>
						<ToggleGroup
							type="single"
							value={feedListMode}
							onValueChange={(v) => {
								if (v) setFeedListMode(v as "list" | "card");
							}}
							className="border rounded-md"
						>
							<Tooltip>
								<TooltipTrigger asChild>
									<ToggleGroupItem value="list" size="sm" className="px-2">
										<List className="h-4 w-4" />
									</ToggleGroupItem>
								</TooltipTrigger>
								<TooltipContent>{t("feed.listView")}</TooltipContent>
							</Tooltip>
							<Tooltip>
								<TooltipTrigger asChild>
									<ToggleGroupItem value="card" size="sm" className="px-2">
										<LayoutGrid className="h-4 w-4" />
									</ToggleGroupItem>
								</TooltipTrigger>
								<TooltipContent>{t("feed.cardView")}</TooltipContent>
							</Tooltip>
						</ToggleGroup>
					</TooltipProvider>
					<Button
						onClick={handleRefresh}
						disabled={refreshing}
						variant="outline"
					>
						<RefreshCw
							className={`mr-2 h-4 w-4 ${refreshing ? "animate-spin" : ""}`}
						/>
						{refreshing ? t("feed.refreshing") : t("common.refresh")}
					</Button>
				</div>
			</div>

			{/* Date Navigation Bar */}
			<div className="flex items-center justify-center gap-2 border-b px-6 py-2 bg-muted/30 select-none">
				<Button
					variant="ghost"
					size="icon"
					className="h-8 w-8"
					onClick={handlePrevDay}
				>
					<ChevronLeft className="h-4 w-4" />
				</Button>

				<div className="flex items-center gap-2">
					<Calendar className="h-4 w-4 text-muted-foreground" />
					<span className="text-sm font-medium min-w-[180px] text-center">
						{currentDate ? formatDateLabel(currentDate) : t("common.loading")}
					</span>
				</div>

				<Button
					variant="ghost"
					size="icon"
					className="h-8 w-8"
					onClick={handleNextDay}
					disabled={isFuture || isLatest}
				>
					<ChevronRight className="h-4 w-4" />
				</Button>

				{!isLatest && (
					<Button
						variant="outline"
						size="sm"
						className="ml-2 h-7 text-xs"
						onClick={handleLatest}
					>
						{t("feed.latest")}
					</Button>
				)}

				<span className="ml-2 text-xs text-muted-foreground">
					{t("feed.papersCount", { count: feedItems.length })}
				</span>
			</div>

			{/* Content */}
			{feedLoading ? (
				<div className="flex flex-1 items-center justify-center text-muted-foreground">
					{t("feed.loadingFeed")}
				</div>
			) : feedItems.length === 0 ? (
				<div className="flex flex-1 flex-col items-center justify-center gap-2 text-muted-foreground">
					{error ? (
						<p className="text-destructive text-sm">{error}</p>
					) : (
						<p>{t("feed.noPapers", { date: formatDateLabel(currentDate) })}</p>
					)}
					<Button onClick={handleRefresh} variant="outline" size="sm">
						<RefreshCw className="mr-2 h-4 w-4" />
						{t("feed.fetchPapers")}
					</Button>
				</div>
			) : feedListMode === "card" ? (
				<ScrollArea className="flex-1">
					<div className="grid grid-cols-[repeat(auto-fill,minmax(300px,1fr))] gap-4 p-4">
						{feedItems.map((item) => (
							<FeedCard
								key={item.id}
								item={item}
								onAddToLibrary={handleAddToLibrary}
								onOpenPdf={handleOpenPdf}
								onOpenWebview={handleOpenWebview}
							/>
						))}
					</div>
				</ScrollArea>
			) : (
				<ScrollArea className="flex-1">
					<div className="divide-y">
						{feedItems.map((item) => (
							<FeedListItem
								key={item.id}
								item={item}
								onAddToLibrary={handleAddToLibrary}
								onOpenPdf={handleOpenPdf}
								onOpenWebview={handleOpenWebview}
							/>
						))}
					</div>
				</ScrollArea>
			)}
		</div>
	);
}

/* ─── Card View ─────────────────────────────────────────────────────── */

function FeedCard({
	item,
	onAddToLibrary,
	onOpenPdf,
	onOpenWebview,
}: {
	item: FeedItemResponse;
	onAddToLibrary: (id: string) => void;
	onOpenPdf: (item: FeedItemResponse) => void;
	onOpenWebview: (item: FeedItemResponse) => void;
}) {
	const { t } = useTranslation();
	const arxivUrl = item.url ?? `https://arxiv.org/abs/${item.external_id}`;
	const authorText =
		item.authors.length > 0 ? item.authors.map((a) => a.name).join(", ") : null;

	const displayMode = useTranslationStore((s) => s.displayMode);
	const translatedTitle = useTranslatedText(
		"subscription_item",
		item.id,
		"title",
	);
	const translatedAiSummary = useTranslatedText(
		"subscription_item",
		item.id,
		"ai_summary",
	);
	const translatedAbstract = useTranslatedText(
		"subscription_item",
		item.id,
		"abstract_text",
	);

	const showTitle =
		displayMode === "original" || !translatedTitle
			? item.title
			: translatedTitle;

	const originalSummary = item.ai_summary ?? item.abstract_text;
	const translatedSummary = translatedAiSummary ?? translatedAbstract;
	const showAbstract =
		displayMode === "original" || !translatedSummary
			? originalSummary
			: translatedSummary;

	// Delay single-click so that double-click has time to register
	const clickTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
	const handleClick = useCallback(() => {
		if (clickTimer.current) clearTimeout(clickTimer.current);
		clickTimer.current = setTimeout(() => {
			onOpenWebview(item);
			clickTimer.current = null;
		}, 300);
	}, [item, onOpenWebview]);
	const handleDoubleClick = useCallback(() => {
		if (clickTimer.current) {
			clearTimeout(clickTimer.current);
			clickTimer.current = null;
		}
		onOpenPdf(item);
	}, [item, onOpenPdf]);

	return (
		<div
			className="flex flex-col rounded-lg border bg-card overflow-hidden transition-all hover:shadow-md hover:border-primary/30 cursor-pointer select-none"
			onClick={handleClick}
			onDoubleClick={handleDoubleClick}
		>
			{/* Media (image / video / gif) */}
			{(() => {
				const media = resolveMedia(item);
				return media ? (
					<div className="aspect-[2/1] overflow-hidden bg-muted shrink-0">
						<FeedMedia media={media} className="h-full w-full object-cover" />
					</div>
				) : null;
			})()}

			{/* Top content zone — grows to fill available space */}
			<div className="flex-1 p-4 pb-0">
				{/* Title */}
				<h3
					className="font-medium text-sm leading-snug line-clamp-2"
					title={item.title}
				>
					{showTitle}
				</h3>
				{displayMode === "bilingual" && translatedTitle && (
					<p className="text-[11px] text-muted-foreground/60 mt-0.5">
						{item.title}
					</p>
				)}

				{/* Authors */}
				{authorText && (
					<p className="mt-1 text-xs text-muted-foreground">{authorText}</p>
				)}

				{/* Abstract or AI Summary */}
				{displayMode === "bilingual" && translatedSummary && originalSummary ? (
					<BilingualCardAbstract
						original={originalSummary}
						translated={translatedSummary}
					/>
				) : (
					(showAbstract || originalSummary) && (
						<ParagraphedCardText text={showAbstract ?? originalSummary ?? ""} />
					)
				)}
			</div>

			{/* Bottom zone — pinned to card bottom */}
			<div className="px-4 pb-3 pt-2">
				{/* Organization badge */}
				{item.organization &&
					(item.organization.fullname || item.organization.name) && (
						<div className="mb-1.5">
							<Badge
								variant="secondary"
								className="text-[10px] px-1.5 py-0.5 gap-1 font-normal"
							>
								{item.organization.avatar ? (
									<img
										src={item.organization.avatar}
										alt=""
										className="h-3 w-3 rounded-sm"
									/>
								) : (
									<Building2 className="h-3 w-3" />
								)}
								{item.organization.fullname || item.organization.name}
							</Badge>
						</div>
					)}

				{/* Metadata row */}
				<div className="flex items-center gap-2 text-xs text-muted-foreground">
					<span className="truncate">{item.external_id}</span>
					{typeof item.upvotes === "number" && (
						<span className="inline-flex items-center gap-0.5 shrink-0">
							<ThumbsUp className="h-3 w-3" />
							{item.upvotes}
						</span>
					)}
					{typeof item.num_comments === "number" && item.num_comments > 0 && (
						<span className="inline-flex items-center gap-0.5 shrink-0">
							<MessageSquare className="h-3 w-3" />
							{item.num_comments}
						</span>
					)}
					{typeof item.github_stars === "number" && item.github_stars > 0 && (
						<span className="inline-flex items-center gap-0.5 shrink-0">
							<Star className="h-3 w-3" />
							{item.github_stars}
						</span>
					)}
				</div>

				{/* Action row */}
				{/* biome-ignore lint: click handler is for event interception only */}
				<div
					className="mt-2 flex items-center justify-between"
					onClick={(e) => e.stopPropagation()}
				>
					<div className="flex items-center gap-1">
						{item.github_repo && (
							<Button variant="ghost" size="icon" className="h-7 w-7" asChild>
								<a
									href={item.github_repo}
									target="_blank"
									rel="noopener noreferrer"
								>
									<Github className="h-3.5 w-3.5" />
								</a>
							</Button>
						)}
						{item.project_page && (
							<Button variant="ghost" size="icon" className="h-7 w-7" asChild>
								<a
									href={item.project_page}
									target="_blank"
									rel="noopener noreferrer"
								>
									<Globe className="h-3.5 w-3.5" />
								</a>
							</Button>
						)}
						<Button variant="ghost" size="icon" className="h-7 w-7" asChild>
							<a href={arxivUrl} target="_blank" rel="noopener noreferrer">
								<ExternalLink className="h-3.5 w-3.5" />
							</a>
						</Button>
					</div>
					{item.added_to_library ? (
						<Button variant="ghost" size="sm" disabled className="h-7 text-xs">
							<Check className="mr-1 h-3.5 w-3.5" /> {t("feed.added")}
						</Button>
					) : (
						<Button
							variant="outline"
							size="sm"
							className="h-7 text-xs"
							onClick={() => onAddToLibrary(item.id)}
						>
							<Plus className="mr-1 h-3.5 w-3.5" /> {t("feed.add")}
						</Button>
					)}
				</div>
			</div>
		</div>
	);
}

/* ─── List View ─────────────────────────────────────────────────────── */

function FeedListItem({
	item,
	onAddToLibrary,
	onOpenPdf,
	onOpenWebview,
}: {
	item: FeedItemResponse;
	onAddToLibrary: (id: string) => void;
	onOpenPdf: (item: FeedItemResponse) => void;
	onOpenWebview: (item: FeedItemResponse) => void;
}) {
	const { t } = useTranslation();
	const arxivUrl = item.url ?? `https://arxiv.org/abs/${item.external_id}`;
	const authorText =
		item.authors.length > 0 ? item.authors.map((a) => a.name).join(", ") : null;

	const displayMode = useTranslationStore((s) => s.displayMode);
	const translatedTitle = useTranslatedText(
		"subscription_item",
		item.id,
		"title",
	);
	const translatedAiSummary = useTranslatedText(
		"subscription_item",
		item.id,
		"ai_summary",
	);
	const translatedAbstract = useTranslatedText(
		"subscription_item",
		item.id,
		"abstract_text",
	);

	const showTitle =
		displayMode === "original" || !translatedTitle
			? item.title
			: translatedTitle;

	const originalSummary = item.ai_summary ?? item.abstract_text;
	const translatedSummary = translatedAiSummary ?? translatedAbstract;
	const showAbstract =
		displayMode === "original" || !translatedSummary
			? originalSummary
			: translatedSummary;

	// Delay single-click so that double-click has time to register
	const clickTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
	const handleClick = useCallback(() => {
		if (clickTimer.current) clearTimeout(clickTimer.current);
		clickTimer.current = setTimeout(() => {
			onOpenWebview(item);
			clickTimer.current = null;
		}, 300);
	}, [item, onOpenWebview]);
	const handleDoubleClick = useCallback(() => {
		if (clickTimer.current) {
			clearTimeout(clickTimer.current);
			clickTimer.current = null;
		}
		onOpenPdf(item);
	}, [item, onOpenPdf]);

	return (
		<div
			className="px-6 py-4 hover:bg-accent/30 transition-colors cursor-pointer select-none"
			onClick={handleClick}
			onDoubleClick={handleDoubleClick}
		>
			<div className="flex items-start gap-3">
				{/* Media (small) */}
				{(() => {
					const media = resolveMedia(item);
					return media ? (
						<div className="hidden sm:block w-24 h-14 rounded overflow-hidden bg-muted shrink-0">
							<FeedMedia media={media} className="h-full w-full object-cover" />
						</div>
					) : null;
				})()}

				<div className="flex-1 min-w-0">
					<h3 className="font-medium text-sm leading-snug" title={item.title}>
						{showTitle}
					</h3>
					{displayMode === "bilingual" && translatedTitle && (
						<p className="text-[11px] text-muted-foreground/60 mt-0.5">
							{item.title}
						</p>
					)}
					{authorText && (
						<p className="mt-1 text-xs text-muted-foreground">{authorText}</p>
					)}
					{(showAbstract || originalSummary) && (
						<p className="mt-1 text-xs text-muted-foreground line-clamp-3">
							{showAbstract ?? originalSummary}
						</p>
					)}

					{/* Organization badge */}
					{item.organization &&
						(item.organization.fullname || item.organization.name) && (
							<div className="mt-1.5">
								<Badge
									variant="secondary"
									className="text-[10px] px-1.5 py-0.5 gap-1 font-normal"
								>
									{item.organization.avatar ? (
										<img
											src={item.organization.avatar}
											alt=""
											className="h-3 w-3 rounded-sm"
										/>
									) : (
										<Building2 className="h-3 w-3" />
									)}
									{item.organization.fullname || item.organization.name}
								</Badge>
							</div>
						)}

					{/* Keywords */}
					{item.ai_keywords && item.ai_keywords.length > 0 && (
						<div className="mt-1.5 flex flex-wrap gap-1">
							{item.ai_keywords.slice(0, 5).map((kw) => (
								<Badge
									key={kw}
									variant="secondary"
									className="text-[10px] px-1.5 py-0"
								>
									{kw}
								</Badge>
							))}
							{item.ai_keywords.length > 5 && (
								<span className="text-[10px] text-muted-foreground">
									+{item.ai_keywords.length - 5}
								</span>
							)}
						</div>
					)}

					<div className="mt-1.5 flex items-center gap-2">
						<span className="text-xs text-muted-foreground">
							{item.external_id}
						</span>
						{typeof item.upvotes === "number" && (
							<Badge variant="outline" className="text-[10px]">
								{item.upvotes} {t("feed.upvotes")}
							</Badge>
						)}
						{typeof item.github_stars === "number" && item.github_stars > 0 && (
							<Badge variant="outline" className="text-[10px]">
								<Star className="mr-0.5 h-2.5 w-2.5" />
								{item.github_stars}
							</Badge>
						)}
						{item.published_at && (
							<span className="text-xs text-muted-foreground">
								{new Date(item.published_at).toLocaleDateString()}
							</span>
						)}
					</div>
				</div>

				{/* biome-ignore lint: click handler is for event interception only */}
				<div
					className="flex items-center gap-1 shrink-0"
					onClick={(e) => e.stopPropagation()}
				>
					{item.github_repo && (
						<Button variant="ghost" size="icon" asChild>
							<a
								href={item.github_repo}
								target="_blank"
								rel="noopener noreferrer"
							>
								<Github className="h-4 w-4" />
							</a>
						</Button>
					)}
					<Button variant="ghost" size="icon" asChild>
						<a href={arxivUrl} target="_blank" rel="noopener noreferrer">
							<ExternalLink className="h-4 w-4" />
						</a>
					</Button>
					{item.added_to_library ? (
						<Button variant="ghost" size="sm" disabled>
							<Check className="mr-1 h-4 w-4" /> {t("feed.added")}
						</Button>
					) : (
						<Button
							variant="outline"
							size="sm"
							onClick={() => onAddToLibrary(item.id)}
						>
							<Plus className="mr-1 h-4 w-4" /> {t("feed.add")}
						</Button>
					)}
				</div>
			</div>
		</div>
	);
}
