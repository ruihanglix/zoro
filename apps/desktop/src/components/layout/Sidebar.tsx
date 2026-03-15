// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import {
	CollectionTreeItem,
	buildCollectionTree,
} from "@/components/library/CollectionTree";
import {
	ContextMenu,
	ContextMenuContent,
	ContextMenuItem,
	ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import { registerDropTarget, unregisterDropTarget } from "@/lib/dragState";
import { cn } from "@/lib/utils";
import { usePluginStore } from "@/plugins/pluginStore";
import { useLibraryStore } from "@/stores/libraryStore";
import { useTabStore } from "@/stores/tabStore";
import { useUiStore } from "@/stores/uiStore";
import {
	BookOpen,
	ChevronDown,
	ChevronRight,
	Folder,
	FolderPlus,
	Hash,
	Inbox,
	Puzzle,
	Rss,
	Trash2,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

export function Sidebar() {
	const { t } = useTranslation();
	const collections = useLibraryStore((s) => s.collections);
	const tags = useLibraryStore((s) => s.tags);
	const subscriptions = useLibraryStore((s) => s.subscriptions);
	const uncategorizedCount = useLibraryStore((s) => s.uncategorizedCount);
	const currentCollectionId = useLibraryStore((s) => s.currentCollectionId);
	const currentTagName = useLibraryStore((s) => s.currentTagName);
	const currentUncategorized = useLibraryStore((s) => s.currentUncategorized);
	const setCurrentCollection = useLibraryStore((s) => s.setCurrentCollection);
	const setCurrentTag = useLibraryStore((s) => s.setCurrentTag);
	const setCurrentUncategorized = useLibraryStore(
		(s) => s.setCurrentUncategorized,
	);
	const createCollection = useLibraryStore((s) => s.createCollection);
	const addTagToPaper = useLibraryStore((s) => s.addTagToPaper);
	const deleteTag = useLibraryStore((s) => s.deleteTag);
	const view = useUiStore((s) => s.view);
	const setView = useUiStore((s) => s.setView);
	const showUncategorized = useUiStore((s) => s.showUncategorized);
	const setActiveTab = useTabStore((s) => s.setActiveTab);

	const [libraryExpanded, setLibraryExpanded] = useState(true);
	const [subsExpanded, setSubsExpanded] = useState(true);
	const [tagsExpanded, setTagsExpanded] = useState(true);
	const [pluginsExpanded, setPluginsExpanded] = useState(true);
	const [creatingCollection, setCreatingCollection] = useState(false);
	const newCollectionInputRef = useRef<HTMLInputElement>(null);
	const collectionBlurGuardRef = useRef(false);

	const collectionTree = buildCollectionTree(collections);

	const isLibraryActive =
		view === "library" && !currentTagName && !currentUncategorized;
	const isUncategorizedActive = currentUncategorized;

	const handleAddCollection = () => {
		setLibraryExpanded(true);
		setCreatingCollection(true);
		collectionBlurGuardRef.current = true;
		setTimeout(() => {
			newCollectionInputRef.current?.focus();
			setTimeout(() => {
				collectionBlurGuardRef.current = false;
			}, 150);
		}, 50);
	};

	const handleNewCollectionSubmit = (value: string) => {
		setCreatingCollection(false);
		if (value.trim()) {
			createCollection(value.trim());
		}
	};

	const handleNewCollectionBlur = (e: React.FocusEvent<HTMLInputElement>) => {
		if (collectionBlurGuardRef.current) {
			e.currentTarget.focus();
			return;
		}
		handleNewCollectionSubmit(e.currentTarget.value);
	};

	/** Navigate to the Home tab and set the appropriate view. */
	const navigateHome = (
		viewName: "library" | "feed" | "papers-cool" | "plugins",
	) => {
		setActiveTab("home");
		setView(viewName);
	};

	return (
		<div className="flex h-full flex-col bg-muted/30 select-none">
			{/* Header */}
			<div className="flex h-14 items-center gap-2 border-b px-4">
				<BookOpen className="h-5 w-5 text-primary" />
				<span className="text-base font-bold">Zoro</span>
			</div>

			<ScrollArea className="flex-1">
				<div className="px-1 py-2">
					{/* ===== My Library ===== */}
					<ContextMenu>
						<ContextMenuTrigger asChild>
							<div>
								<SectionHeader
									label={t("sidebar.myLibrary")}
									expanded={libraryExpanded}
									onToggle={() => setLibraryExpanded(!libraryExpanded)}
									onLabelClick={() => {
										navigateHome("library");
										setCurrentCollection(null);
									}}
									active={isLibraryActive && !currentCollectionId}
								/>
							</div>
						</ContextMenuTrigger>
						<ContextMenuContent className="w-48">
							<ContextMenuItem onSelect={handleAddCollection}>
								<FolderPlus className="mr-2 h-4 w-4" />
								{t("sidebar.newCollection")}
							</ContextMenuItem>
						</ContextMenuContent>
					</ContextMenu>

					{libraryExpanded && (
						<div className="ml-0.5">
							{/* Collection tree */}
							{collectionTree.map((node) => (
								<CollectionTreeItem
									key={node.collection.id}
									node={node}
									depth={1}
									selectedId={currentCollectionId}
									onSelect={(id) => {
										navigateHome("library");
										setCurrentCollection(id);
									}}
								/>
							))}

							{/* Inline input for new collection */}
							{creatingCollection && (
								<div
									className="flex items-center gap-1 px-1.5 py-1"
									style={{ paddingLeft: "18px" }}
								>
									<span className="w-[18px] shrink-0" />
									<Folder className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
									<input
										ref={newCollectionInputRef}
										type="text"
										className="flex-1 min-w-0 bg-transparent text-sm border-b border-primary outline-none py-0.5"
										placeholder={t("sidebar.collectionNamePlaceholder")}
										onKeyDown={(e) => {
											if (e.key === "Enter") {
												handleNewCollectionSubmit(e.currentTarget.value);
											} else if (e.key === "Escape") {
												setCreatingCollection(false);
											}
										}}
										onBlur={handleNewCollectionBlur}
									/>
								</div>
							)}

							{/* Uncategorized */}
							{showUncategorized && uncategorizedCount > 0 && (
								<button
									type="button"
									className={cn(
										"flex w-full items-center gap-1 rounded-sm px-1.5 py-1 text-sm hover:bg-accent/50 transition-colors text-left",
										isUncategorizedActive && "bg-accent text-accent-foreground",
									)}
									style={{ paddingLeft: "18px" }}
									onClick={() => {
										navigateHome("library");
										setCurrentUncategorized(true);
									}}
								>
									<span className="w-[18px] shrink-0" />
									<Inbox className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
									<span className="truncate flex-1 min-w-0">
										{t("sidebar.uncategorized")}
									</span>
									<span className="text-[10px] text-muted-foreground shrink-0 tabular-nums">
										{uncategorizedCount}
									</span>
								</button>
							)}

							{collections.length === 0 && (
								<p className="px-4 py-1.5 text-[11px] text-muted-foreground">
									{t("sidebar.noCollections")}
								</p>
							)}
						</div>
					)}

					{/* ===== Subscriptions ===== */}
					<SectionHeader
						label={t("sidebar.subscriptions")}
						expanded={subsExpanded}
						onToggle={() => setSubsExpanded(!subsExpanded)}
						onLabelClick={() => navigateHome("feed")}
						active={view === "feed" || view === "papers-cool"}
					/>

					{subsExpanded && (
						<div className="ml-0.5">
							{subscriptions.map((sub) => {
								const targetView =
									sub.source_type === "papers-cool" ? "papers-cool" : "feed";
								const isActive = view === targetView;
								return (
									<button
										key={sub.id}
										type="button"
										className={cn(
											"flex w-full items-center gap-1 rounded-sm px-1.5 py-1 text-sm hover:bg-accent/50 transition-colors text-left",
											isActive && "bg-accent text-accent-foreground",
										)}
										style={{ paddingLeft: "18px" }}
										onClick={() =>
											navigateHome(targetView as "feed" | "papers-cool")
										}
									>
										<span className="w-[18px] shrink-0" />
										<Rss className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
										<span className="truncate flex-1 min-w-0">{sub.name}</span>
									</button>
								);
							})}
							{subscriptions.length === 0 && (
								<p className="px-4 py-1.5 text-[11px] text-muted-foreground">
									{t("sidebar.noSubscriptions")}
								</p>
							)}
						</div>
					)}

					{/* ===== Tags ===== */}
					<SectionHeader
						label={t("sidebar.tags")}
						expanded={tagsExpanded}
						onToggle={() => setTagsExpanded(!tagsExpanded)}
					/>

					{tagsExpanded && (
						<div className="ml-0.5">
							{tags.map((tag) => (
								<TagDropItem
									key={tag.id}
									tag={tag}
									isActive={currentTagName === tag.name}
									onClick={() => {
										navigateHome("library");
										setCurrentTag(tag.name);
									}}
									onDelete={() => {
										if (
											confirm(t("sidebar.deleteTagConfirm", { name: tag.name }))
										) {
											deleteTag(tag.id);
										}
									}}
									onDropPaper={(paperId) => addTagToPaper(paperId, tag.name)}
								/>
							))}
							{tags.length === 0 && (
								<p className="px-4 py-1.5 text-[11px] text-muted-foreground">
									{t("sidebar.noTags")}
								</p>
							)}
						</div>
					)}

					{/* ===== Plugins ===== */}
					<SectionHeader
						label={t("sidebar.plugins")}
						expanded={pluginsExpanded}
						onToggle={() => setPluginsExpanded(!pluginsExpanded)}
						onLabelClick={() => navigateHome("plugins")}
						active={view === "plugins"}
						icon={<Puzzle className="h-3.5 w-3.5" />}
					/>

					{pluginsExpanded && (
						<PluginNavItems
							active={view === "plugins"}
							onClick={() => navigateHome("plugins")}
						/>
					)}
				</div>
			</ScrollArea>
		</div>
	);
}

/** A collapsible section header for the sidebar tree. */
function SectionHeader({
	label,
	expanded,
	onToggle,
	onLabelClick,
	active,
	icon,
}: {
	label: string;
	expanded: boolean;
	onToggle: () => void;
	onLabelClick?: () => void;
	active?: boolean;
	icon?: React.ReactNode;
}) {
	return (
		<div
			className={cn(
				"flex items-center gap-1 rounded-sm px-1.5 py-1 mt-1 first:mt-0 hover:bg-accent/30 transition-colors cursor-pointer",
				active && "bg-accent/40",
			)}
		>
			<button type="button" className="shrink-0 p-0.5" onClick={onToggle}>
				{expanded ? (
					<ChevronDown className="h-3.5 w-3.5" />
				) : (
					<ChevronRight className="h-3.5 w-3.5" />
				)}
			</button>
			{icon && <span className="shrink-0 text-muted-foreground">{icon}</span>}
			<span
				className="text-xs font-semibold uppercase tracking-wider text-muted-foreground flex-1 min-w-0 truncate"
				onClick={onLabelClick ?? onToggle}
				onKeyDown={() => {}}
				role="button"
				tabIndex={-1}
			>
				{label}
			</span>
		</div>
	);
}

/** Renders navigation items for plugins that contribute to sidebar_nav. */
function PluginNavItems({
	active,
	onClick,
}: {
	active: boolean;
	onClick: () => void;
}) {
	const { t } = useTranslation();
	const plugins = usePluginStore((s) => s.plugins);
	const loadedModules = usePluginStore((s) => s.loadedModules);

	// Collect all sidebar_nav contributions from all loaded plugins
	const navItems = useMemo(() => {
		const items: Array<{ pluginId: string; id: string; titleKey: string }> = [];
		for (const plugin of plugins) {
			if (!plugin.enabled || !loadedModules[plugin.manifest.id]) continue;
			const navContribs = plugin.manifest.contributions.sidebar_nav_items ?? [];
			for (const item of navContribs) {
				items.push({
					pluginId: plugin.manifest.id,
					id: item.id,
					titleKey:
						(item as unknown as Record<string, string>).titleKey ?? item.id,
				});
			}
		}
		return items;
	}, [plugins, loadedModules]);

	if (navItems.length === 0) {
		return (
			<div className="ml-0.5">
				<p className="px-4 py-1.5 text-[11px] text-muted-foreground">
					{t("sidebar.noPlugins")}
				</p>
			</div>
		);
	}

	return (
		<div className="ml-0.5">
			{navItems.map((item) => (
				<button
					key={`${item.pluginId}-${item.id}`}
					type="button"
					className={cn(
						"flex w-full items-center gap-1 rounded-sm px-1.5 py-1 text-sm hover:bg-accent/50 transition-colors text-left",
						active && "bg-accent text-accent-foreground",
					)}
					style={{ paddingLeft: "18px" }}
					onClick={onClick}
				>
					<span className="w-[18px] shrink-0" />
					<Puzzle className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
					<span className="truncate flex-1 min-w-0">
						{item.titleKey === "knowledgeAssets"
							? t("sidebar.knowledgeAssets")
							: item.titleKey}
					</span>
				</button>
			))}
		</div>
	);
}

/** A single tag item in the sidebar that also acts as a drop target. */
function TagDropItem({
	tag,
	isActive,
	onClick,
	onDelete,
	onDropPaper,
}: {
	tag: { id: string; name: string };
	isActive: boolean;
	onClick: () => void;
	onDelete: () => void;
	onDropPaper: (paperId: string) => Promise<void>;
}) {
	const { t } = useTranslation();
	const buttonRef = useRef<HTMLButtonElement>(null);

	// Register as drop target for paper drag
	useEffect(() => {
		const el = buttonRef.current;
		if (!el) return;
		const targetId = `tag-${tag.id}`;
		registerDropTarget({
			id: targetId,
			type: "tag",
			label: tag.name,
			element: el,
			onDrop: async (paperId: string) => {
				console.log("[Drop] Tag:", tag.name, "paperId:", paperId);
				try {
					await onDropPaper(paperId);
					console.log("[Drop] Successfully added tag:", tag.name);
				} catch (err) {
					console.error("[Drop] Failed to add tag via drag:", err);
				}
			},
		});
		return () => unregisterDropTarget(targetId);
	}, [tag.id, tag.name, onDropPaper]);

	return (
		<ContextMenu>
			<ContextMenuTrigger asChild>
				<button
					ref={buttonRef}
					type="button"
					className={cn(
						"flex w-full items-center gap-1 rounded-sm px-1.5 py-1 text-sm hover:bg-accent/50 transition-colors text-left",
						isActive && "bg-accent text-accent-foreground",
					)}
					style={{ paddingLeft: "18px" }}
					onClick={onClick}
				>
					<span className="w-[18px] shrink-0" />
					<Hash className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
					<span className="truncate flex-1 min-w-0">{tag.name}</span>
				</button>
			</ContextMenuTrigger>
			<ContextMenuContent className="w-40">
				<ContextMenuItem
					onSelect={onDelete}
					className="text-destructive focus:text-destructive"
				>
					<Trash2 className="mr-2 h-4 w-4" />
					{t("sidebar.deleteTag")}
				</ContextMenuItem>
			</ContextMenuContent>
		</ContextMenu>
	);
}
