// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import * as commands from "@/lib/commands";
import { useTabStore } from "@/stores/tabStore";
import { useUiStore } from "@/stores/uiStore";
import { listen } from "@tauri-apps/api/event";
import { writeText, readText } from "@tauri-apps/plugin-clipboard-manager";
import { open } from "@tauri-apps/plugin-shell";
import { useEffect } from "react";

// ---------------------------------------------------------------------------
// Focus & selection tracking (for clipboard support)
// ---------------------------------------------------------------------------
// macOS native menu activation steals WebView focus, so by the time our JS
// handler runs `document.activeElement` is already <body>. We track the last
// focused editable element and its selection via `focusin`/`selectionchange`
// so that clipboard operations can still read/write the correct text.

let _trackedEl: HTMLInputElement | HTMLTextAreaElement | null = null;
let _selStart = 0;
let _selEnd = 0;
let _lastSelectionText = "";

function _onFocusIn(e: FocusEvent) {
	const t = e.target;
	if (t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement) {
		_trackedEl = t;
		_selStart = t.selectionStart ?? 0;
		_selEnd = t.selectionEnd ?? 0;
	}
}

function _onSelectionChange() {
	// Snapshot input/textarea selection
	if (_trackedEl) {
		_selStart = _trackedEl.selectionStart ?? _selStart;
		_selEnd = _trackedEl.selectionEnd ?? _selEnd;
	}
	// Snapshot window selection (for general page text)
	const sel = window.getSelection()?.toString();
	if (sel) _lastSelectionText = sel;
}

async function _doCopy() {
	// Dispatch "menu-copy" for PDF viewer & HTML reader special handlers
	window.dispatchEvent(new CustomEvent("menu-copy"));

	// Handle standard input/textarea selection
	if (_trackedEl) {
		const text = _trackedEl.value.slice(_selStart, _selEnd);
		if (text) {
			try {
				await writeText(text);
			} catch {
				try {
					await navigator.clipboard.writeText(text);
				} catch {
					/* ignore */
				}
			}
			return;
		}
	}

	// Fallback: general page text (non-input selection)
	if (_lastSelectionText) {
		try {
			await writeText(_lastSelectionText);
		} catch {
			try {
				await navigator.clipboard.writeText(_lastSelectionText);
			} catch {
				/* ignore */
			}
		}
	}
}

async function _doCut() {
	// First copy the selected text
	if (_trackedEl) {
		const text = _trackedEl.value.slice(_selStart, _selEnd);
		if (text) {
			try {
				await writeText(text);
			} catch {
				try {
					await navigator.clipboard.writeText(text);
				} catch {
					/* ignore */
				}
			}
			// Delete the selected text
			try {
				// Try to refocus the element and use execCommand
				_trackedEl.focus();
				_trackedEl.setSelectionRange(_selStart, _selEnd);
				document.execCommand("delete");
			} catch {
				/* ignore */
			}
			return;
		}
	}
	// Fallback: try execCommand for contenteditable, etc.
	try {
		document.execCommand("cut");
	} catch {
		/* ignore */
	}
}

async function _doPaste() {
	// Read clipboard text
	let text: string | null = null;
	try {
		text = await readText();
	} catch {
		try {
			text = await navigator.clipboard.readText();
		} catch {
			/* ignore */
		}
	}

	if (!text) return;

	// Paste into tracked input/textarea
	if (_trackedEl) {
		try {
			_trackedEl.focus();
			_trackedEl.setSelectionRange(_selStart, _selEnd);
			// insertText respects undo history and fires input events
			document.execCommand("insertText", false, text);
		} catch {
			/* ignore */
		}
		return;
	}

	// Fallback: try execCommand for contenteditable, etc.
	try {
		document.execCommand("insertText", false, text);
	} catch {
		/* ignore */
	}
}

function _doSelectAll() {
	// Select all in tracked input/textarea
	if (_trackedEl) {
		try {
			_trackedEl.focus();
			_trackedEl.setSelectionRange(0, _trackedEl.value.length);
		} catch {
			/* ignore */
		}
		return;
	}
	// Fallback: select all on the page
	try {
		document.execCommand("selectAll");
	} catch {
		/* ignore */
	}
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

/**
 * Listen for native macOS menu-event emissions from the Rust backend
 * and dispatch them to the appropriate Zustand store actions.
 */
export function useMenuEvents() {
	useEffect(() => {
		document.addEventListener("focusin", _onFocusIn);
		document.addEventListener("selectionchange", _onSelectionChange);

		const unlisten = listen<string>("menu-event", (event) => {
			const id = event.payload;
			switch (id) {
				// Clipboard operations
				case "edit-copy":
					_doCopy();
					break;
				case "edit-cut":
					_doCut();
					break;
				case "edit-paste":
					_doPaste();
					break;
				case "edit-selectAll":
					_doSelectAll();
					break;

				// File
				case "add-paper":
					useUiStore.getState().setAddPaperDialogOpen(true);
					break;
				case "open-library":
					useUiStore.getState().setView("library");
					useTabStore.getState().setActiveTab("home");
					break;
				case "import":
					useUiStore.getState().setImportDialogOpen(true);
					break;
				case "settings":
					useTabStore
						.getState()
						.openTab({ type: "settings", title: "Settings" });
					break;

				// View — zoom
				case "zoom-in": {
					const { uiScale, setUiScale } = useUiStore.getState();
					setUiScale(Math.min(2, +(uiScale + 0.1).toFixed(2)));
					break;
				}
				case "zoom-out": {
					const { uiScale, setUiScale } = useUiStore.getState();
					setUiScale(Math.max(0.5, +(uiScale - 0.1).toFixed(2)));
					break;
				}
				case "actual-size":
					useUiStore.getState().setUiScale(1);
					break;

				// View — sidebar & navigation
				case "toggle-sidebar":
					useUiStore.getState().toggleSidebar();
					break;
				case "view-library":
					useUiStore.getState().setView("library");
					useTabStore.getState().setActiveTab("home");
					break;
				case "view-feed":
					useUiStore.getState().setView("feed");
					useTabStore.getState().setActiveTab("home");
					break;
				case "view-papers-cool":
					useUiStore.getState().setView("papers-cool");
					useTabStore.getState().setActiveTab("home");
					break;

				// View — theme
				case "theme-light":
					useUiStore.getState().setTheme("light");
					break;
				case "theme-dark":
					useUiStore.getState().setTheme("dark");
					break;
				case "theme-system":
					useUiStore.getState().setTheme("system");
					break;

				// Help
				case "about":
					useTabStore
						.getState()
						.openTab({ type: "settings", title: "Settings" });
					break;
				case "check-updates":
					commands.checkForUpdate().catch(() => {});
					break;
				case "github":
					open("https://github.com/zo-ro-dev/zoro").catch(() => {});
					break;
				case "website":
					open("https://zo-ro.org").catch(() => {});
					break;
				case "report-issue":
					open("https://github.com/zo-ro-dev/zoro/issues").catch(
						() => {},
					);
					break;
			}
		});

		return () => {
			document.removeEventListener("focusin", _onFocusIn);
			document.removeEventListener("selectionchange", _onSelectionChange);
			unlisten.then((fn) => fn());
		};
	}, []);
}
