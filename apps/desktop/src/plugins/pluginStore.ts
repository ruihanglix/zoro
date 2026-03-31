// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import * as commands from "@/lib/commands";
import type {
	ContributionItemResponse,
	OverlayContributionResponse,
	PluginInfoResponse,
} from "@/lib/commands";
import { logger } from "@/lib/logger";
import { create } from "zustand";
import { createPluginSDK, loadPluginModule } from "./PluginManager";
import type {
	ContributionWithPlugin,
	LoadedPluginModule,
	SlotLocation,
} from "./types";

interface PluginStoreState {
	plugins: PluginInfoResponse[];
	loadedModules: Record<string, LoadedPluginModule>;
	loading: boolean;
	error: string | null;

	// Actions
	fetchPlugins: () => Promise<void>;
	loadPlugin: (pluginId: string) => Promise<void>;
	unloadPlugin: (pluginId: string) => void;
	loadAllEnabled: () => Promise<void>;
	installFromFile: (path: string) => Promise<void>;
	uninstall: (pluginId: string) => Promise<void>;
	toggle: (pluginId: string, enabled: boolean) => Promise<void>;
	loadDevPlugin: (folderPath: string) => Promise<void>;
	unloadDevPlugin: (pluginId: string) => Promise<void>;
	reloadDevPlugin: (pluginId: string) => Promise<void>;

	// Selectors
	getContributionsForSlot: (location: SlotLocation) => ContributionWithPlugin[];
}

export const usePluginStore = create<PluginStoreState>((set, get) => ({
	plugins: [],
	loadedModules: {},
	loading: false,
	error: null,

	fetchPlugins: async () => {
		set({ loading: true, error: null });
		try {
			const plugins = await commands.listPlugins();
			set({ plugins, loading: false });
		} catch (e) {
			set({ error: String(e), loading: false });
		}
	},

	loadPlugin: async (pluginId: string) => {
		const { plugins, loadedModules } = get();
		if (loadedModules[pluginId]) return;

		const pluginInfo = plugins.find((p) => p.manifest.id === pluginId);
		if (!pluginInfo || !pluginInfo.enabled) return;

		try {
			const module = await loadPluginModule(pluginInfo);
			const sdk = createPluginSDK(pluginInfo);
			module.activate(sdk);

			set({
				loadedModules: { ...get().loadedModules, [pluginId]: module },
			});
		} catch (e) {
			logger.error("plugin", `Failed to load plugin ${pluginId}`, e);
		}
	},

	unloadPlugin: (pluginId: string) => {
		const { loadedModules } = get();
		const module = loadedModules[pluginId];
		if (module) {
			try {
				module.deactivate();
			} catch (e) {
				logger.error("plugin", `Error deactivating ${pluginId}`, e);
			}
			const updated = { ...loadedModules };
			delete updated[pluginId];
			set({ loadedModules: updated });
		}
	},

	loadAllEnabled: async () => {
		const { plugins } = get();
		for (const plugin of plugins) {
			if (plugin.enabled) {
				await get().loadPlugin(plugin.manifest.id);
			}
		}
	},

	installFromFile: async (path: string) => {
		try {
			await commands.installPluginFromFile(path);
			await get().fetchPlugins();
		} catch (e) {
			set({ error: String(e) });
		}
	},

	uninstall: async (pluginId: string) => {
		get().unloadPlugin(pluginId);
		try {
			await commands.uninstallPlugin(pluginId);
			await get().fetchPlugins();
		} catch (e) {
			set({ error: String(e) });
		}
	},

	toggle: async (pluginId: string, enabled: boolean) => {
		try {
			await commands.togglePlugin(pluginId, enabled);
			if (!enabled) {
				get().unloadPlugin(pluginId);
			}
			await get().fetchPlugins();
			if (enabled) {
				await get().loadPlugin(pluginId);
			}
		} catch (e) {
			set({ error: String(e) });
		}
	},

	loadDevPlugin: async (folderPath: string) => {
		try {
			await commands.loadDevPlugin(folderPath);
			await get().fetchPlugins();
			// Auto-load the new plugin
			const { plugins } = get();
			const newPlugin = plugins.find((p) => p.path === folderPath);
			if (newPlugin) {
				await get().loadPlugin(newPlugin.manifest.id);
			}
		} catch (e) {
			set({ error: String(e) });
		}
	},

	unloadDevPlugin: async (pluginId: string) => {
		get().unloadPlugin(pluginId);
		try {
			await commands.unloadDevPlugin(pluginId);
			await get().fetchPlugins();
		} catch (e) {
			set({ error: String(e) });
		}
	},

	reloadDevPlugin: async (pluginId: string) => {
		get().unloadPlugin(pluginId);
		try {
			await commands.reloadDevPlugin(pluginId);
			await get().fetchPlugins();
			await get().loadPlugin(pluginId);
		} catch (e) {
			set({ error: String(e) });
		}
	},

	getContributionsForSlot: (
		location: SlotLocation,
	): ContributionWithPlugin[] => {
		const { plugins, loadedModules } = get();
		const result: ContributionWithPlugin[] = [];

		for (const plugin of plugins) {
			if (!plugin.enabled) continue;
			const module = loadedModules[plugin.manifest.id];
			if (!module) continue;

			const contribs = plugin.manifest.contributions;
			let items: Array<ContributionItemResponse | OverlayContributionResponse> =
				[];

			switch (location) {
				case "reader_sidebar":
					items = contribs.reader_sidebar_tabs ?? [];
					break;
				case "reader_toolbar":
					items = contribs.reader_toolbar_actions ?? [];
					break;
				case "reader_overlay":
					items = contribs.reader_overlays ?? [];
					break;
				case "settings":
					items = contribs.settings_sections ?? [];
					break;
				case "sidebar_nav":
					items = contribs.sidebar_nav_items ?? [];
					break;
			}

			for (const item of items) {
				const component = module.components[item.component];
				if (component) {
					result.push({
						pluginId: plugin.manifest.id,
						pluginName: plugin.manifest.name,
						contribution: item as ContributionWithPlugin["contribution"],
						component,
					});
				}
			}
		}

		return result;
	},
}));
