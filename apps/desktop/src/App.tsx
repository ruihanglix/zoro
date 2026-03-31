// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { BackgroundTaskPanel } from "@/components/BackgroundTaskPanel";
import { LogPanel } from "@/components/LogPanel";
import { OnboardingOverlay } from "@/components/OnboardingOverlay";
import { AgentPanel } from "@/components/agent/AgentPanel";
import { MainLayout } from "@/components/layout/MainLayout";
import { TabBar } from "@/components/layout/TabBar";
import { FileDropZone } from "@/components/library/FileDropZone";
import { MetadataSearchDialog } from "@/components/library/MetadataSearchDialog";
import * as commands from "@/lib/commands";
import { injectPluginSharedDeps } from "@/plugins/PluginSharedDeps";
import { usePluginStore } from "@/plugins/pluginStore";
import { Feed } from "@/routes/feed";
import { Library } from "@/routes/library";
import { StandaloneNoteEditor } from "@/routes/noteEditor";
import { PapersCool } from "@/routes/papersCool";
import { PluginView } from "@/routes/pluginView";
import { Reader } from "@/routes/reader";
import { Settings } from "@/routes/settings";
import { Webview } from "@/routes/webview";
import { useLibraryStore } from "@/stores/libraryStore";
import { useTabStore } from "@/stores/tabStore";
import { useTranslationStore } from "@/stores/translationStore";
import { useUiStore } from "@/stores/uiStore";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

// Inject shared dependencies for plugins before any plugin loads
injectPluginSharedDeps();

