// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import {
	DEFAULT_KEYBINDINGS,
	type KeyBindingOverrides,
	type ResolvedKeyBinding,
	mergeWithOverrides,
	normalizeKey,
} from "@/lib/keybindings";
import { loadSetting, saveSetting } from "@/stores/uiStore";
import { create } from "zustand";

const STORAGE_KEY = "zoro-keybindings";

interface KeybindingState {
	bindings: ResolvedKeyBinding[];
	overrides: KeyBindingOverrides;
	/** Bumped on every mutation so hooks re-attach listeners */
	version: number;

	rebind: (id: string, newKey: string | null) => void;
	resetBinding: (id: string) => void;
	resetAll: () => void;
}

export const useKeybindingStore = create<KeybindingState>((set) => {
	const overrides = loadSetting<KeyBindingOverrides>(STORAGE_KEY, {});
	const bindings = mergeWithOverrides(DEFAULT_KEYBINDINGS, overrides);

	return {
		bindings,
		overrides,
		version: 0,

		rebind: (id, newKey) =>
			set((state) => {
				const next = {
					...state.overrides,
					[id]: newKey ? normalizeKey(newKey) : null,
				};
				saveSetting(STORAGE_KEY, next);
				return {
					overrides: next,
					bindings: mergeWithOverrides(DEFAULT_KEYBINDINGS, next),
					version: state.version + 1,
				};
			}),

		resetBinding: (id) =>
			set((state) => {
				const next = { ...state.overrides };
				delete next[id];
				saveSetting(STORAGE_KEY, next);
				return {
					overrides: next,
					bindings: mergeWithOverrides(DEFAULT_KEYBINDINGS, next),
					version: state.version + 1,
				};
			}),

		resetAll: () =>
			set((state) => {
				saveSetting(STORAGE_KEY, {});
				return {
					overrides: {},
					bindings: mergeWithOverrides(DEFAULT_KEYBINDINGS, {}),
					version: state.version + 1,
				};
			}),
	};
});
