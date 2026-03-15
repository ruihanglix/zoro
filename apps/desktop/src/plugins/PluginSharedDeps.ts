// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import * as LucideIcons from "lucide-react";
/**
 * Inject shared dependencies into window.__ZORO_SHARED__
 * so plugins can use React/ReactDOM without bundling their own copy.
 *
 * This must be called before any plugin is loaded.
 */
import React from "react";
import ReactDOM from "react-dom";
import * as jsxRuntime from "react/jsx-runtime";

declare global {
	interface Window {
		__ZORO_SHARED__?: {
			React: typeof React;
			ReactDOM: typeof ReactDOM;
			icons: typeof LucideIcons;
			jsxRuntime: typeof jsxRuntime;
		};
	}
}

export function injectPluginSharedDeps(): void {
	if (window.__ZORO_SHARED__) return;
	window.__ZORO_SHARED__ = {
		React,
		ReactDOM,
		icons: LucideIcons,
		jsxRuntime,
	};
}
