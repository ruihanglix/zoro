// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { invoke } from "@tauri-apps/api/core";

type LogLevel = "ERROR" | "WARN" | "INFO" | "DEBUG";

/**
 * Unified frontend logger.
 *
 * Every call:
 *  1. Outputs to the browser DevTools console (for dev convenience).
 *  2. Sends the entry to the Rust BufferLayer via Tauri command,
 *     so it appears in the in-app LogPanel alongside backend logs.
 *
 * Usage:
 *   import { logger } from "@/lib/logger";
 *   logger.info("feed", "Refresh started", { subscriptionId });
 *   logger.error("annotation", "Failed to save", error);
 */

function send(level: LogLevel, source: string, message: string) {
	// Fire-and-forget — never block the caller
	invoke("push_frontend_log", { level, source, message }).catch(() => {
		// Silently ignore if the backend is not ready yet (e.g. during startup)
	});
}

function formatArgs(args: unknown[]): string {
	return args
		.map((a) => {
			if (a instanceof Error) return a.message;
			if (typeof a === "string") return a;
			try {
				return JSON.stringify(a);
			} catch {
				return String(a);
			}
		})
		.join(" ");
}

export const logger = {
	error(source: string, msg: string, ...args: unknown[]) {
		const full = args.length ? `${msg} ${formatArgs(args)}` : msg;
		console.error(`[${source}]`, msg, ...args);
		send("ERROR", source, full);
	},

	warn(source: string, msg: string, ...args: unknown[]) {
		const full = args.length ? `${msg} ${formatArgs(args)}` : msg;
		console.warn(`[${source}]`, msg, ...args);
		send("WARN", source, full);
	},

	info(source: string, msg: string, ...args: unknown[]) {
		const full = args.length ? `${msg} ${formatArgs(args)}` : msg;
		console.info(`[${source}]`, msg, ...args);
		send("INFO", source, full);
	},

	debug(source: string, msg: string, ...args: unknown[]) {
		const full = args.length ? `${msg} ${formatArgs(args)}` : msg;
		console.debug(`[${source}]`, msg, ...args);
		send("DEBUG", source, full);
	},
};
