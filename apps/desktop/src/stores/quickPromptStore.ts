// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { loadSetting, saveSetting } from "@/stores/uiStore";
import { create } from "zustand";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type PromptIndicator =
	| { type: "emoji"; value: string }
	| { type: "letter"; value: string; color: string };

export interface QuickPrompt {
	id: string;
	label: string;
	content: string;
	indicator: PromptIndicator;
}

// ---------------------------------------------------------------------------
// Preset colours for "letter" indicators
// ---------------------------------------------------------------------------

export const PRESET_COLORS = [
	{ name: "red", value: "#ef4444" },
	{ name: "orange", value: "#f97316" },
	{ name: "yellow", value: "#eab308" },
	{ name: "green", value: "#22c55e" },
	{ name: "blue", value: "#3b82f6" },
	{ name: "purple", value: "#a855f7" },
	{ name: "pink", value: "#ec4899" },
	{ name: "gray", value: "#6b7280" },
] as const;

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

const DEFAULT_PROMPTS: QuickPrompt[] = [
	{
		id: "default-summarize",
		label: "Summarize",
		content:
			"Summarize the key findings of this paper in 3 bullet points.",
		indicator: { type: "emoji", value: "📝" },
	},
	{
		id: "default-critique",
		label: "Critique",
		content:
			"What are the main limitations and potential issues with this paper's methodology?",
		indicator: { type: "emoji", value: "🔍" },
	},
	{
		id: "default-questions",
		label: "Key Questions",
		content:
			"List 5 key questions to deepen understanding of this paper.",
		indicator: { type: "emoji", value: "❓" },
	},
	{
		id: "default-explain",
		label: "Explain",
		content:
			"Explain this paper's core contribution in simple terms.",
		indicator: { type: "emoji", value: "💡" },
	},
];

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

const STORAGE_KEY = "zoro-quick-prompts";

interface QuickPromptStore {
	prompts: QuickPrompt[];

	addPrompt: (prompt: QuickPrompt) => void;
	updatePrompt: (id: string, patch: Partial<Omit<QuickPrompt, "id">>) => void;
	removePrompt: (id: string) => void;
	reorderPrompts: (orderedIds: string[]) => void;
	resetToDefaults: () => void;
}

export const useQuickPromptStore = create<QuickPromptStore>((set) => {
	const persist = (prompts: QuickPrompt[]) =>
		saveSetting(STORAGE_KEY, prompts);

	return {
		prompts: loadSetting<QuickPrompt[]>(STORAGE_KEY, DEFAULT_PROMPTS),

		addPrompt: (prompt) =>
			set((s) => {
				const next = [...s.prompts, prompt];
				persist(next);
				return { prompts: next };
			}),

		updatePrompt: (id, patch) =>
			set((s) => {
				const next = s.prompts.map((p) =>
					p.id === id ? { ...p, ...patch } : p,
				);
				persist(next);
				return { prompts: next };
			}),

		removePrompt: (id) =>
			set((s) => {
				const next = s.prompts.filter((p) => p.id !== id);
				persist(next);
				return { prompts: next };
			}),

		reorderPrompts: (orderedIds) =>
			set((s) => {
				const map = new Map(s.prompts.map((p) => [p.id, p]));
				const next = orderedIds
					.map((id) => map.get(id))
					.filter(Boolean) as QuickPrompt[];
				persist(next);
				return { prompts: next };
			}),

		resetToDefaults: () =>
			set(() => {
				persist(DEFAULT_PROMPTS);
				return { prompts: [...DEFAULT_PROMPTS] };
			}),
	};
});
