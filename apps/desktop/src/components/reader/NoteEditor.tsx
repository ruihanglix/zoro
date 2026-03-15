// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import * as commands from "@/lib/commands";
import type { PaperResponse } from "@/lib/commands";
import { Citation } from "@/lib/tiptapCitation";
import { PaperLink } from "@/lib/tiptapPaperLink";
import type { PaperLinkAttributes } from "@/lib/tiptapPaperLink";
import { cn } from "@/lib/utils";
import { useNoteStore } from "@/stores/noteStore";
import Image from "@tiptap/extension-image";
import Link from "@tiptap/extension-link";
import Placeholder from "@tiptap/extension-placeholder";
import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import {
	Bold,
	BookOpen,
	ChevronDown,
	Code,
	Eye,
	FileText,
	Globe,
	ImageIcon,
	Italic,
	Link as LinkIcon,
	List,
	ListOrdered,
	Loader2,
	Pencil,
	Quote,
	Search,
	Video,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Markdown } from "tiptap-markdown";

interface NoteEditorProps {
	noteId: string;
	initialContent: string;
	onSave: (content: string) => void;
	onCitationJump?: (detail: {
		format: "pdf" | "html";
		page: number;
		position: string;
	}) => void;
	onPaperLinkClick?: (detail: PaperLinkAttributes) => void;
}

