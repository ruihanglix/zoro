// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { AgentPanel } from "@/components/agent/AgentPanel";
import { BrowserPanel } from "@/components/browser/BrowserPanel";
import { AnnotationSidePanel } from "@/components/reader/AnnotationSidePanel";
import { InfoPanel } from "@/components/reader/InfoPanel";
import { NotesPanel } from "@/components/reader/NotesPanel";
import { TerminalPanel } from "@/components/reader/TerminalPanel";
import {
	ContextMenu,
	ContextMenuCheckboxItem,
	ContextMenuContent,
	ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { getAllAvailableTabs, getTabLabel } from "@/lib/readerTabs";
import type { PaperResponse } from "@/lib/commands";
import { cn } from "@/lib/utils";
import {
	createPluginSDK,
} from "@/plugins/PluginManager";
import type { ContributionWithPlugin } from "@/plugins/types";
import { usePluginStore } from "@/plugins/pluginStore";
import type { AnnotationType, ZoroHighlight } from "@/stores/annotationStore";
import { useAnnotationStore } from "@/stores/annotationStore";
import { useTabStore } from "@/stores/tabStore";
import { useUiStore } from "@/stores/uiStore";
import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

export interface SidebarTabPanelProps {
	side: "left" | "right";
	tabIds: string[];
	paper: PaperResponse;
	tabId: string; // reader tab ID for context
	readerMode: "pdf" | "html";
	// Annotation panel props
	paperId: string | null;
	bilingualMode: boolean;
	translationFile?: string;
	translationAnnotations?: ZoroHighlight[];
	onDeleteTranslationAnnotation?: (id: string) => Promise<void>;
	onUpdateTranslationAnnotation?: (
		id: string,
		color?: string | null,
		comment?: string | null,
	) => Promise<void>;
	onUpdateTranslationAnnotationType?: (
		id: string,
		newType: AnnotationType,
	) => Promise<void>;
	scrollToTranslationHighlight?: React.MutableRefObject<
		((h: ZoroHighlight) => void) | null
	>;
	onNavigateToHtmlAnnotation?: (ann: ZoroHighlight) => void;
	// Plugin sidebar tabs (pre-resolved)
	pluginSidebarTabs: ContributionWithPlugin[];
}

export function SidebarTabPanel({
	side,
	tabIds,
	paper,
	tabId,
	readerMode,
	paperId,
	bilingualMode,
	translationFile,
	translationAnnotations,
	onDeleteTranslationAnnotation,
	onUpdateTranslationAnnotation,
	onUpdateTranslationAnnotationType,
	scrollToTranslationHighlight,
	onNavigateToHtmlAnnotation,
	pluginSidebarTabs,
}: SidebarTabPanelProps) {
	const { t } = useTranslation();
	const [activeTab, setActiveTab] = useState<string>(tabIds[0] ?? "agent");
	const [terminalMounted, setTerminalMounted] = useState(false);
	const [browserMounted, setBrowserMounted] = useState(false);
	const [paperDir, setPaperDir] = useState<string | undefined>();

	const topLevelActiveTabId = useTabStore((s) => s.activeTabId);
	const isReaderTabActive = topLevelActiveTabId === tabId;

	const addTabToSide = useUiStore((s) => s.addTabToSide);
	const removeTabFromSide = useUiStore((s) => s.removeTabFromSide);

	const pluginsList = usePluginStore((s) => s.plugins);

	const allTabs = useMemo(
		() => getAllAvailableTabs(pluginSidebarTabs),
		[pluginSidebarTabs],
	);

	// If active tab was removed from this side, reset to first
	const effectiveActiveTab = tabIds.includes(activeTab)
		? activeTab
		: tabIds[0];

	// Get paper directory for agent/terminal
	useState(() => {
		if (paper?.id) {
			import("@/lib/commands").then((cmds) =>
				cmds.acpGetPaperDir(paper.id).then(setPaperDir).catch(() => {}),
			);
		}
	});

	const handleTabClick = (tab: string) => {
		setActiveTab(tab);
		if (tab === "terminal") setTerminalMounted(true);
		if (tab === "browser") setBrowserMounted(true);
	};

	return (
		<ContextMenu>
			<ContextMenuTrigger asChild>
				<div className="flex h-full min-w-0 flex-col overflow-hidden">
					{/* Tab bar — hidden when only 1 tab */}
					{tabIds.length > 1 && (
						<div className="flex border-b text-xs">
							{tabIds.map((tab) => (
								<button
									key={tab}
									type="button"
									className={`flex-1 py-2 text-center capitalize transition-colors text-nowrap px-1 ${
										effectiveActiveTab === tab
											? "border-b-2 border-primary font-medium text-foreground"
											: "text-muted-foreground hover:text-foreground"
									}`}
									onClick={() => handleTabClick(tab)}
								>
									{getTabLabel(tab, t)}
								</button>
							))}
						</div>
					)}

					{/* Tab content */}
			{effectiveActiveTab === "annotation" && (
				<AnnotationSidePanel
					paperId={paperId}
					readerMode={readerMode}
					bilingualMode={bilingualMode}
					translationFile={translationFile}
					translationAnnotations={translationAnnotations}
					onDeleteTranslationAnnotation={
						onDeleteTranslationAnnotation
					}
					onUpdateTranslationAnnotation={
						onUpdateTranslationAnnotation
					}
					onUpdateTranslationAnnotationType={
						onUpdateTranslationAnnotationType
					}
					scrollToTranslationHighlight={scrollToTranslationHighlight}
					onNavigateToHtmlAnnotation={onNavigateToHtmlAnnotation}
				/>
			)}

			{effectiveActiveTab === "agent" && (
				<div className="flex-1 min-w-0 overflow-hidden">
					<AgentPanel cwd={paperDir} paperId={paper.id} />
				</div>
			)}

			{effectiveActiveTab === "notes" && (
				<div className="flex-1 overflow-hidden">
					<NotesPanel
						paperId={paper.id}
						onCitationJump={(detail) => {
							const targetMode = detail.format;
							const updateTab = useTabStore.getState().updateTab;
							if (targetMode !== readerMode) {
								updateTab(tabId, { readerMode: targetMode });
							}
							const posJson = atob(detail.position);
							if (detail.format === "pdf") {
								const pos = JSON.parse(posJson);
								const { navigateToPage } =
									useAnnotationStore.getState();
								navigateToPage(pos.pageNumber ?? detail.page);
							} else if (detail.format === "html") {
								useAnnotationStore
									.getState()
									.setPendingHtmlCitationJump(posJson);
							}
						}}
					/>
				</div>
			)}

			{effectiveActiveTab === "info" && <InfoPanel paper={paper} />}

			{/* Terminal — lazy-mounted, stays alive once opened */}
			<div
				className={cn(
					"overflow-hidden",
					effectiveActiveTab === "terminal" ? "flex-1" : "hidden",
				)}
			>
				{terminalMounted && (
					<TerminalPanel
						paperId={paper.id}
						visible={effectiveActiveTab === "terminal"}
					/>
				)}
			</div>

			{/* Browser — lazy-mounted, stays alive once opened */}
			<div
				className={cn(
					"overflow-hidden min-h-0",
					effectiveActiveTab === "browser" ? "flex-1" : "hidden",
				)}
			>
				{browserMounted && (
					<BrowserPanel
						storageKey={`${paper.id}-${side}`}
						isActive={
							effectiveActiveTab === "browser" &&
							isReaderTabActive
						}
						paperId={paper.id}
					/>
				)}
			</div>

			{/* Plugin sidebar tab panels */}
			{pluginSidebarTabs.map((contrib) => {
				const pluginTabId = `plugin-${contrib.pluginId}-${contrib.contribution.id}`;
				if (effectiveActiveTab !== pluginTabId) return null;
				const Component = contrib.component;
				const pluginInfo = pluginsList.find(
					(p) => p.manifest.id === contrib.pluginId,
				);
				if (!pluginInfo) return null;
				const sdk = createPluginSDK(pluginInfo, {
					paperId: paper.id,
				});
				return (
					<div key={pluginTabId} className="flex-1 overflow-auto">
						<Component
							sdk={sdk}
							context={{ paperId: paper.id, readerMode, tabId }}
						/>
					</div>
				);
			})}
				</div>
			</ContextMenuTrigger>
			<ContextMenuContent>
				{allTabs.map((tab) => {
					const isPresent = tabIds.includes(tab.id);
					const isLastOnSide = isPresent && tabIds.length === 1;
					return (
						<ContextMenuCheckboxItem
							key={tab.id}
							checked={isPresent}
							disabled={isLastOnSide}
							onCheckedChange={(checked) => {
								if (checked) {
									addTabToSide(side, tab.id);
								} else {
									removeTabFromSide(side, tab.id);
								}
							}}
						>
							{getTabLabel(tab.id, t)}
						</ContextMenuCheckboxItem>
					);
				})}
			</ContextMenuContent>
		</ContextMenu>
	);
}
