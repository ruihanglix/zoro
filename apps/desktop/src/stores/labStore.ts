// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { create } from "zustand";
import { httpProxyGet } from "@/lib/commands";

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

// ── Model type (all models are fetched dynamically from provider APIs) ───────

export interface FreeModel {
	id: string;
	provider: string;
	displayName: string;
}

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

	// Routing
	routingStrategy: RoutingStrategy;
	roundRobinIndex: number;

	// Model exclusion list — models the user has disabled from auto-routing
	disabledModels: Set<string>; // "providerId:modelId" composite keys

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
	setRoutingStrategy: (strategy: RoutingStrategy) => void;

	// Model exclusion actions
	toggleModelDisabled: (providerId: string, modelId: string) => void;
	isModelDisabled: (providerId: string, modelId: string) => boolean;

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
	/** Get available models excluding disabled ones (for sync to AI config) */
	getEnabledModelsForProvider: (providerId: string) => string[];
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
	const results: { model: FreeModel; provider: FreeProvider; apiKey: string }[] = [];
	for (const providerId of state.configuredProviderIds) {
		const provider = FREE_PROVIDERS.find((p) => p.id === providerId);
		if (!provider) continue;
		const apiKey = state.providerKeys[providerId]?.trim();
		if (!apiKey) continue;
		if (!isProviderHealthy(state.providerHealth[providerId], now)) continue;
		const fetched = state.fetchedModels[providerId];
		if (!fetched) continue;
		for (const modelId of fetched) {
			// Skip models the user has disabled
			if (state.disabledModels.has(`${providerId}:${modelId}`)) continue;
			results.push({
				model: {
					id: modelId,
					provider: providerId,
					displayName: `${modelId} (${provider.displayName} 🧪)`,
				},
				provider,
				apiKey,
			});
		}
	}
	return results;
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
		defaultFreeModel: loadSetting<string>("default-free-model", ""),

		routingStrategy: loadSetting<RoutingStrategy>(
			"routing-strategy",
			"auto",
		),
		roundRobinIndex: loadSetting<number>("round-robin-index", 0),

		disabledModels: new Set<string>(
			loadSetting<string[]>("disabled-models", []),
		),

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

		setRoutingStrategy: (strategy) => {
			saveSetting("routing-strategy", strategy);
			set({ routingStrategy: strategy });
		},

		// ── Dynamic model fetching ─────────────────────────────────────

		fetchModelsForProvider: async (providerId: string) => {
			const state = get();
			const apiKey = state.providerKeys[providerId]?.trim();
			if (!apiKey) {
				console.warn(`[lab] Skip fetching models for ${providerId}: no API key configured`);
				return;
			}

			const provider = FREE_PROVIDERS.find((p) => p.id === providerId);
			if (!provider) {
				console.warn(`[lab] Skip fetching models for ${providerId}: unknown provider`);
				return;
			}

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

				// Debug: log the request details (mask the API key)
				const maskedKey = apiKey.length > 8
					? `${apiKey.slice(0, 4)}...${apiKey.slice(-4)}`
					: "****";
				console.log(`[lab] Fetching models from ${providerId}:`);
				console.log(`[lab]   URL: ${url.replace(apiKey, maskedKey)}`);
				console.log(`[lab]   Headers: ${JSON.stringify({ ...headers, Authorization: headers.Authorization ? `Bearer ${maskedKey}` : undefined })}`);

				// Use Rust backend proxy to bypass browser CORS restrictions
				const resp = await httpProxyGet(url, headers);
				console.log(`[lab]   Response status: ${resp.status}`);

				if (resp.status < 200 || resp.status >= 300) {
					console.warn(`[lab]   Response body: ${resp.body.slice(0, 500)}`);
					return;
				}

				const json = JSON.parse(resp.body);
				console.log(`[lab]   Response keys: ${Object.keys(json).join(", ")}`);

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

				console.log(`[lab]   Parsed ${modelIds.length} model(s)${modelIds.length > 0 ? `: ${modelIds.slice(0, 5).join(", ")}${modelIds.length > 5 ? ` ... +${modelIds.length - 5} more` : ""}` : ""}`);

				if (modelIds.length > 0) {
					const currentState = get();
					const prevModels = currentState.fetchedModels[providerId]; // undefined on first fetch
					const disabled = new Set(currentState.disabledModels);
					const newModelSet = new Set(modelIds);

					if (!prevModels) {
						// ── First fetch for this provider ──
						// Only enable models whose id contains "free" (case-insensitive)
						for (const mid of modelIds) {
							if (!mid.toLowerCase().includes("free")) {
								disabled.add(`${providerId}:${mid}`);
							}
						}
					} else {
						// ── Subsequent fetch ──
						const prevSet = new Set(prevModels);
						const userTouched = new Set<string>(); // models that existed before → preserve user choice
						for (const mid of prevModels) {
							userTouched.add(`${providerId}:${mid}`);
						}

						// 1) Remove stale entries for models that no longer exist
						for (const mid of prevModels) {
							if (!newModelSet.has(mid)) {
								disabled.delete(`${providerId}:${mid}`);
							}
						}

						// 2) For newly appeared models, default-disable if not "free"
						for (const mid of modelIds) {
							if (!prevSet.has(mid)) {
								// Brand new model — user has never touched it
								if (!mid.toLowerCase().includes("free")) {
									disabled.add(`${providerId}:${mid}`);
								}
							}
							// Existing models: do NOT touch — preserve user's choice
						}
					}

					saveSetting("disabled-models", Array.from(disabled));
					const fetched = { ...currentState.fetchedModels, [providerId]: modelIds };
					set({ fetchedModels: fetched, disabledModels: disabled });
				} else {
					console.warn(`[lab]   No models parsed from response. Full response (first 500 chars): ${resp.body.slice(0, 500)}`);
				}
			} catch (err) {
				console.error(`[lab] Error fetching models from ${providerId}:`, err);
			}
		},

		fetchAllProviderModels: async () => {
			const state = get();
			set({ modelFetchLoading: true });
			const configured = state.configuredProviderIds;
			console.log(`[lab] Refreshing models for ${configured.length} configured provider(s): ${configured.join(", ") || "(none)"}`);
			const promises = configured.map((id) =>
				get().fetchModelsForProvider(id),
			);
			await Promise.allSettled(promises);
			const finalState = get();
			const totalModels = Object.values(finalState.fetchedModels).reduce((sum, arr) => sum + arr.length, 0);
			console.log(`[lab] Model refresh complete. Total: ${totalModels} model(s) across ${Object.keys(finalState.fetchedModels).length} provider(s)`);
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

		toggleModelDisabled: (providerId, modelId) => {
			const key = `${providerId}:${modelId}`;
			const disabled = new Set(get().disabledModels);
			if (disabled.has(key)) {
				disabled.delete(key);
			} else {
				disabled.add(key);
			}
			saveSetting("disabled-models", Array.from(disabled));
			set({ disabledModels: disabled });
		},

		isModelDisabled: (providerId, modelId) => {
			return get().disabledModels.has(`${providerId}:${modelId}`);
		},

		getAvailableModels: () => {
			const state = get();
			const models: FreeModel[] = [];
			const seen = new Set<string>();

			for (const providerId of state.configuredProviderIds) {
				const fetched = state.fetchedModels[providerId];
				if (!fetched) continue;
				const provider = FREE_PROVIDERS.find((p) => p.id === providerId);
				if (!provider) continue;
				for (const modelId of fetched) {
					// Skip models the user has disabled
					if (state.disabledModels.has(`${providerId}:${modelId}`)) continue;
					if (!seen.has(modelId)) {
						models.push({
							id: modelId,
							provider: providerId,
							displayName: `${modelId} (${provider.displayName} 🧪)`,
						});
						seen.add(modelId);
					}
				}
			}

			return models;
		},

		getEnabledModelsForProvider: (providerId) => {
			const state = get();
			const fetched = state.fetchedModels[providerId] || [];
			return fetched.filter(
				(modelId) => !state.disabledModels.has(`${providerId}:${modelId}`),
			);
		},

		isProviderConfigured: (providerId) => {
			return get().configuredProviderIds.includes(providerId);
		},
	};
});
