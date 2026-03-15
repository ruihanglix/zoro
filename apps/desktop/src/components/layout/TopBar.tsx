// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { useLibraryStore } from "@/stores/libraryStore";
import { useTabStore } from "@/stores/tabStore";
import { useUiStore } from "@/stores/uiStore";
import {
	PanelLeft,
	PanelLeftClose,
	Plus,
	Search,
	Settings,
	Upload,
	X,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

export function TopBar() {
	const { t } = useTranslation();
	const toggleSidebar = useUiStore((s) => s.toggleSidebar);
	const sidebarOpen = useUiStore((s) => s.sidebarOpen);
	const view = useUiStore((s) => s.view);
	const setAddPaperDialogOpen = useUiStore((s) => s.setAddPaperDialogOpen);
	const setImportDialogOpen = useUiStore((s) => s.setImportDialogOpen);
	const openTab = useTabStore((s) => s.openTab);
	const searchPapers = useLibraryStore((s) => s.searchPapers);
	const papers = useLibraryStore((s) => s.papers);
	const searchQuery = useLibraryStore((s) => s.searchQuery);
	const wholeWord = useLibraryStore((s) => s.searchWholeWord);
	const setSearchWholeWord = useLibraryStore((s) => s.setSearchWholeWord);
	const [query, setQuery] = useState("");
	const inputRef = useRef<HTMLInputElement>(null);

	const handleSearch = useCallback(
		(e?: React.FormEvent) => {
			e?.preventDefault();
			searchPapers(query);
		},
		[query, searchPapers],
	);

	const handleClear = useCallback(() => {
		setQuery("");
		searchPapers("");
		inputRef.current?.focus();
	}, [searchPapers]);

	// Cmd+Shift+F to focus the library search bar
	useEffect(() => {
		const handleKeyDown = (e: KeyboardEvent) => {
			if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === "f") {
				e.preventDefault();
				inputRef.current?.focus();
			}
		};
		window.addEventListener("keydown", handleKeyDown);
		return () => window.removeEventListener("keydown", handleKeyDown);
	}, []);

	return (
		<header className="flex h-14 items-center gap-4 border-b px-4 select-none">
			<Button variant="ghost" size="icon" onClick={toggleSidebar}>
				{sidebarOpen ? (
					<PanelLeftClose className="h-5 w-5" />
				) : (
					<PanelLeft className="h-5 w-5" />
				)}
			</Button>

			<form
				onSubmit={handleSearch}
				className="flex flex-1 items-center gap-2 max-w-lg"
			>
				<div className="relative flex-1 flex items-center">
					<Search className="absolute left-2.5 h-4 w-4 text-muted-foreground pointer-events-none" />
					<Input
						ref={inputRef}
						type="text"
						placeholder={t("topBar.searchPlaceholder")}
						className="pl-8 pr-20"
						value={query}
						onChange={(e) => setQuery(e.target.value)}
						onKeyDown={(e) => {
							if (e.key === "Escape" && query) {
								handleClear();
							}
						}}
					/>
					<div className="absolute right-1.5 flex items-center gap-0.5">
						<button
							type="button"
							className={cn(
								"rounded px-1 py-0.5 text-[11px] font-medium transition-colors border",
								wholeWord
									? "bg-primary text-primary-foreground border-primary"
									: "text-muted-foreground hover:bg-muted hover:text-foreground border-transparent",
							)}
							onClick={() => setSearchWholeWord(!wholeWord)}
							title={t("topBar.wholeWord")}
						>
							<span className="underline underline-offset-2">ab</span>
						</button>
						{query && (
							<button
								type="button"
								className="rounded p-0.5 text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
								onClick={handleClear}
								title={t("topBar.clearSearch")}
							>
								<X className="h-3.5 w-3.5" />
							</button>
						)}
					</div>
				</div>
				{searchQuery && (
					<span className="text-xs text-muted-foreground whitespace-nowrap tabular-nums">
						{papers.length}{" "}
						{papers.length !== 1 ? t("common.results") : t("common.result")}
					</span>
				)}
			</form>

			<div className="ml-auto flex items-center gap-4">
				{view === "library" && (
					<>
						<Button
							variant="outline"
							size="sm"
							onClick={() => setImportDialogOpen(true)}
						>
							<Upload className="mr-2 h-4 w-4" />
							{t("topBar.import")}
						</Button>
						<Button size="sm" onClick={() => setAddPaperDialogOpen(true)}>
							<Plus className="mr-2 h-4 w-4" />
							{t("topBar.addPaper")}
						</Button>
					</>
				)}
				<Button
					variant="ghost"
					size="icon"
					onClick={() =>
						openTab({
							id: "settings",
							type: "settings",
							title: t("common.settings"),
						})
					}
					title={t("common.settings")}
				>
					<Settings className="h-5 w-5" />
				</Button>
			</div>
		</header>
	);
}
