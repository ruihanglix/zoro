// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { logger } from "@/lib/logger";

// ── Types (mirroring Rust backend) ──────────────────────────────────────────

export interface FreeProvider {
	id: string;
	name: string;
	display_name: string;
	sign_up_url: string;
	key_prefix: string;
	tier: string;
	has_key: boolean;
	key_count: number;
	model_count: number;
}

export interface LabModel {
	id: string;
	name: string;
	provider_id: string;
	disabled: boolean;
}

export interface LabProxyStatus {
	running: boolean;
	port: number;
	listen_addr: string;
	provider_count: number;
	model_count: number;
	strategy: string;
	health: ProviderHealthStatus[];
}

export interface ProviderHealthStatus {
	provider_id: string;
	healthy: boolean;
	consecutive_fails: number;
	total_requests: number;
	total_failures: number;
	on_cooldown: boolean;
}

export type RoutingStrategy = "auto" | "round-robin" | "manual";

export interface LabConfig {
	provider_keys: Record<string, string[]>;
	disabled_models: string[];
	routing_strategy: RoutingStrategy;
	enabled: boolean;
	proxy_port: number;
	lan_access: boolean;
	access_token: string;
}

// ── Store types ─────────────────────────────────────────────────────────────

interface LabState {
	// State from backend
	enabled: boolean;
	providers: FreeProvider[];
	models: LabModel[];
	proxyStatus: LabProxyStatus | null;
	config: LabConfig | null;

	// UI state
	loading: boolean;
	modelsLoading: boolean;
	error: string | null;

	// Actions — all delegate to Tauri commands
	initialize: () => Promise<void>;
	setEnabled: (enabled: boolean) => Promise<void>;
	addProviderKey: (providerId: string, apiKey: string) => Promise<void>;
	removeProviderKey: (providerId: string, keyIndex: number) => Promise<void>;
	getProviderKeys: (providerId: string) => Promise<string[]>;
	refreshModels: () => Promise<void>;
	toggleModelDisabled: (
		providerId: string,
		modelId: string,
		disabled: boolean,
	) => Promise<void>;
	toggleProviderDisabled: (
		providerId: string,
		disabled: boolean,
	) => Promise<void>;
	setRoutingStrategy: (strategy: RoutingStrategy) => Promise<void>;
	startProxy: () => Promise<void>;
	stopProxy: () => Promise<void>;
	reloadProxy: () => Promise<void>;
	refreshProxyStatus: () => Promise<void>;
	setProxyPort: (port: number) => Promise<void>;
	setLanAccess: (enabled: boolean) => Promise<void>;

	// Derived helpers
	getConfiguredCount: () => number;
	getAvailableModels: () => LabModel[];
	getEnabledModels: () => LabModel[];
	getProxyBaseUrl: () => string;
	isProviderConfigured: (providerId: string) => boolean;
}

// ── Store ───────────────────────────────────────────────────────────────────

