// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { applyTheme, applyUiScale } from "@/stores/uiStore";
import type { Theme } from "@/stores/uiStore";
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "@/lib/i18n";
import "@/styles/globals.css";

// Apply persisted theme before first paint to avoid flash
const savedTheme = (() => {
	try {
		const raw = localStorage.getItem("zoro-theme");
		if (raw) return JSON.parse(raw) as Theme;
	} catch {
		// ignore
	}
	return "system" as Theme;
})();
applyTheme(savedTheme);

// Apply persisted UI scale before first paint
const savedUiScale = (() => {
	try {
		const raw = localStorage.getItem("zoro-ui-scale");
		if (raw) return JSON.parse(raw) as number;
	} catch {
		// ignore
	}
	return 1;
})();
if (savedUiScale !== 1) {
	applyUiScale(savedUiScale);
}

// Re-apply when system preference changes (only relevant for "system" theme)
window
	.matchMedia("(prefers-color-scheme: dark)")
	.addEventListener("change", () => {
		const current = localStorage.getItem("zoro-theme");
		if (!current || JSON.parse(current) === "system") {
			applyTheme("system");
		}
	});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
	<React.StrictMode>
		<App />
	</React.StrictMode>,
);
