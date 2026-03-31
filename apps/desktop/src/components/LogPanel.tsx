// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
	DropdownMenu,
	DropdownMenuCheckboxItem,
	DropdownMenuContent,
	DropdownMenuSeparator,
	DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { LogEntry } from "@/lib/commands";
import * as commands from "@/lib/commands";
import { logger } from "@/lib/logger";
import { cn } from "@/lib/utils";
import { listen } from "@tauri-apps/api/event";
import {
	Check,
	ChevronDown,
	ChevronUp,
	ClipboardCopy,
	Filter,
	Trash2,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
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

/** Normalize a raw tracing target into a short, human-friendly module name.
 *  e.g. "zoro_sync::engine" → "sync", "frontend::feed" → "feed" */
function normalizeModule(target: string): string {
	// Frontend logs pushed via push_frontend_log use "frontend::<source>"
	if (target.startsWith("frontend::")) {
		return target.slice("frontend::".length);
	}
	// Strip common Rust crate prefixes
	let mod = target
		.replace(/^zoro[-_]?/, "")
		.replace(/::.*$/, ""); // keep only the top-level module
	if (mod === "desktop" || mod === "app") mod = "app";
	return mod || target;
}

export function LogPanel() {
	const { t } = useTranslation();
	const [logs, setLogs] = useState<LogEntry[]>([]);
	const [collapsed, setCollapsed] = useState(false);
	const [activeFilters, setActiveFilters] = useState<Set<LevelFilter>>(
		new Set(LEVELS),
	);
	const [moduleFilter, setModuleFilter] = useState<Set<string>>(new Set()); // empty = show all
	const scrollRef = useRef<HTMLDivElement>(null);
	const autoScrollRef = useRef(true);

	// Load existing logs on mount
	useEffect(() => {
		commands
			.getLogs()
			.then((entries) => {
				setLogs(entries.slice(-MAX_DISPLAY_ENTRIES));
			})
			.catch((e) => logger.error("log-panel", "Failed to load logs", e));
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
			logger.error("log-panel", "Failed to clear logs", err);
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
			logger.error("log-panel", "Failed to copy logs", err);
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

	// Extract unique module names from all logs
	const availableModules = useMemo(() => {
		const modules = new Set<string>();
		for (const log of logs) {
			modules.add(normalizeModule(log.target));
		}
		return Array.from(modules).sort();
	}, [logs]);

	const toggleModuleFilter = (mod: string) => {
		setModuleFilter((prev) => {
			const next = new Set(prev);
			if (next.has(mod)) {
				next.delete(mod);
			} else {
				next.add(mod);
			}
			return next;
		});
	};

	const selectAllModules = () => {
		setModuleFilter(new Set());
	};

	const filteredLogs = logs.filter((log) => {
		if (!activeFilters.has(log.level as LevelFilter)) return false;
		if (moduleFilter.size > 0 && !moduleFilter.has(normalizeModule(log.target)))
			return false;
		return true;
	});

	const moduleFilterActive = moduleFilter.size > 0;

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

					{/* Module filter */}
					<DropdownMenu>
						<DropdownMenuTrigger asChild>
							<button
								type="button"
								className={cn(
									"ml-1 px-1.5 py-0.5 rounded text-[10px] font-medium border transition-colors inline-flex items-center gap-1",
									moduleFilterActive
										? "bg-purple-500/15 text-purple-500 border-purple-500/30"
										: "text-muted-foreground/60 border-muted-foreground/20 hover:border-muted-foreground/40",
								)}
							>
								<Filter className="h-2.5 w-2.5" />
								{moduleFilterActive
									? t("log.moduleCount", { count: moduleFilter.size })
									: t("log.allModules")}
							</button>
						</DropdownMenuTrigger>
						<DropdownMenuContent
							className="w-48"
							align="start"
							side="top"
							sideOffset={4}
						>
							<DropdownMenuCheckboxItem
								checked={!moduleFilterActive}
								onCheckedChange={selectAllModules}
								className="text-xs"
							>
								{t("log.allModules")}
							</DropdownMenuCheckboxItem>
							<DropdownMenuSeparator />
							{availableModules.map((mod) => (
								<DropdownMenuCheckboxItem
									key={mod}
									checked={moduleFilter.has(mod)}
									onCheckedChange={() => toggleModuleFilter(mod)}
									className="text-xs"
								>
									<span className="font-mono">{mod}</span>
								</DropdownMenuCheckboxItem>
							))}
						</DropdownMenuContent>
					</DropdownMenu>

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
									<span className="text-purple-400/70 shrink-0 max-w-[200px] truncate select-none">
										{normalizeModule(entry.target)}
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
