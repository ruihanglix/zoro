// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { ScrollArea } from "@/components/ui/scroll-area";
import { useNoteStore } from "@/stores/noteStore";
import { ArrowLeft, Loader2, Plus, StickyNote, Trash2 } from "lucide-react";
import { useCallback, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { NoteEditor } from "./NoteEditor";

interface NotesPanelProps {
	paperId: string;
	onCitationJump?: (detail: {
		format: "pdf" | "html";
		page: number;
		position: string;
	}) => void;
}

export function NotesPanel({ paperId, onCitationJump }: NotesPanelProps) {
	const { t } = useTranslation();
	const notes = useNoteStore((s) => s.notes);
	const activeNoteId = useNoteStore((s) => s.activeNoteId);
	const loading = useNoteStore((s) => s.loading);
	const saving = useNoteStore((s) => s.saving);
	const fetchNotes = useNoteStore((s) => s.fetchNotes);
	const createNote = useNoteStore((s) => s.createNote);
	const updateNote = useNoteStore((s) => s.updateNote);
	const deleteNote = useNoteStore((s) => s.deleteNote);
	const setActiveNote = useNoteStore((s) => s.setActiveNote);

	useEffect(() => {
		fetchNotes(paperId);
		return () => setActiveNote(null);
	}, [paperId, fetchNotes, setActiveNote]);

	const handleCreate = useCallback(async () => {
		await createNote(paperId);
	}, [paperId, createNote]);

	const handleDelete = useCallback(
		async (e: React.MouseEvent, noteId: string) => {
			e.stopPropagation();
			if (!confirm(t("reader.deleteNoteConfirm"))) return;
			await deleteNote(noteId, paperId);
		},
		[paperId, deleteNote],
	);

	const handleSave = useCallback(
		(content: string) => {
			if (activeNoteId) updateNote(activeNoteId, content);
		},
		[activeNoteId, updateNote],
	);

	const activeNote = activeNoteId
		? notes.find((n) => n.id === activeNoteId)
		: null;

	if (activeNote) {
		return (
			<div className="flex h-full flex-col">
				{/* Editor header */}
				<div className="flex items-center gap-2 border-b px-2 py-1.5">
					<button
						type="button"
						className="rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
						onClick={() => setActiveNote(null)}
						title={t("reader.backToNotes")}
					>
						<ArrowLeft className="h-3.5 w-3.5" />
					</button>
					<span className="flex-1 truncate text-xs text-muted-foreground">
						{extractTitle(activeNote.content)}
					</span>
					{saving && (
						<span className="text-[10px] text-muted-foreground flex items-center gap-1">
							<Loader2 className="h-3 w-3 animate-spin" />
							Saving
						</span>
					)}
				</div>

				<NoteEditor
					noteId={activeNote.id}
					initialContent={activeNote.content}
					onSave={handleSave}
					onCitationJump={onCitationJump}
				/>
			</div>
		);
	}

	return (
		<div className="flex h-full flex-col">
			{/* List header */}
			<div className="flex items-center justify-between border-b px-3 py-1.5">
				<span className="text-xs font-medium text-muted-foreground">
					{notes.length} {notes.length === 1 ? "note" : "notes"}
				</span>
				<button
					type="button"
					className="rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
					onClick={handleCreate}
					title={t("reader.newNote")}
				>
					<Plus className="h-3.5 w-3.5" />
				</button>
			</div>

			<ScrollArea className="flex-1">
				{loading ? (
					<div className="flex items-center justify-center py-8">
						<Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
					</div>
				) : notes.length === 0 ? (
					<div className="text-center py-8 px-4">
						<StickyNote className="mx-auto mb-2 h-8 w-8 text-muted-foreground/50" />
						<p className="text-[11px] text-muted-foreground">
							{t("reader.noNotesYet")}
						</p>
						<button
							type="button"
							className="mt-2 rounded-md border border-dashed px-3 py-1.5 text-xs text-muted-foreground hover:text-foreground hover:border-foreground transition-colors"
							onClick={handleCreate}
						>
							{t("reader.createNote")}
						</button>
					</div>
				) : (
					<div className="p-2 space-y-1">
						{notes.map((note) => (
							<button
								key={note.id}
								type="button"
								className="group w-full rounded-md border p-2.5 text-left hover:bg-muted/50 transition-colors"
								onClick={() => setActiveNote(note.id)}
							>
								<div className="flex items-start justify-between gap-2">
									<p className="text-xs font-medium line-clamp-2 flex-1">
										{extractTitle(note.content) || "Untitled note"}
									</p>
									<button
										type="button"
										className="opacity-0 group-hover:opacity-100 rounded p-0.5 text-muted-foreground hover:text-destructive transition-all shrink-0"
										onClick={(e) => handleDelete(e, note.id)}
										title={t("reader.deleteNote")}
									>
										<Trash2 className="h-3 w-3" />
									</button>
								</div>
								<p className="text-[10px] text-muted-foreground mt-1">
									{formatDate(note.modified_date)}
								</p>
							</button>
						))}
					</div>
				)}
			</ScrollArea>
		</div>
	);
}

function extractTitle(content: string): string {
	if (!content.trim()) return "Untitled note";
	const firstLine = content.trim().split("\n")[0];
	return firstLine.replace(/^#+\s*/, "").slice(0, 80) || "Untitled note";
}

function formatDate(dateStr: string): string {
	try {
		const d = new Date(dateStr);
		const now = new Date();
		const diffMs = now.getTime() - d.getTime();
		const diffMin = Math.floor(diffMs / 60000);
		if (diffMin < 1) return "Just now";
		if (diffMin < 60) return `${diffMin}m ago`;
		const diffHr = Math.floor(diffMin / 60);
		if (diffHr < 24) return `${diffHr}h ago`;
		return d.toLocaleDateString();
	} catch {
		return dateStr;
	}
}
