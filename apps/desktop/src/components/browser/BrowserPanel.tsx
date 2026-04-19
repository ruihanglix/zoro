// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { NewTabPage } from "@/components/browser/NewTabPage";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import * as commands from "@/lib/commands";
import { cn } from "@/lib/utils";
import { loadSetting, saveSetting } from "@/stores/uiStore";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-shell";
import {
	ArrowLeft,
	ArrowRight,
	ExternalLink,
	FileText,
	Globe,
	Plus,
	RefreshCw,
	X,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

interface BrowserTab {
	id: string;
	url: string;
	title: string;
	favicon: string;
}

interface BrowserTabsState {
	tabs: BrowserTab[];
	activeTabId: string | null;
}

interface BrowserPanelProps {
	storageKey: string;
	isActive: boolean;
	paperId?: string;
}

export function BrowserPanel({ storageKey, isActive, paperId }: BrowserPanelProps) {
	const { t } = useTranslation();
	const lsKey = `zoro-browser-tabs-${storageKey}`;

	// Load persisted state
	const [tabs, setTabs] = useState<BrowserTab[]>(() => {
		const saved = loadSetting<BrowserTabsState>(lsKey, {
			tabs: [],
			activeTabId: null,
		});
		return saved.tabs;
	});
	const [activeTabId, setActiveTabId] = useState<string | null>(() => {
		const saved = loadSetting<BrowserTabsState>(lsKey, {
			tabs: [],
			activeTabId: null,
		});
		return saved.activeTabId;
	});

	// URL bar editing state
	const [urlBarValue, setUrlBarValue] = useState("");
	const [isEditingUrl, setIsEditingUrl] = useState(false);

	// Track created webviews
	const createdWebviews = useRef<Set<string>>(new Set());
	const containerRefs = useRef<Map<string, HTMLDivElement>>(new Map());
	const resizeObservers = useRef<Map<string, ResizeObserver>>(new Map());
	const panelRef = useRef<HTMLDivElement>(null);
	// Keep refs so async callbacks can read the latest values
	const isActiveRef = useRef(isActive);
	isActiveRef.current = isActive;
	const tabsRef = useRef(tabs);
	tabsRef.current = tabs;

	const activeTab = tabs.find((tab) => tab.id === activeTabId) ?? null;

	// Persist tabs state
	useEffect(() => {
		saveSetting(lsKey, { tabs, activeTabId });
	}, [tabs, activeTabId, lsKey]);

	// Update URL bar when active tab changes
	useEffect(() => {
		if (activeTab) {
			setUrlBarValue(activeTab.url);
			setIsEditingUrl(false);
		}
	}, [activeTab?.url, activeTab?.id]);

	// Helper: get webview label for a tab
	const getLabel = useCallback(
		(tabId: string) => `browser-panel-${storageKey}-${tabId}`,
		[storageKey],
	);

	// Listen for navigation events to update tab URLs
	useEffect(() => {
		const unlisten = listen<{ label: string; url: string }>(
			"browser-navigation",
			(event) => {
				const { label, url } = event.payload;
				setTabs((prev) =>
					prev.map((tab) => {
						if (getLabel(tab.id) === label) {
							if (tab.url === url) return tab;
							return { ...tab, url };
						}
						return tab;
					}),
				);
				// Update URL bar if this is the active tab
				setActiveTabId((currentActiveId) => {
					const matchingTab = tabsRef.current.find(
						(tab) => getLabel(tab.id) === label,
					);
					if (matchingTab && matchingTab.id === currentActiveId) {
						setUrlBarValue(url);
					}
					return currentActiveId;
				});
			},
		);
		return () => {
			unlisten.then((fn) => fn());
		};
	}, [getLabel]);

	// Listen for page info events to update tab title and favicon
	useEffect(() => {
		const unlisten = listen<{ label: string; title: string; favicon: string }>(
			"browser-page-info",
			(event) => {
				const { label, title, favicon } = event.payload;
				setTabs((prev) =>
					prev.map((tab) => {
						if (getLabel(tab.id) === label) {
							const newTitle = title || tab.title;
							const newFavicon = favicon || tab.favicon;
							if (tab.title === newTitle && tab.favicon === newFavicon) return tab;
							return { ...tab, title: newTitle, favicon: newFavicon };
						}
						return tab;
					}),
				);
			},
		);
		return () => {
			unlisten.then((fn) => fn());
		};
	}, [getLabel]);

	// Sync a single webview's position/size to its container
	const syncWebviewPosition = useCallback(
		(tabId: string) => {
			const label = getLabel(tabId);
			if (!createdWebviews.current.has(label)) return;
			const container = containerRefs.current.get(tabId);
			if (!container) return;
			const rect = container.getBoundingClientRect();
			if (rect.width > 0 && rect.height > 0) {
				commands
					.resizeBrowserWebview(
						label,
						rect.left,
						rect.top,
						rect.width,
						rect.height,
					)
					.catch(() => {});
			}
		},
		[getLabel],
	);

	// Show/hide all webviews based on panel visibility
	useEffect(() => {
		const currentTabs = tabsRef.current;
		if (!isActive) {
			// Hide all this panel's webviews
			for (const tab of currentTabs) {
				const label = getLabel(tab.id);
				commands.hideBrowserWebview(label).catch(() => {});
			}
			return;
		}

		// Panel is active — hide all webviews first (clean slate), then show the active one
		commands.hideAllBrowserWebviews().then(() => {
			// Guard: panel might have become inactive while awaiting
			if (!isActiveRef.current) return;

			const latestTabs = tabsRef.current;
			for (const tab of latestTabs) {
				const label = getLabel(tab.id);
				if (tab.id === activeTabId && createdWebviews.current.has(label)) {
					const container = containerRefs.current.get(tab.id);
					const rect = container?.getBoundingClientRect();
					if (rect && rect.width > 0 && rect.height > 0) {
						commands.showBrowserWebview(label).catch(() => {});
						commands
							.resizeBrowserWebview(
								label,
								rect.left,
								rect.top,
								rect.width,
								rect.height,
							)
							.catch(() => {});
					}
				}
			}
		}).catch(() => {});
	}, [isActive, activeTabId, getLabel]);

	// Re-sync webview position on window resize (ResizeObserver misses position-only changes)
	useEffect(() => {
		if (!isActive || !activeTabId) return;
		const handleResize = () => {
			syncWebviewPosition(activeTabId);
		};
		window.addEventListener("resize", handleResize);
		return () => window.removeEventListener("resize", handleResize);
	}, [isActive, activeTabId, syncWebviewPosition]);

	// Observe the outer panel for size changes (catches sidebar panel resizing)
	useEffect(() => {
		if (!isActive || !activeTabId) return;
		const panel = panelRef.current;
		if (!panel) return;
		const observer = new ResizeObserver(() => {
			syncWebviewPosition(activeTabId);
		});
		observer.observe(panel);
		return () => observer.disconnect();
	}, [isActive, activeTabId, syncWebviewPosition]);

	// Cleanup all webviews on unmount
	useEffect(() => {
		return () => {
			for (const label of createdWebviews.current) {
				commands.closeBrowserWebview(label).catch(() => {});
			}
			createdWebviews.current.clear();
			for (const observer of resizeObservers.current.values()) {
				observer.disconnect();
			}
			resizeObservers.current.clear();
		};
	}, []);

	// Create webview for a tab
	const createWebviewForTab = useCallback(
		(tabId: string, url: string) => {
			const label = getLabel(tabId);
			if (createdWebviews.current.has(label)) return;

			const tryCreate = () => {
				const container = containerRefs.current.get(tabId);
				if (!container) {
					requestAnimationFrame(tryCreate);
					return;
				}
				const rect = container.getBoundingClientRect();
				if (rect.width <= 0 || rect.height <= 0) {
					requestAnimationFrame(tryCreate);
					return;
				}

				createdWebviews.current.add(label);
				commands
					.createBrowserWebview(
						label,
						url,
						rect.left,
						rect.top,
						rect.width,
						rect.height,
					)
					.then(() => {
						// After creation, sync visibility with current panel state.
						// The webview is created visible by default, so hide it if
						// the panel is no longer active or this isn't the active tab.
						if (!isActiveRef.current) {
							commands.hideBrowserWebview(label).catch(() => {});
						}
					})
					.catch(() => {
						createdWebviews.current.delete(label);
					});

				// Setup resize observer
				const observer = new ResizeObserver(() => {
					if (!createdWebviews.current.has(label)) return;
					if (!isActiveRef.current) return;
					const r = container.getBoundingClientRect();
					if (r.width > 0 && r.height > 0) {
						commands
							.resizeBrowserWebview(
								label,
								r.left,
								r.top,
								r.width,
								r.height,
							)
							.catch(() => {});
					}
				});
				observer.observe(container);
				resizeObservers.current.set(tabId, observer);
			};

			requestAnimationFrame(tryCreate);
		},
		[getLabel],
	);

	// Destroy webview for a tab
	const destroyWebviewForTab = useCallback(
		(tabId: string) => {
			const label = getLabel(tabId);
			if (createdWebviews.current.has(label)) {
				commands.closeBrowserWebview(label).catch(() => {});
				createdWebviews.current.delete(label);
			}
			const observer = resizeObservers.current.get(tabId);
			if (observer) {
				observer.disconnect();
				resizeObservers.current.delete(tabId);
			}
		},
		[getLabel],
	);

	// Open a new tab
	const openNewTab = useCallback(
		(url?: string) => {
			const id = `bt-${Date.now()}`;
			if (url) {
				const newTab: BrowserTab = { id, url, title: extractTitle(url), favicon: "" };
				setTabs((prev) => [...prev, newTab]);
				setActiveTabId(id);
			} else {
				// Show new tab page (no URL)
				const newTab: BrowserTab = { id, url: "", title: t("browser.newTab"), favicon: "" };
				setTabs((prev) => [...prev, newTab]);
				setActiveTabId(id);
			}
		},
		[t],
	);

	// Navigate current tab to a URL
	const navigateActiveTab = useCallback(
		(url: string) => {
			if (!activeTabId) return;
			const label = getLabel(activeTabId);

			setTabs((prev) =>
				prev.map((tab) =>
					tab.id === activeTabId
						? { ...tab, url, title: extractTitle(url), favicon: "" }
						: tab,
				),
			);

			if (createdWebviews.current.has(label)) {
				// Navigate existing webview
				commands.browserNavigate(label, url).catch(() => {});
			} else {
				// Create new webview
				createWebviewForTab(activeTabId, url);
			}
		},
		[activeTabId, getLabel, createWebviewForTab],
	);

	// Handle new tab page navigation (create webview or navigate)
	const handleNewTabNavigate = useCallback(
		(url: string) => {
			if (activeTabId) {
				navigateActiveTab(url);
			} else {
				openNewTab(url);
			}
		},
		[activeTabId, navigateActiveTab, openNewTab],
	);

	// Switch to a tab
	const switchTab = useCallback(
		(tabId: string) => {
			setActiveTabId(tabId);
		},
		[],
	);

	// Close a tab
	const closeTab = useCallback(
		(tabId: string) => {
			destroyWebviewForTab(tabId);
			setTabs((prev) => {
				const newTabs = prev.filter((t) => t.id !== tabId);
				if (activeTabId === tabId) {
					const idx = prev.findIndex((t) => t.id === tabId);
					const nextActive =
						newTabs[Math.min(idx, newTabs.length - 1)] ?? null;
					setActiveTabId(nextActive?.id ?? null);
				}
				return newTabs;
			});
		},
		[activeTabId, destroyWebviewForTab],
	);

	// Navigation handlers
	const handleGoBack = useCallback(() => {
		if (!activeTabId) return;
		commands.browserGoBack(getLabel(activeTabId)).catch(() => {});
	}, [activeTabId, getLabel]);

	const handleGoForward = useCallback(() => {
		if (!activeTabId) return;
		commands.browserGoForward(getLabel(activeTabId)).catch(() => {});
	}, [activeTabId, getLabel]);

	const handleReload = useCallback(() => {
		if (!activeTabId) return;
		commands.browserReload(getLabel(activeTabId)).catch(() => {});
	}, [activeTabId, getLabel]);

	const handleOpenExternal = useCallback(() => {
		if (activeTab?.url) {
			open(activeTab.url);
		}
	}, [activeTab]);

	const handleUrlBarSubmit = useCallback(() => {
		let url = urlBarValue.trim();
		if (!url) return;
		if (!/^https?:\/\//i.test(url)) {
			url = `https://${url}`;
		}
		navigateActiveTab(url);
		setIsEditingUrl(false);
	}, [urlBarValue, navigateActiveTab]);

	// Set container ref for a tab's webview placeholder
	const setContainerRef = useCallback(
		(tabId: string, el: HTMLDivElement | null) => {
			if (el) {
				containerRefs.current.set(tabId, el);
			} else {
				containerRefs.current.delete(tabId);
			}
		},
		[],
	);

	// Create webview when a tab with a URL becomes active
	useEffect(() => {
		if (!activeTabId || !isActive) return;
		const tab = tabs.find((t) => t.id === activeTabId);
		if (!tab || !tab.url) return;

		const label = getLabel(activeTabId);
		if (!createdWebviews.current.has(label)) {
			createWebviewForTab(activeTabId, tab.url);
		}
	}, [activeTabId, isActive, tabs, getLabel, createWebviewForTab]);

	const tabScrollRef = useRef<HTMLDivElement>(null);

	// Convert vertical scroll to horizontal scroll on the tab bar
	const handleTabWheel = useCallback((e: React.WheelEvent) => {
		if (!tabScrollRef.current) return;
		if (e.deltaY !== 0) {
			e.preventDefault();
			tabScrollRef.current.scrollLeft += e.deltaY;
		}
	}, []);

	const showNewTabPage = !activeTab || !activeTab.url;

	return (
		<div ref={panelRef} className="flex h-full w-full flex-col overflow-hidden">
			{/* Tab bar */}
			<div className="flex items-center border-b bg-muted/30 shrink-0">
				<div
					ref={tabScrollRef}
					className="flex flex-1 items-center overflow-x-auto scrollbar-none min-w-0"
					onWheel={handleTabWheel}
				>
					{tabs.map((tab) => (
						<div
							key={tab.id}
							className={cn(
								"group flex items-center gap-1.5 border-r px-2 py-1.5 text-xs cursor-pointer shrink-0 max-w-[160px]",
								tab.id === activeTabId
									? "bg-background text-foreground"
									: "text-muted-foreground hover:bg-accent/30",
							)}
							onClick={() => switchTab(tab.id)}
							onKeyDown={(e) => {
								if (e.key === "Enter") switchTab(tab.id);
							}}
						>
							<TabFavicon url={tab.favicon} />
							<span className="truncate min-w-0">
								{tab.title || t("browser.newTab")}
							</span>
							<button
								type="button"
								className="shrink-0 rounded p-0.5 opacity-0 group-hover:opacity-100 hover:bg-accent"
								onClick={(e) => {
									e.stopPropagation();
									closeTab(tab.id);
								}}
								title={t("browser.closeTab")}
							>
								<X className="h-3 w-3" />
							</button>
						</div>
					))}
				</div>
				<button
					type="button"
					className="shrink-0 p-1.5 text-muted-foreground hover:text-foreground hover:bg-accent/30 transition-colors"
					onClick={() => openNewTab()}
					title={t("browser.newTab")}
				>
					<Plus className="h-3.5 w-3.5" />
				</button>
				{paperId && !showNewTabPage && (
					<button
						type="button"
						className="shrink-0 p-1.5 text-muted-foreground hover:text-foreground hover:bg-accent/30 transition-colors"
						onClick={async () => {
							try {
								await commands.copyPaperPdfToClipboard(paperId);
							} catch {
								// PDF not available
							}
						}}
						title={t("browser.copyPaperPdf")}
					>
						<FileText className="h-3.5 w-3.5" />
					</button>
				)}
			</div>

			{/* Address bar + navigation (only when a tab is active with a URL) */}
			{!showNewTabPage && (
				<div className="flex items-center gap-1 border-b px-2 py-1 bg-muted/20 shrink-0">
					<Button
						variant="ghost"
						size="icon"
						className="h-6 w-6"
						onClick={handleGoBack}
						title={t("common.back")}
					>
						<ArrowLeft className="h-3 w-3" />
					</Button>
					<Button
						variant="ghost"
						size="icon"
						className="h-6 w-6"
						onClick={handleGoForward}
						title={t("common.forward")}
					>
						<ArrowRight className="h-3 w-3" />
					</Button>
					<Button
						variant="ghost"
						size="icon"
						className="h-6 w-6"
						onClick={handleReload}
						title={t("common.refresh")}
					>
						<RefreshCw className="h-3 w-3" />
					</Button>

					{isEditingUrl ? (
						<Input
							value={urlBarValue}
							onChange={(e) => setUrlBarValue(e.target.value)}
							onKeyDown={(e) => {
								if (e.key === "Enter") handleUrlBarSubmit();
								if (e.key === "Escape") setIsEditingUrl(false);
							}}
							onBlur={() => setIsEditingUrl(false)}
							className="flex-1 h-6 text-xs min-w-0 mx-1"
							autoFocus
						/>
					) : (
						<div
							className="flex-1 min-w-0 rounded border bg-background px-2 py-0.5 text-xs text-muted-foreground truncate mx-1 cursor-text h-6 flex items-center"
							onClick={() => {
								setUrlBarValue(activeTab?.url ?? "");
								setIsEditingUrl(true);
							}}
							onKeyDown={(e) => {
								if (e.key === "Enter") setIsEditingUrl(true);
							}}
						>
							{activeTab?.url}
						</div>
					)}

					<Button
						variant="ghost"
						size="icon"
						className="h-6 w-6 shrink-0"
						onClick={handleOpenExternal}
						title={t("common.openInBrowser")}
					>
						<ExternalLink className="h-3 w-3" />
					</Button>
				</div>
			)}

			{/* Content area */}
			<div className="flex-1 relative min-h-0">
				{showNewTabPage ? (
					<NewTabPage onNavigate={handleNewTabNavigate} />
				) : (
					tabs.map((tab) => (
						<div
							key={tab.id}
							ref={(el) => setContainerRef(tab.id, el)}
							className={cn(
								"absolute inset-0",
								tab.id === activeTabId && tab.url
									? "visible"
									: "invisible pointer-events-none",
							)}
						/>
					))
				)}
			</div>
		</div>
	);
}

function extractTitle(url: string): string {
	try {
		const hostname = new URL(url).hostname;
		return hostname.replace(/^www\./, "");
	} catch {
		return url;
	}
}

function TabFavicon({ url }: { url: string }) {
	const [failed, setFailed] = useState(false);

	// Reset failed state when url changes
	useEffect(() => {
		setFailed(false);
	}, [url]);

	if (!url || failed) {
		return <Globe className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />;
	}

	return (
		<img
			src={url}
			alt=""
			className="h-3.5 w-3.5 shrink-0 rounded-sm"
			onError={() => setFailed(true)}
		/>
	);
}
