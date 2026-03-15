// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { MetadataCandidate, MetadataSearchParams } from "@/lib/commands";
import * as commands from "@/lib/commands";
import { useLibraryStore } from "@/stores/libraryStore";
import { useUiStore } from "@/stores/uiStore";
import {
	Check,
	ChevronDown,
	ChevronUp,
	Loader2,
	Search,
	X,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

interface SearchField {
	key: keyof MetadataSearchParams;
	i18nKey: string;
	placeholder: string;
	primary?: boolean;
}

const SEARCH_FIELDS: SearchField[] = [
	{
		key: "title",
		i18nKey: "metadataSearch.fieldTitle",
		placeholder: "metadataSearch.fieldTitlePlaceholder",
		primary: true,
	},
	{
		key: "author",
		i18nKey: "metadataSearch.fieldAuthor",
		placeholder: "metadataSearch.fieldAuthorPlaceholder",
		primary: true,
	},
	{
		key: "doi",
		i18nKey: "metadataSearch.fieldDoi",
		placeholder: "metadataSearch.fieldDoiPlaceholder",
	},
	{
		key: "arxiv_id",
		i18nKey: "metadataSearch.fieldArxiv",
		placeholder: "metadataSearch.fieldArxivPlaceholder",
	},
	{
		key: "year",
		i18nKey: "metadataSearch.fieldYear",
		placeholder: "metadataSearch.fieldYearPlaceholder",
	},
	{
		key: "journal",
		i18nKey: "metadataSearch.fieldJournal",
		placeholder: "metadataSearch.fieldJournalPlaceholder",
	},
	{
		key: "isbn",
		i18nKey: "metadataSearch.fieldIsbn",
		placeholder: "metadataSearch.fieldIsbnPlaceholder",
	},
];

export function MetadataSearchDialog() {
	const { t } = useTranslation();
	const paperId = useUiStore((s) => s.metadataSearchPaperId);
	const closeDialog = useUiStore((s) => s.closeMetadataSearchDialog);
	const papers = useLibraryStore((s) => s.papers);
	const selectedPaper = useLibraryStore((s) => s.selectedPaper);
	const fetchPapers = useLibraryStore((s) => s.fetchPapers);

	const [params, setParams] = useState<MetadataSearchParams>({});
	const [showAdvanced, setShowAdvanced] = useState(false);
	const [searching, setSearching] = useState(false);
	const [candidates, setCandidates] = useState<MetadataCandidate[]>([]);
	const [applying, setApplying] = useState<number | null>(null);
	const [applied, setApplied] = useState<number | null>(null);
	const [error, setError] = useState<string | null>(null);

	// Pre-fill with current paper's metadata
	useEffect(() => {
		if (!paperId) return;
		// Try to find the paper from papers list first, then fall back to selectedPaper
		const paper =
			papers.find((p) => p.id === paperId) ??
			(selectedPaper?.id === paperId ? selectedPaper : null);
		if (!paper) return;
		const initial: MetadataSearchParams = {};
		if (paper.title) initial.title = paper.title;
		if (paper.authors?.length) {
			initial.author = paper.authors.map((a) => a.name).join(", ");
		}
		if (paper.doi) initial.doi = paper.doi;
		if (paper.arxiv_id) initial.arxiv_id = paper.arxiv_id;
		if (paper.journal) initial.journal = paper.journal;
		if (paper.published_date) {
			const year = paper.published_date.split("-")[0];
			if (year) initial.year = year;
		}
		if (paper.isbn) initial.isbn = paper.isbn;
		setParams(initial);
		// Auto-expand advanced if extra fields have data
		if (
			initial.doi ||
			initial.arxiv_id ||
			initial.year ||
			initial.journal ||
			initial.isbn
		) {
			setShowAdvanced(true);
		}
	}, [paperId, papers, selectedPaper]);

	// Reset state on close
	useEffect(() => {
		if (!paperId) {
			setParams({});
			setCandidates([]);
			setApplying(null);
			setApplied(null);
			setError(null);
			setShowAdvanced(false);
		}
	}, [paperId]);

	const updateField = (key: keyof MetadataSearchParams, value: string) => {
		setParams((prev) => ({ ...prev, [key]: value || undefined }));
	};

	const hasAnyField = Object.values(params).some(
		(v) => typeof v === "string" && v.trim().length > 0,
	);

	const handleSearch = async () => {
		if (!hasAnyField) return;
		setSearching(true);
		setCandidates([]);
		setApplied(null);
		setError(null);
		try {
			const results = await commands.searchMetadataCandidates(params);
			setCandidates(results);
			if (results.length === 0) {
				setError(t("metadataSearch.noResults"));
			}
		} catch (err) {
			setError(String(err));
		}
		setSearching(false);
	};

	const handleApply = async (index: number) => {
		if (!paperId) return;
		const candidate = candidates[index];
		if (!candidate.doi && !candidate.arxiv_id) {
			setError(t("metadataSearch.noIdentifier"));
			return;
		}
		setApplying(index);
		setError(null);
		try {
			await commands.applyMetadataCandidate(
				paperId,
				candidate.doi ?? null,
				candidate.arxiv_id ?? null,
			);
			setApplied(index);
			await fetchPapers();
		} catch (err) {
			setError(String(err));
		}
		setApplying(null);
	};

	const handleKeyDown = (e: React.KeyboardEvent) => {
		if (e.key === "Enter") {
			e.preventDefault();
			handleSearch();
		}
	};

	if (!paperId) return null;

	const primaryFields = SEARCH_FIELDS.filter((f) => f.primary);
	const advancedFields = SEARCH_FIELDS.filter((f) => !f.primary);

	return (
		<div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
			<div className="w-full max-w-2xl rounded-lg border bg-background p-6 shadow-lg max-h-[80vh] flex flex-col">
				{/* Header */}
				<div className="flex items-center justify-between mb-4">
					<h2 className="text-lg font-semibold">{t("metadataSearch.title")}</h2>
					<Button variant="ghost" size="icon" onClick={closeDialog}>
						<X className="h-4 w-4" />
					</Button>
				</div>

				{/* Search form */}
				<div className="space-y-2 mb-3">
					{/* Primary fields: Title + Author */}
					{primaryFields.map((field) => (
						<div key={field.key} className="flex items-center gap-2">
							<label className="text-xs font-medium text-muted-foreground w-16 shrink-0 text-right">
								{t(field.i18nKey)}
							</label>
							<Input
								value={(params[field.key] as string) ?? ""}
								onChange={(e) => updateField(field.key, e.target.value)}
								onKeyDown={handleKeyDown}
								placeholder={t(field.placeholder)}
								className="flex-1 h-8 text-sm"
							/>
						</div>
					))}

					{/* Advanced toggle */}
					<button
						type="button"
						onClick={() => setShowAdvanced((v) => !v)}
						className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors ml-[72px]"
					>
						{showAdvanced ? (
							<ChevronUp className="h-3 w-3" />
						) : (
							<ChevronDown className="h-3 w-3" />
						)}
						{t("metadataSearch.advancedFields")}
					</button>

					{/* Advanced fields — 2-column grid */}
					{showAdvanced && (
						<div className="grid grid-cols-2 gap-x-4 gap-y-2 ml-[72px]">
							{advancedFields.map((field) => (
								<div key={field.key} className="flex items-center gap-2">
									<label className="text-xs font-medium text-muted-foreground w-12 shrink-0 text-right">
										{t(field.i18nKey)}
									</label>
									<Input
										value={(params[field.key] as string) ?? ""}
										onChange={(e) => updateField(field.key, e.target.value)}
										onKeyDown={handleKeyDown}
										placeholder={t(field.placeholder)}
										className="flex-1 h-8 text-sm"
									/>
								</div>
							))}
						</div>
					)}

					{/* Search button */}
					<div className="flex gap-2 ml-[72px]">
						<Button
							onClick={handleSearch}
							disabled={searching || !hasAnyField}
							size="sm"
						>
							{searching ? (
								<Loader2 className="h-4 w-4 animate-spin mr-1.5" />
							) : (
								<Search className="h-4 w-4 mr-1.5" />
							)}
							{t("common.search")}
						</Button>
					</div>
				</div>

				<p className="text-xs text-muted-foreground mb-3">
					{t("metadataSearch.description")}
				</p>

				{/* Error */}
				{error && (
					<div className="text-sm text-destructive mb-3 px-1">{error}</div>
				)}

				{/* Results */}
				<div className="flex-1 overflow-y-auto space-y-2 min-h-0">
					{candidates.map((c, i) => (
						<div
							key={`${c.source}-${i}`}
							className="rounded-md border p-3 hover:bg-accent/30 transition-colors"
						>
							<div className="flex items-start justify-between gap-2">
								<div className="flex-1 min-w-0">
									<div className="font-medium text-sm leading-snug line-clamp-2">
										{c.title || t("metadataSearch.untitled")}
									</div>
									<div className="text-xs text-muted-foreground mt-1 space-y-0.5">
										{c.authors && c.authors.length > 0 && (
											<div className="truncate">
												{c.authors.slice(0, 3).join(", ")}
												{c.authors.length > 3 && ` +${c.authors.length - 3}`}
											</div>
										)}
										<div className="flex items-center gap-2 flex-wrap">
											{c.year && <span>{c.year}</span>}
											{c.venue && (
												<span className="truncate max-w-[200px]">
													{c.venue}
												</span>
											)}
											<span className="text-[10px] px-1.5 py-0.5 rounded bg-muted font-medium">
												{c.source}
											</span>
										</div>
										{(c.doi || c.arxiv_id) && (
											<div className="flex items-center gap-3 text-[11px] font-mono opacity-70">
												{c.doi && (
													<span className="truncate">DOI: {c.doi}</span>
												)}
												{c.arxiv_id && (
													<span className="shrink-0">arXiv: {c.arxiv_id}</span>
												)}
											</div>
										)}
									</div>
								</div>
								<Button
									size="sm"
									variant={applied === i ? "default" : "outline"}
									className="shrink-0 ml-2"
									disabled={
										applying !== null ||
										applied === i ||
										(!c.doi && !c.arxiv_id)
									}
									onClick={() => handleApply(i)}
								>
									{applying === i ? (
										<Loader2 className="h-3.5 w-3.5 animate-spin" />
									) : applied === i ? (
										<>
											<Check className="h-3.5 w-3.5 mr-1" />
											{t("metadataSearch.applied")}
										</>
									) : (
										t("metadataSearch.apply")
									)}
								</Button>
							</div>
						</div>
					))}
				</div>

				{/* Footer */}
				{applied !== null && (
					<div className="mt-4 flex justify-end">
						<Button onClick={closeDialog}>{t("common.close")}</Button>
					</div>
				)}
			</div>
		</div>
	);
}
