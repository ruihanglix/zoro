// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { NoteEditor } from "@/components/reader/NoteEditor";
import { Badge } from "@/components/ui/badge";
import * as commands from "@/lib/commands";
import type { NoteResponse, PaperResponse } from "@/lib/commands";
import type { PaperLinkAttributes } from "@/lib/tiptapPaperLink";
import { useAnnotationStore } from "@/stores/annotationStore";
import { useLibraryStore } from "@/stores/libraryStore";
import { useTabStore } from "@/stores/tabStore";
import { Loader2, StickyNote, Tag, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

interface StandaloneNoteEditorProps {
	paperId: string;
	tabId: string;
}

export function StandaloneNoteEditor({
	paperId,
	tabId,
}: StandaloneNoteEditorProps) {
	const { t } = useTranslation();
	const [paper, setPaper] = useState<PaperResponse | null>(null);
	const [note, setNote] = useState<NoteResponse | null>(null);
	const [loading, setLoading] = useState(true);
	const [saving, setSaving] = useState(false);
	const [editingTitle, setEditingTitle] = useState(false);
	const titleInputRef = useRef<HTMLInputElement>(null);

	const updateTab = useTabStore((s) => s.updateTab);
	const openTab = useTabStore((s) => s.openTab);
	const updatePaper = useLibraryStore((s) => s.updatePaper);
	const addTagToPaper = useLibraryStore((s) => s.addTagToPaper);
	const removeTagFromPaper = useLibraryStore((s) => s.removeTagFromPaper);
	const fetchPapers = useLibraryStore((s) => s.fetchPapers);

	useEffect(() => {
		let cancelled = false;
		setLoading(true);

		(async () => {
			try {
				const [paperData, notes] = await Promise.all([
					commands.getPaper(paperId),
					commands.listNotes(paperId),
				]);
				if (cancelled) return;
				setPaper(paperData);

				if (notes.length > 0) {
					setNote(notes[0]);
				} else {
					const newNote = await commands.addNote(paperId, "");
					if (!cancelled) setNote(newNote);
				}
			} catch (e) {
				console.error("Failed to load note:", e);
			} finally {
				if (!cancelled) setLoading(false);
			}
		})();

		return () => {
			cancelled = true;
		};
	}, [paperId]);

	const handleSave = useCallback(
		async (content: string) => {
			if (!note) return;
			setSaving(true);
			try {
				const updated = await commands.updateNote(note.id, content);
				setNote(updated);

				const firstLine = content.trim().split("\n")[0];
				const title =
					firstLine.replace(/^#+\s*/, "").slice(0, 80) || "Untitled Note";
				if (paper && title !== paper.title) {
					await commands.updatePaper(paperId, { title });
					const updatedPaper = await commands.getPaper(paperId);
					setPaper(updatedPaper);
					updateTab(tabId, { title });
					fetchPapers();
				}
			} catch (e) {
				console.error("Failed to save note:", e);
			} finally {
				setSaving(false);
			}
		},
		[note, paper, paperId, tabId, updateTab, fetchPapers],
	);

	const handleTitleSubmit = useCallback(
		async (value: string) => {
			setEditingTitle(false);
			const trimmed = value.trim() || "Untitled Note";
			if (paper && trimmed !== paper.title) {
				await updatePaper(paperId, { title: trimmed });
				const updatedPaper = await commands.getPaper(paperId);
				setPaper(updatedPaper);
				updateTab(tabId, { title: trimmed });
			}
		},
		[paper, paperId, tabId, updateTab, updatePaper],
	);

	const handlePaperLinkClick = useCallback(
		(detail: PaperLinkAttributes) => {
			const mode =
				detail.format === "html" ? "html" : ("pdf" as "pdf" | "html");
			openTab({
				type: "reader",
				paperId: detail.paperId,
				title: detail.paperTitle || "Paper",
				readerMode: mode,
			});

			if (detail.position || detail.page != null) {
				setTimeout(() => {
					if (mode === "pdf" && detail.page != null) {
						useAnnotationStore.getState().navigateToPage(detail.page);
					} else if (mode === "html" && detail.position) {
						const posJson = atob(detail.position);
						useAnnotationStore.getState().setPendingHtmlCitationJump(posJson);
					}
				}, 500);
			}
		},
		[openTab],
	);

	const [addingTag, setAddingTag] = useState(false);
	const [tagValue, setTagValue] = useState("");
	const [tagSuggestions, setTagSuggestions] = useState<
		{ id: string; name: string }[]
	>([]);
	const tagInputRef = useRef<HTMLInputElement>(null);

	useEffect(() => {
		if (addingTag) tagInputRef.current?.focus();
	}, [addingTag]);

	useEffect(() => {
		if (!tagValue.trim()) {
			setTagSuggestions([]);
			return;
		}
		let cancelled = false;
		commands.searchTags(tagValue.trim(), 6).then((tags) => {
			if (!cancelled) setTagSuggestions(tags);
		});
		return () => {
			cancelled = true;
		};
	}, [tagValue]);

	const handleAddTag = async (name: string) => {
		const trimmed = name.trim();
		if (!trimmed) return;
		await addTagToPaper(paperId, trimmed);
		const updatedPaper = await commands.getPaper(paperId);
		setPaper(updatedPaper);
		setTagValue("");
		setAddingTag(false);
		fetchPapers();
	};

	const handleRemoveTag = async (tagName: string) => {
		await removeTagFromPaper(paperId, tagName);
		const updatedPaper = await commands.getPaper(paperId);
		setPaper(updatedPaper);
		fetchPapers();
	};

	if (loading) {
		return (
			<div className="flex h-full items-center justify-center text-muted-foreground">
				<Loader2 className="h-6 w-6 animate-spin" />
			</div>
		);
	}

	if (!paper || !note) {
		return (
			<div className="flex h-full items-center justify-center text-muted-foreground">
				<div className="text-center">
					<StickyNote className="mx-auto mb-3 h-12 w-12 opacity-50" />
					<p className="text-sm">{t("noteEditor.noteNotFound")}</p>
				</div>
			</div>
		);
	}

	return (
		<div className="flex h-full w-full flex-col bg-background">
			{/* Header */}
			<header className="flex items-center gap-3 border-b px-4 py-2 shrink-0">
				<StickyNote className="h-4 w-4 text-primary shrink-0" />

				{editingTitle ? (
					<input
						ref={titleInputRef}
						type="text"
						defaultValue={paper.title}
						className="flex-1 bg-transparent text-sm font-semibold outline-none border-b border-primary py-0.5"
						autoFocus
						onKeyDown={(e) => {
							if (e.key === "Enter") handleTitleSubmit(e.currentTarget.value);
							if (e.key === "Escape") setEditingTitle(false);
						}}
						onBlur={(e) => handleTitleSubmit(e.currentTarget.value)}
					/>
				) : (
					<button
						type="button"
						className="flex-1 text-left text-sm font-semibold truncate hover:text-primary transition-colors"
						onClick={() => setEditingTitle(true)}
						title={t("noteEditor.clickToRename")}
					>
						{paper.title}
					</button>
				)}

				{saving && (
					<span className="flex items-center gap-1 text-[10px] text-muted-foreground shrink-0">
						<Loader2 className="h-3 w-3 animate-spin" />
						{t("common.saving")}
					</span>
				)}

				<div className="flex items-center gap-1.5 shrink-0">
					{paper.tags.map((tag) => (
						<Badge
							key={tag.id}
							variant="secondary"
							className="text-[10px] gap-0.5 group/tag"
						>
							{tag.name}
							<button
								type="button"
								className="opacity-0 group-hover/tag:opacity-100 transition-opacity"
								onClick={() => handleRemoveTag(tag.name)}
							>
								<X className="h-2.5 w-2.5" />
							</button>
						</Badge>
					))}
					{addingTag ? (
						<div className="relative">
							<input
								ref={tagInputRef}
								type="text"
								className="h-5 w-24 rounded border bg-background px-1.5 text-[10px] outline-none focus:ring-1 focus:ring-primary"
								placeholder={t("noteEditor.tagPlaceholder")}
								value={tagValue}
								onChange={(e) => setTagValue(e.target.value)}
								onKeyDown={(e) => {
									if (e.key === "Enter") handleAddTag(tagValue);
									if (e.key === "Escape") {
										setAddingTag(false);
										setTagValue("");
									}
								}}
								onBlur={() => {
									setTimeout(() => {
										setAddingTag(false);
										setTagValue("");
									}, 150);
								}}
							/>
							{tagSuggestions.length > 0 && (
								<div className="absolute top-full left-0 z-50 mt-1 w-32 rounded-md border bg-popover shadow-md py-0.5">
									{tagSuggestions.map((t) => (
										<button
											key={t.id}
											type="button"
											className="w-full px-2 py-1 text-left text-[10px] hover:bg-muted transition-colors"
											onMouseDown={(e) => {
												e.preventDefault();
												handleAddTag(t.name);
											}}
										>
											{t.name}
										</button>
									))}
								</div>
							)}
						</div>
					) : (
						<button
							type="button"
							className="rounded border border-dashed p-0.5 text-muted-foreground hover:text-foreground hover:border-foreground transition-colors"
							onClick={() => setAddingTag(true)}
							title={t("noteEditor.addTag")}
						>
							<Tag className="h-3 w-3" />
						</button>
					)}
				</div>
			</header>

			{/* Editor area — fills remaining space */}
			<div className="flex-1 overflow-hidden">
				<NoteEditor
					noteId={note.id}
					initialContent={note.content}
					onSave={handleSave}
					onPaperLinkClick={handlePaperLinkClick}
				/>
			</div>
		</div>
	);
}
