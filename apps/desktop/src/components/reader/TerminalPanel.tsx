// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import * as commands from "@/lib/commands";
import { listen } from "@tauri-apps/api/event";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { Loader2 } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

interface TerminalPanelProps {
	paperId: string;
	visible?: boolean;
}

export function TerminalPanel({ paperId, visible = true }: TerminalPanelProps) {
	const containerRef = useRef<HTMLDivElement>(null);
	const termRef = useRef<Terminal | null>(null);
	const fitAddonRef = useRef<FitAddon | null>(null);
	const terminalIdRef = useRef<string | null>(null);
	const [loading, setLoading] = useState(true);
	const [error, setError] = useState<string | null>(null);

	const doFit = useCallback(() => {
		const fitAddon = fitAddonRef.current;
		const terminalId = terminalIdRef.current;
		if (!fitAddon || !terminalId) return;
		try {
			fitAddon.fit();
			const dims = fitAddon.proposeDimensions();
			if (dims) {
				commands.resizeTerminal(terminalId, dims.cols, dims.rows);
			}
		} catch {
			// fit() can throw if container is not visible
		}
	}, []);

	// Re-fit when becoming visible again
	useEffect(() => {
		if (visible) {
			requestAnimationFrame(() => doFit());
		}
	}, [visible, doFit]);

	useEffect(() => {
		if (!containerRef.current) return;

		const term = new Terminal({
			fontSize: 13,
			fontFamily: "'SF Mono', 'Menlo', 'Monaco', 'Courier New', monospace",
			cursorBlink: true,
			theme: {
				background: "#1e1e1e",
				foreground: "#d4d4d4",
				cursor: "#d4d4d4",
				selectionBackground: "#264f78",
			},
			allowProposedApi: true,
		});
		const fitAddon = new FitAddon();
		term.loadAddon(fitAddon);
		term.open(containerRef.current);

		termRef.current = term;
		fitAddonRef.current = fitAddon;

		let destroyed = false;

		const init = async () => {
			try {
				const tid = await commands.spawnTerminal(paperId);
				if (destroyed) return;

				// Replay buffered output before accepting live events,
				// so nothing is duplicated or lost.
				const history = await commands.getTerminalHistory(tid);
				if (destroyed) return;
				if (history) {
					term.write(history);
				}

				terminalIdRef.current = tid;
				setLoading(false);

				requestAnimationFrame(() => doFit());
			} catch (e) {
				if (!destroyed) {
					setError(String(e));
					setLoading(false);
				}
			}
		};

		const dataDisposable = term.onData((data) => {
			if (terminalIdRef.current) {
				commands.writeTerminal(terminalIdRef.current, data);
			}
		});

		let unlistenFn: (() => void) | null = null;
		const setupListener = async () => {
			const unlisten = await listen<{ terminal_id: string; data: string }>(
				"terminal-output",
				(event) => {
					if (event.payload.terminal_id === terminalIdRef.current) {
						term.write(event.payload.data);
					}
				},
			);
			if (destroyed) {
				unlisten();
			} else {
				unlistenFn = unlisten;
			}
		};

		setupListener();
		init();

		const resizeObserver = new ResizeObserver(() => {
			requestAnimationFrame(() => doFit());
		});
		resizeObserver.observe(containerRef.current);

		return () => {
			destroyed = true;
			resizeObserver.disconnect();
			dataDisposable.dispose();
			if (unlistenFn) unlistenFn();
			term.dispose();
			termRef.current = null;
			fitAddonRef.current = null;
			terminalIdRef.current = null;
		};
	}, [paperId, doFit]);

	if (error) {
		return (
			<div className="flex h-full items-center justify-center p-4 text-xs text-destructive">
				{error}
			</div>
		);
	}

	return (
		<div className="relative h-full w-full">
			{loading && (
				<div className="absolute inset-0 z-10 flex items-center justify-center bg-[#1e1e1e]">
					<Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
				</div>
			)}
			<div ref={containerRef} className="h-full w-full" />
		</div>
	);
}
