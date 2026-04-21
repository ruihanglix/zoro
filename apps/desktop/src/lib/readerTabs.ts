// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import type { ContributionWithPlugin } from "@/plugins/types";
import type { BuiltInTabId } from "@/stores/uiStore";

export interface ReaderTabDefinition {
	id: string;
	labelKey: string; // i18n key
	isPlugin?: boolean;
	pluginId?: string;
}

export const BUILT_IN_TABS: ReaderTabDefinition[] = [
	{ id: "annotation", labelKey: "reader.annotation" },
	{ id: "agent", labelKey: "reader.agent" },
	{ id: "notes", labelKey: "reader.notes" },
	{ id: "info", labelKey: "reader.info" },
	{ id: "browser", labelKey: "reader.browser" },
	{ id: "terminal", labelKey: "reader.terminal" },
];

/** Get the label for a built-in or plugin tab */
export function getTabLabel(tabId: string, t: (key: string) => string): string {
	const builtIn = BUILT_IN_TABS.find((tab) => tab.id === tabId);
	if (builtIn) return t(builtIn.labelKey);
	// Plugin tab: strip prefix and humanize
	const stripped = tabId.replace(/^plugin-[^-]+-/, "");
	return stripped.includes(" ")
		? stripped
		: stripped
				.replace(/([A-Z]+)([A-Z][a-z])/g, "$1 $2")
				.replace(/([a-z\d])([A-Z])/g, "$1 $2")
				.replace(/^./, (s) => s.toUpperCase())
				.trim();
}

/** Check if a tab ID is a built-in tab */
export function isBuiltInTab(tabId: string): tabId is BuiltInTabId {
	return BUILT_IN_TABS.some((tab) => tab.id === tabId);
}

/** Get plugin tab ID from contribution */
export function getPluginTabId(contrib: ContributionWithPlugin): string {
	return `plugin-${contrib.pluginId}-${contrib.contribution.id}`;
}

/** Merge built-in tabs with plugin-contributed tabs */
export function getAllAvailableTabs(
	pluginSidebarTabs: ContributionWithPlugin[],
): ReaderTabDefinition[] {
	const pluginTabs: ReaderTabDefinition[] = pluginSidebarTabs.map(
		(contrib) => ({
			id: getPluginTabId(contrib),
			labelKey:
				(contrib.contribution as { titleKey?: string }).titleKey ??
				contrib.contribution.id,
			isPlugin: true,
			pluginId: contrib.pluginId,
		}),
	);
	return [...BUILT_IN_TABS, ...pluginTabs];
}