export default function App() {
	const { t } = useTranslation();
	const showOnboarding = useUiStore((s) => s.showOnboarding);
	const setShowOnboarding = useUiStore((s) => s.setShowOnboarding);
	const view = useUiStore((s) => s.view);
	const debugMode = useUiStore((s) => s.debugMode);
	const fetchPapers = useLibraryStore((s) => s.fetchPapers);
	const fetchCollections = useLibraryStore((s) => s.fetchCollections);
	const fetchTags = useLibraryStore((s) => s.fetchTags);
	const fetchSubscriptions = useLibraryStore((s) => s.fetchSubscriptions);
	const fetchAiConfig = useTranslationStore((s) => s.fetchAiConfig);
	const fetchPlugins = usePluginStore((s) => s.fetchPlugins);
	const loadAllEnabled = usePluginStore((s) => s.loadAllEnabled);
	const tabs = useTabStore((s) => s.tabs);
	const activeTabId = useTabStore((s) => s.activeTabId);
	const metadataSearchPaperId = useUiStore((s) => s.metadataSearchPaperId);

	// Update notification state
	const [updateAvailable, setUpdateAvailable] = useState<{
		version: string;
		body: string;
	} | null>(null);
	const [updateInstalling, setUpdateInstalling] = useState(false);

	// Check for updates on startup (if auto-check is enabled)
	useEffect(() => {
		const checkUpdate = async () => {
			try {
				const config = await commands.getUpdaterConfig();
				if (!config.autoCheck) return;

				const result = await commands.checkForUpdate();
				if (!result.available) return;

				// Skip if user previously chose to skip this version
				if (config.skippedVersion === result.version) return;

				setUpdateAvailable({
					version: result.version,
					body: result.body,
				});
			} catch {
				// Silently ignore update check failures on startup
			}
		};
		// Delay the check slightly so the app loads first
		const timer = setTimeout(checkUpdate, 3000);
		return () => clearTimeout(timer);
	}, []);

	const handleDismissUpdate = useCallback(() => {
		setUpdateAvailable(null);
	}, []);

	const handleSkipUpdate = useCallback(async () => {
		if (updateAvailable) {
			await commands.updateUpdaterConfig(null, updateAvailable.version);
			setUpdateAvailable(null);
		}
	}, [updateAvailable]);

	const handleInstallUpdate = useCallback(async () => {
		setUpdateInstalling(true);
		try {
			await commands.installUpdate();
		} catch {
			setUpdateInstalling(false);
		}
	}, []);

	useEffect(() => {
		fetchPapers();
		fetchCollections();
		fetchTags();
		fetchSubscriptions();
		fetchAiConfig();
		// Initialize plugin system
		fetchPlugins().then(() => loadAllEnabled());
		if (debugMode) {
			commands.setDebugMode(true).catch(() => {});
		}
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [
		fetchPapers,
		fetchCollections,
		fetchTags,
		fetchSubscriptions,
		fetchAiConfig,
		fetchPlugins,
		loadAllEnabled,
	]);

	// Suppress native WebView context menu (useful in dev mode so Radix
	// ContextMenu works without the browser "Inspect Element" menu appearing).
	// Uses bubble phase so Radix handlers fire first — Radix's composeEventHandlers
	// checks defaultPrevented and would skip its logic if we used capture phase.
	const disableNativeContextMenu = useUiStore(
		(s) => s.disableNativeContextMenu,
	);
	useEffect(() => {
		if (!disableNativeContextMenu) return;
		const handler = (e: MouseEvent) => {
			e.preventDefault();
		};
		document.addEventListener("contextmenu", handler);
		return () => document.removeEventListener("contextmenu", handler);
	}, [disableNativeContextMenu]);

	// Listen for paper-saved / paper-updated events from connector servers
	useEffect(() => {
		const unlistenSaved = listen("paper-saved", () => {
			fetchPapers();
			fetchCollections();
			fetchTags();
		});
		const unlistenUpdated = listen("paper-updated", () => {
			fetchPapers();
		});
		// When a PDF translation completes, refresh paper data so the new
		// attachment appears in the library list and reader toolbars.
		const unlistenPdfComplete = listen("pdf-translation-complete", () => {
			fetchPapers();
		});
		// When the background filesystem scanner detects changes (e.g. AI agent
		// wrote a file, manual copy, etc.), refresh the library view.
		const unlistenLibChanged = listen("library-changed", () => {
			fetchPapers();
		});
		return () => {
			unlistenSaved.then((fn) => fn());
			unlistenUpdated.then((fn) => fn());
			unlistenPdfComplete.then((fn) => fn());
			unlistenLibChanged.then((fn) => fn());
		};
	}, [fetchPapers, fetchCollections, fetchTags]);

	const readerTabs = tabs.filter((t) => t.type === "reader");
	const noteTabs = tabs.filter((t) => t.type === "note");
	const webviewTabs = tabs.filter((t) => t.type === "webview");
	const hasSettings = tabs.some((t) => t.type === "settings");

	return (
		<div className="flex h-screen flex-col">
			{/* Tab bar integrated into title bar area */}
			<TabBar />

			{/* Tab content area */}
			<div className="flex-1 overflow-hidden relative">
				{/* Home tab content */}
				<div
					className="absolute inset-0"
					style={{ display: activeTabId === "home" ? "flex" : "none" }}
				>
					<MainLayout>
						{view === "library" && <Library />}
						{view === "feed" && <Feed />}
						{view === "papers-cool" && <PapersCool />}
						{view === "plugins" && <PluginView />}
					</MainLayout>
				</div>

				{/* Agent tab */}
				<div
					className="absolute inset-0"
					style={{ display: activeTabId === "agent" ? "flex" : "none" }}
				>
					<AgentPanel />
				</div>

				{/* Settings tab */}
				{hasSettings && (
					<div
						className="absolute inset-0"
						style={{
							display: activeTabId === "settings" ? "flex" : "none",
						}}
					>
						<Settings />
					</div>
				)}

				{/* Reader tabs — kept mounted but hidden for state preservation */}
				{readerTabs.map((tab) => (
					<div
						key={tab.id}
						className="absolute inset-0"
						style={{ display: activeTabId === tab.id ? "flex" : "none" }}
					>
						<Reader
							tabId={tab.id}
							paperId={tab.paperId ?? null}
							feedItem={tab.feedItem ?? null}
							readerMode={tab.readerMode ?? "pdf"}
							pdfFilename={tab.pdfFilename}
						/>
					</div>
				))}

				{/* Note tabs */}
				{noteTabs.map((tab) => (
					<div
						key={tab.id}
						className="absolute inset-0"
						style={{ display: activeTabId === tab.id ? "flex" : "none" }}
					>
						<StandaloneNoteEditor paperId={tab.paperId ?? ""} tabId={tab.id} />
					</div>
				))}

				{/* Webview tabs */}
				{webviewTabs.map((tab) => (
					<div
						key={tab.id}
						className="absolute inset-0"
						style={{ display: activeTabId === tab.id ? "flex" : "none" }}
					>
						<Webview
							url={tab.url ?? ""}
							feedItem={tab.feedItem}
							isActive={activeTabId === tab.id}
						/>
					</div>
				))}
			</div>

			<FileDropZone />
			<BackgroundTaskPanel />
			{metadataSearchPaperId && <MetadataSearchDialog />}
			{debugMode && <LogPanel />}
			{showOnboarding && (
				<OnboardingOverlay onComplete={() => setShowOnboarding(false)} />
			)}

			{/* Update notification banner */}
			{updateAvailable && (
				<div className="fixed bottom-4 right-4 z-50 max-w-sm rounded-lg border bg-background shadow-lg p-4 space-y-2 animate-in slide-in-from-bottom-4">
					<div className="flex items-start justify-between gap-2">
						<div className="space-y-1">
							<p className="text-sm font-medium">
								{t("settings.updateAvailable")}
							</p>
							<p className="text-xs text-muted-foreground">
								{t("settings.newVersion", { version: updateAvailable.version })}
							</p>
						</div>
						<button
							type="button"
							onClick={handleDismissUpdate}
							className="text-muted-foreground hover:text-foreground shrink-0"
						>
							<svg
								xmlns="http://www.w3.org/2000/svg"
								width="14"
								height="14"
								viewBox="0 0 24 24"
								fill="none"
								stroke="currentColor"
								strokeWidth="2"
								strokeLinecap="round"
								strokeLinejoin="round"
							>
								<path d="M18 6 6 18" />
								<path d="m6 6 12 12" />
							</svg>
						</button>
					</div>
					<div className="flex items-center gap-2">
						<button
							type="button"
							onClick={handleInstallUpdate}
							disabled={updateInstalling}
							className="rounded-md bg-primary px-3 py-1 text-xs font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
						>
							{updateInstalling
								? t("settings.installing")
								: t("settings.downloadAndInstall")}
						</button>
						<button
							type="button"
							onClick={handleSkipUpdate}
							disabled={updateInstalling}
							className="rounded-md px-3 py-1 text-xs text-muted-foreground hover:text-foreground"
						>
							{t("settings.skipThisVersion")}
						</button>
					</div>
				</div>
			)}
		</div>
	);
}
