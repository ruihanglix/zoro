// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useBrowserStore } from "@/stores/browserStore";
import {
	Globe,
	Pencil,
	Plus,
	RotateCcw,
	Trash2,
	X,
} from "lucide-react";
import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";

interface NewTabPageProps {
	onNavigate: (url: string) => void;
}

export function NewTabPage({ onNavigate }: NewTabPageProps) {
	const { t } = useTranslation();
	const bookmarks = useBrowserStore((s) => s.bookmarks);
	const addBookmark = useBrowserStore((s) => s.addBookmark);
	const removeBookmark = useBrowserStore((s) => s.removeBookmark);
	const updateBookmark = useBrowserStore((s) => s.updateBookmark);
	const resetToDefaults = useBrowserStore((s) => s.resetToDefaults);

	const [urlInput, setUrlInput] = useState("");
	const [showAddDialog, setShowAddDialog] = useState(false);
	const [editingBookmark, setEditingBookmark] = useState<string | null>(null);
	const [dialogName, setDialogName] = useState("");
	const [dialogUrl, setDialogUrl] = useState("");

	const handleNavigate = useCallback(
		(input: string) => {
			let url = input.trim();
			if (!url) return;
			if (!/^https?:\/\//i.test(url)) {
				url = `https://${url}`;
			}
			onNavigate(url);
		},
		[onNavigate],
	);

	const handleKeyDown = (e: React.KeyboardEvent) => {
		if (e.key === "Enter") {
			handleNavigate(urlInput);
		}
	};

	const openAddDialog = () => {
		setDialogName("");
		setDialogUrl("");
		setEditingBookmark(null);
		setShowAddDialog(true);
	};

	const openEditDialog = (id: string) => {
		const bm = bookmarks.find((b) => b.id === id);
		if (!bm) return;
		setDialogName(bm.name);
		setDialogUrl(bm.url);
		setEditingBookmark(id);
		setShowAddDialog(true);
	};

	const handleSaveBookmark = () => {
		if (!dialogName.trim() || !dialogUrl.trim()) return;
		let url = dialogUrl.trim();
		if (!/^https?:\/\//i.test(url)) {
			url = `https://${url}`;
		}
		if (editingBookmark) {
			updateBookmark(editingBookmark, { name: dialogName.trim(), url });
		} else {
			addBookmark(dialogName.trim(), url);
		}
		setShowAddDialog(false);
	};

	const handleDialogKeyDown = (e: React.KeyboardEvent) => {
		if (e.key === "Enter") {
			handleSaveBookmark();
		} else if (e.key === "Escape") {
			setShowAddDialog(false);
		}
	};

	return (
		<div className="flex h-full flex-col items-center pt-16 px-6 overflow-auto">
			{/* URL input */}
			<div className="w-full max-w-md mb-8">
				<Input
					value={urlInput}
					onChange={(e) => setUrlInput(e.target.value)}
					onKeyDown={handleKeyDown}
					placeholder={t("browser.addressPlaceholder")}
					className="text-sm"
					autoFocus
				/>
			</div>

			{/* Bookmark grid */}
			<div className="w-full max-w-lg">
				<div className="grid grid-cols-4 gap-3">
					{bookmarks.map((bm) => (
						<div key={bm.id} className="group relative">
							<button
								type="button"
								className="flex w-full flex-col items-center gap-1.5 rounded-lg p-3 transition-colors hover:bg-accent/50"
								onClick={() => onNavigate(bm.url)}
								title={bm.url}
							>
								<div className="flex h-10 w-10 items-center justify-center rounded-lg bg-muted text-lg">
									{bm.icon || <Globe className="h-5 w-5 text-muted-foreground" />}
								</div>
								<span className="text-xs text-foreground truncate w-full text-center">
									{bm.name}
								</span>
							</button>
							{/* Edit/delete overlay */}
							<div className="absolute top-1 right-1 hidden gap-0.5 group-hover:flex">
								<button
									type="button"
									className="rounded p-0.5 hover:bg-accent"
									onClick={(e) => {
										e.stopPropagation();
										openEditDialog(bm.id);
									}}
									title={t("browser.editBookmark")}
								>
									<Pencil className="h-3 w-3 text-muted-foreground" />
								</button>
								<button
									type="button"
									className="rounded p-0.5 hover:bg-destructive/20"
									onClick={(e) => {
										e.stopPropagation();
										removeBookmark(bm.id);
									}}
									title={t("browser.deleteBookmark")}
								>
									<Trash2 className="h-3 w-3 text-muted-foreground" />
								</button>
							</div>
						</div>
					))}

					{/* Add bookmark button */}
					<button
						type="button"
						className="flex flex-col items-center gap-1.5 rounded-lg p-3 transition-colors hover:bg-accent/50"
						onClick={openAddDialog}
					>
						<div className="flex h-10 w-10 items-center justify-center rounded-lg border-2 border-dashed border-muted-foreground/30">
							<Plus className="h-5 w-5 text-muted-foreground" />
						</div>
						<span className="text-xs text-muted-foreground">
							{t("browser.addBookmark")}
						</span>
					</button>
				</div>

				{/* Reset bookmarks */}
				<div className="mt-6 flex justify-center">
					<Button
						variant="ghost"
						size="sm"
						className="text-xs text-muted-foreground"
						onClick={resetToDefaults}
					>
						<RotateCcw className="mr-1.5 h-3 w-3" />
						{t("browser.resetBookmarks")}
					</Button>
				</div>
			</div>

			{/* Add/Edit bookmark dialog overlay */}
			{showAddDialog && (
				<div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
					<div className="w-80 rounded-lg border bg-background p-4 shadow-lg">
						<div className="flex items-center justify-between mb-3">
							<h3 className="text-sm font-medium">
								{editingBookmark
									? t("browser.editBookmark")
									: t("browser.addBookmark")}
							</h3>
							<button
								type="button"
								className="rounded p-1 hover:bg-accent"
								onClick={() => setShowAddDialog(false)}
							>
								<X className="h-3.5 w-3.5" />
							</button>
						</div>
						<div className="space-y-2">
							<div>
								<label className="text-xs text-muted-foreground">
									{t("browser.bookmarkName")}
								</label>
								<Input
									value={dialogName}
									onChange={(e) => setDialogName(e.target.value)}
									onKeyDown={handleDialogKeyDown}
									placeholder={t("browser.bookmarkName")}
									className="mt-1 text-sm"
									autoFocus
								/>
							</div>
							<div>
								<label className="text-xs text-muted-foreground">
									{t("browser.bookmarkUrl")}
								</label>
								<Input
									value={dialogUrl}
									onChange={(e) => setDialogUrl(e.target.value)}
									onKeyDown={handleDialogKeyDown}
									placeholder="https://..."
									className="mt-1 text-sm"
								/>
							</div>
						</div>
						<div className="mt-4 flex justify-end gap-2">
							<Button
								variant="ghost"
								size="sm"
								onClick={() => setShowAddDialog(false)}
							>
								{t("common.cancel")}
							</Button>
							<Button
								size="sm"
								onClick={handleSaveBookmark}
								disabled={!dialogName.trim() || !dialogUrl.trim()}
							>
								{t("common.save")}
							</Button>
						</div>
					</div>
				</div>
			)}
		</div>
	);
}
