// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { LogEntry } from "@/lib/commands";
import * as commands from "@/lib/commands";
import { cn } from "@/lib/utils";
import { listen } from "@tauri-apps/api/event";
import { Check, ChevronDown, ChevronUp, ClipboardCopy, Trash2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

const LEVEL_COLORS: Record<string, string> = {
	ERROR: "text-red-500",
	WARN: "text-yellow-500",
	INFO: "text-blue-400",
	DEBUG: "text-gray-400",
	TRACE: "text-gray-500",
};

type LevelFilter = "ERROR" | "WARN" | "INFO" | "DEBUG";

const LEVELS: LevelFilter[] = ["ERROR", "WARN", "INFO", "DEBUG"];

const MAX_DISPLAY_ENTRIES = 500;

export function LogPanel() {
	const { t } = useTranslation();
	const [logs, setLogs] = useState<LogEntry[]>([]);
	const [collapsed, setCollapsed] = useState(false);
	const [activeFilters, setActiveFilters] = useState<Set<LevelFilter>>(
		new Set(LEVELS),
	);
	const scrollRef = useRef<HTMLDivElement>(null);
	const autoScrollRef = useRef(true);

	// Load existing logs on mount
	useEffect(() => {
		commands
			.getLogs()
			.then((entries) => {
				setLogs(entries.slice(-MAX_DISPLAY_ENTRIES));
			})
			.catch(console.error);
	}, []);

	// Listen for new log entries via Tauri events
	useEffect(() => {
		const unlisten = listen<LogEntry>("log-entry", (event) => {
			setLogs((prev) => {
				const next = [...prev, event.payload];
				if (next.length > MAX_DISPLAY_ENTRIES) {
					return next.slice(-MAX_DISPLAY_ENTRIES);
				}
				return next;
			});
		});

		return () => {
			unlisten.then((fn) => fn());
		};
	}, []);

	// Auto-scroll to bottom when new logs arrive
	useEffect(() => {
		if (autoScrollRef.current && scrollRef.current) {
			scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
		}
	}, [logs]);

	const handleScroll = () => {
		if (!scrollRef.current) return;
		const el = scrollRef.current;
		const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
		autoScrollRef.current = atBottom;
	};

	const [copied, setCopied] = useState(false);

	const handleClear = async () => {
		try {
			await commands.clearLogs();
			setLogs([]);
		} catch (err) {
			console.error("Failed to clear logs:", err);
		}
	};

	const handleCopyAll = async () => {
		const text = filteredLogs
			.map(
				(entry) =>
					`${entry.timestamp} [${entry.level}] ${entry.target}: ${entry.message}`,
			)
			.join("\n");
		try {
			await navigator.clipboard.writeText(text);
			setCopied(true);
			setTimeout(() => setCopied(false), 2000);
		} catch (err) {
			console.error("Failed to copy logs:", err);
		}
	};

	const toggleFilter = (level: LevelFilter) => {
		setActiveFilters((prev) => {
			const next = new Set(prev);
			if (next.has(level)) {
				next.delete(level);
			} else {
				next.add(level);
			}
			return next;
		});
	};

	const filteredLogs = logs.filter((log) =>
		activeFilters.has(log.level as LevelFilter),
	);

	return (
		<div className="flex flex-col border-t bg-background">
			{/* Toolbar */}
			<div className="flex items-center justify-between border-b px-3 py-1.5">
				<div className="flex items-center gap-1.5">
					<span className="text-xs font-semibold text-muted-foreground mr-1">
						{t("common.logs")}
					</span>
					{LEVELS.map((level) => (
						<button
							key={level}
							type="button"
							onClick={() => toggleFilter(level)}
							className={cn(
								"px-1.5 py-0.5 rounded text-[10px] font-medium border transition-colors",
								activeFilters.has(level)
									? level === "ERROR"
										? "bg-red-500/15 text-red-500 border-red-500/30"
										: level === "WARN"
											? "bg-yellow-500/15 text-yellow-500 border-yellow-500/30"
											: level === "INFO"
												? "bg-blue-400/15 text-blue-400 border-blue-400/30"
												: "bg-gray-400/15 text-gray-400 border-gray-400/30"
									: "text-muted-foreground/40 border-transparent",
							)}
						>
							{level}
						</button>
					))}
					<Badge variant="secondary" className="text-[10px] ml-1">
						{filteredLogs.length}
					</Badge>
				</div>
				<div className="flex items-center gap-1">
					<Button
						variant="ghost"
						size="icon"
						className="h-6 w-6"
						onClick={handleCopyAll}
						title={t("log.copyAll")}
					>
						{copied ? (
							<Check className="h-3.5 w-3.5 text-green-500" />
						) : (
							<ClipboardCopy className="h-3.5 w-3.5" />
						)}
					</Button>
					<Button
						variant="ghost"
						size="icon"
						className="h-6 w-6"
						onClick={handleClear}
						title={t("log.clear")}
					>
						<Trash2 className="h-3.5 w-3.5" />
					</Button>
					<Button
						variant="ghost"
						size="icon"
						className="h-6 w-6"
						onClick={() => setCollapsed(!collapsed)}
						title={collapsed ? t("common.expand") : t("common.collapse")}
					>
						{collapsed ? (
							<ChevronUp className="h-3.5 w-3.5" />
						) : (
							<ChevronDown className="h-3.5 w-3.5" />
						)}
					</Button>
				</div>
			</div>

			{/* Log entries */}
			{!collapsed && (
				<ScrollArea className="h-[200px]">
					<div
						ref={scrollRef}
						className="h-full overflow-auto p-2 font-mono text-[11px] leading-[1.6]"
						onScroll={handleScroll}
					>
						{filteredLogs.length === 0 ? (
							<div className="flex items-center justify-center h-full text-muted-foreground text-xs">
								{t("common.noEntries")}
							</div>
						) : (
							filteredLogs.map((entry) => (
								<div key={entry.id} className="flex gap-2 hover:bg-accent/30">
									<span className="text-muted-foreground/60 shrink-0 select-none">
										{new Date(entry.timestamp).toLocaleTimeString()}
									</span>
									<span
										className={cn(
											"shrink-0 w-12 text-right select-none",
											LEVEL_COLORS[entry.level] ?? "text-foreground",
										)}
									>
										{entry.level}
									</span>
									<span className="text-muted-foreground/70 shrink-0 max-w-[200px] truncate select-none">
										{entry.target.replace(/^zoro_?/, "")}
									</span>
									<span className="text-foreground break-all">
										{entry.message}
									</span>
								</div>
							))
						)}
					</div>
				</ScrollArea>
			)}
		</div>
	);
}
