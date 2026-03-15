// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { cn } from "@/lib/utils";
import { useTranslationStore } from "@/stores/translationStore";
import { Languages, Loader2 } from "lucide-react";

interface BilingualTextProps {
	original: string;
	translated?: string | null;
	loading?: boolean;
	onRequestTranslation?: () => void;
	variant: "title" | "abstract" | "inline";
	className?: string;
}

/**
 * Renders text in bilingual mode (immersive-translation style).
 *
 * - `title`: translated text prominent, original smaller below
 * - `abstract`: paragraph-by-paragraph interleaved (translated prominent, original muted)
 * - `inline`: compact — translated text only, original as tooltip
 */
export function BilingualText({
	original,
	translated,
	loading,
	onRequestTranslation,
	variant,
	className,
}: BilingualTextProps) {
	const displayMode = useTranslationStore((s) => s.displayMode);

	const hasTranslation = translated && translated.trim().length > 0;

	// Strip any legacy "===" delimiters from cached translations,
	// keeping only the translated portions.
	const cleanTranslated = hasTranslation ? stripDelimiters(translated) : null;

	// Original-only mode
	if (displayMode === "original" || (!hasTranslation && !loading)) {
		return (
			<div className={cn("group relative", className)}>
				{variant === "title" && (
					<span className="text-base font-semibold leading-tight">
						{original}
					</span>
				)}
				{variant === "abstract" && (
					<p className="text-sm text-foreground/80 leading-relaxed">
						{original}
					</p>
				)}
				{variant === "inline" && <span>{original}</span>}
				{!hasTranslation &&
					!loading &&
					onRequestTranslation &&
					displayMode !== "original" && (
						<button
							type="button"
							onClick={onRequestTranslation}
							className="ml-1.5 inline-flex items-center gap-0.5 text-[10px] text-muted-foreground hover:text-foreground opacity-0 group-hover:opacity-100 transition-opacity"
							title="Translate"
						>
							<Languages className="h-3 w-3" />
						</button>
					)}
				{loading && (
					<Loader2 className="ml-1.5 inline h-3 w-3 animate-spin text-muted-foreground" />
				)}
			</div>
		);
	}

	// Translated-only mode
	if (displayMode === "translated") {
		const text = cleanTranslated ?? original;
		return (
			<div className={className}>
				{variant === "title" && (
					<span className="text-base font-semibold leading-tight">{text}</span>
				)}
				{variant === "abstract" && (
					<div className="text-sm text-foreground/80 leading-relaxed space-y-2">
						{splitParagraphs(text).map((p, i) => (
							<p key={`to-${i}`}>{p}</p>
						))}
					</div>
				)}
				{variant === "inline" && <span>{text}</span>}
			</div>
		);
	}

	// Bilingual mode (default)
	if (variant === "title") {
		return (
			<div className={className}>
				{/* Translated title — prominent */}
				<span className="text-base font-semibold leading-tight block">
					{cleanTranslated ?? original}
				</span>
				{/* Original title — smaller, muted */}
				<span className="text-xs text-muted-foreground leading-snug mt-0.5 block">
					{original}
				</span>
			</div>
		);
	}

	if (variant === "abstract") {
		return (
			<BilingualAbstract
				original={original}
				translated={cleanTranslated ?? ""}
				className={className}
			/>
		);
	}

	// inline variant
	return (
		<span className={className} title={original}>
			{cleanTranslated ?? original}
		</span>
	);
}

/**
 * Paragraph-by-paragraph bilingual abstract display.
 *
 * Splits both original and translated text into paragraphs and aligns them.
 * Translated text is shown prominently, original text is shown muted below each paragraph.
 */
function BilingualAbstract({
	original,
	translated,
	className,
}: {
	original: string;
	translated: string;
	className?: string;
}) {
	const transParagraphs = splitParagraphs(translated);
	const origParagraphs = splitParagraphs(original);

	// Align paragraphs: pair them up by index.
	// If counts differ, distribute original paragraphs proportionally.
	const pairs = alignParagraphs(origParagraphs, transParagraphs);

	return (
		<div className={cn("space-y-3", className)}>
			{pairs.map((pair, i) => (
				<div key={`p-${i}`} className="space-y-0.5">
					<p className="text-sm text-foreground/80 leading-relaxed">
						{pair.translated}
					</p>
					<p className="text-xs text-muted-foreground/60 leading-relaxed">
						{pair.original}
					</p>
				</div>
			))}
		</div>
	);
}

