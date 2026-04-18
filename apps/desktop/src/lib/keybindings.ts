// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

export type KeyScope = "global" | "library" | "reader" | "editor";

export interface KeyBinding {
	id: string;
	scope: KeyScope;
	category: string;
	labelKey: string;
	defaultKey: string | null;
}

export type KeyBindingOverrides = Record<string, string | null>;

export interface ResolvedKeyBinding extends KeyBinding {
	key: string | null;
	isOverridden: boolean;
}

const isMac =
	typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);

// Canonical modifier order
const MOD_ORDER = ["Ctrl", "Alt", "Shift", "Meta"] as const;

/**
 * Normalize a key accelerator string to a canonical form.
 * "Mod" is resolved to "Meta" on macOS, "Ctrl" elsewhere.
 * Modifiers are sorted in canonical order, key is lowercased.
 */
export function normalizeKey(raw: string): string {
	const parts = raw.split("+").map((s) => s.trim());
	const mods = new Set<string>();
	let key = "";

	for (const p of parts) {
		const lower = p.toLowerCase();
		if (lower === "mod") {
			mods.add(isMac ? "Meta" : "Ctrl");
		} else if (lower === "ctrl" || lower === "control") {
			mods.add("Ctrl");
		} else if (lower === "alt" || lower === "option") {
			mods.add("Alt");
		} else if (lower === "shift") {
			mods.add("Shift");
		} else if (lower === "meta" || lower === "cmd" || lower === "command") {
			mods.add("Meta");
		} else {
			key = p;
		}
	}

	const sorted = MOD_ORDER.filter((m) => mods.has(m));
	return [...sorted, key].join("+");
}

// ---- Default keybinding table ----

