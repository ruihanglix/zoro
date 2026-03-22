// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { create } from "zustand";

// ── Free provider definitions ──────────────────────────────────────────────

export interface FreeProvider {
	id: string;
	name: string;
	displayName: string;
	baseURL: string;
	signUpUrl: string;
	keyPrefix: string;
	/** Top 3 providers are shown by default, rest behind "show more" */
	tier: "primary" | "secondary";
}

export const FREE_PROVIDERS: FreeProvider[] = [
	{
		id: "openrouter",
		name: "OpenRouter",
		displayName: "OpenRouter",
		baseURL: "https://openrouter.ai/api/v1",
		signUpUrl: "https://openrouter.ai/keys",
		keyPrefix: "sk-or-",
		tier: "primary",
	},
	{
		id: "groq",
		name: "Groq",
		displayName: "Groq",
		baseURL: "https://api.groq.com/openai/v1",
		signUpUrl: "https://console.groq.com/keys",
		keyPrefix: "gsk_",
		tier: "primary",
	},
	{
		id: "gemini",
		name: "Gemini",
		displayName: "Google Gemini",
		baseURL: "https://generativelanguage.googleapis.com/v1beta",
		signUpUrl: "https://aistudio.google.com/app/apikey",
		keyPrefix: "AIza",
		tier: "primary",
	},
	{
		id: "github",
		name: "GitHub Models",
		displayName: "GitHub Models",
		baseURL: "https://models.github.ai/inference",
		signUpUrl: "https://github.com/settings/tokens",
		keyPrefix: "ghp_",
		tier: "secondary",
	},
	{
		id: "mistral",
		name: "Mistral",
		displayName: "Mistral AI",
		baseURL: "https://api.mistral.ai/v1",
		signUpUrl: "https://console.mistral.ai/api-keys",
		keyPrefix: "",
		tier: "secondary",
	},
	{
		id: "opencode",
		name: "OpenCode",
		displayName: "OpenCode Zen",
		baseURL: "https://opencode.ai/zen/v1",
		signUpUrl: "https://opencode.ai/auth",
		keyPrefix: "",
		tier: "secondary",
	},
	{
		id: "cerebras",
		name: "Cerebras",
		displayName: "Cerebras",
		baseURL: "https://api.cerebras.ai/v1",
		signUpUrl: "https://cloud.cerebras.ai",
		keyPrefix: "",
		tier: "secondary",
	},
	{
		id: "sambanova",
		name: "SambaNova",
		displayName: "SambaNova",
		baseURL: "https://api.sambanova.ai/v1",
		signUpUrl: "https://cloud.sambanova.ai",
		keyPrefix: "",
		tier: "secondary",
	},
];

// ── Recommended free models per provider ────────────────────────────────────

export interface FreeModel {
	id: string;
	provider: string;
	displayName: string;
}

export const FREE_MODELS: FreeModel[] = [
	{
		id: "google/gemini-2.0-flash-exp:free",
		provider: "openrouter",
		displayName: "Gemini 2.0 Flash (OpenRouter)",
	},
	{
		id: "meta-llama/llama-4-maverick:free",
		provider: "openrouter",
		displayName: "Llama 4 Maverick (OpenRouter)",
	},
	{
		id: "llama-3.3-70b-versatile",
		provider: "groq",
		displayName: "Llama 3.3 70B (Groq)",
	},
	{
		id: "gemini-2.0-flash",
		provider: "gemini",
		displayName: "Gemini 2.0 Flash",
	},
	{
		id: "mistral-small-latest",
		provider: "mistral",
		displayName: "Mistral Small",
	},
	{
		id: "Meta-Llama-3.1-8B-Instruct",
		provider: "github",
		displayName: "Llama 3.1 8B (GitHub)",
	},
	{
		id: "llama-3.3-70b",
		provider: "cerebras",
		displayName: "Llama 3.3 70B (Cerebras)",
	},
	{
		id: "Meta-Llama-3.1-8B-Instruct",
		provider: "sambanova",
		displayName: "Llama 3.1 8B (SambaNova)",
	},
];

// ── localStorage helpers (same pattern as uiStore) ──────────────────────────

const LAB_STORAGE_PREFIX = "zoro-lab-";