export function NoteEditor({
	noteId,
	initialContent,
	onSave,
	onCitationJump,
	onPaperLinkClick,
}: NoteEditorProps) {
	const { t } = useTranslation();
	const [rawMode, setRawMode] = useState(false);
	const [rawContent, setRawContent] = useState(initialContent);
	const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
	const citationClipboard = useNoteStore((s) => s.citationClipboard);
	const setCitationClipboard = useNoteStore((s) => s.setCitationClipboard);
	const editorContainerRef = useRef<HTMLDivElement>(null);

	const editor = useEditor(
		{
			extensions: [
				StarterKit,
				Image.configure({ inline: false, allowBase64: true }),
				Link.configure({ openOnClick: false }),
				Placeholder.configure({ placeholder: t("noteEditor.startWriting") }),
				Markdown.configure({ html: true, transformPastedText: true }),
				Citation,
				PaperLink,
			],
			content: initialContent,
			editorProps: {
				attributes: {
					class:
						"prose prose-sm dark:prose-invert max-w-none px-3 py-2 min-h-[200px] outline-none text-xs leading-relaxed",
				},
				handleDrop: (view, event, _slice, moved) => {
					if (moved || !event.dataTransfer?.files?.length) return false;
					const file = event.dataTransfer.files[0];
					if (!file.type.startsWith("image/")) return false;
					event.preventDefault();
					const reader = new FileReader();
					reader.onload = () => {
						const src = reader.result as string;
						const { tr } = view.state;
						const pos = view.posAtCoords({
							left: event.clientX,
							top: event.clientY,
						});
						const node = view.state.schema.nodes.image.create({ src });
						if (pos) {
							view.dispatch(tr.insert(pos.pos, node));
						}
					};
					reader.readAsDataURL(file);
					return true;
				},
				handlePaste: (view, event) => {
					const items = event.clipboardData?.items;
					if (!items) return false;
					for (let i = 0; i < items.length; i++) {
						const item = items[i];
						if (item.type.startsWith("image/")) {
							event.preventDefault();
							const file = item.getAsFile();
							if (!file) return true;
							const reader = new FileReader();
							reader.onload = () => {
								const src = reader.result as string;
								const node = view.state.schema.nodes.image.create({ src });
								const tr = view.state.tr.replaceSelectionWith(node);
								view.dispatch(tr);
							};
							reader.readAsDataURL(file);
							return true;
						}
					}
					return false;
				},
			},
			onUpdate: ({ editor: ed }) => {
				if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
				saveTimerRef.current = setTimeout(() => {
					const mdStorage = (
						ed.storage as unknown as Record<
							string,
							{ getMarkdown: () => string }
						>
					).markdown;
					onSave(mdStorage.getMarkdown());
				}, 1000);
			},
		},
		[noteId],
	);

	useEffect(() => {
		if (!editorContainerRef.current) return;
		const container = editorContainerRef.current;
		const citationHandler = (e: Event) => {
			const detail = (e as CustomEvent).detail;
			if (detail && onCitationJump) onCitationJump(detail);
		};
		const paperLinkHandler = (e: Event) => {
			const detail = (e as CustomEvent).detail;
			if (detail && onPaperLinkClick) onPaperLinkClick(detail);
		};
		container.addEventListener("citation-jump", citationHandler);
		container.addEventListener("paper-link-click", paperLinkHandler);
		return () => {
			container.removeEventListener("citation-jump", citationHandler);
			container.removeEventListener("paper-link-click", paperLinkHandler);
		};
	}, [onCitationJump, onPaperLinkClick]);

	useEffect(() => {
		return () => {
			if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
		};
	}, []);

	const handleToggleMode = useCallback(() => {
		if (rawMode) {
			editor?.commands.setContent(rawContent);
			setRawMode(false);
		} else {
			const md = editor
				? (
						editor.storage as unknown as Record<
							string,
							{ getMarkdown: () => string }
						>
					).markdown.getMarkdown()
				: "";
			setRawContent(md);
			setRawMode(true);
		}
	}, [rawMode, rawContent, editor]);

	const handleRawChange = useCallback(
		(value: string) => {
			setRawContent(value);
			if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
			saveTimerRef.current = setTimeout(() => {
				onSave(value);
			}, 1000);
		},
		[onSave],
	);

	const handleRawBlur = useCallback(() => {
		if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
		onSave(rawContent);
	}, [rawContent, onSave]);

	const handleInsertCitation = useCallback(() => {
		if (!editor || !citationClipboard) return;
		editor
			.chain()
			.focus()
			.insertCitation({
				format: citationClipboard.format,
				page: citationClipboard.pageNumber,
				position: btoa(citationClipboard.position),
				text: citationClipboard.selectedText,
			})
			.run();
		setCitationClipboard(null);
	}, [editor, citationClipboard, setCitationClipboard]);

	const handleInsertImage = useCallback(async () => {
		if (!editor) return;
		try {
			const { open } = await import("@tauri-apps/plugin-dialog");
			const { readFile } = await import("@tauri-apps/plugin-fs");
			const selected = await open({
				multiple: false,
				filters: [
					{ name: "Images", extensions: ["png", "jpg", "jpeg", "gif", "webp"] },
				],
			});
			if (!selected) return;
			const path = typeof selected === "string" ? selected : selected;
			const data = await readFile(path);
			const ext = path.split(".").pop()?.toLowerCase() || "png";
			const mime =
				ext === "jpg" || ext === "jpeg"
					? "image/jpeg"
					: ext === "gif"
						? "image/gif"
						: ext === "webp"
							? "image/webp"
							: "image/png";
			const blob = new Blob([data], { type: mime });
			const reader = new FileReader();
			reader.onload = () => {
				editor
					.chain()
					.focus()
					.setImage({ src: reader.result as string })
					.run();
			};
			reader.readAsDataURL(blob);
		} catch (e) {
			console.error("Failed to insert image:", e);
		}
	}, [editor]);

	const handleInsertLink = useCallback(() => {
		if (!editor) return;
		const url = prompt(t("noteEditor.enterUrl"));
		if (url) {
			editor.chain().focus().setLink({ href: url }).run();
		}
	}, [editor]);

	const handleInsertVideo = useCallback(() => {
		if (!editor) return;
		const url = prompt(t("noteEditor.enterVideoUrl"));
		if (!url) return;
		const youtubeMatch = url.match(
			/(?:youtube\.com\/watch\?v=|youtu\.be\/|youtube\.com\/embed\/)([^&?]+)/,
		);
		if (youtubeMatch) {
			editor
				.chain()
				.focus()
				.insertContent(
					`<div data-video="youtube"><iframe src="https://www.youtube.com/embed/${youtubeMatch[1]}" width="100%" height="315" frameborder="0" allowfullscreen></iframe></div>`,
				)
				.run();
		} else {
			editor
				.chain()
				.focus()
				.insertContent(
					`<div data-video="generic"><video src="${url}" controls width="100%" style="max-height:400px"></video></div>`,
				)
				.run();
		}
	}, [editor]);

	const [headingOpen, setHeadingOpen] = useState(false);
	const [paperSearchOpen, setPaperSearchOpen] = useState(false);
	const [paperSearchQuery, setPaperSearchQuery] = useState("");
	const [paperSearchResults, setPaperSearchResults] = useState<PaperResponse[]>(
		[],
	);
	const [paperSearchLoading, setPaperSearchLoading] = useState(false);
	const [paperSearchIdx, setPaperSearchIdx] = useState(-1);
	const paperSearchInputRef = useRef<HTMLInputElement>(null);
	const paperSearchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(
		null,
	);

	// @ mention detection: watch for "@" typed in the editor
	const [mentionOpen, setMentionOpen] = useState(false);
	const [_mentionQuery, setMentionQuery] = useState("");
	const [mentionResults, setMentionResults] = useState<PaperResponse[]>([]);
	const [mentionIdx, setMentionIdx] = useState(-1);
	const [mentionPos, setMentionPos] = useState<{
		top: number;
		left: number;
	} | null>(null);
	const mentionTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
	const mentionStartPosRef = useRef<number | null>(null);

	// Track @ mentions via editor transaction
	useEffect(() => {
		if (!editor) return;
		const handleUpdate = () => {
			const { state } = editor;
			const { from } = state.selection;
			const textBefore = state.doc.textBetween(
				Math.max(0, from - 50),
				from,
				"\n",
			);
			const match = textBefore.match(/@([^\s@]*)$/);
			if (match) {
				const query = match[1];
				mentionStartPosRef.current = from - query.length - 1;
				setMentionQuery(query);
				setMentionOpen(true);

				const coords = editor.view.coordsAtPos(from);
				const containerRect =
					editorContainerRef.current?.getBoundingClientRect();
				if (containerRect) {
					setMentionPos({
						top: coords.bottom - containerRect.top + 4,
						left: coords.left - containerRect.left,
					});
				}

				if (mentionTimerRef.current) clearTimeout(mentionTimerRef.current);
				mentionTimerRef.current = setTimeout(async () => {
					if (query.length > 0) {
						try {
							const results = await commands.searchPapers(query, 8);
							setMentionResults(results.filter((p) => p.entry_type !== "note"));
							setMentionIdx(0);
						} catch {
							setMentionResults([]);
						}
					} else {
						try {
							const results = await commands.listPapers({ limit: 8 });
							setMentionResults(results.filter((p) => p.entry_type !== "note"));
							setMentionIdx(0);
						} catch {
							setMentionResults([]);
						}
					}
				}, 200);
			} else {
				if (mentionOpen) {
					setMentionOpen(false);
					setMentionResults([]);
					mentionStartPosRef.current = null;
				}
			}
		};
		editor.on("update", handleUpdate);
		editor.on("selectionUpdate", handleUpdate);
		return () => {
			editor.off("update", handleUpdate);
			editor.off("selectionUpdate", handleUpdate);
		};
	}, [editor, mentionOpen]);

	const handleMentionSelect = useCallback(
		(paper: PaperResponse) => {
			if (!editor || mentionStartPosRef.current == null) return;
			const from = mentionStartPosRef.current;
			const to = editor.state.selection.from;
			editor
				.chain()
				.focus()
				.deleteRange({ from, to })
				.insertPaperLink({
					paperId: paper.id,
					paperTitle: paper.title,
				})
				.run();
			setMentionOpen(false);
			setMentionResults([]);
			mentionStartPosRef.current = null;
		},
		[editor],
	);

	// Handle keyboard nav in mention popup
	useEffect(() => {
		if (!mentionOpen || !editor) return;
		const handleKeyDown = (e: KeyboardEvent) => {
			if (e.key === "ArrowDown") {
				e.preventDefault();
				setMentionIdx((i) => Math.min(i + 1, mentionResults.length - 1));
			} else if (e.key === "ArrowUp") {
				e.preventDefault();
				setMentionIdx((i) => Math.max(i - 1, 0));
			} else if (
				e.key === "Enter" &&
				mentionResults.length > 0 &&
				mentionIdx >= 0
			) {
				e.preventDefault();
				handleMentionSelect(mentionResults[mentionIdx]);
			} else if (e.key === "Escape") {
				e.preventDefault();
				setMentionOpen(false);
			}
		};
		document.addEventListener("keydown", handleKeyDown, true);
		return () => document.removeEventListener("keydown", handleKeyDown, true);
	}, [mentionOpen, mentionResults, mentionIdx, handleMentionSelect, editor]);

	const handlePaperSearchOpen = useCallback(() => {
		setPaperSearchOpen(true);
		setPaperSearchQuery("");
		setPaperSearchResults([]);
		setPaperSearchIdx(-1);
		setTimeout(() => paperSearchInputRef.current?.focus(), 50);
	}, []);

	const handlePaperSearchChange = useCallback((value: string) => {
		setPaperSearchQuery(value);
		setPaperSearchIdx(-1);
		if (paperSearchTimerRef.current) clearTimeout(paperSearchTimerRef.current);
		paperSearchTimerRef.current = setTimeout(async () => {
			setPaperSearchLoading(true);
			try {
				const results = value.trim()
					? await commands.searchPapers(value.trim(), 10)
					: await commands.listPapers({ limit: 10 });
				setPaperSearchResults(results.filter((p) => p.entry_type !== "note"));
				setPaperSearchIdx(results.length > 0 ? 0 : -1);
			} catch {
				setPaperSearchResults([]);
			} finally {
				setPaperSearchLoading(false);
			}
		}, 200);
	}, []);

	const handlePaperSearchSelect = useCallback(
		(paper: PaperResponse) => {
			if (!editor) return;
			editor
				.chain()
				.focus()
				.insertPaperLink({
					paperId: paper.id,
					paperTitle: paper.title,
				})
				.run();
			setPaperSearchOpen(false);
		},
		[editor],
	);

	const handlePaperSearchKeyDown = useCallback(
		(e: React.KeyboardEvent) => {
			if (e.key === "ArrowDown") {
				e.preventDefault();
				setPaperSearchIdx((i) =>
					Math.min(i + 1, paperSearchResults.length - 1),
				);
			} else if (e.key === "ArrowUp") {
				e.preventDefault();
				setPaperSearchIdx((i) => Math.max(i - 1, 0));
			} else if (
				e.key === "Enter" &&
				paperSearchResults.length > 0 &&
				paperSearchIdx >= 0
			) {
				e.preventDefault();
				handlePaperSearchSelect(paperSearchResults[paperSearchIdx]);
			} else if (e.key === "Escape") {
				e.preventDefault();
				setPaperSearchOpen(false);
				editor?.chain().focus().run();
			}
		},
		[paperSearchResults, paperSearchIdx, handlePaperSearchSelect, editor],
	);

	if (!editor) return null;

	const headingLabel = (() => {
		for (let i = 1; i <= 6; i++) {
			if (editor.isActive("heading", { level: i })) return `H${i}`;
		}
		return "Text";
	})();

	const headingOptions: { label: string; action: () => void }[] = [
		{
			label: t("noteEditor.text"),
			action: () => {
				editor.chain().focus().setParagraph().run();
				setHeadingOpen(false);
			},
		},
		...([1, 2, 3, 4, 5, 6] as const).map((level) => ({
			label: `${t("noteEditor.heading")} ${level}`,
			action: () => {
				editor.chain().focus().toggleHeading({ level }).run();
				setHeadingOpen(false);
			},
		})),
	];

	const ToolButton = ({
		active,
		onClick,
		children,
		title,
		disabled,
	}: {
		active?: boolean;
		onClick: () => void;
		children: React.ReactNode;
		title: string;
		disabled?: boolean;
	}) => (
		<button
			type="button"
			className={cn(
				"rounded p-1 transition-colors",
				active
					? "bg-primary text-primary-foreground"
					: "text-muted-foreground hover:bg-muted hover:text-foreground",
				disabled && "opacity-30 cursor-not-allowed",
			)}
			onClick={onClick}
			title={title}
			disabled={disabled}
		>
			{children}
		</button>
	);

	return (
		<div
			className="flex flex-col h-full w-full relative"
			ref={editorContainerRef}
		>
			{/* Toolbar */}
			<div className="flex flex-wrap items-center gap-0.5 border-b px-2 py-1">
				{/* Heading / paragraph dropdown */}
				<div className="relative">
					<button
						type="button"
						className="flex items-center gap-0.5 rounded px-1.5 py-1 text-[11px] text-muted-foreground hover:bg-muted hover:text-foreground transition-colors min-w-[52px]"
						onClick={() => setHeadingOpen(!headingOpen)}
					>
						<span className="font-medium">{headingLabel}</span>
						<ChevronDown className="h-3 w-3" />
					</button>
					{headingOpen && (
						<>
							<div
								className="fixed inset-0 z-40"
								onClick={() => setHeadingOpen(false)}
								onKeyDown={(e) => {
									if (e.key === "Escape") setHeadingOpen(false);
								}}
							/>
							<div className="absolute top-full left-0 mt-0.5 z-50 rounded-md border bg-popover shadow-md py-0.5 min-w-[120px]">
								{headingOptions.map((opt) => (
									<button
										key={opt.label}
										type="button"
										className={cn(
											"w-full text-left px-2 py-1 text-xs hover:bg-muted transition-colors",
											headingLabel === opt.label.replace("Heading ", "H") ||
												(opt.label === "Text" && headingLabel === "Text")
												? "bg-muted font-medium"
												: "",
										)}
										onClick={opt.action}
									>
										{opt.label}
									</button>
								))}
							</div>
						</>
					)}
				</div>

				<div className="h-4 w-px bg-border mx-0.5" />

				<ToolButton
					active={editor.isActive("bold")}
					onClick={() => editor.chain().focus().toggleBold().run()}
					title={t("noteEditor.bold")}
				>
					<Bold className="h-3.5 w-3.5" />
				</ToolButton>
				<ToolButton
					active={editor.isActive("italic")}
					onClick={() => editor.chain().focus().toggleItalic().run()}
					title={t("noteEditor.italic")}
				>
					<Italic className="h-3.5 w-3.5" />
				</ToolButton>

				<div className="h-4 w-px bg-border mx-0.5" />

				<ToolButton
					active={editor.isActive("bulletList")}
					onClick={() => editor.chain().focus().toggleBulletList().run()}
					title={t("noteEditor.bulletList")}
				>
					<List className="h-3.5 w-3.5" />
				</ToolButton>
				<ToolButton
					active={editor.isActive("orderedList")}
					onClick={() => editor.chain().focus().toggleOrderedList().run()}
					title={t("noteEditor.orderedList")}
				>
					<ListOrdered className="h-3.5 w-3.5" />
				</ToolButton>
				<ToolButton
					active={editor.isActive("blockquote")}
					onClick={() => editor.chain().focus().toggleBlockquote().run()}
					title={t("noteEditor.quote")}
				>
					<Quote className="h-3.5 w-3.5" />
				</ToolButton>
				<ToolButton
					active={editor.isActive("codeBlock")}
					onClick={() => editor.chain().focus().toggleCodeBlock().run()}
					title={t("noteEditor.codeBlock")}
				>
					<Code className="h-3.5 w-3.5" />
				</ToolButton>

				<div className="h-4 w-px bg-border mx-0.5" />

				<ToolButton
					onClick={handleInsertImage}
					title={t("noteEditor.insertImage")}
				>
					<ImageIcon className="h-3.5 w-3.5" />
				</ToolButton>
				<ToolButton
					onClick={handleInsertVideo}
					title={t("noteEditor.embedVideo")}
				>
					<Video className="h-3.5 w-3.5" />
				</ToolButton>
				<ToolButton
					active={editor.isActive("link")}
					onClick={handleInsertLink}
					title={t("noteEditor.insertLink")}
				>
					<LinkIcon className="h-3.5 w-3.5" />
				</ToolButton>

				<div className="h-4 w-px bg-border mx-0.5" />

				<div className="relative">
					<ToolButton
						onClick={handlePaperSearchOpen}
						title={t("noteEditor.linkPaper")}
					>
						<BookOpen className="h-3.5 w-3.5" />
					</ToolButton>
					{paperSearchOpen && (
						<>
							<div
								className="fixed inset-0 z-40"
								onClick={() => {
									setPaperSearchOpen(false);
									editor?.chain().focus().run();
								}}
							/>
							<div className="absolute top-full left-0 mt-1 z-50 w-72 rounded-lg border bg-popover shadow-lg">
								<div className="flex items-center gap-2 border-b px-2.5 py-2">
									<Search className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
									<input
										ref={paperSearchInputRef}
										type="text"
										className="flex-1 bg-transparent text-xs outline-none placeholder:text-muted-foreground"
										placeholder={t("noteEditor.searchPapers")}
										value={paperSearchQuery}
										onChange={(e) => handlePaperSearchChange(e.target.value)}
										onKeyDown={handlePaperSearchKeyDown}
									/>
									{paperSearchLoading && (
										<Loader2 className="h-3 w-3 animate-spin text-muted-foreground shrink-0" />
									)}
								</div>
								<div className="max-h-56 overflow-y-auto py-1">
									{paperSearchResults.length === 0 && !paperSearchLoading && (
										<p className="px-3 py-4 text-center text-xs text-muted-foreground">
											{paperSearchQuery
												? t("noteEditor.noPapersFound")
												: t("noteEditor.typeToSearchPapers")}
										</p>
									)}
									{paperSearchResults.map((paper, i) => (
										<button
											key={paper.id}
											type="button"
											className={cn(
												"w-full text-left px-3 py-1.5 text-xs hover:bg-muted transition-colors flex items-start gap-2",
												i === paperSearchIdx && "bg-muted",
											)}
											onMouseDown={(e) => {
												e.preventDefault();
												handlePaperSearchSelect(paper);
											}}
										>
											<FileText className="h-3.5 w-3.5 shrink-0 mt-0.5 text-muted-foreground" />
											<div className="min-w-0">
												<p className="truncate font-medium">{paper.title}</p>
												{paper.authors.length > 0 && (
													<p className="truncate text-[10px] text-muted-foreground mt-0.5">
														{paper.authors.map((a) => a.name).join(", ")}
													</p>
												)}
											</div>
										</button>
									))}
								</div>
							</div>
						</>
					)}
				</div>

				{citationClipboard && (
					<>
						<div className="h-4 w-px bg-border mx-0.5" />
						<button
							type="button"
							className="flex items-center gap-1 rounded bg-primary/10 px-1.5 py-0.5 text-[10px] text-primary hover:bg-primary/20 transition-colors"
							onClick={handleInsertCitation}
							title={t("noteEditor.pasteCitation")}
						>
							{citationClipboard.format === "pdf" ? (
								<FileText className="h-3 w-3" />
							) : (
								<Globe className="h-3 w-3" />
							)}
							{t("noteEditor.pasteCitation")}
						</button>
					</>
				)}

				<div className="ml-auto">
					<ToolButton
						active={rawMode}
						onClick={handleToggleMode}
						title={
							rawMode
								? t("noteEditor.switchToWysiwyg")
								: t("noteEditor.switchToMarkdown")
						}
					>
						{rawMode ? (
							<Eye className="h-3.5 w-3.5" />
						) : (
							<Pencil className="h-3.5 w-3.5" />
						)}
					</ToolButton>
				</div>
			</div>

			{/* Editor / Raw textarea */}
			{rawMode ? (
				<textarea
					value={rawContent}
					onChange={(e) => handleRawChange(e.target.value)}
					onBlur={handleRawBlur}
					className="flex-1 w-full resize-none bg-transparent px-3 py-2 text-xs font-mono outline-none"
					spellCheck={false}
				/>
			) : (
				<div className="flex-1 overflow-auto">
					<EditorContent editor={editor} className="h-full" />
				</div>
			)}

			{mentionOpen && mentionPos && mentionResults.length > 0 && (
				<div
					className="absolute z-50 w-64 rounded-lg border bg-popover shadow-lg py-1"
					style={{ top: mentionPos.top, left: mentionPos.left }}
				>
					{mentionResults.map((paper, i) => (
						<button
							key={paper.id}
							type="button"
							className={cn(
								"w-full text-left px-3 py-1.5 text-xs hover:bg-muted transition-colors flex items-start gap-2",
								i === mentionIdx && "bg-muted",
							)}
							onMouseDown={(e) => {
								e.preventDefault();
								handleMentionSelect(paper);
							}}
						>
							<FileText className="h-3.5 w-3.5 shrink-0 mt-0.5 text-muted-foreground" />
							<div className="min-w-0">
								<p className="truncate font-medium">{paper.title}</p>
								{paper.authors.length > 0 && (
									<p className="truncate text-[10px] text-muted-foreground mt-0.5">
										{paper.authors.map((a) => a.name).join(", ")}
									</p>
								)}
							</div>
						</button>
					))}
				</div>
			)}
		</div>
	);
}
