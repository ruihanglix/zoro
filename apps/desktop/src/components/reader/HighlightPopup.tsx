// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { Button } from "@/components/ui/button";
import { useAnnotationStore } from "@/stores/annotationStore";
import type { ZoroHighlight } from "@/stores/annotationStore";
import { Check, Copy, MessageSquare, Quote, Trash2 } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ColorPicker } from "./ColorPicker";

interface HighlightPopupProps {
	highlight: ZoroHighlight;
	onClose: () => void;
	onCite?: () => void;
	/** Override update (for isolated/bilingual mode) */
	onUpdateAnnotation?: (
		id: string,
		color?: string | null,
		comment?: string | null,
	) => Promise<void>;
	/** Override delete (for isolated/bilingual mode) */
	onDeleteAnnotation?: (id: string) => Promise<void>;
}

export function HighlightPopup({
	highlight,
	onClose,
	onCite,
	onUpdateAnnotation,
	onDeleteAnnotation,
}: HighlightPopupProps) {
	const storeUpdate = useAnnotationStore((s) => s.updateAnnotation);
	const storeDelete = useAnnotationStore((s) => s.deleteAnnotation);
	const updateAnnotation = onUpdateAnnotation ?? storeUpdate;
	const deleteAnnotation = onDeleteAnnotation
		? (id: string, _paperId: string) => onDeleteAnnotation(id)
		: storeDelete;
	const [editingComment, setEditingComment] = useState(false);
	const [commentText, setCommentText] = useState(highlight.comment.text);
	const [copied, setCopied] = useState(false);
	const { t } = useTranslation();

	const handleColorChange = async (color: string) => {
		await updateAnnotation(highlight.id, color);
	};

	const handleSaveComment = async () => {
		await updateAnnotation(highlight.id, undefined, commentText || null);
		setEditingComment(false);
	};

	const handleDelete = async () => {
		await deleteAnnotation(highlight.id, highlight.paperId);
		onClose();
	};

	return (
		<div className="rounded-lg border bg-popover p-2 shadow-lg max-w-sm">
			{/* Row 1: action buttons */}
			<div className="flex items-center gap-1">
				{highlight.selectedText && (
					<>
						<button
							type="button"
							className={`rounded p-1 text-xs transition-colors ${
								copied ? "text-green-500" : "hover:bg-muted"
							}`}
							onClick={async () => {
								try {
									await navigator.clipboard.writeText(highlight.selectedText!);
									setCopied(true);
									setTimeout(() => setCopied(false), 1500);
								} catch {
									// ignore
								}
							}}
							title={t("reader.copyText")}
						>
							{copied ? (
								<Check className="h-3.5 w-3.5" />
							) : (
								<Copy className="h-3.5 w-3.5" />
							)}
						</button>
						<div className="h-4 w-px bg-border" />
					</>
				)}
				<button
					type="button"
					className={`rounded p-1 text-xs transition-colors ${
						editingComment ? "bg-muted" : "hover:bg-muted"
					}`}
					onClick={() => setEditingComment(!editingComment)}
					title={t("reader.editComment")}
				>
					<MessageSquare className="h-3.5 w-3.5" />
				</button>
				{onCite && (
					<>
						<div className="h-4 w-px bg-border" />
						<button
							type="button"
							className="rounded p-1 text-xs hover:bg-muted transition-colors"
							onClick={onCite}
							title={t("reader.copyAsCitation")}
						>
							<Quote className="h-3.5 w-3.5" />
						</button>
					</>
				)}
				<div className="h-4 w-px bg-border" />
				<button
					type="button"
					className="rounded p-1 text-xs hover:bg-destructive/10 hover:text-destructive transition-colors"
					onClick={handleDelete}
					title={t("common.delete")}
				>
					<Trash2 className="h-3.5 w-3.5" />
				</button>
			</div>

			{/* Row 2: color picker */}
			<div className="mt-1.5">
				<ColorPicker
					selectedColor={highlight.color}
					onColorSelect={handleColorChange}
				/>
			</div>

			{/* Comment section */}
			{(editingComment || highlight.comment.text) && (
				<div className="mt-2">
					{editingComment ? (
						<div>
							<textarea
								value={commentText}
								onChange={(e) => setCommentText(e.target.value)}
								placeholder={t("reader.addCommentPlaceholder")}
								className="w-full resize-none rounded border bg-transparent px-2 py-1 text-xs outline-none placeholder:text-muted-foreground focus:border-primary"
								rows={2}
								autoFocus
							/>
							<div className="mt-1 flex justify-end gap-1">
								<Button
									size="sm"
									variant="ghost"
									className="h-5 text-[10px] px-2"
									onClick={() => {
										setEditingComment(false);
										setCommentText(highlight.comment.text);
									}}
								>
									{t("common.cancel")}
								</Button>
								<Button
									size="sm"
									className="h-5 text-[10px] px-2"
									onClick={handleSaveComment}
								>
									{t("common.save")}
								</Button>
							</div>
						</div>
					) : (
						<p className="text-xs text-foreground/80 whitespace-pre-wrap">
							{highlight.comment.text}
						</p>
					)}
				</div>
			)}
		</div>
	);
}
