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
import { useEffect } from "react";

// Inject shared dependencies for plugins before any plugin loads
injectPluginSharedDeps();

export default function App() {
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
			console.log(
				"[contextmenu] target:",
				(e.target as HTMLElement)?.tagName,
				(e.target as HTMLElement)?.className?.slice(0, 60),
				"| defaultPrevented:",
				e.defaultPrevented,
			);
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
		</div>
	);
}
