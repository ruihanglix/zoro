// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import * as commands from "@/lib/commands";
import { useTabStore } from "@/stores/tabStore";
import { useUiStore } from "@/stores/uiStore";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-shell";
import { useEffect } from "react";

/**
 * Listen for native macOS menu-event emissions from the Rust backend
 * and dispatch them to the appropriate Zustand store actions.
 */
export function useMenuEvents() {
	useEffect(() => {
		const unlisten = listen<string>("menu-event", (event) => {
			const id = event.payload;
			switch (id) {
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
			unlisten.then((fn) => fn());
		};
	}, []);
}
