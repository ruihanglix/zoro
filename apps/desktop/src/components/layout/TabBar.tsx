// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuLabel,
	DropdownMenuSeparator,
	DropdownMenuShortcut,
	DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { cn } from "@/lib/utils";
import { useTabStore } from "@/stores/tabStore";
import type { Tab } from "@/stores/tabStore";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
	Bot,
	FileText,
	Library,
	Menu,
	Minus,
	Settings,
	Square,
	StickyNote,
	X,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

const isMacOS = /Mac/.test(navigator.platform);

/**
 * Start window dragging on mousedown.
 *
 * We call `getCurrentWindow().startDragging()` explicitly instead of relying
 * solely on `data-tauri-drag-region`.  The attribute-based approach breaks
 * after interacting with the app because `react-resizable-panels` registers a
 * capturing `pointerdown` listener on the document that calls
 * `e.preventDefault()` + `element.focus()`, which corrupts the native event
 * chain Tauri needs for automatic drag-region detection.
 */
function handleDragRegionMouseDown(e: React.MouseEvent) {
	if (e.button !== 0) return;
	e.preventDefault();
	getCurrentWindow().startDragging();
}

export function TabBar() {
	const tabs = useTabStore((s) => s.tabs);
	const activeTabId = useTabStore((s) => s.activeTabId);
	const setActiveTab = useTabStore((s) => s.setActiveTab);
	const closeTab = useTabStore((s) => s.closeTab);
	const scrollRef = useRef<HTMLDivElement>(null);
	const rootRef = useRef<HTMLDivElement>(null);
	const [isFullscreen, setIsFullscreen] = useState(false);
	const [isMaximized, setIsMaximized] = useState(false);

	useEffect(() => {
		const appWindow = getCurrentWindow();
		let unlisten: (() => void) | undefined;

		appWindow.isFullscreen().then(setIsFullscreen);

		if (!isMacOS) {
			appWindow.isMaximized().then(setIsMaximized);
		}

		appWindow
			.onResized(() => {
				appWindow.isFullscreen().then(setIsFullscreen);
				if (!isMacOS) {
					appWindow.isMaximized().then(setIsMaximized);
				}
			})
			.then((fn) => {
				unlisten = fn;
			});

		return () => {
			unlisten?.();
		};
	}, []);

	// Convert vertical scroll to horizontal scroll on the tab container
	const handleWheel = useCallback((e: React.WheelEvent) => {
		if (!scrollRef.current) return;
		// Use deltaY for horizontal scrolling (mouse wheel / trackpad vertical gesture)
		if (e.deltaY !== 0) {
			e.preventDefault();
			scrollRef.current.scrollLeft += e.deltaY;
		}
	}, []);

	return (
		<div
			ref={rootRef}
			className="tab-bar-root flex items-stretch shrink-0 border-b bg-muted/40 select-none"
		>
			{isMacOS ? (
				!isFullscreen && (
					<div
						className="w-[78px] shrink-0 h-full"
						onMouseDown={handleDragRegionMouseDown}
					/>
				)
			) : (
				<AppMenu />
			)}

			{/* Tabs — horizontally scrollable via wheel/trackpad.
				 Empty space in this container also acts as a drag region. */}
			<div
				ref={scrollRef}
				className="flex items-stretch min-w-0 flex-1 overflow-x-auto scrollbar-none h-full"
				onWheel={handleWheel}
				onMouseDown={(e) => {
					if (e.target === e.currentTarget) {
						handleDragRegionMouseDown(e);
					}
				}}
			>
				{tabs.map((tab) => (
					<TabItem
						key={tab.id}
						tab={tab}
						active={tab.id === activeTabId}
						onActivate={() => setActiveTab(tab.id)}
						onClose={() => closeTab(tab.id)}
					/>
				))}
			</div>

			{isMacOS ? (
				<div
					className="shrink-0 w-[40px] h-full"
					onMouseDown={handleDragRegionMouseDown}
				/>
			) : (
				<>
					{/* Drag region gap before window controls */}
					<div
						className="shrink-0 w-[40px] h-full"
						onMouseDown={handleDragRegionMouseDown}
					/>
					<WindowControls isMaximized={isMaximized} />
				</>
			)}
		</div>
	);
}

function AppMenu() {
	const { t } = useTranslation();
	return (
		<DropdownMenu>
			<DropdownMenuTrigger asChild>
				<button
					type="button"
					className="flex items-center justify-center w-[46px] shrink-0 h-full transition-colors hover:bg-accent/60 cursor-pointer"
				>
					<Menu className="h-4 w-4" />
				</button>
			</DropdownMenuTrigger>
			<DropdownMenuContent
				align="start"
				sideOffset={2}
				className="min-w-[200px]"
			>
				<DropdownMenuLabel>{t("tabBar.edit")}</DropdownMenuLabel>
				<DropdownMenuSeparator />
				<DropdownMenuItem onSelect={() => document.execCommand("undo")}>
					{t("tabBar.undo")}
					<DropdownMenuShortcut>Ctrl+Z</DropdownMenuShortcut>
				</DropdownMenuItem>
				<DropdownMenuItem onSelect={() => document.execCommand("redo")}>
					{t("tabBar.redo")}
					<DropdownMenuShortcut>Ctrl+Y</DropdownMenuShortcut>
				</DropdownMenuItem>
				<DropdownMenuSeparator />
				<DropdownMenuItem onSelect={() => document.execCommand("cut")}>
					{t("tabBar.cut")}
					<DropdownMenuShortcut>Ctrl+X</DropdownMenuShortcut>
				</DropdownMenuItem>
				<DropdownMenuItem onSelect={() => document.execCommand("copy")}>
					{t("tabBar.copy")}
					<DropdownMenuShortcut>Ctrl+C</DropdownMenuShortcut>
				</DropdownMenuItem>
				<DropdownMenuItem onSelect={() => document.execCommand("paste")}>
					{t("tabBar.paste")}
					<DropdownMenuShortcut>Ctrl+V</DropdownMenuShortcut>
				</DropdownMenuItem>
				<DropdownMenuSeparator />
				<DropdownMenuItem onSelect={() => document.execCommand("selectAll")}>
					{t("tabBar.selectAll")}
					<DropdownMenuShortcut>Ctrl+A</DropdownMenuShortcut>
				</DropdownMenuItem>
			</DropdownMenuContent>
		</DropdownMenu>
	);
}