/**
 * Align original and translated paragraph arrays into pairs.
 *
 * The translated text (from LLM) is split into N paragraphs.
 * The original text (from paper metadata) is usually 1 paragraph.
 * We split the original into sentences and distribute them across
 * the N translated paragraphs proportionally by sentence count.
 */
export function alignParagraphs(
	origParagraphs: string[],
	transParagraphs: string[],
): { original: string; translated: string }[] {
	if (transParagraphs.length === 0) return [];

	if (origParagraphs.length === 0) {
		return transParagraphs.map((t) => ({ original: "", translated: t }));
	}

	// --- Fallback: if both sides are single paragraphs and long enough,
	// split them into ~3-sentence chunks for readability. ---
	const CHUNK_SIZE = 3;
	if (transParagraphs.length === 1 && origParagraphs.length <= 1) {
		const transSentences = splitSentences(transParagraphs[0]);
		if (transSentences.length > CHUNK_SIZE) {
			const transChunks = chunkSentences(transSentences, CHUNK_SIZE);
			const origSentences = splitSentences(origParagraphs.join(" "));
			const origChunks = chunkSentences(origSentences, CHUNK_SIZE);
			return transChunks.map((tc, i) => ({
				translated: tc,
				original: origChunks[i] ?? "",
			}));
		}
		return [
			{
				original: origParagraphs.join(" "),
				translated: transParagraphs[0],
			},
		];
	}

	// Same count — 1:1 alignment
	if (origParagraphs.length === transParagraphs.length) {
		return transParagraphs.map((t, i) => ({
			original: origParagraphs[i],
			translated: t,
		}));
	}

	if (transParagraphs.length === 1) {
		return [
			{
				original: origParagraphs.join(" "),
				translated: transParagraphs[0],
			},
		];
	}

	// Split all original text into sentences for fine-grained distribution
	const allOrigText = origParagraphs.join(" ");
	const origSentences = splitSentences(allOrigText);

	// Count sentences in each translated paragraph to determine proportions
	const transSentenceCounts = transParagraphs.map(
		(tp) => splitSentences(tp).length,
	);
	const totalTransSentences = transSentenceCounts.reduce((a, b) => a + b, 0);

	// Distribute original sentences proportionally based on translated sentence counts
	let origIdx = 0;
	return transParagraphs.map((t, i) => {
		const proportion =
			totalTransSentences > 0
				? transSentenceCounts[i] / totalTransSentences
				: 1 / transParagraphs.length;

		// For the last paragraph, take all remaining sentences
		let count: number;
		if (i === transParagraphs.length - 1) {
			count = origSentences.length - origIdx;
		} else {
			count = Math.max(1, Math.round(origSentences.length * proportion));
		}

		const slice = origSentences.slice(origIdx, origIdx + count);
		origIdx += count;

		return {
			original: slice.join(" "),
			translated: t,
		};
	});
}

/**
 * Split text into sentences. Handles both English and CJK punctuation.
 * Splits on ". " / "! " / "? " followed by uppercase or "(" (English),
 * and on Chinese period/exclamation/question marks (。！？).
 */
