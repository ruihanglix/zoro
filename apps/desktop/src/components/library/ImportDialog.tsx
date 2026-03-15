// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { Button } from "@/components/ui/button";
import * as commands from "@/lib/commands";
import { useLibraryStore } from "@/stores/libraryStore";
import { useUiStore } from "@/stores/uiStore";
import { Upload, X } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

export function ImportDialog() {
	const { t } = useTranslation();
	const setOpen = useUiStore((s) => s.setImportDialogOpen);
	const fetchPapers = useLibraryStore((s) => s.fetchPapers);

	const [content, setContent] = useState("");
	const [format, setFormat] = useState<"bibtex" | "ris">("bibtex");
	const [importing, setImporting] = useState(false);
	const [result, setResult] = useState<string | null>(null);

	const handleImport = async () => {
		if (!content.trim()) return;
		setImporting(true);
		try {
			const count =
				format === "ris"
					? await commands.importRis(content)
					: await commands.importBibtex(content);
			setResult(t("importDialog.successMessage", { count }));
			await fetchPapers();
		} catch (err) {
			setResult(t("importDialog.errorPrefix", { error: err }));
		}
		setImporting(false);
	};

	return (
		<div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
			<div className="w-full max-w-lg rounded-lg border bg-background p-6 shadow-lg">
				<div className="flex items-center justify-between mb-4">
					<h2 className="text-lg font-semibold">{t("importDialog.title")}</h2>
					<Button variant="ghost" size="icon" onClick={() => setOpen(false)}>
						<X className="h-4 w-4" />
					</Button>
				</div>

				<div className="space-y-3">
					<div className="flex gap-2">
						<Button
							variant={format === "bibtex" ? "default" : "outline"}
							size="sm"
							onClick={() => setFormat("bibtex")}
						>
							BibTeX
						</Button>
						<Button
							variant={format === "ris" ? "default" : "outline"}
							size="sm"
							onClick={() => setFormat("ris")}
						>
							RIS
						</Button>
					</div>

					<textarea
						className="w-full h-48 rounded-md border bg-background px-3 py-2 text-sm font-mono resize-none focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
						placeholder={
							format === "bibtex" ? "@article{...}" : "TY  - JOUR\nTI  - ..."
						}
						value={content}
						onChange={(e) => setContent(e.target.value)}
					/>

					{result && (
						<p
							className={`text-sm ${result.startsWith("Error") ? "text-destructive" : "text-green-600"}`}
						>
							{result}
						</p>
					)}

					<div className="flex justify-end gap-2">
						<Button variant="outline" onClick={() => setOpen(false)}>
							{t("common.close")}
						</Button>
						<Button
							onClick={handleImport}
							disabled={importing || !content.trim()}
						>
							<Upload className="mr-2 h-4 w-4" />
							{importing ? t("importDialog.importing") : t("common.import")}
						</Button>
					</div>
				</div>
			</div>
		</div>
	);
}
