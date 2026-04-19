// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { type KeyScope, matchesEvent } from "@/lib/keybindings";
import { useKeybindingStore } from "@/stores/keybindingStore";
import { useEffect, type RefObject } from "react";

/**
 * Scope-aware keyboard shortcut dispatcher.
 *
 * Attaches a keydown listener for all bindings in the given scope,
 * calling the corresponding handler when a match is found.
 *
 * @param scope   - The keybinding scope to listen for
 * @param handlers - Map of binding id → handler function. Return true (or void) to preventDefault.
 * @param options  - enabled: whether the listener is active; target: element to attach to (default: window)
 */
export function useKeybindings(
	scope: KeyScope,
	handlers: Record<string, () => boolean | void>,
	options?: { enabled?: boolean; target?: RefObject<HTMLElement | null> },
): void {
	const bindings = useKeybindingStore((s) => s.bindings);
	const version = useKeybindingStore((s) => s.version);
	const enabled = options?.enabled ?? true;
	const target = options?.target;

	useEffect(() => {
		if (!enabled) return;

		// Build a map of accelerator → handler id for this scope
		const scopeBindings = bindings.filter(
			(b) => b.scope === scope && b.key !== null,
		);

		const handleKeyDown = (e: KeyboardEvent) => {
			// Respect scope priority: if an inner scope (e.g. editor via ProseMirror)
			// already handled this event, don't process it again.
			if (e.defaultPrevented) return;

			// Skip if focus is in an input/textarea/contenteditable
			// (unless we're in the editor scope which handles its own input)
			if (scope !== "editor") {
				const el = e.target as HTMLElement | null;
				if (
					el?.tagName === "INPUT" ||
					el?.tagName === "TEXTAREA" ||
					el?.isContentEditable
				) {
					return;
				}
			}

			for (const binding of scopeBindings) {
				if (binding.key && matchesEvent(binding.key, e)) {
					const handler = handlers[binding.id];
					if (handler) {
						const result = handler();
						if (result !== false) {
							e.preventDefault();
						}
						return;
					}
				}
			}
		};

		const el = target?.current ?? window;
		el.addEventListener("keydown", handleKeyDown as EventListener);
		return () =>
			el.removeEventListener("keydown", handleKeyDown as EventListener);
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [scope, enabled, bindings, version, handlers, target?.current]);
}
