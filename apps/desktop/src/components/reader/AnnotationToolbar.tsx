// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { Button } from "@/components/ui/button";
import * as commands from "@/lib/commands";
import { useAnnotationStore } from "@/stores/annotationStore";
import type { AnnotationType } from "@/stores/annotationStore";
import {
	Check,
	Copy,
	Highlighter,
	Languages,
	Loader2,
	MessageSquare,
	Quote,
	Underline,
} from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ColorPicker } from "./ColorPicker";

interface AnnotationToolbarProps {
	onConfirm: (type: AnnotationType, color: string, comment: string) => void;
	onCancel: () => void;
	selectedText?: string;
	onCite?: () => void;
}

export function AnnotationToolbar({
	onConfirm,
	onCancel,
	selectedText,
	onCite,
}: AnnotationToolbarProps) {
	const activeColor = useAnnotationStore((s) => s.activeColor);
	const setActiveColor = useAnnotationStore((s) => s.setActiveColor);
	const [type, setType] = useState<AnnotationType>("highlight");
	const [showComment, setShowComment] = useState(false);
	const [comment, setComment] = useState("");
	const [translating, setTranslating] = useState(false);
	const [translatedText, setTranslatedText] = useState<string | null>(null);
	const [translateError, setTranslateError] = useState<string | null>(null);
	const [copied, setCopied] = useState(false);
	const { t } = useTranslation();

	const handleConfirm = () => {
		onConfirm(type, activeColor, comment);
	};

	const handleTranslate = async () => {
		if (!selectedText?.trim()) return;
		setTranslating(true);
		setTranslateError(null);
		setTranslatedText(null);
		try {
			const result = await commands.translateSelection(selectedText);
			setTranslatedText(result);
		} catch (e) {
			setTranslateError(String(e));
		} finally {
			setTranslating(false);
		}
	};

	const handleCopyTranslation = async () => {
		if (!translatedText) return;
		try {
			await navigator.clipboard.writeText(translatedText);
			setCopied(true);
			setTimeout(() => setCopied(false), 1500);
		} catch {
			// ignore
		}
	};

	return (
		<div className="rounded-lg border bg-popover p-2 shadow-lg max-w-sm">
			{/* Row 1: action buttons */}
			<div className="flex items-center gap-1">
				<div className="flex items-center gap-0.5 rounded-md border p-0.5">
					<button
						type="button"
						className={`rounded px-1.5 py-1 text-xs transition-colors ${
							type === "highlight"
								? "bg-primary text-primary-foreground"
								: "hover:bg-muted"
						}`}
						onClick={() => setType("highlight")}
						title={t("reader.highlight")}
					>
						<Highlighter className="h-3.5 w-3.5" />
					</button>
					<button
						type="button"
						className={`rounded px-1.5 py-1 text-xs transition-colors ${
							type === "underline"
								? "bg-primary text-primary-foreground"
								: "hover:bg-muted"
						}`}
						onClick={() => setType("underline")}
						title={t("reader.underline")}
					>
						<Underline className="h-3.5 w-3.5" />
					</button>
				</div>

				<div className="h-4 w-px bg-border" />

				<button
					type="button"
					className={`rounded p-1 text-xs transition-colors ${
						showComment ? "bg-muted" : "hover:bg-muted"
					}`}
					onClick={() => setShowComment(!showComment)}
					title={t("reader.addComment")}
				>
					<MessageSquare className="h-3.5 w-3.5" />
				</button>

				{selectedText && (
					<>
						<div className="h-4 w-px bg-border" />
						<button
							type="button"
							className={`rounded p-1 text-xs transition-colors ${
								copied ? "text-green-500" : "hover:bg-muted"
							}`}
							onClick={async () => {
								try {
									await navigator.clipboard.writeText(selectedText);
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
						<button
							type="button"
							className={`rounded p-1 text-xs transition-colors ${
								translatedText !== null || translating
									? "bg-muted"
									: "hover:bg-muted"
							}`}
							onClick={handleTranslate}
							disabled={translating}
							title={t("reader.translateSelection")}
						>
							{translating ? (
								<Loader2 className="h-3.5 w-3.5 animate-spin" />
							) : (
								<Languages className="h-3.5 w-3.5" />
							)}
						</button>
					</>
				)}

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
			</div>

			{/* Row 2: color picker */}
			<div className="mt-1.5">
				<ColorPicker
					selectedColor={activeColor}
					onColorSelect={setActiveColor}
				/>
			</div>

			{/* Translation result */}
			{translatedText !== null && (
				<div className="mt-2 rounded border bg-muted/50 px-2 py-1.5">
					<div className="flex items-start justify-between gap-1">
						<p className="text-xs text-foreground whitespace-pre-wrap flex-1 select-text">
							{translatedText}
						</p>
						<button
							type="button"
							className="shrink-0 rounded p-0.5 text-muted-foreground hover:text-foreground"
							onClick={handleCopyTranslation}
							title={t("reader.copyTranslation")}
						>
							{copied ? (
								<Check className="h-3 w-3 text-green-500" />
							) : (
								<Copy className="h-3 w-3" />
							)}
						</button>
					</div>
				</div>
			)}

			{translateError && (
				<p className="mt-2 text-[11px] text-destructive line-clamp-2">
					{translateError}
				</p>
			)}

			{/* Comment input (conditionally shown) */}
			{showComment && (
				<div className="mt-2">
					<textarea
						value={comment}
						onChange={(e) => setComment(e.target.value)}
						placeholder={t("reader.addCommentPlaceholder")}
						className="w-full resize-none rounded border bg-transparent px-2 py-1 text-xs outline-none placeholder:text-muted-foreground focus:border-primary"
						rows={2}
						autoFocus
					/>
				</div>
			)}

			{/* Action buttons */}
			<div className="mt-2 flex justify-end gap-1">
				<Button
					size="sm"
					variant="ghost"
					className="h-6 text-[11px] px-2"
					onClick={onCancel}
				>
					{t("common.cancel")}
				</Button>
				<Button
					size="sm"
					className="h-6 text-[11px] px-2"
					onClick={handleConfirm}
				>
					{t("common.save")}
				</Button>
			</div>
		</div>
	);
}