function loadSetting<T>(key: string, fallback: T): T {
	try {
		const raw = localStorage.getItem(`${LAB_STORAGE_PREFIX}${key}`);
		if (raw === null) return fallback;
		return JSON.parse(raw) as T;
	} catch {
		return fallback;
	}
}

function saveSetting<T>(key: string, value: T): void {
	try {
		localStorage.setItem(`${LAB_STORAGE_PREFIX}${key}`, JSON.stringify(value));
	} catch {
		// silently ignore
	}
}

// ── Store types ─────────────────────────────────────────────────────────────

export type ListenAddress = "127.0.0.1" | "0.0.0.0";

interface LabState {
	// Free LLM Proxy
	freeLlmEnabled: boolean;
	providerKeys: Record<string, string>; // providerId → API key (stored locally)
	defaultFreeModel: string;
	port: number;

	// Share API
	shareEnabled: boolean;
	listenAddress: ListenAddress;

	// Service status (runtime, not persisted)
	serviceRunning: boolean;
	configuredProviderIds: string[]; // derived from providerKeys

	// Actions
	setFreeLlmEnabled: (enabled: boolean) => void;
	setProviderKey: (providerId: string, key: string) => void;
	removeProviderKey: (providerId: string) => void;
	setDefaultFreeModel: (model: string) => void;
	setPort: (port: number) => void;
	setShareEnabled: (enabled: boolean) => void;
	setListenAddress: (addr: ListenAddress) => void;
	setServiceRunning: (running: boolean) => void;

	// Derived helpers
	getConfiguredCount: () => number;
	getAvailableModels: () => FreeModel[];
	isProviderConfigured: (providerId: string) => boolean;
}

export const useLabStore = create<LabState>((set, get) => {
	// Load persisted provider keys
	const savedKeys = loadSetting<Record<string, string>>("provider-keys", {});
	const configuredIds = Object.keys(savedKeys).filter(
		(k) => savedKeys[k]?.trim().length > 0,
	);

	return {
		freeLlmEnabled: loadSetting<boolean>("free-llm-enabled", true),
		providerKeys: savedKeys,
		defaultFreeModel: loadSetting<string>(
			"default-free-model",
			"google/gemini-2.0-flash-exp:free",
		),
		port: loadSetting<number>("port", 8765),

		shareEnabled: loadSetting<boolean>("share-enabled", false),
		listenAddress: loadSetting<ListenAddress>(
			"listen-address",
			"127.0.0.1",
		),

		serviceRunning: false,
		configuredProviderIds: configuredIds,

		setFreeLlmEnabled: (enabled) => {
			saveSetting("free-llm-enabled", enabled);
			set({ freeLlmEnabled: enabled });
		},

		setProviderKey: (providerId, key) => {
			const keys = { ...get().providerKeys, [providerId]: key };
			saveSetting("provider-keys", keys);
			const configuredIds = Object.keys(keys).filter(
				(k) => keys[k]?.trim().length > 0,
			);
			set({ providerKeys: keys, configuredProviderIds: configuredIds });
		},

		removeProviderKey: (providerId) => {
			const keys = { ...get().providerKeys };
			delete keys[providerId];
			saveSetting("provider-keys", keys);
			const configuredIds = Object.keys(keys).filter(
				(k) => keys[k]?.trim().length > 0,
			);
			set({ providerKeys: keys, configuredProviderIds: configuredIds });
		},

		setDefaultFreeModel: (model) => {
			saveSetting("default-free-model", model);
			set({ defaultFreeModel: model });
		},

		setPort: (port) => {
			saveSetting("port", port);
			set({ port });
		},

		setShareEnabled: (enabled) => {
			saveSetting("share-enabled", enabled);
			set({ shareEnabled: enabled });
		},

		setListenAddress: (addr) => {
			saveSetting("listen-address", addr);
			set({ listenAddress: addr });
		},

		setServiceRunning: (running) => {
			set({ serviceRunning: running });
		},

		getConfiguredCount: () => {
			return get().configuredProviderIds.length;
		},

		getAvailableModels: () => {
			const configured = get().configuredProviderIds;
			return FREE_MODELS.filter((m) => configured.includes(m.provider));
		},

		isProviderConfigured: (providerId) => {
			return get().configuredProviderIds.includes(providerId);
		},
	};
});