export function splitSentences(text: string): string[] {
	if (!text) return [];
	// Split on English sentence boundaries (period/exclamation/question
	// followed by space + uppercase/parenthesis) and CJK sentence-ending
	// punctuation (。！？) followed by any character.
	const sentences = text
		.split(/(?<=[.!?])\s+(?=[A-Z(])|(?<=[。！？])(?=.)/g)
		.map((s) => s.trim())
		.filter((s) => s.length > 0);
	return sentences.length > 0 ? sentences : [text];
}

/**
 * Group an array of sentences into chunks of `size` sentences each.
 * The last chunk may have fewer sentences.
 */
function chunkSentences(sentences: string[], size: number): string[] {
	const chunks: string[] = [];
	for (let i = 0; i < sentences.length; i += size) {
		chunks.push(sentences.slice(i, i + size).join(""));
	}
	return chunks;
}

/**
 * Strip legacy "===" delimiters from cached translations.
 *
 * Old translations may contain paired format: "original\n===\ntranslated".
 * This function detects the pattern and extracts only the non-original
 * (translated) portions. Uses a heuristic: if the text contains "===",
 * take every other chunk (the translated side).
 *
 * For new translations (no "==="), returns the text unchanged.
 */
function stripDelimiters(text: string): string {
	if (!text.includes("===")) return text;

	const chunks = text
		.split(/^={3,}$/m)
		.map((c) => c.trim())
		.filter((c) => c.length > 0);

	if (chunks.length < 2) return text;

	// The prompt format was: original === translated === original === translated...
	// So odd-indexed chunks (1, 3, 5...) are the translated portions.
	const translatedChunks: string[] = [];
	for (let i = 1; i < chunks.length; i += 2) {
		translatedChunks.push(chunks[i]);
	}

	return translatedChunks.length > 0 ? translatedChunks.join("\n\n") : text;
}

/** Split text into paragraphs by double-newline or single-newline boundaries. */
export function splitParagraphs(text: string): string[] {
	if (!text) return [];
	// Try double-newline first
	const byDouble = text
		.split(/\n\s*\n/)
		.map((p) => p.trim())
		.filter((p) => p.length > 0);
	if (byDouble.length > 1) return byDouble;

	// Fall back to single newlines
	const bySingle = text
		.split(/\n/)
		.map((p) => p.trim())
		.filter((p) => p.length > 0);
	if (bySingle.length > 1) return bySingle;

	// Single paragraph
	return [text.trim()];
}

/**
 * Compact bilingual abstract for card views (feed, library grid, papersCool).
 *
 * Splits translated text into paragraphs, aligns original sentences
 * proportionally, and renders each pair in a compact card-friendly style.
 */
export function BilingualCardAbstract({
	original,
	translated,
	className,
}: {
	original: string;
	translated: string;
	className?: string;
}) {
	const transParagraphs = splitParagraphs(translated);
	const origParagraphs = splitParagraphs(original);
	const pairs = alignParagraphs(origParagraphs, transParagraphs);

	return (
		<div className={cn("mt-1.5 space-y-2", className)}>
			{pairs.map((pair, i) => (
				<div key={`card-p-${i}`} className="space-y-0.5">
					<p className="text-xs text-muted-foreground leading-relaxed">
						{pair.translated}
					</p>
					<p className="text-[11px] text-muted-foreground/50 leading-relaxed">
						{pair.original}
					</p>
				</div>
			))}
		</div>
	);
}

/**
 * Paragraphed text display for card views (translated-only or original-only mode).
 *
 * Splits a single block of text into ~3-sentence chunks so that long abstracts
 * are displayed in readable short paragraphs rather than one giant block.
 */
export function ParagraphedCardText({
	text,
	className,
}: {
	text: string;
	className?: string;
}) {
	const paragraphs = splitParagraphs(text);
	// If already multi-paragraph, render as-is
	if (paragraphs.length > 1) {
		return (
			<div className={cn("mt-1.5 space-y-2", className)}>
				{paragraphs.map((p, i) => (
					<p
						key={`pp-${i}`}
						className="text-xs text-muted-foreground leading-relaxed"
					>
						{p}
					</p>
				))}
			</div>
		);
	}
	// Single paragraph — try splitting into sentence chunks
	const CHUNK_SIZE = 3;
	const sentences = splitSentences(text);
	if (sentences.length > CHUNK_SIZE) {
		const chunks = chunkSentences(sentences, CHUNK_SIZE);
		return (
			<div className={cn("mt-1.5 space-y-2", className)}>
				{chunks.map((c, i) => (
					<p
						key={`pc-${i}`}
						className="text-xs text-muted-foreground leading-relaxed"
					>
						{c}
					</p>
				))}
			</div>
		);
	}
	// Short text — render as single paragraph
	return (
		<p
			className={cn(
				"mt-1.5 text-xs text-muted-foreground leading-relaxed",
				className,
			)}
		>
			{text}
		</p>
	);
}