export const useLabStore = create<LabState>((set, get) => ({
	enabled: false,
	providers: [],
	models: [],
	proxyStatus: null,
	config: null,
	loading: false,
	modelsLoading: false,
	error: null,

	initialize: async () => {
		try {
			set({ loading: true, error: null });
			const [config, providers, models, proxyStatus] = await Promise.all([
				invoke<LabConfig>("lab_get_config"),
				invoke<FreeProvider[]>("lab_list_providers"),
				invoke<LabModel[]>("lab_list_models"),
				invoke<LabProxyStatus>("lab_get_proxy_status"),
			]);
			set({
				config,
				enabled: config.enabled,
				providers,
				models,
				proxyStatus,
				loading: false,
			});
		} catch (err) {
			logger.error("lab", "Failed to initialize", err);
			set({ error: String(err), loading: false });
		}
	},

	setEnabled: async (enabled) => {
		try {
			// Optimistically update enabled state so the toggle reflects immediately
			set({ loading: true, error: null, enabled });
			const status = await invoke<LabProxyStatus>("lab_set_enabled", {
				enabled,
			});
			set({ proxyStatus: status, loading: false });
			// Refresh full state
			await get().initialize();
		} catch (err) {
			logger.error("lab", "Failed to set enabled", err);
			// Keep the enabled state (backend already persisted it),
			// only clear loading and show the error
			set({ error: String(err), loading: false });
		}
	},

	addProviderKey: async (providerId, apiKey) => {
		try {
			await invoke("lab_add_provider_key", { providerId, apiKey });
			// Refresh models for this provider
			if (apiKey.trim()) {
				set({ modelsLoading: true });
				await invoke("lab_refresh_models");
				set({ modelsLoading: false });
			}
			// Refresh full state to pick up new models/providers
			const [providers, models] = await Promise.all([
				invoke<FreeProvider[]>("lab_list_providers"),
				invoke<LabModel[]>("lab_list_models"),
			]);
			set({ providers, models });
			// If proxy is running, reload it with new providers
			const status = get().proxyStatus;
			if (status?.running) {
				await get().reloadProxy();
			}
		} catch (err) {
			logger.error("lab", "Failed to add provider key", err);
			set({ error: String(err), modelsLoading: false });
		}
	},

	removeProviderKey: async (providerId, keyIndex) => {
		try {
			await invoke("lab_remove_provider_key", { providerId, keyIndex });
			// Refresh full state to pick up updated providers
			const [providers, models] = await Promise.all([
				invoke<FreeProvider[]>("lab_list_providers"),
				invoke<LabModel[]>("lab_list_models"),
			]);
			set({ providers, models });
			// If proxy is running, reload it with updated providers
			const status = get().proxyStatus;
			if (status?.running) {
				await get().reloadProxy();
			}
		} catch (err) {
			logger.error("lab", "Failed to remove provider key", err);
			set({ error: String(err) });
		}
	},

	getProviderKeys: async (providerId) => {
		try {
			return await invoke<string[]>("lab_get_provider_keys", { providerId });
		} catch (err) {
			logger.error("lab", "Failed to get provider keys", err);
			return [];
		}
	},

	refreshModels: async () => {
		try {
			set({ modelsLoading: true, error: null });
			await invoke("lab_refresh_models");
			const models = await invoke<LabModel[]>("lab_list_models");
			const providers = await invoke<FreeProvider[]>("lab_list_providers");
			set({ models, providers, modelsLoading: false });
		} catch (err) {
			logger.error("lab", "Failed to refresh models", err);
			set({ error: String(err), modelsLoading: false });
		}
	},

	toggleModelDisabled: async (providerId, modelId, disabled) => {
		// Optimistic update: reflect change immediately in UI
		const prevModels = get().models;
		set({
			models: prevModels.map((m) =>
				m.id === modelId && m.provider_id === providerId
					? { ...m, disabled }
					: m,
			),
		});
		try {
			await invoke("lab_toggle_model", { providerId, modelId, disabled });
			// Sync with backend to ensure consistency
			const models = await invoke<LabModel[]>("lab_list_models");
			set({ models });
			// If proxy is running, reload to apply model changes
			const status = get().proxyStatus;
			if (status?.running) {
				await get().reloadProxy();
			}
		} catch (err) {
			// Rollback on failure
			set({ models: prevModels, error: String(err) });
			logger.error("lab", "Failed to toggle model", err);
		}
	},

	toggleProviderDisabled: async (providerId, disabled) => {
		// Optimistic update: reflect change immediately in UI
		const prevModels = get().models;
		set({
			models: prevModels.map((m) =>
				m.provider_id === providerId ? { ...m, disabled } : m,
			),
		});
		try {
			await invoke("lab_toggle_provider", { providerId, disabled });
			// Sync with backend to ensure consistency
			const models = await invoke<LabModel[]>("lab_list_models");
			set({ models });
			// If proxy is running, reload to apply model changes
			const status = get().proxyStatus;
			if (status?.running) {
				await get().reloadProxy();
			}
		} catch (err) {
			// Rollback on failure
			set({ models: prevModels, error: String(err) });
			logger.error("lab", "Failed to toggle provider", err);
		}
	},

	setRoutingStrategy: async (strategy) => {
		try {
			await invoke("lab_set_strategy", { strategy });
			const config = await invoke<LabConfig>("lab_get_config");
			set({ config });
		} catch (err) {
			logger.error("lab", "Failed to set routing strategy", err);
			set({ error: String(err) });
		}
	},

	startProxy: async () => {
		try {
			set({ loading: true, error: null });
			const status = await invoke<LabProxyStatus>("lab_start_proxy");
			set({ proxyStatus: status, loading: false });
		} catch (err) {
			logger.error("lab", "Failed to start proxy", err);
			set({ error: String(err), loading: false });
		}
	},

	stopProxy: async () => {
		try {
			set({ loading: true, error: null });
			await invoke("lab_stop_proxy");
			const status = await invoke<LabProxyStatus>("lab_get_proxy_status");
			set({ proxyStatus: status, loading: false });
		} catch (err) {
			logger.error("lab", "Failed to stop proxy", err);
			set({ error: String(err), loading: false });
		}
	},

	reloadProxy: async () => {
		try {
			const status = await invoke<LabProxyStatus>("lab_reload_proxy");
			set({ proxyStatus: status });
		} catch (err) {
			logger.error("lab", "Failed to reload proxy", err);
			set({ error: String(err) });
		}
	},

	refreshProxyStatus: async () => {
		try {
			const status = await invoke<LabProxyStatus>("lab_get_proxy_status");
			set({ proxyStatus: status });
		} catch (err) {
			logger.error("lab", "Failed to refresh proxy status", err);
		}
	},

	setProxyPort: async (port) => {
		try {
			await invoke("lab_set_proxy_port", { port });
			const config = await invoke<LabConfig>("lab_get_config");
			set({ config });
		} catch (err) {
			logger.error("lab", "Failed to set proxy port", err);
			set({ error: String(err) });
		}
	},

	setLanAccess: async (enabled) => {
		try {
			await invoke("lab_set_lan_access", { enabled });
			const config = await invoke<LabConfig>("lab_get_config");
			set({ config });
		} catch (err) {
			logger.error("lab", "Failed to set LAN access", err);
			set({ error: String(err) });
		}
	},

	// ── Derived helpers ─────────────────────────────────────────────────

	getConfiguredCount: () => {
		return get().providers.filter((p) => p.has_key).length;
	},

	getAvailableModels: () => {
		return get().models;
	},

	getEnabledModels: () => {
		return get().models.filter((m) => !m.disabled);
	},

	getProxyBaseUrl: () => {
		const status = get().proxyStatus;
		if (!status?.running) return "";
		return `http://127.0.0.1:${status.port}/v1`;
	},

	isProviderConfigured: (providerId) => {
		return get().providers.some((p) => p.id === providerId && p.has_key);
	},
}));
