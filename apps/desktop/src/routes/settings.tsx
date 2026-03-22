// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { ZoteroImportDialog } from "@/components/library/ZoteroImportDialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import * as commands from "@/lib/commands";
import type {
	AiConfigResponse,
	AiProviderResponse,
	GlossaryTermResponse,
	McpStatusResponse,
	StorageInfoResponse,
	SyncConfig,
	SyncStatus,
} from "@/lib/commands";
import { supportedLanguages } from "@/lib/i18n";
import type { SupportedLanguage } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import { createPluginSDK } from "@/plugins/PluginManager";
import { usePluginStore } from "@/plugins/pluginStore";
import {
	FREE_PROVIDERS,
	useLabStore,
} from "@/stores/labStore";
import { useLibraryStore } from "@/stores/libraryStore";
import { useTranslationStore } from "@/stores/translationStore";
import { useUiStore } from "@/stores/uiStore";
import type {
	CitationPreviewMode,
	HtmlReaderFontFamily,
	Theme,
} from "@/stores/uiStore";
import { listen } from "@tauri-apps/api/event";
import {
	AlertCircle,
	ArrowUp,
	Book,
	Bot,
	CheckCircle,
	ChevronDown,
	ChevronRight,
	Cloud,
	Download,
	ExternalLink,
	FileText,
	FlaskConical,
	FolderOpen,
	Globe,
	GraduationCap,
	HardDrive,
	Info,
	Languages,
	Loader2,
	MessageCircle,
	Monitor,
	Moon,
	Pencil,
	Play,
	Plus,
	Puzzle,
	RefreshCw,
	RotateCcw,
	Rss,
	Search,
	SlidersHorizontal,
	Square,
	Star,
	Sun,
	Terminal,
	Trash2,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

type SettingsSection =
	| "ai-general"
	| "ai-chat"
	| "ai-translation"
	| "ai-pdf-translation"
	| "ai-mcp"
	| "ai-lab"
	| "general"
	| "connector"
	| "subscriptions"
	| "storage"
	| "sync"
	| "export"
	| "about"
	| "plugins-general"
	| (string & {}); // Allow dynamic plugin section IDs

type NavItem = { id: SettingsSection; labelKey: string; icon: LucideIcon };
type NavGroup = { groupLabelKey: string; items: NavItem[] };

const NAV_GROUPS: NavGroup[] = [
	{
		groupLabelKey: "settings.navAI",
		items: [
			{ id: "ai-general", labelKey: "settings.navGeneral", icon: Bot },
			{ id: "ai-chat", labelKey: "settings.navChat", icon: MessageCircle },
			{
				id: "ai-translation",
				labelKey: "settings.navTranslation",
				icon: Languages,
			},
			{
				id: "ai-pdf-translation",
				labelKey: "settings.navPdfTranslation",
				icon: FileText,
			},
			{ id: "ai-mcp", labelKey: "settings.navMcpServer", icon: Terminal },
			{ id: "ai-lab", labelKey: "settings.navLab", icon: FlaskConical },
		],
	},
	{
		groupLabelKey: "settings.navSettings",
		items: [
			{
				id: "general",
				labelKey: "settings.navGeneral",
				icon: SlidersHorizontal,
			},
			{
				id: "connector",
				labelKey: "settings.navBrowserConnector",
				icon: Globe,
			},
			{ id: "subscriptions", labelKey: "settings.navSubscriptions", icon: Rss },
			{ id: "storage", labelKey: "settings.navStorage", icon: HardDrive },
			{ id: "sync", labelKey: "settings.navWebdavSync", icon: Cloud },
			{ id: "export", labelKey: "settings.navExportData", icon: Download },
			{ id: "about", labelKey: "settings.navAbout", icon: Info },
		],
	},
	{
		groupLabelKey: "settings.navPlugins",
		items: [
			{
				id: "plugins-general",
				labelKey: "settings.navPluginsGeneral",
				icon: Puzzle,
			},
		],
	},
];
// Flat list for lookup
const ALL_NAV_ITEMS: NavItem[] = NAV_GROUPS.flatMap((g) => g.items);

const SORT_FIELD_OPTIONS = [
	{ value: "added_date", labelKey: "settings.sortDateAdded" },
	{ value: "title", labelKey: "settings.sortTitle" },
	{ value: "published_date", labelKey: "settings.sortYearPublished" },
	{ value: "modified_date", labelKey: "settings.sortDateModified" },
	{ value: "authors", labelKey: "settings.sortAuthors" },
];

const THEME_OPTIONS: { value: Theme; labelKey: string; icon: LucideIcon }[] = [
	{ value: "light", labelKey: "settings.themeLight", icon: Sun },
	{ value: "dark", labelKey: "settings.themeDark", icon: Moon },
	{ value: "system", labelKey: "settings.themeSystem", icon: Monitor },
];

function GlossarySection({
	aiConfig,
	glossaryEnabled,
	setGlossaryEnabled,
	glossaryThreshold,
	setGlossaryThreshold,
	onSaveGlossaryConfig,
}: {
	aiConfig: AiConfigResponse | null;
	glossaryEnabled: boolean;
	setGlossaryEnabled: (v: boolean) => void;
	glossaryThreshold: number;
	setGlossaryThreshold: (v: number) => void;
	onSaveGlossaryConfig: () => void;
}) {
	const { t } = useTranslation();
	const [terms, setTerms] = useState<GlossaryTermResponse[]>([]);
	const [loading, setLoading] = useState(false);
	const [showCandidates, setShowCandidates] = useState(false);
	const [searchQuery, setSearchQuery] = useState("");
	const [addSource, setAddSource] = useState("");
	const [addTranslated, setAddTranslated] = useState("");
	const [editingId, setEditingId] = useState<string | null>(null);
	const [editValue, setEditValue] = useState("");

	const loadTerms = useCallback(async () => {
		setLoading(true);
		try {
			const data = await commands.getGlossary();
			setTerms(data);
		} catch (e) {
			console.error("Failed to load glossary:", e);
		} finally {
			setLoading(false);
		}
	}, []);

	useEffect(() => {
		if (aiConfig?.nativeLang) {
			loadTerms();
		}
	}, [aiConfig?.nativeLang, loadTerms]);

	const activeTerms = terms.filter(
		(t) => t.source === "manual" || t.occurrenceCount >= glossaryThreshold,
	);
	const candidateTerms = terms.filter(
		(t) => t.source !== "manual" && t.occurrenceCount < glossaryThreshold,
	);

	const filterBySearch = (list: GlossaryTermResponse[]) => {
		if (!searchQuery.trim()) return list;
		const q = searchQuery.toLowerCase();
		return list.filter(
			(t) =>
				t.sourceTerm.toLowerCase().includes(q) ||
				t.translatedTerm.toLowerCase().includes(q),
		);
	};

	const filteredActive = filterBySearch(activeTerms);
	const filteredCandidates = filterBySearch(candidateTerms);

	const handleAdd = async () => {
		if (!addSource.trim() || !addTranslated.trim()) return;
		try {
			await commands.addGlossaryTerm({
				sourceTerm: addSource.trim(),
				translatedTerm: addTranslated.trim(),
			});
			setAddSource("");
			setAddTranslated("");
			await loadTerms();
		} catch (e) {
			console.error("Failed to add term:", e);
		}
	};

	const handleDelete = async (id: string) => {
		try {
			await commands.deleteGlossaryTerm(id);
			await loadTerms();
		} catch (e) {
			console.error("Failed to delete term:", e);
		}
	};

	const handlePromote = async (id: string) => {
		try {
			await commands.promoteGlossaryTerm(id);
			await loadTerms();
		} catch (e) {
			console.error("Failed to promote term:", e);
		}
	};

	const handleEditSave = async (id: string) => {
		if (!editValue.trim()) return;
		try {
			await commands.updateGlossaryTerm({
				id,
				translatedTerm: editValue.trim(),
			});
			setEditingId(null);
			await loadTerms();
		} catch (e) {
			console.error("Failed to update term:", e);
		}
	};

	const handleClear = async () => {
		if (!confirm(t("settings.clearGlossaryConfirm"))) return;
		try {
			await commands.clearGlossary();
			await loadTerms();
		} catch (e) {
			console.error("Failed to clear glossary:", e);
		}
	};

	const TermRow = ({
		term,
		showPromote,
	}: { term: GlossaryTermResponse; showPromote?: boolean }) => (
		<div className="flex items-center gap-2 py-1.5 px-2 rounded hover:bg-muted/50 group text-xs">
			<span className="w-[35%] truncate font-mono" title={term.sourceTerm}>
				{term.sourceTerm}
			</span>
			{editingId === term.id ? (
				<div className="flex-1 flex items-center gap-1">
					<input
						value={editValue}
						onChange={(e) => setEditValue(e.target.value)}
						onKeyDown={(e) => {
							if (e.key === "Enter") handleEditSave(term.id);
							if (e.key === "Escape") setEditingId(null);
						}}
						className="flex-1 h-6 rounded border bg-transparent px-1.5 text-xs"
						autoFocus
					/>
					<Button
						variant="ghost"
						size="sm"
						className="h-6 px-1.5"
						onClick={() => handleEditSave(term.id)}
					>
						<CheckCircle className="h-3 w-3" />
					</Button>
				</div>
			) : (
				<span className="flex-1 truncate" title={term.translatedTerm}>
					{term.translatedTerm}
				</span>
			)}
			<span className="w-10 text-center text-muted-foreground tabular-nums">
				{term.occurrenceCount}
			</span>
			<span className="w-12 text-center">
				{term.source === "manual" ? (
					<Badge variant="secondary" className="text-[10px] px-1 py-0">
						{t("settings.manual")}
					</Badge>
				) : (
					<Badge variant="outline" className="text-[10px] px-1 py-0">
						{t("settings.auto")}
					</Badge>
				)}
			</span>
			<div className="w-20 flex items-center gap-0.5 opacity-0 group-hover:opacity-100 justify-end">
				{showPromote && (
					<Button
						variant="ghost"
						size="sm"
						className="h-6 w-6 p-0"
						title={t("settings.promoteToActive")}
						onClick={() => handlePromote(term.id)}
					>
						<ArrowUp className="h-3 w-3" />
					</Button>
				)}
				<Button
					variant="ghost"
					size="sm"
					className="h-6 w-6 p-0"
					title={t("settings.editTranslation")}
					onClick={() => {
						setEditingId(term.id);
						setEditValue(term.translatedTerm);
					}}
				>
					<Pencil className="h-3 w-3" />
				</Button>
				<Button
					variant="ghost"
					size="sm"
					className="h-6 w-6 p-0 text-destructive"
					title={t("common.delete")}
					onClick={() => handleDelete(term.id)}
				>
					<Trash2 className="h-3 w-3" />
				</Button>
			</div>
		</div>
	);

	return (
		<div className="pt-4 space-y-3">
			<div className="flex items-center justify-between">
				<div className="flex items-center gap-2">
					<Book className="h-4 w-4" />
					<p className="text-sm font-semibold">
						{t("settings.translationGlossary")}
					</p>
				</div>
				<div className="flex items-center gap-2">
					<Button
						variant="ghost"
						size="sm"
						className="text-xs h-7"
						onClick={loadTerms}
						disabled={loading}
					>
						<RefreshCw
							className={cn("mr-1 h-3 w-3", loading && "animate-spin")}
						/>
						{t("common.refresh")}
					</Button>
					{terms.length > 0 && (
						<Button
							variant="ghost"
							size="sm"
							className="text-xs h-7 text-destructive"
							onClick={handleClear}
						>
							<Trash2 className="mr-1 h-3 w-3" />
							{t("settings.clearAll")}
						</Button>
					)}
				</div>
			</div>
			<p className="text-[11px] text-muted-foreground">
				{t("settings.glossaryDesc")}
			</p>

			<div className="flex items-center gap-4">
				<label className="flex items-center gap-2 text-sm cursor-pointer">
					<input
						type="checkbox"
						checked={glossaryEnabled}
						onChange={(e) => {
							setGlossaryEnabled(e.target.checked);
							onSaveGlossaryConfig();
						}}
						className="rounded"
					/>
					{t("settings.enableGlossary")}
				</label>
				<div className="flex items-center gap-2">
					<label className="text-xs text-muted-foreground">
						{t("settings.threshold")}:
					</label>
					<input
						type="number"
						min={1}
						max={100}
						value={glossaryThreshold}
						onChange={(e) => {
							const v = Math.max(
								1,
								Math.min(100, Number.parseInt(e.target.value) || 5),
							);
							setGlossaryThreshold(v);
						}}
						onBlur={onSaveGlossaryConfig}
						className="h-7 w-14 rounded-md border bg-transparent px-1.5 text-xs text-center"
					/>
					<span className="text-[11px] text-muted-foreground">
						{t("settings.occurrences")}
					</span>
				</div>
			</div>

			{glossaryEnabled && (
				<>
					{/* Add term form */}
					<div className="flex items-center gap-2">
						<input
							placeholder={t("settings.sourceTerm")}
							value={addSource}
							onChange={(e) => setAddSource(e.target.value)}
							onKeyDown={(e) => e.key === "Enter" && handleAdd()}
							className="h-7 flex-1 rounded-md border bg-transparent px-2 text-xs"
						/>
						<input
							placeholder={t("settings.translation")}
							value={addTranslated}
							onChange={(e) => setAddTranslated(e.target.value)}
							onKeyDown={(e) => e.key === "Enter" && handleAdd()}
							className="h-7 flex-1 rounded-md border bg-transparent px-2 text-xs"
						/>
						<Button
							variant="outline"
							size="sm"
							className="h-7"
							onClick={handleAdd}
							disabled={!addSource.trim() || !addTranslated.trim()}
						>
							<Plus className="mr-1 h-3 w-3" />
							{t("common.add")}
						</Button>
					</div>

					{/* Search */}
					{terms.length > 5 && (
						<div className="relative">
							<Search className="absolute left-2 top-1/2 -translate-y-1/2 h-3 w-3 text-muted-foreground" />
							<input
								placeholder={t("settings.searchTerms")}
								value={searchQuery}
								onChange={(e) => setSearchQuery(e.target.value)}
								className="h-7 w-full rounded-md border bg-transparent pl-7 pr-2 text-xs"
							/>
						</div>
					)}

					{/* Active terms table */}
					<div className="space-y-1">
						<div className="flex items-center gap-2 px-2 text-[10px] font-semibold text-muted-foreground uppercase tracking-wide">
							<span className="w-[35%]">{t("settings.sourceTerm")}</span>
							<span className="flex-1">{t("settings.translation")}</span>
							<span className="w-10 text-center">{t("settings.count")}</span>
							<span className="w-12 text-center">{t("paper.source")}</span>
							<span className="w-20" />
						</div>

						<p className="text-[11px] font-medium text-muted-foreground px-2 pt-1">
							{t("settings.activeTerms", { count: filteredActive.length })}
						</p>
						{filteredActive.length === 0 && (
							<p className="text-[11px] text-muted-foreground px-2 py-2 italic">
								{t("settings.noActiveTerms")}
							</p>
						)}
						<div className="max-h-[280px] overflow-y-auto">
							{filteredActive.map((t) => (
								<TermRow key={t.id} term={t} />
							))}
						</div>
					</div>

					{/* Candidates (collapsed by default) */}
					{candidateTerms.length > 0 && (
						<div className="space-y-1">
							<button
								type="button"
								className="flex items-center gap-1 text-[11px] font-medium text-muted-foreground px-2 py-1 hover:text-foreground cursor-pointer"
								onClick={() => setShowCandidates(!showCandidates)}
							>
								{showCandidates ? (
									<ChevronDown className="h-3 w-3" />
								) : (
									<ChevronRight className="h-3 w-3" />
								)}
								Candidates ({filteredCandidates.length})
								<span className="font-normal ml-1">
									— {t("settings.belowThreshold")}
								</span>
							</button>
							{showCandidates && (
								<div className="max-h-[200px] overflow-y-auto">
									{filteredCandidates.map((t) => (
										<TermRow key={t.id} term={t} showPromote />
									))}
								</div>
							)}
						</div>
					)}
				</>
			)}
		</div>
	);
}

/** Map lucide icon name strings from plugin manifest to actual icon components. */
const LUCIDE_ICON_MAP: Record<string, LucideIcon> = {
	Zap: Star,
	Star,
	Puzzle,
	Bot,
	Globe,
	Search,
	FileText,
	Info,
	Download,
};

function resolveLucideIcon(name: string): LucideIcon {
	return LUCIDE_ICON_MAP[name] ?? Puzzle;
}

export function Settings() {
	const { t } = useTranslation();
	const [section, setSection] = useState<SettingsSection>("ai-general");

	// Dynamic plugin settings contributions
	const plugins = usePluginStore((s) => s.plugins);
	const loadedModules = usePluginStore((s) => s.loadedModules);
	const getContributions = usePluginStore((s) => s.getContributionsForSlot);

	const pluginSettingsContribs = useMemo(() => {
		void plugins;
		void loadedModules;
		return getContributions("settings");
	}, [plugins, loadedModules, getContributions]);

	// Build dynamic nav groups that include plugin settings entries
	const dynamicNavGroups = useMemo(() => {
		const pluginNavItems: NavItem[] = pluginSettingsContribs.map((c) => ({
			id: `plugin-settings-${c.pluginId}-${c.contribution.id}` as SettingsSection,
			labelKey: c.pluginName, // Use plugin name directly; t() falls back to key itself
			icon: resolveLucideIcon(
				(c.contribution as { icon?: string }).icon ?? "Puzzle",
			),
		}));

		return NAV_GROUPS.map((group) => {
			if (
				group.groupLabelKey === "settings.navPlugins" &&
				pluginNavItems.length > 0
			) {
				return { ...group, items: [...group.items, ...pluginNavItems] };
			}
			return group;
		});
	}, [pluginSettingsContribs]);

	const allDynamicNavItems = useMemo(
		() => dynamicNavGroups.flatMap((g) => g.items),
		[dynamicNavGroups],
	);

	const showUncategorized = useUiStore((s) => s.showUncategorized);
	const setShowUncategorized = useUiStore((s) => s.setShowUncategorized);
	const debugMode = useUiStore((s) => s.debugMode);
	const setDebugMode = useUiStore((s) => s.setDebugMode);
	const disableNativeContextMenu = useUiStore(
		(s) => s.disableNativeContextMenu,
	);
	const setDisableNativeContextMenu = useUiStore(
		(s) => s.setDisableNativeContextMenu,
	);
	const showBackgroundTasks = useUiStore((s) => s.showBackgroundTasks);
	const setShowBackgroundTasks = useUiStore((s) => s.setShowBackgroundTasks);
	const theme = useUiStore((s) => s.theme);
	const setTheme = useUiStore((s) => s.setTheme);
	const confirmBeforeDelete = useUiStore((s) => s.confirmBeforeDelete);
	const setConfirmBeforeDelete = useUiStore((s) => s.setConfirmBeforeDelete);
	const defaultView = useUiStore((s) => s.defaultView);
	const setDefaultView = useUiStore((s) => s.setDefaultView);
	const defaultListMode = useUiStore((s) => s.defaultListMode);
	const setDefaultListMode = useUiStore((s) => s.setDefaultListMode);
	const defaultSortBy = useUiStore((s) => s.defaultSortBy);
	const setDefaultSortBy = useUiStore((s) => s.setDefaultSortBy);
	const defaultSortOrder = useUiStore((s) => s.defaultSortOrder);
	const setDefaultSortOrder = useUiStore((s) => s.setDefaultSortOrder);
	const resetColumns = useUiStore((s) => s.resetColumns);
	const citationPreviewMode = useUiStore((s) => s.citationPreviewMode);
	const setCitationPreviewMode = useUiStore((s) => s.setCitationPreviewMode);
	const showReaderTerminal = useUiStore((s) => s.showReaderTerminal);
	const setShowReaderTerminal = useUiStore((s) => s.setShowReaderTerminal);
	const language = useUiStore((s) => s.language);
	const setLanguage = useUiStore((s) => s.setLanguage);
	const uiScale = useUiStore((s) => s.uiScale);
	const setUiScale = useUiStore((s) => s.setUiScale);
	const htmlReaderTypography = useUiStore((s) => s.htmlReaderTypography);
	const setHtmlReaderTypography = useUiStore((s) => s.setHtmlReaderTypography);
	const resetHtmlReaderTypography = useUiStore(
		(s) => s.resetHtmlReaderTypography,
	);

	// Lab store — Free LLM Proxy
	const labFreeLlmEnabled = useLabStore((s) => s.freeLlmEnabled);
	const labAvailableModels = useLabStore((s) => s.getAvailableModels)();
	const labProviderKeys = useLabStore((s) => s.providerKeys);
	const labConfiguredProviderIds = useLabStore((s) => s.configuredProviderIds);
	const labFetchedModels = useLabStore((s) => s.fetchedModels);
	const labDisabledModels = useLabStore((s) => s.disabledModels);
	const labGetEnabledModelsForProvider = useLabStore((s) => s.getEnabledModelsForProvider);

	const subscriptions = useLibraryStore((s) => s.subscriptions);
	const fetchSubscriptions = useLibraryStore((s) => s.fetchSubscriptions);
	const exportBibtex = useLibraryStore((s) => s.exportBibtex);
	const collections = useLibraryStore((s) => s.collections);
	const tags = useLibraryStore((s) => s.tags);
	const fetchCollections = useLibraryStore((s) => s.fetchCollections);
	const fetchTags = useLibraryStore((s) => s.fetchTags);

	const [connectorStatus, setConnectorStatus] = useState<{
		enabled: boolean;
		port: number;
		running: boolean;
		zotero_compat_enabled: boolean;
		zotero_compat_port: number;
		zotero_compat_running: boolean;
		zotero_compat_error: string | null;
	} | null>(null);
	const [exportResult, setExportResult] = useState<string | null>(null);
	const [zoteroCompatSaving, setZoteroCompatSaving] = useState(false);
	const [storageInfo, setStorageInfo] = useState<StorageInfoResponse | null>(
		null,
	);
	const [retentionDays, setRetentionDays] = useState(7);
	const [retentionSaving, setRetentionSaving] = useState(false);
	const [clearingCache, setClearingCache] = useState(false);
	const [changingDataDir, setChangingDataDir] = useState(false);
	const [zoteroImportOpen, setZoteroImportOpen] = useState(false);

	// Sync state
	const [syncUrl, setSyncUrl] = useState("");
	const [syncUsername, setSyncUsername] = useState("");
	const [syncPassword, setSyncPassword] = useState("");
	const [syncPasswordSet, setSyncPasswordSet] = useState(false);
	const [syncRemotePath, setSyncRemotePath] = useState("/");
	const [syncEnabled, setSyncEnabled] = useState(false);
	const [syncInterval, setSyncInterval] = useState(5);
	const [syncDeviceId, setSyncDeviceId] = useState("");
	const [syncDeviceName, setSyncDeviceName] = useState("");
	const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);
	const [syncTesting, setSyncTesting] = useState(false);
	const [syncTestResult, setSyncTestResult] = useState<{
		ok: boolean;
		msg: string;
	} | null>(null);
	const [syncSaving, setSyncSaving] = useState(false);
	const [syncTriggering, setSyncTriggering] = useState(false);
	// Sync content options
	const [syncCollections, setSyncCollections] = useState(true);
	const [syncTags, setSyncTags] = useState(true);
	const [syncAnnotations, setSyncAnnotations] = useState(true);
	const [syncReaderState, setSyncReaderState] = useState(false);
	const [syncNotes, setSyncNotes] = useState(true);
	const [syncAttachments, setSyncAttachments] = useState(false);
	const [maxFileSizeMb, setMaxFileSizeMb] = useState(0);
	const [pdfDownloadMode, setPdfDownloadMode] = useState("on_demand");
	const [conflictStrategy, setConflictStrategy] = useState("auto_merge");

	// AI / Translation state
	const fetchAiConfig = useTranslationStore((s) => s.fetchAiConfig);
	const [aiConfig, setAiConfig] = useState<AiConfigResponse | null>(null);
	const [aiNativeLang, setAiNativeLang] = useState("");
	const [aiAutoTranslate, setAiAutoTranslate] = useState(false);
	const [aiTesting, setAiTesting] = useState(false);
	const [aiTestResult, setAiTestResult] = useState<{
		ok: boolean;
		msg: string;
	} | null>(null);
	const [aiSaving, setAiSaving] = useState(false);
	// Unified provider list — includes the "main" config as a virtual provider
	const [aiProviders, setAiProviders] = useState<
		(AiProviderResponse & { isDefault?: boolean })[]
	>([]);
	// NOTE: defaultProviderId removed — the global default model is the
	// single source of truth; providers are all treated equally.
	const [editingProvider, setEditingProvider] = useState<{
		id: string;
		name: string;
		baseUrl: string;
		apiKey: string;
		models: string;
	} | null>(null);
	// Track pending API keys for providers that haven't been saved yet
	const [pendingProviderKeys, setPendingProviderKeys] = useState<
		Record<string, string>
	>({});
	const [fetchingProviderModels, setFetchingProviderModels] = useState(false);
	const [customNativeLang, setCustomNativeLang] = useState("");
	const [promptTitleSystem, setPromptTitleSystem] = useState("");
	const [promptTitleUser, setPromptTitleUser] = useState("");
	const [promptAbstractSystem, setPromptAbstractSystem] = useState("");
	const [promptAbstractUser, setPromptAbstractUser] = useState("");
	const [htmlConcurrency, setHtmlConcurrency] = useState(8);

	// Glossary state
	const [glossaryEnabled, setGlossaryEnabled] = useState(true);
	const [glossaryThreshold, setGlossaryThreshold] = useState(5);

	// Global default model state
	const [globalDefaultModel, setGlobalDefaultModel] = useState("");

	// Task Model Defaults state
	const [taskQuickModel, setTaskQuickModel] = useState("");
	const [taskNormalModel, setTaskNormalModel] = useState("");
	const [taskHeavyModel, setTaskHeavyModel] = useState("");
	const [taskGlossaryModel, setTaskGlossaryModel] = useState("");

	// PDF Translation state
	const [pdfEnabled, setPdfEnabled] = useState(false);
	const [pdfBabeldocCmd, setPdfBabeldocCmd] = useState("babeldoc");
	const [pdfUseAiConfig, setPdfUseAiConfig] = useState(true);
	const [pdfCustomApiKey, setPdfCustomApiKey] = useState("");
	const [pdfCustomBaseUrl, setPdfCustomBaseUrl] = useState("");
	const [pdfCustomModel, setPdfCustomModel] = useState("");
	const [pdfQps, setPdfQps] = useState(4);
	const [pdfExtraArgs, setPdfExtraArgs] = useState("--no-dual");
	const [pdfSaving, setPdfSaving] = useState(false);
	const [pdfTesting, setPdfTesting] = useState(false);
	const [pdfTestResult, setPdfTestResult] = useState<{
		ok: boolean;
		msg: string;
	} | null>(null);

	// Chat settings state
	const [chatPresets, setChatPresets] = useState<commands.SystemPromptPreset[]>(
		[],
	);
	const [chatActivePreset, setChatActivePreset] = useState("");
	const [chatConfirmToolCalls, setChatConfirmToolCalls] = useState(true);
	const [chatSaving, setChatSaving] = useState(false);

	// MCP Server state
	const [mcpStatus, setMcpStatus] = useState<McpStatusResponse | null>(null);
	const [mcpPort, setMcpPort] = useState(23121);
	const [mcpTransport, setMcpTransport] = useState("http");
	const [mcpSaving, setMcpSaving] = useState(false);
	const [mcpStarting, setMcpStarting] = useState(false);

	// Subscription toggling/refreshing
	const [togglingSubId, setTogglingSubId] = useState<string | null>(null);
	const [refreshingSubId, setRefreshingSubId] = useState<string | null>(null);
	const [refreshResult, setRefreshResult] = useState<{
		subId: string;
		count: number;
	} | null>(null);

	// Export extra state
	const [exportRisResult, setExportRisResult] = useState<string | null>(null);
	const [clearingTranslations, setClearingTranslations] = useState(false);

	const fetchStorageInfo = useCallback(async () => {
		try {
			const info = await commands.getStorageInfo();
			setStorageInfo(info);
			setRetentionDays(info.feed_cache_retention_days);
		} catch (err) {
			console.error("Failed to fetch storage info:", err);
		}
	}, []);

	const fetchSyncStatus = useCallback(async () => {
		try {
			const [status, config] = await Promise.all([
				commands.getSyncStatus(),
				commands.getSyncConfig(),
			]);
			setSyncStatus(status);
			setSyncEnabled(status.enabled);
			// Populate form fields from saved config
			if (config.url) setSyncUrl(config.url);
			if (config.username) setSyncUsername(config.username);
			setSyncPasswordSet(config.password_set);
			setSyncRemotePath(config.remote_path || "/");
			setSyncInterval(config.interval_minutes || 5);
			if (config.device_id) setSyncDeviceId(config.device_id);
			if (config.device_name) setSyncDeviceName(config.device_name);
			setSyncCollections(config.sync_collections);
			setSyncTags(config.sync_tags);
			setSyncAnnotations(config.sync_annotations);
			setSyncReaderState(config.sync_reader_state);
			setSyncNotes(config.sync_notes);
			setSyncAttachments(config.sync_attachments);
			setMaxFileSizeMb(config.max_file_size_mb || 0);
			setPdfDownloadMode(config.pdf_download_mode || "on_demand");
			setConflictStrategy(config.conflict_strategy || "auto_merge");
		} catch (err) {
			console.error("Failed to fetch sync status:", err);
		}
	}, []);

	const loadAiConfig = useCallback(async () => {
		try {
			const config = await commands.getAiConfig();
			setAiConfig(config);
			setAiNativeLang(config.nativeLang);
			setAiAutoTranslate(config.autoTranslate);
			setPromptTitleSystem(config.translationPrompts.titleSystem);
			setPromptTitleUser(config.translationPrompts.titleUser);
			setPromptAbstractSystem(config.translationPrompts.abstractSystem);
			setPromptAbstractUser(config.translationPrompts.abstractUser);
			setHtmlConcurrency(config.htmlConcurrency || 8);
			setGlossaryEnabled(config.glossaryEnabled ?? true);
			setGlossaryThreshold(config.glossaryThreshold ?? 5);
			setPdfEnabled(config.pdfTranslation.enabled);
			setPdfBabeldocCmd(config.pdfTranslation.babeldocCommand || "babeldoc");
			setPdfUseAiConfig(config.pdfTranslation.useAiConfig);
			setPdfCustomBaseUrl(config.pdfTranslation.customBaseUrl);
			setPdfCustomModel(config.pdfTranslation.customModel);
			setPdfQps(config.pdfTranslation.qps || 4);
			setPdfExtraArgs(config.pdfTranslation.extraArgs ?? "--no-dual");
			setPendingProviderKeys({});

			// Load global default model
			setGlobalDefaultModel(config.model || "");

			// Load task model defaults
			setTaskQuickModel(config.taskModelDefaults?.quickTranslation || "");
			setTaskNormalModel(config.taskModelDefaults?.normalTranslation || "");
			setTaskHeavyModel(config.taskModelDefaults?.heavyTranslation || "");
			setTaskGlossaryModel(config.taskModelDefaults?.glossaryExtraction || "");

			// Build the unified providers list:
			// - The main config becomes a virtual provider with id "__main__"
			// - Additional providers follow
			const mainProvider: AiProviderResponse & { isDefault?: boolean } = {
				id: "__main__",
				name:
					config.provider === "openai"
						? "OpenAI"
						: config.provider === "deepseek"
							? "DeepSeek"
							: config.provider === "ollama"
								? "Ollama"
								: config.baseUrl
									? new URL(config.baseUrl).hostname
									: "Default",
				baseUrl: config.baseUrl,
				apiKeySet: config.apiKeySet,
				models: config.model ? [config.model] : [],
				isDefault: false,
			};
			const additionalProviders = (config.providers || [])
				// Hide internal lab providers (__lab_openrouter, __lab_groq, etc.)
				// but keep __lab_router visible — it's the user-facing "Lab Smart Router 🧪"
				.filter((p) => !p.id.startsWith("__lab_") || p.id === "__lab_router")
				.map((p) => ({
					...p,
					isDefault: false,
				}));
			setAiProviders([mainProvider, ...additionalProviders]);

			// Detect if native lang is a custom (non-preset) value
			const presetLangs = [
				"zh",
				"ja",
				"ko",
				"es",
				"fr",
				"de",
				"pt",
				"ru",
				"ar",
				"it",
				"vi",
				"th",
				"id",
				"hi",
				"en",
				"",
			];
			if (config.nativeLang && !presetLangs.includes(config.nativeLang)) {
				setCustomNativeLang(config.nativeLang);
				setAiNativeLang("__custom__");
			} else {
				setCustomNativeLang("");
			}
		} catch (err) {
			console.error("Failed to load AI config:", err);
		}
	}, []);

	const loadChatConfig = useCallback(async () => {
		try {
			const config = await commands.chatGetConfig();
			setChatPresets(config.presets);
			setChatActivePreset(config.activePreset);
			setChatConfirmToolCalls(config.confirmToolCalls);
		} catch (err) {
			console.error("Failed to load chat config:", err);
		}
	}, []);

	const loadMcpStatus = useCallback(async () => {
		try {
			const status = await commands.getMcpStatus();
			setMcpStatus(status);
			setMcpPort(status.port);
			setMcpTransport(status.transport);
		} catch (err) {
			console.error("Failed to load MCP status:", err);
		}
	}, []);

	useEffect(() => {
		commands.getConnectorStatus().then(setConnectorStatus).catch(console.error);
		fetchSubscriptions();
		fetchStorageInfo();
		fetchSyncStatus();
		loadAiConfig();
		loadChatConfig();
		loadMcpStatus();
		fetchCollections();
		fetchTags();

		const unlistenZoteroError = listen<string>("zotero-compat-error", () => {
			commands
				.getConnectorStatus()
				.then(setConnectorStatus)
				.catch(console.error);
		});
		return () => {
			unlistenZoteroError.then((fn) => fn());
		};
	}, [
		fetchSubscriptions,
		fetchStorageInfo,
		fetchSyncStatus,
		loadAiConfig,
		loadChatConfig,
		loadMcpStatus,
		fetchCollections,
		fetchTags,
	]);

	// Auto-sync lab providers to Rust backend AiConfig.providers
	// when lab configuration changes (enabled/disabled, keys, models, disabled models)
	useEffect(() => {
		const syncLabProviders = async () => {
			try {
				const config = await commands.getAiConfig();
				// Get existing non-lab providers
				const existingProviders = (config.providers || []).filter(
					(p) => !p.id.startsWith("__lab_"),
				);
				// Build lab providers if enabled (only include enabled models)
				const labProviders: { id: string; name: string; baseUrl: string; apiKey?: string; models: string[] }[] = [];
				if (labFreeLlmEnabled) {
					for (const pid of labConfiguredProviderIds) {
						const fp = FREE_PROVIDERS.find((p) => p.id === pid);
						const enabledModels = labGetEnabledModelsForProvider(pid);
						labProviders.push({
							id: `__lab_${pid}`,
							name: `${fp?.displayName || pid} 🧪`,
							baseUrl: fp?.baseURL || "",
							apiKey: labProviderKeys[pid] || undefined,
							models: enabledModels,
						});
					}
					// Virtual router provider — gives users a single "__lab_auto__" model
					// that triggers round-robin across all lab providers in the backend
					labProviders.push({
						id: "__lab_router",
						name: "Lab Smart Router 🧪",
						baseUrl: "",
						apiKey: "",
						models: ["__lab_auto__"],
					});
				}
				// Merge and save
				await commands.updateAiConfig({
					providers: [...existingProviders.map((p) => ({
						id: p.id,
						name: p.name,
						baseUrl: p.baseUrl,
						apiKey: undefined, // preserve existing
						models: p.models,
					})), ...labProviders],
				});
			} catch (err) {
				console.error("[lab] Failed to sync lab providers to AI config:", err);
			}
		};
		syncLabProviders();
	}, [labFreeLlmEnabled, labConfiguredProviderIds, labProviderKeys, labFetchedModels, labDisabledModels, labGetEnabledModelsForProvider]);

	const handleExportAll = async () => {
		try {
			const bibtex = await exportBibtex();
			setExportResult(bibtex);
		} catch (err) {
			console.error("Export failed:", err);
		}
	};

	const handleDebugToggle = async (enabled: boolean) => {
		try {
			await commands.setDebugMode(enabled);
			setDebugMode(enabled);
		} catch (err) {
			console.error("Failed to toggle debug mode:", err);
		}
	};

	const handleZoteroCompatToggle = async (enabled: boolean) => {
		setZoteroCompatSaving(true);
		try {
			await commands.updateConnectorConfig({
				zotero_compat_enabled: enabled,
			});
			const status = await commands.getConnectorStatus();
			setConnectorStatus(status);
		} catch (err) {
			console.error("Failed to update Zotero compat setting:", err);
		} finally {
			setZoteroCompatSaving(false);
		}
	};

	const handleRetentionSave = async () => {
		setRetentionSaving(true);
		try {
			await commands.updateSubscriptionsConfig(retentionDays);
			await fetchStorageInfo();
		} catch (err) {
			console.error("Failed to update retention:", err);
		} finally {
			setRetentionSaving(false);
		}
	};

	const handleClearCache = async () => {
		if (
			!confirm(
				"Delete all cached feed items that haven't been added to your library?",
			)
		) {
			return;
		}
		setClearingCache(true);
		try {
			const deleted = await commands.clearFeedCache();
			console.log(`Cleared ${deleted} cached items`);
			await fetchStorageInfo();
		} catch (err) {
			console.error("Failed to clear cache:", err);
		} finally {
			setClearingCache(false);
		}
	};

	const handleChangeDataDir = async () => {
		try {
			const { open } = await import("@tauri-apps/plugin-dialog");
			const selected = await open({
				directory: true,
				multiple: false,
				title: t("settings.selectNewDataDirectory"),
			});
			if (!selected) return;

			const newPath = selected as string;
			const moveData = confirm(t("settings.moveDataConfirm"));

			setChangingDataDir(true);
			await commands.changeDataDir(newPath, moveData);
			alert(t("settings.dataDirectoryChangedRestart"));
			await fetchStorageInfo();
		} catch (err) {
			console.error("Failed to change data directory:", err);
			alert(`${t("settings.dataDirectoryChangeFailed")}: ${err}`);
		} finally {
			setChangingDataDir(false);
		}
	};

	const handleSyncTest = async () => {
		setSyncTesting(true);
		setSyncTestResult(null);
		try {
			const msg = await commands.testWebdavConnection(
				syncUrl,
				syncUsername,
				syncPassword,
			);
			setSyncTestResult({ ok: true, msg });
		} catch (err: any) {
			setSyncTestResult({ ok: false, msg: String(err) });
		} finally {
			setSyncTesting(false);
		}
	};

	const handleSyncSave = async () => {
		setSyncSaving(true);
		try {
			let deviceId = syncDeviceId;
			if (!deviceId) {
				deviceId = crypto.randomUUID().replace(/-/g, "").slice(0, 16);
				setSyncDeviceId(deviceId);
			}
			let deviceName = syncDeviceName;
			if (!deviceName) {
				deviceName = `${navigator.platform || "Unknown"}-${Date.now().toString(36)}`;
				setSyncDeviceName(deviceName);
			}
			const config: SyncConfig = {
				enabled: true,
				url: syncUrl,
				username: syncUsername,
				password: syncPassword,
				remote_path: syncRemotePath || "/",
				device_id: deviceId,
				device_name: deviceName,
				interval_minutes: syncInterval,
				sync_collections: syncCollections,
				sync_tags: syncTags,
				sync_annotations: syncAnnotations,
				sync_reader_state: syncReaderState,
				sync_notes: syncNotes,
				sync_attachments: syncAttachments,
				max_file_size_mb: maxFileSizeMb,
				pdf_download_mode: pdfDownloadMode,
				conflict_strategy: conflictStrategy,
			};
			await commands.saveSyncConfig(config);
			setSyncEnabled(true);
			await fetchSyncStatus();
		} catch (err) {
			console.error("Failed to save sync config:", err);
		} finally {
			setSyncSaving(false);
		}
	};

	const handleSyncDisable = async () => {
		setSyncSaving(true);
		try {
			const config: SyncConfig = {
				enabled: false,
				url: syncUrl,
				username: syncUsername,
				password: syncPassword,
				remote_path: syncRemotePath,
				device_id: syncDeviceId,
				device_name: syncDeviceName,
				interval_minutes: syncInterval,
				sync_collections: syncCollections,
				sync_tags: syncTags,
				sync_annotations: syncAnnotations,
				sync_reader_state: syncReaderState,
				sync_notes: syncNotes,
				sync_attachments: syncAttachments,
				max_file_size_mb: maxFileSizeMb,
				pdf_download_mode: pdfDownloadMode,
				conflict_strategy: conflictStrategy,
			};
			await commands.saveSyncConfig(config);
			setSyncEnabled(false);
			await fetchSyncStatus();
		} catch (err) {
			console.error("Failed to disable sync:", err);
		} finally {
			setSyncSaving(false);
		}
	};

	const handleTriggerSync = async () => {
		setSyncTriggering(true);
		try {
			await commands.triggerSync();
			await fetchSyncStatus();
		} catch (err) {
			console.error("Sync failed:", err);
		} finally {
			setSyncTriggering(false);
		}
	};

	const handleAiSave = async () => {
		setAiSaving(true);
		try {
			// All user-configured providers (exclude internal lab providers and __main__)
			const userProviders = aiProviders.filter(
				(p) => p.id !== "__main__" && !p.id.startsWith("__lab_"),
			);

			// __main__ provider holds the primary (fallback) base_url and api_key.
			// The backend's resolve_for_model() will override these when the global
			// default model belongs to a different provider.
			const mainP = aiProviders.find((p) => p.id === "__main__");

			await commands.updateAiConfig({
				provider: "",
				baseUrl: mainP?.baseUrl || "",
				apiKey: mainP
					? pendingProviderKeys[mainP.id] || undefined
					: undefined,
				model: globalDefaultModel || "",
				nativeLang:
					aiNativeLang === "__custom__" ? customNativeLang : aiNativeLang,
				autoTranslate: aiAutoTranslate,
				htmlConcurrency: htmlConcurrency,
				translationPrompts: {
					titleSystem: promptTitleSystem,
					titleUser: promptTitleUser,
					abstractSystem: promptAbstractSystem,
					abstractUser: promptAbstractUser,
				},
			providers: [
					// All user-configured providers (treated equally)
					...userProviders.map((p) => ({
						id: p.id,
						name: p.name,
						baseUrl: p.baseUrl,
						apiKey: pendingProviderKeys[p.id] || undefined,
						models: p.models,
					})),
					// Auto-inject lab providers (when lab is enabled)
					...(labFreeLlmEnabled
						? [
								...labConfiguredProviderIds.map((pid) => {
									const fp = FREE_PROVIDERS.find((p) => p.id === pid);
									const enabledModels = labGetEnabledModelsForProvider(pid);
									return {
										id: `__lab_${pid}`,
										name: `${fp?.displayName || pid} 🧪`,
										baseUrl: fp?.baseURL || "",
										apiKey: labProviderKeys[pid] || undefined,
										models: enabledModels,
									};
								}),
								{
									id: "__lab_router",
									name: "Lab Smart Router 🧪",
									baseUrl: "",
									apiKey: "",
									models: ["__lab_auto__"],
								},
							]
						: []),
				],
				taskModelDefaults: {
					quickTranslation: taskQuickModel,
					normalTranslation: taskNormalModel,
					heavyTranslation: taskHeavyModel,
					glossaryExtraction: taskGlossaryModel,
				},
			});
			await loadAiConfig();
			await fetchAiConfig(); // Refresh the global store
		} catch (err) {
			console.error("Failed to save AI config:", err);
		} finally {
			setAiSaving(false);
		}
	};

	const handleGlossaryConfigSave = async () => {
		try {
			await commands.updateAiConfig({
				glossaryEnabled,
				glossaryThreshold,
			});
			await loadAiConfig();
		} catch (err) {
			console.error("Failed to save glossary config:", err);
		}
	};

	const handleAiTest = async () => {
		setAiTesting(true);
		setAiTestResult(null);
		try {
			const mainP = aiProviders.find((p) => p.id === "__main__");
			// Save first so the backend has the latest config
			await commands.updateAiConfig({
				provider: "",
				baseUrl: mainP?.baseUrl || "",
				apiKey: mainP
					? pendingProviderKeys[mainP.id] || undefined
					: undefined,
				model: globalDefaultModel || "",
			});
			const msg = await commands.testAiConnection();
			setAiTestResult({ ok: true, msg: `Connection OK: ${msg}` });
		} catch (err) {
			setAiTestResult({ ok: false, msg: String(err) });
		} finally {
			setAiTesting(false);
		}
	};

	const handleResetPrompts = async () => {
		try {
			await commands.resetTranslationPrompts();
			await loadAiConfig();
		} catch (err) {
			console.error("Failed to reset prompts:", err);
		}
	};

	const handlePdfTest = async () => {
		setPdfTesting(true);
		setPdfTestResult(null);
		try {
			// Save first so the backend tests the latest command
			await commands.updateAiConfig({
				pdfTranslation: {
					babeldocCommand: pdfBabeldocCmd,
				},
			});
			const msg = await commands.testBabeldoc();
			setPdfTestResult({ ok: true, msg });
		} catch (err) {
			setPdfTestResult({ ok: false, msg: String(err) });
		} finally {
			setPdfTesting(false);
		}
	};

	const handlePdfTranslationSave = async () => {
		setPdfSaving(true);
		try {
			await commands.updateAiConfig({
				pdfTranslation: {
					enabled: pdfEnabled,
					babeldocCommand: pdfBabeldocCmd,
					useAiConfig: pdfUseAiConfig,
					customApiKey: pdfCustomApiKey || undefined,
					customBaseUrl: pdfCustomBaseUrl,
					customModel: pdfCustomModel,
					qps: pdfQps,
					extraArgs: pdfExtraArgs,
				},
			});
			setPdfCustomApiKey("");
			await loadAiConfig();
			await fetchAiConfig();
		} catch (err) {
			console.error("Failed to save PDF translation config:", err);
		} finally {
			setPdfSaving(false);
		}
	};

	const handleMcpSave = async () => {
		setMcpSaving(true);
		try {
			const status = await commands.updateMcpConfig({
				transport: mcpTransport,
				port: mcpPort,
			});
			setMcpStatus(status);
		} catch (err) {
			console.error("Failed to save MCP config:", err);
		} finally {
			setMcpSaving(false);
		}
	};

	const handleMcpToggle = async (enabled: boolean) => {
		setMcpStarting(true);
		try {
			await commands.updateMcpConfig({
				enabled,
				transport: mcpTransport,
				port: mcpPort,
			});
			if (enabled) {
				const status = await commands.startMcpServer();
				setMcpStatus(status);
			} else {
				const status = await commands.stopMcpServer();
				setMcpStatus(status);
			}
		} catch (err) {
			console.error("Failed to toggle MCP server:", err);
		} finally {
			setMcpStarting(false);
		}
	};

	const handleMcpStart = async () => {
		setMcpStarting(true);
		try {
			const status = await commands.startMcpServer();
			setMcpStatus(status);
		} catch (err) {
			console.error("Failed to start MCP server:", err);
		} finally {
			setMcpStarting(false);
		}
	};

	const handleMcpStop = async () => {
		setMcpStarting(true);
		try {
			const status = await commands.stopMcpServer();
			setMcpStatus(status);
		} catch (err) {
			console.error("Failed to stop MCP server:", err);
		} finally {
			setMcpStarting(false);
		}
	};

	const handleMcpRestart = async () => {
		setMcpStarting(true);
		try {
			const status = await commands.restartMcpServer();
			setMcpStatus(status);
		} catch (err) {
			console.error("Failed to restart MCP server:", err);
		} finally {
			setMcpStarting(false);
		}
	};

	const handleToggleSubscription = async (id: string, enabled: boolean) => {
		setTogglingSubId(id);
		try {
			await commands.toggleSubscription(id, enabled);
			await fetchSubscriptions();
		} catch (err) {
			console.error("Failed to toggle subscription:", err);
		} finally {
			setTogglingSubId(null);
		}
	};

	const handleRefreshSubscription = async (id: string) => {
		setRefreshingSubId(id);
		setRefreshResult(null);
		try {
			const count = await commands.refreshSubscription(id);
			setRefreshResult({ subId: id, count });
			await fetchSubscriptions();
		} catch (err) {
			console.error("Failed to refresh subscription:", err);
		} finally {
			setRefreshingSubId(null);
		}
	};

	const handleExportRis = async () => {
		try {
			const ris = await commands.exportRis();
			setExportRisResult(ris);
		} catch (err) {
			console.error("RIS export failed:", err);
		}
	};

	const handleClearTranslations = async () => {
		if (
			!confirm(
				"Delete all cached translations? They will be re-generated on next view.",
			)
		) {
			return;
		}
		setClearingTranslations(true);
		try {
			// Clear by deleting for all papers — bulk approach
			const papers = await commands.listPapers({});
			let cleared = 0;
			for (const p of papers) {
				const count = await commands.deleteTranslations("paper", p.id);
				cleared += count;
			}
			console.log(`Cleared ${cleared} translations`);
		} catch (err) {
			console.error("Failed to clear translations:", err);
		} finally {
			setClearingTranslations(false);
		}
	};

	const activeNav =
		allDynamicNavItems.find((n) => n.id === section) ?? ALL_NAV_ITEMS[0];
	// Find which group the active nav belongs to
	const activeGroup = dynamicNavGroups.find((g) =>
		g.items.some((item) => item.id === section),
	);

	return (
		<div className="flex h-full w-full">
			{/* Left sidebar nav */}
			<nav className="w-48 shrink-0 border-r bg-muted/30 p-3 space-y-3 select-none overflow-y-auto">
				{dynamicNavGroups.map((group) => (
					<div key={group.groupLabelKey} className="space-y-0.5">
						<p className="px-2 pb-1 text-xs font-semibold text-muted-foreground uppercase tracking-wider">
							{t(group.groupLabelKey)}
						</p>
						{group.items.map((item) => (
							<button
								key={item.id}
								type="button"
								onClick={() => setSection(item.id)}
								className={cn(
									"flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm transition-colors",
									section === item.id
										? "bg-primary text-primary-foreground"
										: "text-muted-foreground hover:bg-accent hover:text-foreground",
								)}
							>
								<item.icon className="h-4 w-4 shrink-0" />
								{t(item.labelKey)}
							</button>
						))}
					</div>
				))}
			</nav>

			{/* Right content area */}
			<div className="flex-1 min-w-0 flex flex-col">
				<header className="flex h-12 items-center border-b px-6 shrink-0 select-none">
					<activeNav.icon className="h-4 w-4 mr-2 text-muted-foreground" />
					<h2 className="text-sm font-semibold">
						{activeGroup && (
							<span className="text-muted-foreground">
								{t(activeGroup.groupLabelKey)}
								{" › "}
							</span>
						)}
						{t(activeNav.labelKey)}
					</h2>
				</header>

				<ScrollArea className="flex-1">
					<div className="p-6">
						{section === "general" && (
							<div className="space-y-5">
								{/* Appearance */}
								<div className="space-y-1.5">
									<p className="text-xs font-medium">
										{t("settings.appearance")}
									</p>
									<div className="flex gap-1">
										{THEME_OPTIONS.map((opt) => (
											<button
												key={opt.value}
												type="button"
												onClick={() => setTheme(opt.value)}
												className={cn(
													"flex items-center gap-1.5 rounded-md border px-3 py-1.5 text-xs transition-colors",
													theme === opt.value
														? "border-primary bg-primary/10 text-primary"
														: "border-border text-muted-foreground hover:bg-accent hover:text-foreground",
												)}
											>
												<opt.icon className="h-3.5 w-3.5" />
												{t(opt.labelKey)}
											</button>
										))}
									</div>
									{/* Language selector */}
									<div className="mt-3">
										<label
											className="text-xs text-muted-foreground"
											htmlFor="ui-language"
										>
											{t("settings.language")}
										</label>
										<select
											id="ui-language"
											value={language}
											onChange={(e) =>
												setLanguage(e.target.value as SupportedLanguage)
											}
											className="mt-1 h-8 w-full max-w-xs rounded-md border bg-transparent px-2 text-sm"
										>
											{supportedLanguages.map((lang) => (
												<option key={lang.code} value={lang.code}>
													{lang.label}
												</option>
											))}
										</select>
										<p className="text-[11px] text-muted-foreground mt-1">
											{t("settings.languageDescription")}
										</p>
									</div>
									{/* UI Scale */}
									<div className="mt-3">
										<label
											className="text-xs text-muted-foreground"
											htmlFor="ui-scale"
										>
											{t("settings.uiScale")}
										</label>
										<div className="flex items-center gap-3 mt-1">
											<input
												id="ui-scale"
												type="range"
												min="0.5"
												max="2"
												step="0.05"
												value={uiScale}
												onChange={(e) => setUiScale(Number(e.target.value))}
												className="w-full max-w-xs accent-primary"
											/>
											<button
												type="button"
												className="min-w-[3.5rem] rounded-md border px-2 py-0.5 text-xs text-center tabular-nums hover:bg-accent transition-colors"
												onClick={() => setUiScale(1)}
												title={t("settings.resetUiScale")}
											>
												{Math.round(uiScale * 100)}%
											</button>
										</div>
										<p className="text-[11px] text-muted-foreground mt-1">
											{t("settings.uiScaleDescription")}
										</p>
									</div>
								</div>

								<Separator />

								{/* Startup defaults */}
								<div className="space-y-3">
									<p className="text-xs font-medium">
										{t("settings.startupDefaults")}
									</p>
									<div className="grid grid-cols-2 gap-3">
										<div>
											<label
												className="text-xs text-muted-foreground"
												htmlFor="default-view"
											>
												{t("settings.defaultView")}
											</label>
											<select
												id="default-view"
												value={defaultView}
												onChange={(e) =>
													setDefaultView(e.target.value as "library" | "feed")
												}
												className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm"
											>
												<option value="library">{t("settings.library")}</option>
												<option value="feed">{t("settings.feedLabel")}</option>
											</select>
										</div>
										<div>
											<label
												className="text-xs text-muted-foreground"
												htmlFor="default-list-mode"
											>
												{t("settings.libraryListMode")}
											</label>
											<select
												id="default-list-mode"
												value={defaultListMode}
												onChange={(e) =>
													setDefaultListMode(e.target.value as "list" | "card")
												}
												className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm"
											>
												<option value="list">{t("settings.table")}</option>
												<option value="card">{t("settings.cards")}</option>
											</select>
										</div>
									</div>
									<div className="grid grid-cols-2 gap-3">
										<div>
											<label
												className="text-xs text-muted-foreground"
												htmlFor="default-sort-by"
											>
												{t("settings.defaultSortField")}
											</label>
											<select
												id="default-sort-by"
												value={defaultSortBy}
												onChange={(e) => setDefaultSortBy(e.target.value)}
												className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm"
											>
												{SORT_FIELD_OPTIONS.map((opt) => (
													<option key={opt.value} value={opt.value}>
														{t(opt.labelKey)}
													</option>
												))}
											</select>
										</div>
										<div>
											<label
												className="text-xs text-muted-foreground"
												htmlFor="default-sort-order"
											>
												{t("settings.sortOrder")}
											</label>
											<select
												id="default-sort-order"
												value={defaultSortOrder}
												onChange={(e) => setDefaultSortOrder(e.target.value)}
												className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm"
											>
												<option value="desc">
													{t("settings.newestFirst")}
												</option>
												<option value="asc">{t("settings.oldestFirst")}</option>
											</select>
										</div>
									</div>
									<p className="text-[11px] text-muted-foreground">
										{t("settings.startupDefaultsNote")}
									</p>
								</div>

								<Separator />

								{/* Behavior */}
								<div className="space-y-2">
									<p className="text-xs font-medium">
										{t("settings.behavior")}
									</p>
									<label className="flex items-center gap-2 text-sm cursor-pointer">
										<input
											type="checkbox"
											checked={confirmBeforeDelete}
											onChange={(e) => setConfirmBeforeDelete(e.target.checked)}
											className="rounded"
										/>
										{t("settings.confirmBeforeDeleting")}
									</label>
									<label className="flex items-center gap-2 text-sm cursor-pointer">
										<input
											type="checkbox"
											checked={showUncategorized}
											onChange={(e) => setShowUncategorized(e.target.checked)}
											className="rounded"
										/>
										{t("settings.showUncategorized")}
									</label>
								</div>

								<Separator />

								{/* Reader */}
								<div className="space-y-2">
									<p className="text-xs font-medium">{t("settings.reader")}</p>
									<div>
										<label
											className="text-xs text-muted-foreground"
											htmlFor="citation-preview-mode"
										>
											{t("settings.citationHoverPreview")}
										</label>
										<select
											id="citation-preview-mode"
											value={citationPreviewMode}
											onChange={(e) =>
												setCitationPreviewMode(
													e.target.value as CitationPreviewMode,
												)
											}
											className="mt-1 h-8 w-full max-w-xs rounded-md border bg-transparent px-2 text-sm"
										>
											<option value="text">{t("settings.textMode")}</option>
											<option value="image">{t("settings.imageMode")}</option>
											<option value="off">{t("settings.offMode")}</option>
										</select>
										<p className="text-[11px] text-muted-foreground mt-1">
											{t("settings.citationHoverPreviewDesc")}
										</p>
									</div>
									<label className="flex items-center gap-2 text-sm cursor-pointer">
										<input
											type="checkbox"
											checked={showReaderTerminal}
											onChange={(e) => setShowReaderTerminal(e.target.checked)}
											className="rounded"
										/>
										{t("settings.showTerminalTab")}
									</label>
								</div>

								<Separator />

								{/* HTML Reader Typography */}
								<div className="space-y-3">
									<div>
										<p className="text-xs font-medium">
											{t("settings.htmlTypography")}
										</p>
										<p className="text-[11px] text-muted-foreground mt-0.5">
											{t("settings.htmlTypographyDesc")}
										</p>
									</div>

									{/* Font Family + Custom input row */}
									<div className="flex items-end gap-3 flex-wrap">
										<div className="flex-1 min-w-[140px]">
											<label
												className="text-xs text-muted-foreground"
												htmlFor="html-font-family"
											>
												{t("settings.fontFamily")}
											</label>
											<select
												id="html-font-family"
												value={htmlReaderTypography.fontFamily}
												onChange={(e) =>
													setHtmlReaderTypography({
														fontFamily: e.target.value as HtmlReaderFontFamily,
													})
												}
												className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm"
											>
												<option value="system">
													{t("settings.fontFamilySystem")}
												</option>
												<option value="serif">
													{t("settings.fontFamilySerif")}
												</option>
												<option value="sans-serif">
													{t("settings.fontFamilySansSerif")}
												</option>
												<option value="cjk">
													{t("settings.fontFamilyCjk")}
												</option>
												<option value="custom">
													{t("settings.fontFamilyCustom")}
												</option>
											</select>
										</div>
										{htmlReaderTypography.fontFamily === "custom" && (
											<div className="flex-1 min-w-[140px]">
												<label
													className="text-xs text-muted-foreground"
													htmlFor="html-custom-font"
												>
													{t("settings.customFontFamily")}
												</label>
												<input
													id="html-custom-font"
													type="text"
													value={htmlReaderTypography.customFontFamily}
													onChange={(e) =>
														setHtmlReaderTypography({
															customFontFamily: e.target.value,
														})
													}
													placeholder={t(
														"settings.customFontFamilyPlaceholder",
													)}
													className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm"
												/>
											</div>
										)}
									</div>

									{/* Sliders in 2-column grid */}
									<div className="grid grid-cols-2 gap-x-6 gap-y-3">
										{/* Font Size */}
										<div>
											<div className="flex items-center justify-between">
												<label
													className="text-xs text-muted-foreground"
													htmlFor="html-font-size"
												>
													{t("settings.fontSize")}
												</label>
												<span className="text-xs text-muted-foreground tabular-nums">
													{htmlReaderTypography.fontSize}px
												</span>
											</div>
											<input
												id="html-font-size"
												type="range"
												min="12"
												max="24"
												step="1"
												value={htmlReaderTypography.fontSize}
												onChange={(e) =>
													setHtmlReaderTypography({
														fontSize: Number(e.target.value),
													})
												}
												className="mt-1 w-full accent-primary"
											/>
										</div>

										{/* Line Height */}
										<div>
											<div className="flex items-center justify-between">
												<label
													className="text-xs text-muted-foreground"
													htmlFor="html-line-height"
												>
													{t("settings.lineHeight")}
												</label>
												<span className="text-xs text-muted-foreground tabular-nums">
													{htmlReaderTypography.lineHeight.toFixed(1)}
												</span>
											</div>
											<input
												id="html-line-height"
												type="range"
												min="1.2"
												max="2.4"
												step="0.1"
												value={htmlReaderTypography.lineHeight}
												onChange={(e) =>
													setHtmlReaderTypography({
														lineHeight: Number(e.target.value),
													})
												}
												className="mt-1 w-full accent-primary"
											/>
										</div>

										{/* Font Weight */}
										<div>
											<div className="flex items-center justify-between">
												<label
													className="text-xs text-muted-foreground"
													htmlFor="html-font-weight"
												>
													{t("settings.fontWeight")}
												</label>
												<span className="text-xs text-muted-foreground tabular-nums">
													{htmlReaderTypography.fontWeight <= 300
														? t("settings.fontWeightLight")
														: htmlReaderTypography.fontWeight >= 600
															? t("settings.fontWeightBold")
															: t("settings.fontWeightNormal")}
												</span>
											</div>
											<input
												id="html-font-weight"
												type="range"
												min="300"
												max="700"
												step="100"
												value={htmlReaderTypography.fontWeight}
												onChange={(e) =>
													setHtmlReaderTypography({
														fontWeight: Number(e.target.value),
													})
												}
												className="mt-1 w-full accent-primary"
											/>
										</div>

										{/* Max Content Width */}
										<div>
											<div className="flex items-center justify-between">
												<label
													className="text-xs text-muted-foreground"
													htmlFor="html-max-width"
												>
													{t("settings.maxContentWidth")}
												</label>
												<span className="text-xs text-muted-foreground tabular-nums">
													{htmlReaderTypography.maxWidth === 0
														? t("settings.maxContentWidthUnlimited")
														: `${htmlReaderTypography.maxWidth}px`}
												</span>
											</div>
											<input
												id="html-max-width"
												type="range"
												min="0"
												max="1200"
												step="50"
												value={htmlReaderTypography.maxWidth}
												onChange={(e) =>
													setHtmlReaderTypography({
														maxWidth: Number(e.target.value),
													})
												}
												className="mt-1 w-full accent-primary"
											/>
										</div>
									</div>

									{/* Reset */}
									<Button
										variant="outline"
										size="sm"
										onClick={resetHtmlReaderTypography}
									>
										{t("settings.resetTypography")}
									</Button>
								</div>

								<Separator />

								{/* Column layout */}
								<div className="space-y-2">
									<p className="text-xs font-medium">
										{t("settings.columnLayout")}
									</p>
									<p className="text-[11px] text-muted-foreground">
										{t("settings.resetColumnLayoutNote")}
									</p>
									<Button variant="outline" size="sm" onClick={resetColumns}>
										<RotateCcw className="mr-1.5 h-3.5 w-3.5" />
										{t("settings.resetColumnLayout")}
									</Button>
								</div>

								<Separator />

								{/* Developer */}
								<div className="space-y-2">
									<p className="text-xs font-medium">
										{t("settings.developer")}
									</p>
									<label className="flex items-center gap-2 text-sm cursor-pointer">
										<input
											type="checkbox"
											checked={showBackgroundTasks}
											onChange={(e) => setShowBackgroundTasks(e.target.checked)}
											className="rounded"
										/>
										{t("settings.showBackgroundTasks")}
									</label>
									<p className="text-[11px] text-muted-foreground ml-5">
										{t("settings.showBackgroundTasksDesc")}
									</p>
									<label className="flex items-center gap-2 text-sm cursor-pointer">
										<input
											type="checkbox"
											checked={debugMode}
											onChange={(e) => handleDebugToggle(e.target.checked)}
											className="rounded"
										/>
										{t("settings.debugMode")}
									</label>
									{debugMode && (
										<p className="text-[11px] text-muted-foreground ml-5">
											{t("settings.debugModeDesc")}
										</p>
									)}
									{import.meta.env.DEV && (
										<>
											<label className="flex items-center gap-2 text-sm cursor-pointer">
												<input
													type="checkbox"
													checked={disableNativeContextMenu}
													onChange={(e) =>
														setDisableNativeContextMenu(e.target.checked)
													}
													className="rounded"
												/>
												{t("settings.disableContextMenu")}
											</label>
											<p className="text-[11px] text-muted-foreground ml-5">
												{t("settings.disableContextMenuDesc")}
											</p>
										</>
									)}
								</div>

								<Separator />

								{/* Restart Onboarding */}
								<div className="space-y-2">
									<p className="text-xs font-medium">
										{t("settings.restartOnboarding")}
									</p>
									<p className="text-[11px] text-muted-foreground">
										{t("settings.restartOnboardingDesc")}
									</p>
									<Button
										variant="outline"
										size="sm"
										onClick={() => useUiStore.getState().restartOnboarding()}
									>
										<GraduationCap className="mr-1.5 h-3.5 w-3.5" />
										{t("settings.restartOnboarding")}
									</Button>
								</div>
							</div>
						)}

						{section === "ai-general" && (
							<div className="space-y-5">
								{/* Native Language */}
								<div>
									<label className="text-xs font-medium" htmlFor="native-lang">
										{t("settings.nativeLang")}
									</label>
									<div className="flex gap-2 mt-1">
										<select
											id="native-lang"
											value={
												[
													"zh",
													"ja",
													"ko",
													"es",
													"fr",
													"de",
													"pt",
													"ru",
													"ar",
													"it",
													"vi",
													"th",
													"id",
													"hi",
													"en",
													"",
												].includes(aiNativeLang)
													? aiNativeLang
													: "__custom__"
											}
											onChange={(e) => {
												const v = e.target.value;
												if (v === "__custom__") {
													setAiNativeLang("__custom__");
												} else {
													setAiNativeLang(v);
													setCustomNativeLang("");
												}
											}}
											className="h-8 flex-1 rounded-md border bg-transparent px-2 text-sm"
										>
											<option value="">{t("settings.disabled")}</option>
											<option value="zh">简体中文</option>
											<option value="ja">日本語</option>
											<option value="ko">한국어</option>
											<option value="es">Español</option>
											<option value="fr">Français</option>
											<option value="de">Deutsch</option>
											<option value="pt">Português</option>
											<option value="ru">Русский</option>
											<option value="ar">العربية</option>
											<option value="it">Italiano</option>
											<option value="vi">Tiếng Việt</option>
											<option value="th">ไทย</option>
											<option value="id">Bahasa Indonesia</option>
											<option value="hi">हिन्दी</option>
											<option value="en">English</option>
											<option value="__custom__">
												{t("settings.customLang")}
											</option>
										</select>
										{aiNativeLang === "__custom__" && (
											<input
												type="text"
												placeholder={t("settings.customLangPlaceholder")}
												value={customNativeLang}
												onChange={(e) => {
													setCustomNativeLang(e.target.value);
												}}
												className="h-8 w-32 rounded-md border bg-transparent px-2 text-sm"
											/>
										)}
									</div>
									<p className="text-[11px] text-muted-foreground mt-1">
										{t("settings.nativeLangDesc")}
									</p>
								</div>

								<Separator />

								{/* Unified Providers */}
								<div>
									<div className="flex items-center justify-between mb-2">
										<h4 className="text-sm font-semibold">
											{t("settings.providers")}
										</h4>
										<Button
											variant="outline"
											size="sm"
											onClick={() =>
												setEditingProvider({
													id: crypto.randomUUID(),
													name: "",
													baseUrl: "",
													apiKey: "",
													models: "",
												})
											}
										>
											<Plus className="mr-1.5 h-3.5 w-3.5" />
											{t("settings.addProvider")}
										</Button>
									</div>
									<p className="text-[11px] text-muted-foreground mb-3">
										{t("settings.providersDesc")}
									</p>

									{/* Provider list */}
									{aiProviders.length > 0 && (
										<div className="space-y-2 mb-3">
											{aiProviders.map((p) => (
												<div
									key={p.id}
									className="flex items-center justify-between rounded-md border p-2.5"
												>
													<div className="min-w-0 flex-1">
														<div className="flex items-center gap-1.5">
															<span className="text-sm font-medium truncate">
																{p.name || t("settings.unnamed")}
															</span>

														</div>
													<div className="text-[11px] text-muted-foreground truncate">
															{p.id === "__lab_router" ? (
																<span>{t("settings.labSmartRouterDesc")}</span>
															) : (
																<>
																	{p.baseUrl || t("settings.noUrlSet")}
																	{p.models.length > 0 && (
																		<span className="ml-2">
																			· {p.models.join(", ")}
																		</span>
																	)}
																	{(p.apiKeySet || pendingProviderKeys[p.id]) && (
																		<span className="ml-2 text-green-600">
																			· {t("settings.keySet")}
																		</span>
																	)}
																</>
															)}
														</div>
													</div>
											<div className="flex items-center gap-1 ml-2 shrink-0">
												{p.id === "__lab_router" ? (
													<Badge variant="outline" className="text-[10px] h-5 px-1.5">
														Auto
													</Badge>
												) : (
													<>
														<Button
															variant="ghost"
															size="sm"
															className="h-7 w-7 p-0"
															onClick={() =>
																setEditingProvider({
																	id: p.id,
																	name: p.name,
																	baseUrl: p.baseUrl,
																	apiKey: "",
																	models: p.models.join(", "),
																})
															}
														>
															<Pencil className="h-3.5 w-3.5" />
														</Button>
														{aiProviders.length > 1 && (
															<Button
																variant="ghost"
																size="sm"
																className="h-7 w-7 p-0 text-destructive"
																onClick={() => {
																	const remaining = aiProviders.filter(
																		(x) => x.id !== p.id,
																	);
															setAiProviders(remaining);
																}}
															>
																<Trash2 className="h-3.5 w-3.5" />
															</Button>
														)}
													</>
												)}
											</div>
												</div>
											))}
										</div>
									)}

									{/* Editing / Adding a provider */}
									{editingProvider && (
										<div className="rounded-md border p-3 space-y-2 bg-muted/30">
											<div>
												<label className="text-xs font-medium">
													{t("settings.nickname")}
												</label>
												<input
													type="text"
													placeholder="e.g. My Anthropic, Work OpenAI..."
													value={editingProvider.name}
													onChange={(e) =>
														setEditingProvider({
															...editingProvider,
															name: e.target.value,
														})
													}
													className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm"
												/>
											</div>
											<div>
												<label className="text-xs font-medium">
													{t("settings.apiBaseUrl")}
												</label>
												<input
													type="url"
													placeholder="https://api.openai.com/v1"
													value={editingProvider.baseUrl}
													onChange={(e) =>
														setEditingProvider({
															...editingProvider,
															baseUrl: e.target.value,
														})
													}
													className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm"
												/>
											</div>
											<div>
												<label className="text-xs font-medium">
													{t("settings.apiKey")}
												</label>
												<input
													type="password"
													placeholder={
														aiProviders.find((x) => x.id === editingProvider.id)
															?.apiKeySet
															? t("settings.apiKeyKeepPlaceholder")
															: "sk-..."
													}
													value={editingProvider.apiKey}
													onChange={(e) =>
														setEditingProvider({
															...editingProvider,
															apiKey: e.target.value,
														})
													}
													className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm"
												/>
											</div>
											<div>
												<label className="text-xs font-medium">
													{t("settings.models")}
												</label>
												<div className="mt-1 flex flex-wrap items-center gap-1.5 rounded-md border bg-transparent p-2 min-h-[2rem]">
													{editingProvider.models
														.split(",")
														.map((m) => m.trim())
														.filter(Boolean)
														.map((model) => (
															<Badge
																key={model}
																variant="secondary"
																className="text-xs gap-1 pr-1"
															>
																{model}
																<button
																	type="button"
																	className="ml-0.5 rounded-full p-0.5 hover:bg-muted-foreground/20"
																	onClick={() => {
																		const models = editingProvider.models
																			.split(",")
																			.map((m) => m.trim())
																			.filter((m) => m && m !== model);
																		setEditingProvider({
																			...editingProvider,
																			models: models.join(", "),
																		});
																	}}
																>
																	<Trash2 className="h-2.5 w-2.5" />
																</button>
															</Badge>
														))}
													<input
														type="text"
														placeholder={t("settings.modelNamePlaceholder")}
														className="flex-1 min-w-[120px] bg-transparent text-sm outline-none border-none h-6 px-1"
														onKeyDown={(e) => {
															if (
																e.key === "Enter" &&
																(e.target as HTMLInputElement).value.trim()
															) {
																e.preventDefault();
																const newModel = (
																	e.target as HTMLInputElement
																).value.trim();
																const existing = editingProvider.models
																	.split(",")
																	.map((m) => m.trim())
																	.filter(Boolean);
																if (!existing.includes(newModel)) {
																	setEditingProvider({
																		...editingProvider,
																		models: [...existing, newModel].join(", "),
																	});
																}
																(e.target as HTMLInputElement).value = "";
															}
														}}
													/>
												</div>
										<p className="text-[11px] text-muted-foreground mt-0.5">
											{t("settings.modelsDesc")}
										</p>
										<Button
											variant="outline"
											size="sm"
											className="mt-1.5 h-7 text-xs"
											disabled={!editingProvider.baseUrl || fetchingProviderModels}
											onClick={async () => {
												setFetchingProviderModels(true);
												try {
													const baseUrl = editingProvider.baseUrl.replace(/\/$/, "");
													const apiKey = editingProvider.apiKey || pendingProviderKeys[editingProvider.id] || "";
													const headers: Record<string, string> = { "Content-Type": "application/json" };
													if (apiKey) {
														headers.Authorization = `Bearer ${apiKey}`;
													}
													const resp = await commands.httpProxyGet(`${baseUrl}/models`, headers);
													if (resp.status >= 200 && resp.status < 300) {
														const json = JSON.parse(resp.body);
														let modelIds: string[] = [];
														if (Array.isArray(json.data)) {
															modelIds = json.data.map((m: { id?: string }) => m.id ?? "").filter(Boolean);
														} else if (Array.isArray(json.models)) {
															modelIds = json.models.map((m: { name?: string; id?: string }) => m.id || m.name?.replace("models/", "") || "").filter(Boolean);
														}
														if (modelIds.length > 0) {
															const existing = editingProvider.models.split(",").map((m) => m.trim()).filter(Boolean);
															const merged = Array.from(new Set([...existing, ...modelIds]));
															setEditingProvider({ ...editingProvider, models: merged.join(", ") });
														}
													}
												} catch (err) {
													console.error("Failed to fetch models:", err);
												} finally {
													setFetchingProviderModels(false);
												}
											}}
										>
											{fetchingProviderModels ? (
												<Loader2 className="mr-1 h-3 w-3 animate-spin" />
											) : (
												<RefreshCw className="mr-1 h-3 w-3" />
											)}
											{t("settings.fetchModels")}
										</Button>
											</div>
											<div className="flex items-center gap-2 pt-1">
												<Button
													variant="default"
													size="sm"
													disabled={
														!editingProvider.name || !editingProvider.baseUrl
													}
													onClick={() => {
														const models = editingProvider.models
															.split(",")
															.map((m) => m.trim())
															.filter(Boolean);
														// Store pending API key if provided
														if (editingProvider.apiKey) {
															setPendingProviderKeys((prev) => ({
																...prev,
																[editingProvider.id]: editingProvider.apiKey,
															}));
														}
														const existing = aiProviders.find(
															(p) => p.id === editingProvider.id,
														);
														if (existing) {
															// Update existing
															setAiProviders((prev) =>
																prev.map((p) =>
																	p.id === editingProvider.id
																		? {
																				...p,
																				name: editingProvider.name,
																				baseUrl: editingProvider.baseUrl,
																				apiKeySet: editingProvider.apiKey
																					? true
																					: p.apiKeySet,
																				models,
																			}
																		: p,
																),
															);
														} else {
															// Add new
															setAiProviders((prev) => [
																...prev,
																{
																	id: editingProvider.id,
																	name: editingProvider.name,
																	baseUrl: editingProvider.baseUrl,
																	apiKeySet: !!editingProvider.apiKey,
																	models,
																},
															]);
														}
														setEditingProvider(null);
													}}
												>
													{aiProviders.find((p) => p.id === editingProvider.id)
														? t("common.update")
														: t("common.add")}
												</Button>
												<Button
													variant="outline"
													size="sm"
													onClick={() => setEditingProvider(null)}
												>
													{t("common.cancel")}
												</Button>
											</div>
										</div>
									)}
								</div>

							{/* --- Global Default Model --- */}
								<div className="space-y-2 border rounded-lg p-3">
									<div>
										<h4 className="text-sm font-medium">
											{t("settings.globalDefaultModel")}
										</h4>
										<p className="text-[11px] text-muted-foreground mt-0.5">
											{t("settings.globalDefaultModelDesc")}
										</p>
									</div>
									{(() => {
										const providerModels = Array.from(
											new Set(aiProviders.flatMap((p) => p.models)),
										).sort();
										const freeModels = labFreeLlmEnabled
											? labAvailableModels.filter(
													(m) => !providerModels.includes(m.id),
												)
											: [];
										const hasModels = providerModels.length > 0 || freeModels.length > 0;
										return (
											<select
												value={globalDefaultModel}
												onChange={(e) => setGlobalDefaultModel(e.target.value)}
												className="h-8 w-full rounded-md border bg-transparent px-2 text-sm"
											>
												{!hasModels && (
													<option value="">
														{t("settings.noModelsConfigured")}
													</option>
												)}
											{providerModels.map((m) => (
													<option key={m} value={m}>
														{m === "__lab_auto__" ? "🧪 Lab Smart Router (Auto)" : m}
													</option>
												))}
												{freeModels.length > 0 && providerModels.length > 0 && (
													<option disabled>────────────</option>
												)}
										{freeModels.map((m) => (
											<option key={m.id} value={m.id}>
												{m.displayName}
											</option>
												))}
											</select>
										);
									})()}
								</div>

								{/* --- Task Model Defaults --- */}
								<div className="space-y-3 border rounded-lg p-3">
									<div>
										<h4 className="text-sm font-medium">
											{t("settings.taskModelOverrides")}
										</h4>
										<p className="text-[11px] text-muted-foreground mt-0.5">
											{t("settings.taskModelOverridesDesc")}
										</p>
									</div>
									{(() => {
										// Collect all unique model names across all providers
										const providerModels = Array.from(
											new Set(aiProviders.flatMap((p) => p.models)),
										).sort();
										const freeModels = labFreeLlmEnabled
											? labAvailableModels.filter(
													(m) => !providerModels.includes(m.id),
												)
											: [];
									const modelOptions = [
											{ value: "", label: t("settings.useGlobalDefault") },
											...providerModels.map((m) => ({ value: m, label: m === "__lab_auto__" ? "🧪 Lab Smart Router (Auto)" : m })),
										...freeModels.map((m) => ({
											value: m.id,
											label: m.displayName,
										})),
										];
										return (
											<div className="grid gap-2.5">
												<div className="flex items-center gap-3">
													<label className="text-xs font-medium w-[140px] shrink-0 flex items-center gap-1.5">
														⚡ {t("settings.quickTranslation")}
													</label>
													<select
														value={taskQuickModel}
														onChange={(e) => setTaskQuickModel(e.target.value)}
														className="h-7 flex-1 rounded-md border bg-transparent px-2 text-xs"
													>
														{modelOptions.map((o) => (
															<option key={o.value} value={o.value}>
																{o.label}
															</option>
														))}
													</select>
												</div>
												<p className="text-[10px] text-muted-foreground ml-[152px] -mt-1.5">
													{t("settings.quickTranslationDesc")}
												</p>

												<div className="flex items-center gap-3">
													<label className="text-xs font-medium w-[140px] shrink-0 flex items-center gap-1.5">
														🔄 {t("settings.normalTranslation")}
													</label>
													<select
														value={taskNormalModel}
														onChange={(e) => setTaskNormalModel(e.target.value)}
														className="h-7 flex-1 rounded-md border bg-transparent px-2 text-xs"
													>
														{modelOptions.map((o) => (
															<option key={o.value} value={o.value}>
																{o.label}
															</option>
														))}
													</select>
												</div>
												<p className="text-[10px] text-muted-foreground ml-[152px] -mt-1.5">
													{t("settings.normalTranslationDesc")}
												</p>

												<div className="flex items-center gap-3">
													<label className="text-xs font-medium w-[140px] shrink-0 flex items-center gap-1.5">
														🐢 {t("settings.heavyTranslation")}
													</label>
													<select
														value={taskHeavyModel}
														onChange={(e) => setTaskHeavyModel(e.target.value)}
														className="h-7 flex-1 rounded-md border bg-transparent px-2 text-xs"
													>
														{modelOptions.map((o) => (
															<option key={o.value} value={o.value}>
																{o.label}
															</option>
														))}
													</select>
												</div>
												<p className="text-[10px] text-muted-foreground ml-[152px] -mt-1.5">
													{t("settings.heavyTranslationDesc")}
												</p>

												<div className="flex items-center gap-3">
													<label className="text-xs font-medium w-[140px] shrink-0 flex items-center gap-1.5">
														📖 {t("settings.glossaryExtraction")}
													</label>
													<select
														value={taskGlossaryModel}
														onChange={(e) =>
															setTaskGlossaryModel(e.target.value)
														}
														className="h-7 flex-1 rounded-md border bg-transparent px-2 text-xs"
													>
														{modelOptions.map((o) => (
															<option key={o.value} value={o.value}>
																{o.label}
															</option>
														))}
													</select>
												</div>
												<p className="text-[10px] text-muted-foreground ml-[152px] -mt-1.5">
													{t("settings.glossaryExtractionDesc")}
												</p>
											</div>
										);
									})()}
								</div>

								{aiTestResult && (
									<div
										className={`flex items-center gap-2 rounded-md border p-2 text-xs ${
											aiTestResult.ok
												? "border-green-500/30 bg-green-50 text-green-700 dark:bg-green-950 dark:text-green-300"
												: "border-red-500/30 bg-red-50 text-red-700 dark:bg-red-950 dark:text-red-300"
										}`}
									>
										{aiTestResult.ok ? (
											<CheckCircle className="h-3.5 w-3.5 shrink-0" />
										) : (
											<AlertCircle className="h-3.5 w-3.5 shrink-0" />
										)}
										{aiTestResult.msg}
									</div>
								)}

								<div className="flex items-center gap-2 flex-wrap">
									<Button
										variant="outline"
										size="sm"
										onClick={handleAiTest}
									disabled={
											aiTesting ||
											!globalDefaultModel
										}
									>
										{aiTesting ? (
											<>
												<Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
												{t("settings.testing")}
											</>
										) : (
											t("settings.testConnection")
										)}
									</Button>
									<Button
										variant="default"
										size="sm"
										onClick={handleAiSave}
										disabled={aiSaving}
									>
										{aiSaving ? (
											<>
												<Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
												{t("common.saving")}
											</>
										) : (
											t("common.save")
										)}
									</Button>
								</div>
							</div>
						)}

						{section === "ai-translation" && (
							<div className="space-y-5">
								<div className="grid grid-cols-1 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.2fr)] gap-6">
									{/* Left column: Translation config */}
									<div className="space-y-3">
										<label className="flex items-center gap-2 text-sm cursor-pointer">
											<input
												type="checkbox"
												checked={aiAutoTranslate}
												onChange={(e) => setAiAutoTranslate(e.target.checked)}
												className="rounded"
											/>
											{t("settings.autoTranslate")}
										</label>
										<p className="text-[11px] text-muted-foreground ml-5">
											{t("settings.autoTranslateDesc")}
										</p>

										<div>
											<label
												className="text-xs font-medium"
												htmlFor="html-concurrency"
											>
												{t("settings.htmlConcurrency")}
											</label>
											<div className="flex items-center gap-2 mt-1">
												<input
													id="html-concurrency"
													type="number"
													min={1}
													max={32}
													value={htmlConcurrency}
													onChange={(e) =>
														setHtmlConcurrency(
															Math.max(
																1,
																Math.min(
																	32,
																	Number.parseInt(e.target.value) || 8,
																),
															),
														)
													}
													className="h-8 w-20 rounded-md border bg-transparent px-2 text-sm"
												/>
												<span className="text-xs text-muted-foreground">
													{t("settings.parallelRequests")}
												</span>
											</div>
											<p className="text-[11px] text-muted-foreground mt-1">
												{t("settings.htmlConcurrencyDesc")}
											</p>
										</div>
									</div>
									<div className="space-y-3">
										<div className="flex items-center justify-between">
											<p className="text-xs font-medium">
												{t("settings.customTranslationPrompts")}
											</p>
											<Button
												variant="ghost"
												size="sm"
												className="text-xs h-7"
												onClick={handleResetPrompts}
											>
												<RotateCcw className="mr-1 h-3 w-3" />
												{t("settings.resetToDefaults")}
											</Button>
										</div>
										<p className="text-[11px] text-muted-foreground">
											{t("settings.promptVariablesDesc")}
										</p>
										<div className="space-y-3">
											<div className="space-y-2">
												<p className="text-[11px] font-semibold text-muted-foreground uppercase tracking-wide">
													{t("settings.promptTitle")}
												</p>
												<div>
													<label className="text-[11px] font-medium">
														{t("settings.systemPrompt")}
														<textarea
															value={promptTitleSystem}
															onChange={(e) =>
																setPromptTitleSystem(e.target.value)
															}
															rows={4}
															className="mt-0.5 w-full rounded-md border bg-transparent px-2 py-1 text-xs font-mono resize-y"
														/>
													</label>
												</div>
												<div>
													<label className="text-[11px] font-medium">
														{t("settings.userPrompt")}
														<textarea
															value={promptTitleUser}
															onChange={(e) =>
																setPromptTitleUser(e.target.value)
															}
															rows={3}
															className="mt-0.5 w-full rounded-md border bg-transparent px-2 py-1 text-xs font-mono resize-y"
														/>
													</label>
												</div>
											</div>
											<div className="space-y-2">
												<p className="text-[11px] font-semibold text-muted-foreground uppercase tracking-wide">
													{t("settings.promptAbstract")}
												</p>
												<div>
													<label className="text-[11px] font-medium">
														{t("settings.systemPrompt")}
														<textarea
															value={promptAbstractSystem}
															onChange={(e) =>
																setPromptAbstractSystem(e.target.value)
															}
															rows={4}
															className="mt-0.5 w-full rounded-md border bg-transparent px-2 py-1 text-xs font-mono resize-y"
														/>
													</label>
												</div>
												<div>
													<label className="text-[11px] font-medium">
														{t("settings.userPrompt")}
														<textarea
															value={promptAbstractUser}
															onChange={(e) =>
																setPromptAbstractUser(e.target.value)
															}
															rows={3}
															className="mt-0.5 w-full rounded-md border bg-transparent px-2 py-1 text-xs font-mono resize-y"
														/>
													</label>
												</div>
											</div>
										</div>
									</div>
								</div>

								<Separator className="my-4" />

								{/* Glossary */}
								<GlossarySection
									aiConfig={aiConfig}
									glossaryEnabled={glossaryEnabled}
									setGlossaryEnabled={setGlossaryEnabled}
									glossaryThreshold={glossaryThreshold}
									setGlossaryThreshold={setGlossaryThreshold}
									onSaveGlossaryConfig={handleGlossaryConfigSave}
								/>

								<Separator className="my-4" />

								<div className="flex items-center gap-2 flex-wrap">
									<Button
										variant="default"
										size="sm"
										onClick={handleAiSave}
										disabled={aiSaving}
									>
										{aiSaving ? (
											<>
												<Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
												{t("common.saving")}
											</>
										) : (
											t("common.save")
										)}
									</Button>
								</div>
							</div>
						)}

						{section === "ai-pdf-translation" && (
							<div className="space-y-5">
								{/* PDF Translation Service */}
								<div className="space-y-3">
									<p className="text-sm font-semibold">
										{t("settings.pdfTranslationService")}
									</p>
									<p className="text-[11px] text-muted-foreground">
										{t("settings.pdfTranslationDesc", {
											lang: aiNativeLang || "xx",
										})}
									</p>

									<label className="flex items-center gap-2 text-sm cursor-pointer">
										<input
											type="checkbox"
											checked={pdfEnabled}
											onChange={(e) => setPdfEnabled(e.target.checked)}
											className="rounded"
										/>
										{t("settings.enablePdfTranslation")}
									</label>

									{pdfEnabled && (
										<div className="space-y-3 pl-1">
											<div>
												<label
													className="text-xs font-medium"
													htmlFor="pdf-babeldoc-cmd"
												>
													{t("settings.pdfServerCommand")}
												</label>
												<input
													id="pdf-babeldoc-cmd"
													type="text"
													value={pdfBabeldocCmd}
													onChange={(e) => setPdfBabeldocCmd(e.target.value)}
													placeholder="babeldoc"
													className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm font-mono"
												/>
												<p className="text-[11px] text-muted-foreground mt-1">
													{t("settings.pdfServerCommandDesc")}
												</p>
											</div>

											<label className="flex items-center gap-2 text-sm cursor-pointer">
												<input
													type="checkbox"
													checked={pdfUseAiConfig}
													onChange={(e) => setPdfUseAiConfig(e.target.checked)}
													className="rounded"
												/>
												{t("settings.useMainAiConfig")}
											</label>

											{!pdfUseAiConfig && (
												<div className="space-y-2 rounded-md border p-3">
													<p className="text-[11px] font-medium text-muted-foreground">
														{t("settings.customApiConfigForPdf")}
													</p>
													<div>
														<label
															className="text-xs font-medium"
															htmlFor="pdf-base-url"
														>
															{t("settings.apiBaseUrl")}
														</label>
														<input
															id="pdf-base-url"
															type="url"
															value={pdfCustomBaseUrl}
															onChange={(e) =>
																setPdfCustomBaseUrl(e.target.value)
															}
															placeholder="https://api.openai.com/v1"
															className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm"
														/>
													</div>
													<div className="grid grid-cols-2 gap-3">
														<div>
															<label
																className="text-xs font-medium"
																htmlFor="pdf-api-key"
															>
																{t("settings.apiKey")}{" "}
																{aiConfig?.pdfTranslation?.customApiKeySet && (
																	<span className="text-green-600">
																		({t("settings.set")})
																	</span>
																)}
															</label>
															<input
																id="pdf-api-key"
																type="password"
																value={pdfCustomApiKey}
																onChange={(e) =>
																	setPdfCustomApiKey(e.target.value)
																}
																placeholder={
																	aiConfig?.pdfTranslation?.customApiKeySet
																		? "••••••••"
																		: "sk-..."
																}
																className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm"
															/>
														</div>
														<div>
															<label
																className="text-xs font-medium"
																htmlFor="pdf-model"
															>
																{t("settings.model")}
															</label>
															<input
																id="pdf-model"
																type="text"
																value={pdfCustomModel}
																onChange={(e) =>
																	setPdfCustomModel(e.target.value)
																}
																placeholder="gpt-4o-mini"
																className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm"
															/>
														</div>
													</div>
												</div>
											)}

											<div>
												<label
													className="text-xs font-medium"
													htmlFor="pdf-qps"
												>
													{t("settings.qps")}
												</label>
												<div className="flex items-center gap-2 mt-1">
													<input
														id="pdf-qps"
														type="number"
														min={1}
														max={20}
														value={pdfQps}
														onChange={(e) =>
															setPdfQps(
																Math.max(
																	1,
																	Math.min(
																		20,
																		Number.parseInt(e.target.value) || 4,
																	),
																),
															)
														}
														className="h-8 w-20 rounded-md border bg-transparent px-2 text-sm"
													/>
													<span className="text-xs text-muted-foreground">
														{t("settings.requestsPerSec")}
													</span>
												</div>
											</div>

											<div>
												<label
													className="text-xs font-medium"
													htmlFor="pdf-extra-args"
												>
													{t("settings.pdfExtraArgs")}
												</label>
												<input
													id="pdf-extra-args"
													type="text"
													value={pdfExtraArgs}
													onChange={(e) => setPdfExtraArgs(e.target.value)}
													placeholder="--no-dual"
													className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm font-mono"
												/>
												<p className="text-[11px] text-muted-foreground mt-1">
													{t("settings.pdfExtraArgsDesc")}
												</p>
											</div>

											<div className="flex items-center gap-2 flex-wrap">
												<Button
													variant="outline"
													size="sm"
													onClick={handlePdfTest}
													disabled={pdfTesting}
												>
													{pdfTesting ? (
														<>
															<Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
															{t("settings.testing")}
														</>
													) : (
														t("common.test")
													)}
												</Button>
												<Button
													variant="default"
													size="sm"
													onClick={handlePdfTranslationSave}
													disabled={pdfSaving}
												>
													{pdfSaving ? (
														<>
															<Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
															{t("common.saving")}
														</>
													) : (
														t("settings.savePdfSettings")
													)}
												</Button>
											</div>

											{pdfTestResult && (
												<div
													className={`mt-2 rounded-md p-3 text-xs whitespace-pre-wrap ${
														pdfTestResult.ok
															? "bg-green-50 text-green-800 dark:bg-green-950 dark:text-green-200"
															: "bg-red-50 text-red-800 dark:bg-red-950 dark:text-red-200"
													}`}
												>
													{pdfTestResult.msg}
												</div>
											)}

											<Separator />

											<div className="space-y-2">
												<p className="text-xs font-medium">
													{t("settings.installPdfServer")}
												</p>
												<p className="text-[11px] text-muted-foreground">
													{t("settings.installPdfServerDesc")}
												</p>
												<div className="rounded-md bg-muted p-3 text-xs select-all whitespace-pre-wrap break-all">
													Install the PDF translation server package (requires
													Python 3.12+) into an isolated environment so it does
													not pollute other projects. The recommended way is{" "}
													<code className="bg-background px-0.5 rounded">
														uv tool install --python 3.12 BabelDOC
													</code>
													, which automatically creates a dedicated virtualenv
													and puts the{" "}
													<code className="bg-background px-0.5 rounded">
														babeldoc
													</code>{" "}
													command on PATH. Alternatively, create a venv manually
													(e.g.{" "}
													<code className="bg-background px-0.5 rounded">
														python3.12 -m venv ~/.babeldoc-venv &&
														~/.babeldoc-venv/bin/pip install BabelDOC
													</code>
													) and set the PDF Translation Server Command above to
													the full path{" "}
													<code className="bg-background px-0.5 rounded">
														~/.babeldoc-venv/bin/babeldoc
													</code>
													. After installation, verify with{" "}
													<code className="bg-background px-0.5 rounded">
														babeldoc --help
													</code>
													.
												</div>
											</div>
										</div>
									)}
								</div>
							</div>
						)}

						{section === "ai-chat" && (
							<div className="space-y-5">
								<div className="space-y-3">
									<div className="flex items-center justify-between">
										<div>
											<h3 className="text-sm font-medium">
												{t("settings.systemPromptPresets")}
											</h3>
											<p className="text-xs text-muted-foreground">
												{t("settings.systemPromptPresetsDesc")}
											</p>
										</div>
										<Button
											size="sm"
											variant="outline"
											onClick={() =>
												setChatPresets([
													...chatPresets,
													{
														name: `Preset ${chatPresets.length + 1}`,
														prompt: "",
													},
												])
											}
										>
											<Plus className="mr-1 h-3 w-3" />
											{t("common.add")}
										</Button>
									</div>

									{chatPresets.map((preset, idx) => (
										<div
											key={`preset-${
												// biome-ignore lint/suspicious/noArrayIndexKey: stable list edited by user
												idx
											}`}
											className={cn(
												"rounded-md border p-3 space-y-2",
												chatActivePreset === preset.name &&
													"border-primary/50 bg-primary/5",
											)}
										>
											<div className="flex items-center gap-2">
												<input
													className="flex-1 rounded-md border bg-background px-2 py-1 text-sm font-medium focus:outline-none focus:ring-1 focus:ring-ring"
													value={preset.name}
													onChange={(e) => {
														const updated = [...chatPresets];
														if (chatActivePreset === updated[idx].name) {
															setChatActivePreset(e.target.value);
														}
														updated[idx] = {
															...updated[idx],
															name: e.target.value,
														};
														setChatPresets(updated);
													}}
													placeholder={t("settings.presetName")}
												/>
												<Button
													size="sm"
													variant={
														chatActivePreset === preset.name
															? "default"
															: "outline"
													}
													className="text-xs shrink-0"
													onClick={() => setChatActivePreset(preset.name)}
												>
													{chatActivePreset === preset.name
														? t("settings.active")
														: t("settings.setActive")}
												</Button>
												<Button
													size="icon"
													variant="ghost"
													className="h-7 w-7 shrink-0 text-muted-foreground hover:text-destructive"
													onClick={() => {
														const updated = chatPresets.filter(
															(_, i) => i !== idx,
														);
														setChatPresets(updated);
														if (
															chatActivePreset === preset.name &&
															updated.length > 0
														) {
															setChatActivePreset(updated[0].name);
														}
													}}
												>
													<Trash2 className="h-3.5 w-3.5" />
												</Button>
											</div>
											<textarea
												className="w-full rounded-md border bg-background px-2 py-1.5 text-xs resize-none focus:outline-none focus:ring-1 focus:ring-ring min-h-[80px] max-h-[200px]"
												value={preset.prompt}
												onChange={(e) => {
													const updated = [...chatPresets];
													updated[idx] = {
														...updated[idx],
														prompt: e.target.value,
													};
													setChatPresets(updated);
												}}
												rows={3}
												placeholder={t("settings.enterSystemPrompt")}
											/>
										</div>
									))}

									{chatPresets.length === 0 && (
										<p className="text-xs text-muted-foreground py-4 text-center">
											{t("settings.noPresetsYet")}
										</p>
									)}
								</div>

								<Separator />

								<div className="space-y-3">
									<h3 className="text-sm font-medium">
										{t("settings.toolCallConfirmation")}
									</h3>
									<div className="flex items-center justify-between">
										<div>
											<p className="text-sm">{t("settings.confirmWriteOps")}</p>
											<p className="text-xs text-muted-foreground">
												{t("settings.confirmWriteOpsDesc")}
											</p>
										</div>
										<button
											type="button"
											className={cn(
												"relative inline-flex h-5 w-9 items-center rounded-full transition-colors",
												chatConfirmToolCalls
													? "bg-primary"
													: "bg-muted-foreground/30",
											)}
											onClick={() =>
												setChatConfirmToolCalls(!chatConfirmToolCalls)
											}
										>
											<span
												className={cn(
													"inline-block h-4 w-4 rounded-full bg-white transition-transform",
													chatConfirmToolCalls
														? "translate-x-[18px]"
														: "translate-x-[2px]",
												)}
											/>
										</button>
									</div>
								</div>

								<Separator />

								<div className="flex items-center gap-2">
									<Button
										onClick={async () => {
											setChatSaving(true);
											try {
												await commands.chatUpdateConfig({
													activePreset: chatActivePreset,
													confirmToolCalls: chatConfirmToolCalls,
													presets: chatPresets,
												});
											} catch (err) {
												console.error("Failed to save chat config:", err);
											} finally {
												setChatSaving(false);
											}
										}}
										disabled={chatSaving}
										size="sm"
									>
										{chatSaving ? (
											<Loader2 className="mr-1 h-3 w-3 animate-spin" />
										) : null}
										{t("common.save")}
									</Button>
								</div>
							</div>
						)}

						{section === "connector" && connectorStatus && (
							<div className="space-y-3 text-sm">
								<div className="flex items-center gap-2">
									<span>{t("settings.zoroConnector")}</span>
									<Badge
										variant={connectorStatus.running ? "default" : "secondary"}
									>
										{connectorStatus.running
											? t("settings.running")
											: t("settings.stopped")}
									</Badge>
									<code className="bg-muted px-1 rounded text-xs">
										:{connectorStatus.port}
									</code>
								</div>

								<Separator />

								<div className="space-y-2">
									<label className="flex items-center gap-2 cursor-pointer">
										<input
											type="checkbox"
											checked={connectorStatus.zotero_compat_enabled}
											disabled={zoteroCompatSaving}
											onChange={(e) =>
												handleZoteroCompatToggle(e.target.checked)
											}
											className="rounded"
										/>
										<span>{t("settings.enableZoteroCompat")}</span>
									</label>
									<p className="text-[11px] text-muted-foreground ml-5">
										{t("settings.zoteroCompatDesc", {
											port: connectorStatus.zotero_compat_port,
										})}
									</p>
									{connectorStatus.zotero_compat_enabled && (
										<div className="flex items-center gap-2 ml-5">
											<span className="text-xs">{t("settings.status")}:</span>
											<Badge
												variant={
													connectorStatus.zotero_compat_running
														? "default"
														: "secondary"
												}
											>
												{connectorStatus.zotero_compat_running
													? t("settings.running")
													: t("settings.stopped")}
											</Badge>
											<code className="bg-muted px-1 rounded text-xs">
												:{connectorStatus.zotero_compat_port}
											</code>
										</div>
									)}
									{connectorStatus.zotero_compat_error && (
										<div className="flex items-start gap-2 ml-5 rounded-md border border-amber-500/30 bg-amber-50 p-2 text-xs text-amber-700 dark:bg-amber-950 dark:text-amber-300">
											<AlertCircle className="h-3.5 w-3.5 shrink-0 mt-0.5" />
											<span>{connectorStatus.zotero_compat_error}</span>
										</div>
									)}
								</div>
							</div>
						)}

						{section === "ai-mcp" && (
							<div className="space-y-5">
								{/* Status overview */}
								<div className="space-y-3">
									<div className="flex items-center gap-3">
										<span className="text-sm">{t("settings.mcpServer")}:</span>
										<Badge
											variant={mcpStatus?.running ? "default" : "secondary"}
										>
											{mcpStatus?.running
												? t("settings.running")
												: t("settings.stopped")}
										</Badge>
										{mcpStatus?.running && mcpStatus.pid && (
											<span className="text-xs text-muted-foreground">
												PID {mcpStatus.pid}
											</span>
										)}
									</div>
									{mcpStatus && !mcpStatus.binary_found && (
										<div className="flex items-center gap-2 rounded-md border border-amber-500/30 bg-amber-50 p-2 text-xs text-amber-700 dark:bg-amber-950 dark:text-amber-300">
											<AlertCircle className="h-3.5 w-3.5 shrink-0" />
											{t("settings.mcpBinaryNotFound")}
										</div>
									)}
								</div>

								<Separator />

								{/* Auto-start toggle */}
								<div className="space-y-2">
									<label className="flex items-center gap-2 text-sm cursor-pointer">
										<input
											type="checkbox"
											checked={mcpStatus?.enabled ?? false}
											disabled={mcpStarting || !mcpStatus?.binary_found}
											onChange={(e) => handleMcpToggle(e.target.checked)}
											className="rounded"
										/>
										<span>{t("settings.autoStartMcp")}</span>
									</label>
									<p className="text-[11px] text-muted-foreground ml-5">
										{t("settings.autoStartMcpDesc")}
									</p>
								</div>

								<Separator />

								{/* Config */}
								<div className="space-y-3">
									<p className="text-xs font-medium">
										{t("settings.configuration")}
									</p>
									<div className="grid grid-cols-2 gap-3">
										<div>
											<label
												className="text-xs text-muted-foreground"
												htmlFor="mcp-transport"
											>
												{t("settings.transport")}
											</label>
											<select
												id="mcp-transport"
												value={mcpTransport}
												onChange={(e) => setMcpTransport(e.target.value)}
												className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm"
											>
												<option value="http">HTTP (Streamable)</option>
												<option value="stdio">{t("settings.stdio")}</option>
											</select>
										</div>
										<div>
											<label
												className="text-xs text-muted-foreground"
												htmlFor="mcp-port"
											>
												{t("settings.httpPort")}
											</label>
											<input
												id="mcp-port"
												type="number"
												min={1024}
												max={65535}
												value={mcpPort}
												onChange={(e) =>
													setMcpPort(
														Math.max(
															1024,
															Number.parseInt(e.target.value) || 23121,
														),
													)
												}
												disabled={mcpTransport !== "http"}
												className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm disabled:opacity-50"
											/>
										</div>
									</div>
									<Button
										variant="outline"
										size="sm"
										onClick={handleMcpSave}
										disabled={
											mcpSaving ||
											(mcpPort === mcpStatus?.port &&
												mcpTransport === mcpStatus?.transport)
										}
									>
										{mcpSaving ? (
											<>
												<Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
												{t("common.saving")}
											</>
										) : (
											t("settings.saveConfiguration")
										)}
									</Button>
								</div>

								<Separator />

								{/* Controls */}
								<div className="space-y-3">
									<p className="text-xs font-medium">
										{t("settings.controls")}
									</p>
									<div className="flex items-center gap-2 flex-wrap">
										{mcpStatus?.running ? (
											<>
												<Button
													variant="outline"
													size="sm"
													onClick={handleMcpStop}
													disabled={mcpStarting}
												>
													{mcpStarting ? (
														<Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
													) : (
														<Square className="mr-1.5 h-3.5 w-3.5" />
													)}
													{t("settings.stop")}
												</Button>
												<Button
													variant="outline"
													size="sm"
													onClick={handleMcpRestart}
													disabled={mcpStarting}
												>
													{mcpStarting ? (
														<Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
													) : (
														<RefreshCw className="mr-1.5 h-3.5 w-3.5" />
													)}
													{t("settings.restart")}
												</Button>
											</>
										) : (
											<Button
												variant="default"
												size="sm"
												onClick={handleMcpStart}
												disabled={mcpStarting || !mcpStatus?.binary_found}
											>
												{mcpStarting ? (
													<Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
												) : (
													<Play className="mr-1.5 h-3.5 w-3.5" />
												)}
												{t("settings.start")}
											</Button>
										)}
									</div>
								</div>

								<Separator />

								{/* Connection info */}
								{mcpStatus?.running && mcpTransport === "http" && (
									<div className="space-y-2">
										<p className="text-xs font-medium">
											{t("settings.connectionInfo")}
										</p>
										<div className="space-y-1.5">
											<div className="flex items-center gap-2 text-xs">
												<span className="text-muted-foreground">
													{t("settings.endpoint")}:
												</span>
												<code className="bg-muted px-1.5 py-0.5 rounded">
													http://127.0.0.1:{mcpStatus.port}/mcp
												</code>
											</div>
										</div>
										<p className="text-[11px] text-muted-foreground">
											{t("settings.connectionInfoDesc")}
										</p>
									</div>
								)}

								{/* Usage hint */}
								<div className="space-y-2">
									<p className="text-xs font-medium">
										{t("settings.aboutMcp")}
									</p>
									<p className="text-[11px] text-muted-foreground leading-relaxed">
										{t("settings.aboutMcpDesc")}
									</p>
								</div>
							</div>
						)}

						{section === "subscriptions" && (
							<div className="space-y-2">
								{subscriptions.length === 0 && (
									<p className="text-sm text-muted-foreground">
										{t("settings.noSubscriptions")}
									</p>
								)}
								{subscriptions.map((sub) => (
									<div key={sub.id} className="rounded-md border p-3 space-y-2">
										<div className="flex items-center justify-between">
											<div>
												<p className="font-medium text-sm">{sub.name}</p>
												<p className="text-[11px] text-muted-foreground">
													{t("settings.pollsEvery", {
														minutes: sub.poll_interval_minutes,
													})}
													{sub.last_polled &&
														` · ${t("settings.lastPolled")}: ${new Date(sub.last_polled).toLocaleString()}`}
												</p>
											</div>
											<div className="flex items-center gap-2">
												<Button
													variant="ghost"
													size="sm"
													className="h-7 px-2 text-xs"
													disabled={refreshingSubId === sub.id || !sub.enabled}
													onClick={() => handleRefreshSubscription(sub.id)}
												>
													{refreshingSubId === sub.id ? (
														<Loader2 className="mr-1 h-3 w-3 animate-spin" />
													) : (
														<RefreshCw className="mr-1 h-3 w-3" />
													)}
													{t("settings.refresh")}
												</Button>
												<button
													type="button"
													disabled={togglingSubId === sub.id}
													onClick={() =>
														handleToggleSubscription(sub.id, !sub.enabled)
													}
													className={cn(
														"relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors",
														sub.enabled
															? "bg-primary"
															: "bg-muted-foreground/30",
														togglingSubId === sub.id && "opacity-50",
													)}
												>
													<span
														className={cn(
															"pointer-events-none block h-4 w-4 rounded-full bg-background shadow-sm transition-transform",
															sub.enabled ? "translate-x-4" : "translate-x-0",
														)}
													/>
												</button>
											</div>
										</div>
										{refreshResult?.subId === sub.id && (
											<p className="text-[11px] text-green-600 dark:text-green-400">
												{t("settings.fetchedNewItems", {
													count: refreshResult.count,
												})}
											</p>
										)}
									</div>
								))}
							</div>
						)}

						{section === "storage" && storageInfo && (
							<div className="space-y-3">
								<div className="text-sm">
									<span className="text-xs text-muted-foreground">
										{t("settings.dataDirectory")}:
									</span>
									<code className="ml-2 bg-muted px-1.5 py-0.5 rounded text-xs">
										{storageInfo.data_dir}
									</code>
									<Button
										variant="outline"
										size="sm"
										className="ml-2 h-6 px-2 text-xs"
										onClick={handleChangeDataDir}
										disabled={changingDataDir}
									>
										<FolderOpen className="mr-1 h-3 w-3" />
										{changingDataDir
											? t("settings.changingDataDir")
											: t("settings.changeDataDir")}
									</Button>
								</div>

								<div className="grid grid-cols-2 gap-2 text-xs">
									<div className="rounded-md border p-2">
										<p className="text-muted-foreground">
											{t("settings.papersInLibrary")}
										</p>
										<p className="text-lg font-semibold">
											{storageInfo.total_papers}
										</p>
									</div>
									<div className="rounded-md border p-2">
										<p className="text-muted-foreground">
											{t("settings.feedCacheItems")}
										</p>
										<p className="text-lg font-semibold">
											{storageInfo.feed_cache_items}
										</p>
										<p className="text-[10px] text-muted-foreground">
											{t("settings.ofTotal", {
												count: storageInfo.feed_total_items,
											})}
										</p>
									</div>
								</div>

								<div className="space-y-1.5">
									<label
										className="text-xs font-medium"
										htmlFor="retention-days"
									>
										{t("settings.autoDeleteCachedFeedItems")}
									</label>
									<div className="flex items-center gap-2">
										<input
											id="retention-days"
											type="number"
											min={1}
											max={365}
											value={retentionDays}
											onChange={(e) =>
												setRetentionDays(
													Math.max(1, Number.parseInt(e.target.value) || 7),
												)
											}
											className="h-8 w-20 rounded-md border bg-transparent px-2 text-sm"
										/>
										<span className="text-xs text-muted-foreground">
											{t("settings.days")}
										</span>
										<Button
											variant="outline"
											size="sm"
											className="h-8"
											onClick={handleRetentionSave}
											disabled={
												retentionSaving ||
												retentionDays === storageInfo.feed_cache_retention_days
											}
										>
											{retentionSaving ? t("common.saving") : t("common.save")}
										</Button>
									</div>
									<p className="text-[11px] text-muted-foreground">
										{t("settings.itemsNeverDeleted")}
									</p>
								</div>

								<Button
									variant="outline"
									size="sm"
									className="text-destructive hover:text-destructive"
									onClick={handleClearCache}
									disabled={clearingCache || storageInfo.feed_cache_items === 0}
								>
									<Trash2 className="mr-1.5 h-3.5 w-3.5" />
									{clearingCache
										? t("settings.clearing")
										: t("settings.clearFeedCache", {
												count: storageInfo.feed_cache_items,
											})}
								</Button>
							</div>
						)}

						{section === "sync" && (
							<div className="space-y-3">
								{syncStatus && (
									<div className="flex items-center gap-2 text-sm flex-wrap">
										<span>{t("settings.status")}:</span>
										<Badge
											variant={syncStatus.enabled ? "default" : "secondary"}
										>
											{syncStatus.syncing
												? t("settings.syncing")
												: syncStatus.enabled
													? t("settings.enabled")
													: t("settings.disabled")}
										</Badge>
										{syncStatus.last_sync_time && (
											<span className="text-xs text-muted-foreground">
												{t("settings.lastSync")}:{" "}
												{new Date(syncStatus.last_sync_time).toLocaleString()}
											</span>
										)}
										{syncStatus.last_error && (
											<span className="text-xs text-destructive flex items-center gap-1">
												<AlertCircle className="h-3 w-3" />
												{syncStatus.last_error.slice(0, 60)}
											</span>
										)}
									</div>
								)}

								<div className="space-y-2">
									<div>
										<label className="text-xs font-medium" htmlFor="sync-url">
											{t("settings.webdavServerUrl")}
										</label>
										<input
											id="sync-url"
											type="url"
											placeholder="https://dav.jianguoyun.com/dav/"
											value={syncUrl}
											onChange={(e) => setSyncUrl(e.target.value)}
											className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm"
										/>
									</div>
									<div className="grid grid-cols-2 gap-2">
										<div>
											<label
												className="text-xs font-medium"
												htmlFor="sync-user"
											>
												{t("settings.username")}
											</label>
											<input
												id="sync-user"
												type="text"
												value={syncUsername}
												onChange={(e) => setSyncUsername(e.target.value)}
												className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm"
											/>
										</div>
										<div>
											<label
												className="text-xs font-medium"
												htmlFor="sync-pass"
											>
												{t("settings.password")}
											</label>
											<input
												id="sync-pass"
												type="password"
												placeholder={
													syncPasswordSet
														? t("settings.passwordKeepPlaceholder")
														: ""
												}
												value={syncPassword}
												onChange={(e) => setSyncPassword(e.target.value)}
												className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm"
											/>
										</div>
									</div>
									<div className="grid grid-cols-2 gap-2">
										<div>
											<label
												className="text-xs font-medium"
												htmlFor="sync-path"
											>
												{t("settings.remotePath")}
											</label>
											<input
												id="sync-path"
												type="text"
												placeholder="/"
												value={syncRemotePath}
												onChange={(e) => setSyncRemotePath(e.target.value)}
												className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm"
											/>
										</div>
										<div>
											<label
												className="text-xs font-medium"
												htmlFor="sync-interval"
											>
												{t("settings.syncInterval")}
											</label>
											<input
												id="sync-interval"
												type="number"
												min={1}
												max={60}
												value={syncInterval}
												onChange={(e) =>
													setSyncInterval(
														Math.max(1, Number.parseInt(e.target.value) || 5),
													)
												}
												className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm"
											/>
										</div>
									</div>
								</div>

								{/* Sync content options */}
								<div className="space-y-3">
									<h4 className="text-xs font-semibold text-muted-foreground uppercase tracking-wide">
										{t("settings.syncContentOptions")}
									</h4>
									<div className="grid grid-cols-2 gap-2">
										<label className="flex items-center gap-2 text-xs cursor-pointer">
											<input
												type="checkbox"
												checked={syncCollections}
												onChange={(e) => setSyncCollections(e.target.checked)}
												className="h-3.5 w-3.5 rounded border"
											/>
											{t("settings.syncCollectionsLabel")}
										</label>
										<label className="flex items-center gap-2 text-xs cursor-pointer">
											<input
												type="checkbox"
												checked={syncTags}
												onChange={(e) => setSyncTags(e.target.checked)}
												className="h-3.5 w-3.5 rounded border"
											/>
											{t("settings.syncTagsLabel")}
										</label>
										<label className="flex items-center gap-2 text-xs cursor-pointer">
											<input
												type="checkbox"
												checked={syncAnnotations}
												onChange={(e) => setSyncAnnotations(e.target.checked)}
												className="h-3.5 w-3.5 rounded border"
											/>
											{t("settings.syncAnnotationsLabel")}
										</label>
										<label className="flex items-center gap-2 text-xs cursor-pointer">
											<input
												type="checkbox"
												checked={syncReaderState}
												onChange={(e) => setSyncReaderState(e.target.checked)}
												className="h-3.5 w-3.5 rounded border"
											/>
											{t("settings.syncReaderStateLabel")}
										</label>
										<label className="flex items-center gap-2 text-xs cursor-pointer">
											<input
												type="checkbox"
												checked={syncNotes}
												onChange={(e) => setSyncNotes(e.target.checked)}
												className="h-3.5 w-3.5 rounded border"
											/>
											{t("settings.syncNotesLabel")}
										</label>
										<label className="flex items-center gap-2 text-xs cursor-pointer">
											<input
												type="checkbox"
												checked={syncAttachments}
												onChange={(e) => setSyncAttachments(e.target.checked)}
												className="h-3.5 w-3.5 rounded border"
											/>
											{t("settings.syncAttachmentsLabel")}
										</label>
									</div>

									{/* Max file size limit */}
									<div className="flex items-center gap-2">
										<label
											className="text-xs font-medium whitespace-nowrap"
											htmlFor="max-file-size"
										>
											{t("settings.maxFileSizeLabel")}
										</label>
										<input
											id="max-file-size"
											type="number"
											min={0}
											value={maxFileSizeMb}
											onChange={(e) =>
												setMaxFileSizeMb(
													Math.max(0, Number.parseInt(e.target.value) || 0),
												)
											}
											className="h-8 w-24 rounded-md border bg-transparent px-2 text-sm"
										/>
										<span className="text-xs text-muted-foreground">
											{t("settings.maxFileSizeHint")}
										</span>
									</div>

									<div className="grid grid-cols-2 gap-2">
										<div>
											<label
												className="text-xs font-medium"
												htmlFor="pdf-download-mode"
											>
												{t("settings.pdfDownloadMode")}
											</label>
											<select
												id="pdf-download-mode"
												value={pdfDownloadMode}
												onChange={(e) => setPdfDownloadMode(e.target.value)}
												className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm"
											>
												<option value="on_demand">
													{t("settings.pdfOnDemand")}
												</option>
												<option value="full">{t("settings.pdfFull")}</option>
											</select>
										</div>
										<div>
											<label
												className="text-xs font-medium"
												htmlFor="conflict-strategy"
											>
												{t("settings.conflictStrategy")}
											</label>
											<select
												id="conflict-strategy"
												value={conflictStrategy}
												onChange={(e) => setConflictStrategy(e.target.value)}
												className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm"
											>
												<option value="auto_merge">
													{t("settings.conflictAutoMerge")}
												</option>
												<option value="prefer_local">
													{t("settings.conflictPreferLocal")}
												</option>
												<option value="prefer_remote">
													{t("settings.conflictPreferRemote")}
												</option>
											</select>
										</div>
									</div>
								</div>

								{syncTestResult && (
									<div
										className={`flex items-center gap-2 rounded-md border p-2 text-xs ${
											syncTestResult.ok
												? "border-green-500/30 bg-green-50 text-green-700 dark:bg-green-950 dark:text-green-300"
												: "border-red-500/30 bg-red-50 text-red-700 dark:bg-red-950 dark:text-red-300"
										}`}
									>
										{syncTestResult.ok ? (
											<CheckCircle className="h-3.5 w-3.5 shrink-0" />
										) : (
											<AlertCircle className="h-3.5 w-3.5 shrink-0" />
										)}
										{syncTestResult.msg}
									</div>
								)}

								<div className="flex items-center gap-2 flex-wrap">
									<Button
										variant="outline"
										size="sm"
										onClick={handleSyncTest}
										disabled={
											syncTesting ||
											!syncUrl ||
											!syncUsername ||
											(!syncPassword && !syncPasswordSet)
										}
									>
										{syncTesting ? (
											<>
												<Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
												{t("settings.testing")}
											</>
										) : (
											t("settings.testConnection")
										)}
									</Button>
									<Button
										variant="default"
										size="sm"
										onClick={handleSyncSave}
										disabled={
											syncSaving ||
											!syncUrl ||
											!syncUsername ||
											(!syncPassword && !syncPasswordSet) ||
											(!syncTestResult?.ok && !syncEnabled)
										}
									>
										{syncSaving ? (
											<>
												<Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
												{t("common.saving")}
											</>
										) : syncEnabled ? (
											t("settings.updateConfig")
										) : (
											t("settings.enableSync")
										)}
									</Button>
									{syncEnabled && (
										<>
											<Button
												variant="outline"
												size="sm"
												onClick={handleTriggerSync}
												disabled={syncTriggering || syncStatus?.syncing}
											>
												{syncTriggering || syncStatus?.syncing ? (
													<>
														<Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
														{t("settings.syncing")}
													</>
												) : (
													<>
														<RefreshCw className="mr-1.5 h-3.5 w-3.5" />
														{t("settings.syncNow")}
													</>
												)}
											</Button>
											<Button
												variant="outline"
												size="sm"
												className="text-destructive hover:text-destructive"
												onClick={handleSyncDisable}
												disabled={syncSaving}
											>
												{t("settings.disable")}
											</Button>
										</>
									)}
								</div>

								{syncStatus && syncStatus.devices.length > 0 && (
									<div className="space-y-1">
										<p className="text-xs font-medium">
											{t("settings.connectedDevices")}
										</p>
										{syncStatus.devices.map((dev) => (
											<div
												key={dev.device_id}
												className="flex items-center justify-between rounded-md border p-2 text-xs"
											>
												<span className="font-medium">{dev.device_name}</span>
												<span className="text-muted-foreground">
													{dev.last_sync_time
														? new Date(dev.last_sync_time).toLocaleString()
														: t("settings.neverSynced")}
												</span>
											</div>
										))}
									</div>
								)}
							</div>
						)}

						{section === "export" && (
							<div className="space-y-5">
								<div className="space-y-2">
									<h3 className="text-sm font-semibold">
										{t("settings.export")}
									</h3>
									<div className="flex items-center gap-2">
										<Button
											variant="outline"
											size="sm"
											onClick={handleExportAll}
										>
											{t("settings.exportAllBibtex")}
										</Button>
										<Button
											variant="outline"
											size="sm"
											onClick={handleExportRis}
										>
											{t("settings.exportAllRis")}
										</Button>
									</div>
									{exportResult && (
										<div className="space-y-1">
											<p className="text-[11px] font-medium text-muted-foreground">
												{t("settings.bibtexOutput")}:
											</p>
											<pre className="max-h-32 overflow-auto rounded-md bg-muted p-2 text-[11px] font-mono">
												{exportResult}
											</pre>
										</div>
									)}
									{exportRisResult && (
										<div className="space-y-1">
											<p className="text-[11px] font-medium text-muted-foreground">
												{t("settings.risOutput")}:
											</p>
											<pre className="max-h-32 overflow-auto rounded-md bg-muted p-2 text-[11px] font-mono">
												{exportRisResult}
											</pre>
										</div>
									)}
								</div>

								<Separator />

								<div className="space-y-2">
									<h3 className="text-sm font-semibold">
										{t("zoteroImport.sectionTitle")}
									</h3>
									<p className="text-[11px] text-muted-foreground">
										{t("zoteroImport.sectionDesc")}
									</p>
									<Button
										variant="outline"
										size="sm"
										onClick={() => setZoteroImportOpen(true)}
									>
										{t("zoteroImport.importFromZotero")}
									</Button>
									<ZoteroImportDialog
										open={zoteroImportOpen}
										onClose={() => setZoteroImportOpen(false)}
									/>
								</div>

								<Separator />

								<div className="space-y-2">
									<h3 className="text-sm font-semibold">
										{t("settings.libraryStatistics")}
									</h3>
									<div className="grid grid-cols-3 gap-2 text-xs">
										<div className="rounded-md border p-2.5">
											<p className="text-muted-foreground">
												{t("settings.papers")}
											</p>
											<p className="text-lg font-semibold">
												{storageInfo?.total_papers ?? 0}
											</p>
										</div>
										<div className="rounded-md border p-2.5">
											<p className="text-muted-foreground">
												{t("settings.collections")}
											</p>
											<p className="text-lg font-semibold">
												{collections.length}
											</p>
										</div>
										<div className="rounded-md border p-2.5">
											<p className="text-muted-foreground">{t("paper.tags")}</p>
											<p className="text-lg font-semibold">{tags.length}</p>
										</div>
									</div>
								</div>

								<Separator />

								<div className="space-y-2">
									<h3 className="text-sm font-semibold">
										{t("settings.cache")}
									</h3>
									<Button
										variant="outline"
										size="sm"
										className="text-destructive hover:text-destructive"
										onClick={handleClearTranslations}
										disabled={clearingTranslations}
									>
										<Trash2 className="mr-1.5 h-3.5 w-3.5" />
										{clearingTranslations
											? t("settings.clearing")
											: t("settings.clearTranslationCache")}
									</Button>
									<p className="text-[11px] text-muted-foreground">
										{t("settings.clearTranslationCacheDesc")}
									</p>
								</div>

								<Separator />

								<div className="space-y-2">
									<h3 className="text-sm font-semibold">
										{t("settings.dataDirectory")}
									</h3>
									<div className="flex items-center gap-2">
										<code className="bg-muted px-2 py-0.5 rounded text-xs">
											{storageInfo?.data_dir ?? "~/.zoro/"}
										</code>
										<Button
											variant="outline"
											size="sm"
											onClick={handleChangeDataDir}
											disabled={changingDataDir}
										>
											<FolderOpen className="mr-1.5 h-3.5 w-3.5" />
											{changingDataDir
												? t("settings.changingDataDir")
												: t("settings.changeDataDir")}
										</Button>
									</div>
								</div>
							</div>
						)}

						{section === "about" && (
							<div className="space-y-5">
								<div className="space-y-1">
									<h3 className="text-lg font-bold">Zoro</h3>
									<p className="text-sm text-muted-foreground">
										{t("settings.version", { version: "0.1.0" })}
									</p>
								</div>

								<p className="text-sm leading-relaxed">
									{t("settings.aboutDesc")}
								</p>

								<Separator />

								<div className="space-y-1.5 text-sm">
									<p className="text-xs font-medium">{t("settings.links")}</p>
									<div className="flex flex-col gap-1">
										<a
											href="https://github.com/ruihanglix/zoro"
											target="_blank"
											rel="noopener noreferrer"
											className="text-primary hover:underline text-xs"
										>
											{t("settings.githubRepo")}
										</a>
										<a
											href="https://github.com/ruihanglix/zoro/issues"
											target="_blank"
											rel="noopener noreferrer"
											className="text-primary hover:underline text-xs"
										>
											{t("settings.reportIssue")}
										</a>
									</div>
								</div>

								<Separator />

								<p className="text-xs text-muted-foreground">
									{t("settings.madeBy")}
								</p>
							</div>
						)}

						{section === "ai-lab" && <LabSection />}

						{section === "plugins-general" && <PluginsGeneralSection />}

						{/* Dynamic plugin settings sections */}
						{pluginSettingsContribs.map((contrib) => {
							const sectionId = `plugin-settings-${contrib.pluginId}-${contrib.contribution.id}`;
							if (section !== sectionId) return null;
							const Component = contrib.component;
							const pluginInfo = plugins.find(
								(p) => p.manifest.id === contrib.pluginId,
							);
							if (!pluginInfo) return null;
							const sdk = createPluginSDK(pluginInfo);
							return (
								<Component
									key={sectionId}
									sdk={sdk}
									context={{ section: "settings" }}
								/>
							);
						})}
					</div>
				</ScrollArea>
			</div>
		</div>
	);
}

