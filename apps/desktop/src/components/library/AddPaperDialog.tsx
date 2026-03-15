// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useLibraryStore } from "@/stores/libraryStore";
import { useUiStore } from "@/stores/uiStore";
import { X } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

export function AddPaperDialog() {
	const { t } = useTranslation();
	const setOpen = useUiStore((s) => s.setAddPaperDialogOpen);
	const addPaper = useLibraryStore((s) => s.addPaper);

	const [title, setTitle] = useState("");
	const [authors, setAuthors] = useState("");
	const [doi, setDoi] = useState("");
	const [arxivId, setArxivId] = useState("");
	const [url, setUrl] = useState("");
	const [pdfUrl, setPdfUrl] = useState("");
	const [htmlUrl, setHtmlUrl] = useState("");
	const [tags, setTags] = useState("");
	const [saving, setSaving] = useState(false);

	const handleSubmit = async (e: React.FormEvent) => {
		e.preventDefault();
		if (!title.trim()) return;
		setSaving(true);
		try {
			await addPaper({
				title: title.trim(),
				authors: authors
					.split(",")
					.map((a) => ({ name: a.trim() }))
					.filter((a) => a.name),
				doi: doi || undefined,
				arxiv_id: arxivId || undefined,
				url: url || undefined,
				pdf_url: pdfUrl || undefined,
				html_url: htmlUrl || undefined,
				tags: tags
					.split(",")
					.map((t) => t.trim())
					.filter(Boolean),
			});
			setOpen(false);
		} catch (err) {
			console.error("Failed to add paper:", err);
		}
		setSaving(false);
	};

	return (
		<div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
			<div className="w-full max-w-lg rounded-lg border bg-background p-6 shadow-lg">
				<div className="flex items-center justify-between mb-4">
					<h2 className="text-lg font-semibold">{t("addPaperDialog.title")}</h2>
					<Button variant="ghost" size="icon" onClick={() => setOpen(false)}>
						<X className="h-4 w-4" />
					</Button>
				</div>

				<form onSubmit={handleSubmit} className="space-y-3">
					<div>
						<label className="text-sm font-medium">
							{t("addPaperDialog.paperTitle")}
						</label>
						<Input
							value={title}
							onChange={(e) => setTitle(e.target.value)}
							placeholder={t("addPaperDialog.paperTitlePlaceholder")}
							required
						/>
					</div>
					<div>
						<label className="text-sm font-medium">
							{t("addPaperDialog.authors")}
						</label>
						<Input
							value={authors}
							onChange={(e) => setAuthors(e.target.value)}
							placeholder={t("addPaperDialog.authorsPlaceholder")}
						/>
					</div>
					<div className="grid grid-cols-2 gap-3">
						<div>
							<label className="text-sm font-medium">
								{t("addPaperDialog.doi")}
							</label>
							<Input
								value={doi}
								onChange={(e) => setDoi(e.target.value)}
								placeholder={t("addPaperDialog.doiPlaceholder")}
							/>
						</div>
						<div>
							<label className="text-sm font-medium">
								{t("addPaperDialog.arxivId")}
							</label>
							<Input
								value={arxivId}
								onChange={(e) => setArxivId(e.target.value)}
								placeholder={t("addPaperDialog.arxivIdPlaceholder")}
							/>
						</div>
					</div>
					<div>
						<label className="text-sm font-medium">
							{t("addPaperDialog.url")}
						</label>
						<Input
							value={url}
							onChange={(e) => setUrl(e.target.value)}
							placeholder={t("addPaperDialog.urlPlaceholder")}
						/>
					</div>
					<div className="grid grid-cols-2 gap-3">
						<div>
							<label className="text-sm font-medium">
								{t("addPaperDialog.pdfUrl")}
							</label>
							<Input
								value={pdfUrl}
								onChange={(e) => setPdfUrl(e.target.value)}
								placeholder={t("addPaperDialog.pdfUrlPlaceholder")}
							/>
						</div>
						<div>
							<label className="text-sm font-medium">
								{t("addPaperDialog.htmlUrl")}
							</label>
							<Input
								value={htmlUrl}
								onChange={(e) => setHtmlUrl(e.target.value)}
								placeholder={t("addPaperDialog.htmlUrlPlaceholder")}
							/>
						</div>
					</div>
					<div>
						<label className="text-sm font-medium">
							{t("addPaperDialog.tags")}
						</label>
						<Input
							value={tags}
							onChange={(e) => setTags(e.target.value)}
							placeholder={t("addPaperDialog.tagsPlaceholder")}
						/>
					</div>
					<div className="flex justify-end gap-2 pt-2">
						<Button
							type="button"
							variant="outline"
							onClick={() => setOpen(false)}
						>
							{t("common.cancel")}
						</Button>
						<Button type="submit" disabled={saving}>
							{saving
								? t("addPaperDialog.saving")
								: t("addPaperDialog.addPaper")}
						</Button>
					</div>
				</form>
			</div>
		</div>
	);
}