function WindowControls({ isMaximized }: { isMaximized: boolean }) {
	const { t } = useTranslation();
	const appWindow = getCurrentWindow();

	return (
		<div className="flex items-stretch shrink-0 h-full">
			<button
				type="button"
				className="flex items-center justify-center w-[46px] h-full transition-colors hover:bg-foreground/10 cursor-pointer"
				onClick={() => appWindow.minimize()}
				title={t("tabBar.minimize")}
			>
				<Minus className="h-4 w-4" />
			</button>
			<button
				type="button"
				className="flex items-center justify-center w-[46px] h-full transition-colors hover:bg-foreground/10 cursor-pointer"
				onClick={() => appWindow.toggleMaximize()}
				title={isMaximized ? t("tabBar.restore") : t("tabBar.maximize")}
			>
				{isMaximized ? (
					<svg
						className="h-3.5 w-3.5"
						viewBox="0 0 16 16"
						fill="none"
						stroke="currentColor"
						strokeWidth="1.5"
						aria-hidden="true"
					>
						<rect x="3" y="5" width="10" height="10" rx="1" />
						<path d="M5 5V3a1 1 0 0 1 1-1h8a1 1 0 0 1 1 1v8a1 1 0 0 1-1 1h-2" />
					</svg>
				) : (
					<Square className="h-3.5 w-3.5" />
				)}
			</button>
			<button
				type="button"
				className="flex items-center justify-center w-[46px] h-full transition-colors hover:bg-red-500 hover:text-white cursor-pointer"
				onClick={() => appWindow.close()}
				title={t("tabBar.closeWindow")}
			>
				<X className="h-4 w-4" />
			</button>
		</div>
	);
}

function TabItem({
	tab,
	active,
	onActivate,
	onClose,
}: {
	tab: Tab;
	active: boolean;
	onActivate: () => void;
	onClose: () => void;
}) {
	const { t } = useTranslation();
	const isAgent = tab.type === "agent";
	const isHome = tab.type === "home";
	const isSettings = tab.type === "settings";
	const isNote = tab.type === "note";
	const isFeedReader = !!tab.feedItem && !tab.paperId;

	const closable = !isHome && !isAgent;

	const handleMouseDown = (e: React.MouseEvent) => {
		if (e.button === 1 && closable) {
			e.preventDefault();
			onClose();
		}
	};

	const handleCloseClick = (e: React.MouseEvent) => {
		e.stopPropagation();
		onClose();
	};

	if (isAgent) {
		return (
			<button
				type="button"
				role="tab"
				className={cn(
					"tab-item flex items-center justify-center px-2.5 h-full shrink-0 transition-colors border-r border-border/50 cursor-pointer",
					active
						? "bg-background text-primary rounded-t-md border-t-2 border-t-primary"
						: "text-muted-foreground hover:bg-accent/40 hover:text-foreground",
				)}
				onClick={onActivate}
				title={t("tabBar.agent")}
			>
				<Bot className="h-4 w-4" />
			</button>
		);
	}

	return (
		<div
			role="tab"
			className={cn(
				"tab-item group relative flex items-center gap-1.5 px-3 h-full text-xs shrink-0 max-w-[200px] transition-colors border-r border-border/50 cursor-pointer",
				active
					? "bg-background text-foreground rounded-t-md border-t-2 border-t-primary"
					: "bg-transparent text-muted-foreground hover:bg-accent/40 hover:text-foreground",
			)}
			onClick={onActivate}
			onMouseDown={handleMouseDown}
			title={tab.title}
		>
			{/* Icon */}
			{isHome ? (
				<Library className="h-3.5 w-3.5 shrink-0" />
			) : isSettings ? (
				<Settings className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
			) : isNote ? (
				<StickyNote className="h-3.5 w-3.5 shrink-0 text-primary" />
			) : isFeedReader ? (
				<FileText className="h-3.5 w-3.5 shrink-0 text-orange-500" />
			) : (
				<FileText className="h-3.5 w-3.5 shrink-0 text-blue-500" />
			)}

			{/* Title */}
			<span className="truncate min-w-0 text-left">{tab.title}</span>

			{/* Close button */}
			{closable && (
				<button
					type="button"
					className={cn(
						"shrink-0 rounded-sm p-0.5 transition-colors",
						"opacity-0 group-hover:opacity-100",
						active && "opacity-60",
						"hover:bg-foreground/10",
					)}
					onClick={handleCloseClick}
					tabIndex={-1}
				>
					<X className="h-3 w-3" />
				</button>
			)}
		</div>
	);
}