/** Plugins General section — manage installed + dev plugins. */
function LabSection() {
	const { t } = useTranslation();
	const {
		freeLlmEnabled,
		setFreeLlmEnabled,
		setProviderKey,
		defaultFreeModel,
		setDefaultFreeModel,
		configuredProviderIds,
		getAvailableModels,
		routingStrategy,
		setRoutingStrategy,
		fetchAllProviderModels,
		modelFetchLoading,
		fetchedModels,
		toggleModelDisabled,
		isModelDisabled,
	} = useLabStore();

	const [showMoreProviders, setShowMoreProviders] = useState(false);
	const [editingKeys, setEditingKeys] = useState<Record<string, string>>({});

	const primaryProviders = FREE_PROVIDERS.filter((p) => p.tier === "primary");
	const secondaryProviders = FREE_PROVIDERS.filter(
		(p) => p.tier === "secondary",
	);
	const visibleProviders = showMoreProviders
		? FREE_PROVIDERS
		: primaryProviders;

	const availableModels = getAvailableModels();
	const configuredCount = configuredProviderIds.length;

	// Auto-fetch models from all configured providers on mount
	useEffect(() => {
		if (configuredCount > 0) {
			fetchAllProviderModels();
		}
	}, [configuredCount, fetchAllProviderModels]);

	const handleSaveKey = (providerId: string) => {
		const key = editingKeys[providerId]?.trim();
		if (key) {
			setProviderKey(providerId, key);
			setEditingKeys((prev) => {
				const next = { ...prev };
				delete next[providerId];
				return next;
			});
		}
	};

	return (
		<div className="space-y-5">
			{/* Title + Toggle */}
			<div>
				<h3 className="text-sm font-semibold">{t("settings.labFreeLlmTitle")}</h3>
				<p className="text-[11px] text-muted-foreground mt-1">
					{t("settings.labFreeLlmDesc")}
				</p>
				<div className="flex items-center gap-3 mt-3">
					<button
						type="button"
						onClick={() => setFreeLlmEnabled(!freeLlmEnabled)}
						className={cn(
							"relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors",
							freeLlmEnabled ? "bg-primary" : "bg-muted-foreground/30",
						)}
					>
						<span
							className={cn(
								"pointer-events-none inline-block h-4 w-4 rounded-full bg-white shadow-sm transition-transform",
								freeLlmEnabled ? "translate-x-4" : "translate-x-0",
							)}
						/>
					</button>
					<span className="text-xs font-medium">
						{t("settings.labFreeLlmEnabled")}
					</span>
				</div>
			</div>

			{freeLlmEnabled && (
				<>
					{/* Provider count */}
					<div className="flex items-center gap-2 text-xs">
						<span className="text-muted-foreground">
							{t("settings.labProvidersConfiguredCount", {
								configured: configuredCount,
								total: FREE_PROVIDERS.length,
							})}
						</span>
					</div>

					<Separator />

					{/* Provider Configuration */}
					<div>
						<h4 className="text-sm font-medium mb-1">
							{t("settings.labProviderConfig")}
						</h4>
						<p className="text-[11px] text-muted-foreground mb-3">
							{t("settings.labProviderConfigHint")}
						</p>

						<div className="space-y-2">
							{visibleProviders.map((provider) => {
								const isConfigured = configuredProviderIds.includes(provider.id);
								const isEditing = editingKeys[provider.id] !== undefined;
								return (
									<div
										key={provider.id}
										className={cn(
											"flex items-center gap-3 rounded-md border p-2.5 transition-colors",
											isConfigured
												? "border-green-500/30 bg-green-50/50 dark:bg-green-950/20"
												: "border-border",
										)}
									>
										<div className="min-w-0 flex-1">
											<div className="flex items-center gap-2">
												<span className="text-sm font-medium">
													{provider.displayName}
												</span>
												{isConfigured && (
													<Badge
														variant="secondary"
														className="text-[10px] h-4 px-1.5 text-green-700 dark:text-green-400 bg-green-100 dark:bg-green-900/40"
													>
														{t("settings.labConfigured")}
													</Badge>
												)}
											</div>

											{isEditing ? (
												<div className="flex items-center gap-1.5 mt-1.5">
													<input
														type="password"
														placeholder={provider.keyPrefix ? `${provider.keyPrefix}...` : t("onboarding.freeProviderApiKeyPlaceholder")}
														value={editingKeys[provider.id] || ""}
														onChange={(e) =>
															setEditingKeys((prev) => ({
																...prev,
																[provider.id]: e.target.value,
															}))
														}
														onKeyDown={(e) => {
															if (e.key === "Enter") handleSaveKey(provider.id);
														}}
														className="h-7 flex-1 rounded-md border bg-transparent px-2 text-xs"
														autoFocus
													/>
													<Button
														variant="default"
														size="sm"
														className="h-7 px-2 text-xs"
														disabled={!editingKeys[provider.id]?.trim()}
														onClick={() => handleSaveKey(provider.id)}
													>
														{t("common.save")}
													</Button>
													<Button
														variant="ghost"
														size="sm"
														className="h-7 px-2 text-xs"
														onClick={() =>
															setEditingKeys((prev) => {
																const next = { ...prev };
																delete next[provider.id];
																return next;
															})
														}
													>
														{t("common.cancel")}
													</Button>
												</div>
											) : (
												<div className="flex items-center gap-1.5 mt-0.5">
													{!isConfigured && (
														<Button
															variant="outline"
															size="sm"
															className="h-6 px-2 text-[11px]"
															onClick={() =>
																setEditingKeys((prev) => ({
																	...prev,
																	[provider.id]: "",
																}))
															}
														>
															{t("settings.apiKey")}
														</Button>
													)}
													{isConfigured && (
														<Button
															variant="ghost"
															size="sm"
															className="h-6 px-2 text-[11px]"
															onClick={() =>
																setEditingKeys((prev) => ({
																	...prev,
																	[provider.id]: "",
																}))
															}
														>
															<Pencil className="h-3 w-3 mr-1" />
															{t("common.update")}
														</Button>
													)}
												</div>
											)}
										</div>
										<a
											href={provider.signUpUrl}
											target="_blank"
											rel="noopener noreferrer"
											className="flex items-center gap-1 text-[11px] text-primary hover:underline shrink-0"
										>
											{t("settings.labGetKey")}
											<ExternalLink className="h-3 w-3" />
										</a>
									</div>
								);
							})}
						</div>

						{secondaryProviders.length > 0 && (
							<button
								type="button"
								className="flex items-center gap-1 mt-2 text-xs text-muted-foreground hover:text-foreground transition-colors"
								onClick={() => setShowMoreProviders(!showMoreProviders)}
							>
								{showMoreProviders ? (
									<>
										<ChevronDown className="h-3 w-3" />
										{t("settings.labShowLess")}
									</>
								) : (
									<>
										<ChevronRight className="h-3 w-3" />
										{t("settings.labShowMore")} ({secondaryProviders.length})
									</>
								)}
							</button>
						)}
					</div>

				<Separator />

					{/* Routing Strategy */}
					<div className="space-y-2">
						<div>
							<h4 className="text-sm font-medium">
								{t("settings.labRoutingStrategy")}
							</h4>
							<p className="text-[11px] text-muted-foreground mt-0.5">
								{t("settings.labRoutingStrategyDesc")}
							</p>
						</div>
						<div className="grid grid-cols-3 gap-2">
							{(["auto", "round-robin", "manual"] as const).map((strategy) => (
								<button
									key={strategy}
									type="button"
									onClick={() => setRoutingStrategy(strategy)}
									className={cn(
										"flex flex-col items-start gap-0.5 rounded-lg border-2 p-2.5 text-left transition-all",
										routingStrategy === strategy
											? "border-primary bg-primary/5"
											: "border-border hover:border-muted-foreground/40",
									)}
								>
									<span className="text-xs font-semibold">
										{t(`settings.labRouting_${strategy}`)}
									</span>
									<span className="text-[10px] text-muted-foreground leading-tight">
										{t(`settings.labRouting_${strategy}_desc`)}
									</span>
								</button>
							))}
						</div>
					</div>

					<Separator />

					{/* Default Free Model */}
					<div className="space-y-2">
						<div className="flex items-center justify-between">
							<div>
								<h4 className="text-sm font-medium">
									{t("settings.labDefaultModel")}
								</h4>
								<p className="text-[11px] text-muted-foreground mt-0.5">
									{routingStrategy === "round-robin"
										? t("settings.labDefaultModelDescRR")
										: t("settings.labDefaultModelDesc")}
								</p>
							</div>
							<Button
								variant="outline"
								size="sm"
								onClick={() => fetchAllProviderModels(true)}
								disabled={modelFetchLoading}
								className="h-7 text-xs shrink-0"
							>
								{modelFetchLoading ? (
									<Loader2 className="mr-1 h-3 w-3 animate-spin" />
								) : (
									<RefreshCw className="mr-1 h-3 w-3" />
								)}
								{t("settings.labRefreshModels")}
							</Button>
						</div>
						<select
							value={defaultFreeModel}
							onChange={(e) => setDefaultFreeModel(e.target.value)}
							className="h-8 w-full rounded-md border bg-transparent px-2 text-sm"
							disabled={routingStrategy === "round-robin"}
						>
							{availableModels.length === 0 && (
								<option value="">{t("settings.noModelsConfigured")}</option>
							)}
							{availableModels.map((m) => (
								<option key={m.id} value={m.id}>
									{m.displayName}
								</option>
							))}
					</select>
					</div>

					<Separator />

					{/* Model Management — disable/enable individual models */}
					<div className="space-y-2">
						<div>
							<h4 className="text-sm font-medium">
								{t("settings.labModelManagement")}
							</h4>
							<p className="text-[11px] text-muted-foreground mt-0.5">
								{t("settings.labModelManagementDesc")}
							</p>
						</div>

						{configuredProviderIds.length === 0 ? (
							<p className="text-xs text-muted-foreground italic">
								{t("settings.noModelsConfigured")}
							</p>
						) : (
							<div className="space-y-3">
								{configuredProviderIds.map((providerId) => {
									const provider = FREE_PROVIDERS.find((p) => p.id === providerId);
									const models = fetchedModels[providerId] || [];
									if (models.length === 0) return null;
									return (
										<div key={providerId}>
											<p className="text-xs font-medium mb-1.5">
												{provider?.displayName || providerId} 🧪
											</p>
											<div className="flex flex-wrap gap-1.5">
												{models.map((modelId) => {
													const disabled = isModelDisabled(providerId, modelId);
													return (
														<button
															key={modelId}
															type="button"
															onClick={() => toggleModelDisabled(providerId, modelId)}
															className={cn(
																"px-2 py-0.5 rounded-md text-[11px] border transition-all cursor-pointer",
																disabled
																	? "border-border bg-muted/30 text-muted-foreground line-through opacity-50"
																	: "border-primary/30 bg-primary/5 text-foreground",
															)}
															title={disabled ? t("settings.labModelDisabled") : t("settings.labModelEnabled")}
														>
															{modelId}
														</button>
													);
												})}
											</div>
										</div>
									);
								})}
							</div>
						)}
					</div>
				</>
			)}
		</div>
	);
}