export const DEFAULT_KEYBINDINGS: KeyBinding[] = [
	// Global: Navigation
	{ id: "global.focusLibrarySearch", scope: "global", category: "keybindings.catNavigation", labelKey: "keybindings.focusLibrarySearch", defaultKey: "Mod+Shift+F" },
	{ id: "global.openSettings", scope: "global", category: "keybindings.catNavigation", labelKey: "keybindings.openSettings", defaultKey: "Mod+," },
	{ id: "global.toggleSidebar", scope: "global", category: "keybindings.catNavigation", labelKey: "keybindings.toggleSidebar", defaultKey: "Mod+\\" },
	{ id: "global.toggleAgentPanel", scope: "global", category: "keybindings.catNavigation", labelKey: "keybindings.toggleAgentPanel", defaultKey: "Mod+Shift+A" },
	{ id: "global.newNote", scope: "global", category: "keybindings.catNavigation", labelKey: "keybindings.newNote", defaultKey: "Mod+N" },
	{ id: "global.viewLibrary", scope: "global", category: "keybindings.catNavigation", labelKey: "keybindings.viewLibrary", defaultKey: "Mod+1" },
	{ id: "global.viewFeed", scope: "global", category: "keybindings.catNavigation", labelKey: "keybindings.viewFeed", defaultKey: "Mod+2" },
	{ id: "global.viewPapersCool", scope: "global", category: "keybindings.catNavigation", labelKey: "keybindings.viewPapersCool", defaultKey: "Mod+3" },
	{ id: "global.actualSize", scope: "global", category: "keybindings.catNavigation", labelKey: "keybindings.actualSize", defaultKey: "Mod+0" },

	// Global: Tabs
	{ id: "global.closeTab", scope: "global", category: "keybindings.catTabs", labelKey: "keybindings.closeTab", defaultKey: "Mod+W" },
	{ id: "global.nextTab", scope: "global", category: "keybindings.catTabs", labelKey: "keybindings.nextTab", defaultKey: "Mod+Shift+]" },
	{ id: "global.prevTab", scope: "global", category: "keybindings.catTabs", labelKey: "keybindings.prevTab", defaultKey: "Mod+Shift+[" },
	{ id: "global.tab1", scope: "global", category: "keybindings.catTabs", labelKey: "keybindings.tab1", defaultKey: null },
	{ id: "global.tab2", scope: "global", category: "keybindings.catTabs", labelKey: "keybindings.tab2", defaultKey: null },
	{ id: "global.tab3", scope: "global", category: "keybindings.catTabs", labelKey: "keybindings.tab3", defaultKey: null },
	{ id: "global.tab4", scope: "global", category: "keybindings.catTabs", labelKey: "keybindings.tab4", defaultKey: null },
	{ id: "global.tab5", scope: "global", category: "keybindings.catTabs", labelKey: "keybindings.tab5", defaultKey: null },
	{ id: "global.tab6", scope: "global", category: "keybindings.catTabs", labelKey: "keybindings.tab6", defaultKey: null },
	{ id: "global.tab7", scope: "global", category: "keybindings.catTabs", labelKey: "keybindings.tab7", defaultKey: null },
	{ id: "global.tab8", scope: "global", category: "keybindings.catTabs", labelKey: "keybindings.tab8", defaultKey: null },
	{ id: "global.tab9", scope: "global", category: "keybindings.catTabs", labelKey: "keybindings.tab9", defaultKey: null },

	// Library
	{ id: "library.selectAll", scope: "library", category: "keybindings.catLibrary", labelKey: "keybindings.selectAll", defaultKey: "Mod+A" },

	// Reader
	{ id: "reader.openSearch", scope: "reader", category: "keybindings.catReader", labelKey: "keybindings.openSearch", defaultKey: "Mod+F" },
	{ id: "reader.terminalFontIncrease", scope: "reader", category: "keybindings.catReaderTerminal", labelKey: "keybindings.terminalFontIncrease", defaultKey: "Mod+=" },
	{ id: "reader.terminalFontDecrease", scope: "reader", category: "keybindings.catReaderTerminal", labelKey: "keybindings.terminalFontDecrease", defaultKey: "Mod+-" },
	{ id: "reader.terminalFontReset", scope: "reader", category: "keybindings.catReaderTerminal", labelKey: "keybindings.terminalFontReset", defaultKey: "Mod+0" },

	// Editor: Formatting
	{ id: "editor.heading1", scope: "editor", category: "keybindings.catEditorFormatting", labelKey: "keybindings.heading1", defaultKey: "Mod+1" },
	{ id: "editor.heading2", scope: "editor", category: "keybindings.catEditorFormatting", labelKey: "keybindings.heading2", defaultKey: "Mod+2" },
	{ id: "editor.heading3", scope: "editor", category: "keybindings.catEditorFormatting", labelKey: "keybindings.heading3", defaultKey: "Mod+3" },
	{ id: "editor.heading4", scope: "editor", category: "keybindings.catEditorFormatting", labelKey: "keybindings.heading4", defaultKey: "Mod+4" },
	{ id: "editor.heading5", scope: "editor", category: "keybindings.catEditorFormatting", labelKey: "keybindings.heading5", defaultKey: "Mod+5" },
	{ id: "editor.heading6", scope: "editor", category: "keybindings.catEditorFormatting", labelKey: "keybindings.heading6", defaultKey: "Mod+6" },
	{ id: "editor.paragraph", scope: "editor", category: "keybindings.catEditorFormatting", labelKey: "keybindings.paragraph", defaultKey: "Mod+0" },
	{ id: "editor.codeBlock", scope: "editor", category: "keybindings.catEditorFormatting", labelKey: "keybindings.codeBlock", defaultKey: "Mod+Shift+M" },
	{ id: "editor.blockquote", scope: "editor", category: "keybindings.catEditorFormatting", labelKey: "keybindings.blockquote", defaultKey: "Mod+/" },
	{ id: "editor.horizontalRule", scope: "editor", category: "keybindings.catEditorInsert", labelKey: "keybindings.horizontalRule", defaultKey: "Mod+Shift+-" },

	// Editor: Insert
	{ id: "editor.insertLink", scope: "editor", category: "keybindings.catEditorInsert", labelKey: "keybindings.insertLink", defaultKey: "Mod+K" },
	{ id: "editor.insertImage", scope: "editor", category: "keybindings.catEditorInsert", labelKey: "keybindings.insertImage", defaultKey: "Mod+Shift+I" },
];

/**
 * Merge default bindings with user overrides, returning resolved bindings.
 */
export function mergeWithOverrides(
	defaults: KeyBinding[],
	overrides: KeyBindingOverrides,
): ResolvedKeyBinding[] {
	return defaults.map((def) => {
		const hasOverride = def.id in overrides;
		const key = hasOverride ? (overrides[def.id] ?? null) : def.defaultKey;
		return {
			...def,
			key: key ? normalizeKey(key) : null,
			isOverridden: hasOverride,
		};
	});
}

/**
 * Find same-scope conflicts for a proposed binding change.
 * Returns the conflicting binding id, or null if no conflict.
 */
