// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import type {
	AcpProxyConfig,
	AcpProxyStatus,
	AgentInfoResponse,
	ConfigOptionInfo,
} from "@/lib/commands";
import * as commands from "@/lib/commands";
import { create } from "zustand";

// ── Store types ─────────────────────────────────────────────────────────────

interface AcpProxyState {
	// Backend state
	config: AcpProxyConfig | null;
	status: AcpProxyStatus | null;
	agents: AgentInfoResponse[];
	configOptions: ConfigOptionInfo[];
	configOptionsLoading: boolean;
	/** Per-agent cache for config options (mode/model lists) */
	configOptionsCache: Record<string, ConfigOptionInfo[]>;

	// UI state
	loading: boolean;
	error: string | null;

	// Actions
	initialize: () => Promise<void>;
	setEnabled: (enabled: boolean) => Promise<void>;
	updateConfig: (config: AcpProxyConfig) => Promise<void>;
	switchAgent: (agentName: string) => Promise<void>;
	start: () => Promise<void>;
	stop: () => Promise<void>;
	refreshStatus: () => Promise<void>;
	fetchConfigOptions: (agentName: string) => Promise<void>;

	// Derived helpers
	getProxyBaseUrl: () => string;
}

// ── Store ───────────────────────────────────────────────────────────────────

export const useAcpProxyStore = create<AcpProxyState>((set, get) => ({
	config: null,
	status: null,
	agents: [],
	configOptions: [],
	configOptionsLoading: false,
	configOptionsCache: {},
	loading: false,
	error: null,

	initialize: async () => {
		try {
			set({ loading: true, error: null });
			const [config, status, agents, persistedCache] = await Promise.all([
				commands.acpProxyGetConfig(),
				commands.acpProxyGetStatus(),
				commands.acpListAgents(),
				commands.acpProxyGetOptionsCache(),
			]);

			// Merge persisted cache (from disk) with any in-memory cache
			const mergedCache = { ...persistedCache, ...get().configOptionsCache };

			// Restore cached config options for the selected agent (instant display)
			const cached = config.agentName
				? mergedCache[config.agentName] ?? []
				: [];

			set({
				config,
				status,
				agents,
				configOptions: cached,
				configOptionsCache: mergedCache,
				loading: false,
			});

			// Fetch fresh config options in the background (only if no cache)
			if (config.agentName) {
				if (cached.length === 0) {
					get().fetchConfigOptions(config.agentName);
				} else {
					// Have cache — still refresh in background but don't show loading
					get().fetchConfigOptions(config.agentName);
				}
			}
		} catch (err) {
			console.error("[acp-proxy] Failed to initialize:", err);
			set({ error: String(err), loading: false });
		}
	},

	setEnabled: async (enabled) => {
		try {
			set({ loading: true, error: null });
			const status = await commands.acpProxySetEnabled(enabled);
			// Re-fetch config because backend may have auto-selected an agent
			const config = await commands.acpProxyGetConfig();
			set({ config, status, loading: false });
		} catch (err) {
			console.error("[acp-proxy] Failed to set enabled:", err);
			set({ error: String(err), loading: false });
		}
	},

	updateConfig: async (config) => {
		try {
			set({ loading: true, error: null });
			await commands.acpProxyUpdateConfig(config);
			set({ config, loading: false });

			// Auto-start: if proxy is enabled but not running and we now have an agent, kick off start
			const { status } = get();
			if (config.enabled && config.agentName && !status?.running) {
				try {
					const newStatus = await commands.acpProxyStart();
					set({ status: newStatus });
				} catch {
					// Not critical — proxy just stays not-running until user manually starts
				}
			}
		} catch (err) {
			console.error("[acp-proxy] Failed to update config:", err);
			set({ error: String(err), loading: false });
		}
	},

	switchAgent: async (agentName) => {
		const { config, configOptionsCache } = get();
		if (!config) return;

		// Restore cached config options for the target agent (instant display, no flicker)
		const cached = configOptionsCache[agentName] ?? [];
		const hasCached = cached.length > 0;

		// Immediately update UI: highlight the new agent, show cached options or loading
		const newConfig = {
			...config,
			agentName,
			modeConfigId: "",
			modeValue: "",
			modelConfigId: "",
			modelValue: "",
		};
		set({
			config: newConfig,
			configOptions: cached,
			configOptionsLoading: !hasCached, // Only show loading spinner if no cache
		});

		// Persist the config change and auto-start if needed
		try {
			await commands.acpProxyUpdateConfig(newConfig);

			const { status } = get();
			if (newConfig.enabled && newConfig.agentName && !status?.running) {
				try {
					const newStatus = await commands.acpProxyStart();
					set({ status: newStatus });
				} catch {
					// Not critical
				}
			}
		} catch (err) {
			console.error("[acp-proxy] Failed to update config:", err);
			set({ error: String(err) });
		}

		// Fetch fresh config options in the background (updates cache + UI)
		try {
			const options = await commands.acpProxyFetchConfigOptions(agentName);
			// Only update if the user hasn't switched to another agent in the meantime
			if (get().config?.agentName === agentName) {
				const newCache = {
					...get().configOptionsCache,
					[agentName]: options,
				};
				set({
					configOptions: options,
					configOptionsLoading: false,
					configOptionsCache: newCache,
				});
				// Persist cache to disk
				commands.acpProxySaveOptionsCache(newCache).catch(() => {});
			}
		} catch (err) {
			console.error("[acp-proxy] Failed to fetch config options:", err);
			if (get().config?.agentName === agentName) {
				set({ configOptionsLoading: false });
			}
		}
	},

	start: async () => {
		try {
			set({ loading: true, error: null });
			const status = await commands.acpProxyStart();
			set({ status, loading: false });
		} catch (err) {
			console.error("[acp-proxy] Failed to start:", err);
			set({ error: String(err), loading: false });
		}
	},

	stop: async () => {
		try {
			set({ loading: true, error: null });
			await commands.acpProxyStop();
			const status = await commands.acpProxyGetStatus();
			set({ status, loading: false });
		} catch (err) {
			console.error("[acp-proxy] Failed to stop:", err);
			set({ error: String(err), loading: false });
		}
	},

	refreshStatus: async () => {
		try {
			const status = await commands.acpProxyGetStatus();
			set({ status });
		} catch (err) {
			console.error("[acp-proxy] Failed to refresh status:", err);
		}
	},

	getProxyBaseUrl: () => {
		const status = get().status;
		if (!status?.running) return "";
		return `http://127.0.0.1:${status.port}/v1`;
	},

	fetchConfigOptions: async (agentName) => {
		try {
			// If we have cached options, show them immediately without loading spinner
			const cached = get().configOptionsCache[agentName];
			if (cached && cached.length > 0) {
				set({ configOptions: cached, configOptionsLoading: false });
			} else {
				set({ configOptionsLoading: true });
			}

			const options = await commands.acpProxyFetchConfigOptions(agentName);
			// Only update if the user hasn't switched to another agent in the meantime
			if (get().config?.agentName === agentName) {
				const newCache = {
					...get().configOptionsCache,
					[agentName]: options,
				};
				set({
					configOptions: options,
					configOptionsLoading: false,
					configOptionsCache: newCache,
				});
				// Persist cache to disk
				commands.acpProxySaveOptionsCache(newCache).catch(() => {});
			}
		} catch (err) {
			console.error("[acp-proxy] Failed to fetch config options:", err);
			if (get().config?.agentName === agentName) {
				set({ configOptions: [], configOptionsLoading: false });
			}
		}
	},
}));