function PluginsGeneralSection() {
	const { t } = useTranslation();
	const plugins = usePluginStore((s) => s.plugins);
	const loading = usePluginStore((s) => s.loading);
	const fetchPlugins = usePluginStore((s) => s.fetchPlugins);
	const installFromFile = usePluginStore((s) => s.installFromFile);
	const uninstall = usePluginStore((s) => s.uninstall);
	const toggle = usePluginStore((s) => s.toggle);
	const loadDevPlugin = usePluginStore((s) => s.loadDevPlugin);
	const unloadDevPlugin = usePluginStore((s) => s.unloadDevPlugin);
	const reloadDevPlugin = usePluginStore((s) => s.reloadDevPlugin);

	useEffect(() => {
		fetchPlugins();
	}, [fetchPlugins]);

	const handleInstallZcx = async () => {
		const { open } = await import("@tauri-apps/plugin-dialog");
		const selected = await open({
			multiple: false,
			filters: [{ name: "Zoro Plugin", extensions: ["zcx"] }],
		});
		if (selected) {
			await installFromFile(selected as string);
		}
	};

	const handleLoadDevFolder = async () => {
		const { open } = await import("@tauri-apps/plugin-dialog");
		const selected = await open({
			directory: true,
			multiple: false,
		});
		if (selected) {
			await loadDevPlugin(selected as string);
		}
	};

	return (
		<div className="space-y-5">
			{/* Installed Plugins */}
			<div className="space-y-2">
				<div className="flex items-center justify-between">
					<p className="text-xs font-medium">{t("plugins.installed")}</p>
					<Button
						variant="outline"
						size="sm"
						onClick={handleInstallZcx}
						className="h-7 text-xs"
					>
						<Plus className="h-3 w-3 mr-1" />
						{t("plugins.installZcx")}
					</Button>
				</div>

				{loading && (
					<div className="flex items-center gap-2 text-sm text-muted-foreground py-4">
						<Loader2 className="h-4 w-4 animate-spin" />
						{t("common.loading")}
					</div>
				)}

				{!loading && plugins.length === 0 && (
					<p className="text-sm text-muted-foreground py-4">
						{t("plugins.noPlugins")}
					</p>
				)}

				{plugins.map((plugin) => (
					<div
						key={plugin.manifest.id}
						className="rounded-lg border p-3 space-y-2"
					>
						<div className="flex items-start justify-between">
							<div className="flex-1 min-w-0">
								<div className="flex items-center gap-2">
									<h4 className="text-sm font-medium">
										{plugin.manifest.name}
									</h4>
									<Badge variant="outline" className="text-[10px]">
										v{plugin.manifest.version}
									</Badge>
									<Badge
										variant={plugin.mode === "dev" ? "secondary" : "default"}
										className="text-[10px]"
									>
										{plugin.mode === "dev"
											? t("plugins.devMode")
											: t("plugins.installedMode")}
									</Badge>
								</div>
								<p className="text-xs text-muted-foreground mt-0.5">
									{plugin.manifest.description}
								</p>
								<p className="text-[10px] text-muted-foreground mt-1 font-mono truncate">
									{plugin.path}
								</p>
							</div>
							<button
								type="button"
								onClick={() => toggle(plugin.manifest.id, !plugin.enabled)}
								className={cn(
									"relative inline-flex h-5 w-9 items-center rounded-full transition-colors shrink-0 ml-3",
									plugin.enabled ? "bg-primary" : "bg-muted-foreground/30",
								)}
							>
								<span
									className={cn(
										"inline-block h-3.5 w-3.5 rounded-full bg-white transition-transform",
										plugin.enabled ? "translate-x-[18px]" : "translate-x-[3px]",
									)}
								/>
							</button>
						</div>

						{/* Permissions */}
						{plugin.manifest.permissions.length > 0 && (
							<div className="flex flex-wrap gap-1">
								{plugin.manifest.permissions.slice(0, 4).map((perm) => (
									<Badge
										key={perm}
										variant="outline"
										className="text-[9px] font-mono"
									>
										{perm}
									</Badge>
								))}
								{plugin.manifest.permissions.length > 4 && (
									<Badge variant="outline" className="text-[9px]">
										+{plugin.manifest.permissions.length - 4}
									</Badge>
								)}
							</div>
						)}

						{/* Actions */}
						<div className="flex gap-1.5">
							{plugin.mode === "dev" && (
								<>
									<Button
										variant="outline"
										size="sm"
										className="h-6 text-[10px]"
										onClick={() => reloadDevPlugin(plugin.manifest.id)}
									>
										<RefreshCw className="h-3 w-3 mr-1" />
										{t("plugins.reload")}
									</Button>
									<Button
										variant="outline"
										size="sm"
										className="h-6 text-[10px] text-destructive hover:text-destructive"
										onClick={() => {
											if (confirm(t("plugins.confirmUnload"))) {
												unloadDevPlugin(plugin.manifest.id);
											}
										}}
									>
										<Trash2 className="h-3 w-3 mr-1" />
										{t("plugins.unload")}
									</Button>
								</>
							)}
							{plugin.mode === "installed" && (
								<Button
									variant="outline"
									size="sm"
									className="h-6 text-[10px] text-destructive hover:text-destructive"
									onClick={() => {
										if (confirm(t("plugins.confirmUninstall"))) {
											uninstall(plugin.manifest.id);
										}
									}}
								>
									<Trash2 className="h-3 w-3 mr-1" />
									{t("plugins.uninstall")}
								</Button>
							)}
						</div>
					</div>
				))}
			</div>

			<Separator />

			{/* Developer Mode */}
			<div className="space-y-2">
				<p className="text-xs font-medium">{t("plugins.developerMode")}</p>
				<p className="text-xs text-muted-foreground">
					{t("plugins.developerModeDesc")}
				</p>
				<Button
					variant="outline"
					size="sm"
					onClick={handleLoadDevFolder}
					className="h-7 text-xs"
				>
					<Plus className="h-3 w-3 mr-1" />
					{t("plugins.loadFolder")}
				</Button>
			</div>

			<Separator />

			<p className="text-xs text-muted-foreground">
				{t("plugins.securityNote")}
			</p>
		</div>
	);
}