export function findConflicts(
	bindings: ResolvedKeyBinding[],
	id: string,
	newKey: string,
): ResolvedKeyBinding | null {
	const binding = bindings.find((b) => b.id === id);
	if (!binding) return null;
	const normalized = normalizeKey(newKey);
	return (
		bindings.find(
			(b) => b.id !== id && b.scope === binding.scope && b.key === normalized,
		) ?? null
	);
}

/**
 * Find cross-scope collisions (informational, not blocking).
 */
export function findCrossScopeCollisions(
	bindings: ResolvedKeyBinding[],
	id: string,
	newKey: string,
): ResolvedKeyBinding[] {
	const binding = bindings.find((b) => b.id === id);
	if (!binding) return [];
	const normalized = normalizeKey(newKey);
	return bindings.filter(
		(b) => b.id !== id && b.scope !== binding.scope && b.key === normalized,
	);
}

/**
 * Convert a live KeyboardEvent into an accelerator string (e.g. "Meta+Shift+F").
 * Used in the recording UI.
 */
export function keyboardEventToAccelerator(e: KeyboardEvent): string | null {
	// Ignore standalone modifier presses
	if (
		["Control", "Alt", "Shift", "Meta", "OS"].includes(e.key)
	)
		return null;

	const mods: string[] = [];
	if (e.ctrlKey) mods.push("Ctrl");
	if (e.altKey) mods.push("Alt");
	if (e.shiftKey) mods.push("Shift");
	if (e.metaKey) mods.push("Meta");

	// Need at least one modifier for a valid shortcut
	if (mods.length === 0) return null;

	let key = e.key;
	// Normalize common key names
	if (key === " ") key = "Space";
	if (key.length === 1) key = key.toUpperCase();

	return normalizeKey([...mods, key].join("+"));
}

/**
 * Test if an accelerator string matches a KeyboardEvent.
 */
export function matchesEvent(accelerator: string, e: KeyboardEvent): boolean {
	const normalized = normalizeKey(accelerator);
	const parts = normalized.split("+");
	const key = parts[parts.length - 1];
	const mods = new Set(parts.slice(0, -1));

	// Check modifiers
	if (mods.has("Ctrl") !== e.ctrlKey) return false;
	if (mods.has("Alt") !== e.altKey) return false;
	if (mods.has("Shift") !== e.shiftKey) return false;
	if (mods.has("Meta") !== e.metaKey) return false;

	// Check key
	let eventKey = e.key;
	if (eventKey === " ") eventKey = "Space";

	// Case-insensitive comparison for letter keys
	return key.toLowerCase() === eventKey.toLowerCase();
}

/**
 * Format an accelerator for display: "Meta+Shift+F" → "⌘⇧F" on macOS.
 */
export function formatForDisplay(accelerator: string | null): string {
	if (!accelerator) return "";
	const normalized = normalizeKey(accelerator);
	const parts = normalized.split("+");
	const key = parts[parts.length - 1];
	const mods = parts.slice(0, -1);

	if (isMac) {
		const symbolMap: Record<string, string> = {
			Ctrl: "⌃",
			Alt: "⌥",
			Shift: "⇧",
			Meta: "⌘",
		};
		const modSymbols = mods.map((m) => symbolMap[m] || m).join("");
		return modSymbols + formatKeyForDisplay(key);
	}

	const modLabels = mods.map((m) => (m === "Meta" ? "Ctrl" : m));
	return [...modLabels, formatKeyForDisplay(key)].join("+");
}

function formatKeyForDisplay(key: string): string {
	const map: Record<string, string> = {
		Space: "Space",
		Backspace: "⌫",
		Delete: "Del",
		Enter: "↵",
		ArrowUp: "↑",
		ArrowDown: "↓",
		ArrowLeft: "←",
		ArrowRight: "→",
		Escape: "Esc",
		Tab: "Tab",
	};
	return map[key] || key;
}

/**
 * Convert an accelerator string to ProseMirror keymap format.
 * "Meta+Shift+M" → "Mod-Shift-m" (ProseMirror uses "Mod" for platform key)
 */
export function toProseMirrorKey(accelerator: string): string {
	const normalized = normalizeKey(accelerator);
	const parts = normalized.split("+");
	const key = parts[parts.length - 1];
	const mods = parts.slice(0, -1);

	const pmMods = mods.map((m) => {
		if (m === "Meta" && isMac) return "Mod";
		if (m === "Ctrl" && !isMac) return "Mod";
		return m;
	});

	return [...pmMods, key.toLowerCase()].join("-");
}
