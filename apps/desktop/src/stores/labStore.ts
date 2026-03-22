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
		id: "deepseek-ai/DeepSeek-V3-0324",
		provider: "opencode",
		displayName: "DeepSeek V3 (OpenCode Zen)",
	},
	{
		id: "Qwen/Qwen2.5-Coder-32B-Instruct",
		provider: "opencode",
		displayName: "Qwen 2.5 Coder 32B (OpenCode Zen)",
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

// ── Routing strategy ────────────────────────────────────────────────────────

export type RoutingStrategy = "auto" | "round-robin" | "manual";

/**
 * Runtime health record for each provider.
 * NOT persisted — resets on app restart.
 */
interface ProviderHealth {
	lastSuccess: number;
	lastFailure: number;
	consecutiveFails: number;
	/** Timestamp: skip this provider until this time */
	cooldownUntil: number;
}

/** After this many consecutive failures, put the provider on cooldown */
const COOLDOWN_FAIL_THRESHOLD = 3;
/** Cooldown duration in ms (5 minutes) */
const COOLDOWN_DURATION_MS = 5 * 60 * 1000;

export interface ResolvedRoute {
	provider: FreeProvider;
	model: FreeModel;
	apiKey: string;
}

interface LabState {
	// Free LLM Proxy
	freeLlmEnabled: boolean;
	providerKeys: Record<string, string>; // providerId → API key (stored locally)
	defaultFreeModel: string;
	port: number;

	// Routing
	routingStrategy: RoutingStrategy;
	roundRobinIndex: number;

	// Share API
	shareEnabled: boolean;
	listenAddress: ListenAddress;

	// Service status (runtime, not persisted)
	serviceRunning: boolean;
	configuredProviderIds: string[]; // derived from providerKeys

	// Provider health (runtime, not persisted)
	providerHealth: Record<string, ProviderHealth>;

	// Dynamically fetched models (runtime, not persisted)
	fetchedModels: Record<string, string[]>; // providerId → model id list
	modelFetchLoading: boolean;

	// Actions
	setFreeLlmEnabled: (enabled: boolean) => void;
	setProviderKey: (providerId: string, key: string) => void;
	removeProviderKey: (providerId: string) => void;
	setDefaultFreeModel: (model: string) => void;
	setPort: (port: number) => void;
	setShareEnabled: (enabled: boolean) => void;
	setListenAddress: (addr: ListenAddress) => void;
	setServiceRunning: (running: boolean) => void;
	setRoutingStrategy: (strategy: RoutingStrategy) => void;

	// Routing actions
	/**
	 * Resolve the next provider+model to use for a request.
	 * @param requestedModelId - The model the user/system wants. Pass empty string for "any".
	 * @returns A resolved route, or null if no provider is available.
	 */
	resolveProvider: (requestedModelId?: string) => ResolvedRoute | null;
	/** Fetch models from a provider's /models endpoint */
	fetchModelsForProvider: (providerId: string) => Promise<void>;
	/** Fetch models from all configured providers */
	fetchAllProviderModels: () => Promise<void>;

	/** Call after a successful request to update health */
	reportSuccess: (providerId: string) => void;
	/** Call after a failed request to update health & trigger cooldown */
	reportFailure: (providerId: string) => void;

	// Derived helpers
	getConfiguredCount: () => number;
	getAvailableModels: () => FreeModel[];
	isProviderConfigured: (providerId: string) => boolean;
}

// ── Routing helpers (pure functions) ────────────────────────────────────────

function isProviderHealthy(
	health: ProviderHealth | undefined,
	now: number,
): boolean {
	if (!health) return true; // never failed → healthy
	if (health.cooldownUntil > now) return false; // on cooldown
	return true;
}

function getHealthyAvailableModels(
	state: LabState,
	now: number,
): { model: FreeModel; provider: FreeProvider; apiKey: string }[] {
	const configured = state.configuredProviderIds;
	return FREE_MODELS.filter((m) => configured.includes(m.provider))
		.map((m) => {
			const provider = FREE_PROVIDERS.find((p) => p.id === m.provider);
			if (!provider) return null;
			const apiKey = state.providerKeys[provider.id];
			if (!apiKey?.trim()) return null;
			if (!isProviderHealthy(state.providerHealth[provider.id], now))
				return null;
			return { model: m, provider, apiKey };
		})
		.filter(Boolean) as {
		model: FreeModel;
		provider: FreeProvider;
		apiKey: string;
	}[];
}

// ── Store ───────────────────────────────────────────────────────────────────

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

		routingStrategy: loadSetting<RoutingStrategy>(
			"routing-strategy",
			"auto",
		),
		roundRobinIndex: loadSetting<number>("round-robin-index", 0),

		shareEnabled: loadSetting<boolean>("share-enabled", false),
		listenAddress: loadSetting<ListenAddress>(
			"listen-address",
			"127.0.0.1",
		),

		serviceRunning: false,
		configuredProviderIds: configuredIds,
		providerHealth: {},
		fetchedModels: {},
		modelFetchLoading: false,

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

		setRoutingStrategy: (strategy) => {
			saveSetting("routing-strategy", strategy);
			set({ routingStrategy: strategy });
		},

		// ── Dynamic model fetching ─────────────────────────────────────

		fetchModelsForProvider: async (providerId: string) => {
			const state = get();
			const apiKey = state.providerKeys[providerId]?.trim();
			if (!apiKey) return;

			const provider = FREE_PROVIDERS.find((p) => p.id === providerId);
			if (!provider) return;

			try {
				// Gemini uses a different endpoint format
				const isGemini = providerId === "gemini";
				const url = isGemini
					? `${provider.baseURL}/models?key=${apiKey}`
					: `${provider.baseURL}/models`;

				const headers: Record<string, string> = {
					"Content-Type": "application/json",
				};
				if (!isGemini) {
					headers.Authorization = `Bearer ${apiKey}`;
				}

				const resp = await fetch(url, { headers, signal: AbortSignal.timeout(15000) });
				if (!resp.ok) {
					console.warn(`[lab] Failed to fetch models from ${providerId}: ${resp.status}`);
					return;
				}

				const json = await resp.json();

				// Parse model list (OpenAI-compatible vs Gemini format)
				let modelIds: string[] = [];
				if (isGemini && Array.isArray(json.models)) {
					modelIds = json.models
						.map((m: { name?: string }) => m.name?.replace("models/", "") ?? "")
						.filter(Boolean);
				} else if (Array.isArray(json.data)) {
					modelIds = json.data
						.map((m: { id?: string }) => m.id ?? "")
						.filter(Boolean);
				}

				if (modelIds.length > 0) {
					const fetched = { ...get().fetchedModels, [providerId]: modelIds };
					set({ fetchedModels: fetched });
					console.log(`[lab] Fetched ${modelIds.length} models from ${providerId}`);
				}
			} catch (err) {
				console.warn(`[lab] Error fetching models from ${providerId}:`, err);
			}
		},

		fetchAllProviderModels: async () => {
			const state = get();
			set({ modelFetchLoading: true });
			const configured = state.configuredProviderIds;
			const promises = configured.map((id) =>
				get().fetchModelsForProvider(id),
			);
			await Promise.allSettled(promises);
			set({ modelFetchLoading: false });
		},

		// ── Core routing logic ─────────────────────────────────────────

		resolveProvider: (requestedModelId?: string) => {
			const state = get();
			const now = Date.now();
			const strategy = state.routingStrategy;
			const healthy = getHealthyAvailableModels(state, now);

			if (healthy.length === 0) return null;

			// ── Manual: fixed model, no fallback ──
			if (strategy === "manual") {
				const modelId = requestedModelId || state.defaultFreeModel;
				const match = healthy.find((h) => h.model.id === modelId);
				return match
					? {
							provider: match.provider,
							model: match.model,
							apiKey: match.apiKey,
						}
					: null;
			}

			// ── Round Robin: cycle through ALL available models ──
			if (strategy === "round-robin") {
				const idx = state.roundRobinIndex % healthy.length;
				const pick = healthy[idx];
				const nextIdx = (idx + 1) % healthy.length;
				saveSetting("round-robin-index", nextIdx);
				set({ roundRobinIndex: nextIdx });
				return {
					provider: pick.provider,
					model: pick.model,
					apiKey: pick.apiKey,
				};
			}

			// ── Auto: try preferred first, then fallback across all models ──
			const modelId = requestedModelId || state.defaultFreeModel;

			// 1) Try the exact requested model
			const preferred = healthy.find((h) => h.model.id === modelId);
			if (preferred) {
				return {
					provider: preferred.provider,
					model: preferred.model,
					apiKey: preferred.apiKey,
				};
			}

			// 2) Fallback: pick the first healthy model (cross-series allowed)
			const fallback = healthy[0];
			return {
				provider: fallback.provider,
				model: fallback.model,
				apiKey: fallback.apiKey,
			};
		},

		reportSuccess: (providerId) => {
			const health = { ...get().providerHealth };
			health[providerId] = {
				lastSuccess: Date.now(),
				lastFailure: health[providerId]?.lastFailure ?? 0,
				consecutiveFails: 0,
				cooldownUntil: 0,
			};
			set({ providerHealth: health });
		},

		reportFailure: (providerId) => {
			const now = Date.now();
			const health = { ...get().providerHealth };
			const prev = health[providerId];
			const fails = (prev?.consecutiveFails ?? 0) + 1;
			health[providerId] = {
				lastSuccess: prev?.lastSuccess ?? 0,
				lastFailure: now,
				consecutiveFails: fails,
				cooldownUntil:
					fails >= COOLDOWN_FAIL_THRESHOLD
						? now + COOLDOWN_DURATION_MS
						: 0,
			};
			set({ providerHealth: health });
		},

		getConfiguredCount: () => {
			return get().configuredProviderIds.length;
		},

		getAvailableModels: () => {
			const state = get();
			const configured = state.configuredProviderIds;

			// Start with static models for configured providers
			const staticModels = FREE_MODELS.filter((m) =>
				configured.includes(m.provider),
			);
			const staticIds = new Set(staticModels.map((m) => m.id));

			// Merge dynamically fetched models (skip duplicates)
			const dynamicModels: FreeModel[] = [];
			for (const providerId of configured) {
				const fetched = state.fetchedModels[providerId];
				if (!fetched) continue;
				const provider = FREE_PROVIDERS.find((p) => p.id === providerId);
				if (!provider) continue;
				for (const modelId of fetched) {
					if (!staticIds.has(modelId)) {
						dynamicModels.push({
							id: modelId,
							provider: providerId,
							displayName: `${modelId} (${provider.displayName})`,
						});
						staticIds.add(modelId); // prevent cross-provider dupes
					}
				}
			}

			return [...staticModels, ...dynamicModels];
		},

		isProviderConfigured: (providerId) => {
			return get().configuredProviderIds.includes(providerId);
		},
	};
});
