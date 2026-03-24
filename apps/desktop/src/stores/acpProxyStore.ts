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

	// UI state
	loading: boolean;
	error: string | null;

	// Actions
	initialize: () => Promise<void>;
	setEnabled: (enabled: boolean) => Promise<void>;
	updateConfig: (config: AcpProxyConfig) => Promise<void>;
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
	loading: false,
	error: null,

	initialize: async () => {
		try {
			set({ loading: true, error: null });
			const [config, status, agents] = await Promise.all([
				commands.acpProxyGetConfig(),
				commands.acpProxyGetStatus(),
				commands.acpListAgents(),
			]);
			set({
				config,
				status,
				agents,
				loading: false,
			});

			// Auto-fetch config options for the selected agent
			if (config.agentName) {
				get().fetchConfigOptions(config.agentName);
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
					const freshConfig = await commands.acpProxyGetConfig();
					set({ status: newStatus, config: freshConfig });
				} catch {
					// Not critical — proxy just stays not-running until user manually starts
				}
			}
		} catch (err) {
			console.error("[acp-proxy] Failed to update config:", err);
			set({ error: String(err), loading: false });
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
			set({ configOptionsLoading: true });
			const options = await commands.acpProxyFetchConfigOptions(agentName);
			set({ configOptions: options, configOptionsLoading: false });
		} catch (err) {
			console.error("[acp-proxy] Failed to fetch config options:", err);
			set({ configOptions: [], configOptionsLoading: false });
		}
	},
}));
