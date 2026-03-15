// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { useLibraryStore } from "@/stores/libraryStore";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

const FILE_DROP_HIGHLIGHT_CLASS = "file-drop-target-highlight";

function findPaperIdAtPosition(x: number, y: number): string | null {
	const el = document.elementFromPoint(x, y);
	if (!el) return null;
	const paperEl = (el as HTMLElement).closest("[data-paper-id]");
	if (!paperEl) return null;
	return paperEl.getAttribute("data-paper-id");
}

function setDropHighlight(x: number, y: number): string | null {
	for (const el of document.querySelectorAll(`.${FILE_DROP_HIGHLIGHT_CLASS}`)) {
		el.classList.remove(FILE_DROP_HIGHLIGHT_CLASS);
	}

	const el = document.elementFromPoint(x, y);
	if (!el) return null;
	const paperEl = (el as HTMLElement).closest("[data-paper-id]");
	if (!paperEl) return null;
	paperEl.classList.add(FILE_DROP_HIGHLIGHT_CLASS);
	return paperEl.getAttribute("data-paper-id");
}

function clearDropHighlight() {
	for (const el of document.querySelectorAll(`.${FILE_DROP_HIGHLIGHT_CLASS}`)) {
		el.classList.remove(FILE_DROP_HIGHLIGHT_CLASS);
	}
}

// Inject CSS for file drop target highlighting
if (
	typeof document !== "undefined" &&
	!document.getElementById("file-drop-styles")
) {
	const style = document.createElement("style");
	style.id = "file-drop-styles";
	style.textContent = `
    .${FILE_DROP_HIGHLIGHT_CLASS} {
      outline: 2px solid hsl(var(--primary) / 0.6) !important;
      outline-offset: -1px;
      background-color: hsl(var(--primary) / 0.08) !important;
      border-radius: 4px;
      transition: outline 0.15s, background-color 0.15s;
    }
  `;
	document.head.appendChild(style);
}

export function FileDropZone() {
	const { t } = useTranslation();
	const [importStatus, setImportStatus] = useState<string | null>(null);
	const importLocalFiles = useLibraryStore((s) => s.importLocalFiles);
	const addAttachmentFiles = useLibraryStore((s) => s.addAttachmentFiles);
	const importing = useLibraryStore((s) => s.importing);

	useEffect(() => {
		const webview = getCurrentWebviewWindow();

		const unlisten = webview.onDragDropEvent((event) => {
			if (event.payload.type === "over") {
				const pos = event.payload.position;
				setDropHighlight(pos.x, pos.y);
				return;
			}

			if (event.payload.type === "leave") {
				clearDropHighlight();
				return;
			}

			if (event.payload.type === "drop") {
				clearDropHighlight();
				const paths = event.payload.paths;
				if (paths.length === 0) return;

				const pos = event.payload.position;
				const targetPaperId = findPaperIdAtPosition(pos.x, pos.y);

				if (targetPaperId) {
					handleAddAttachments(targetPaperId, paths);
				} else {
					const pdfPaths = paths.filter((p) =>
						p.toLowerCase().endsWith(".pdf"),
					);
					if (pdfPaths.length > 0) {
						handleImport(pdfPaths);
					}
				}
			}
		});

		return () => {
			unlisten.then((fn) => fn());
		};
	}, []);

	const handleAddAttachments = async (paperId: string, filePaths: string[]) => {
		try {
			await addAttachmentFiles(paperId, filePaths);
			const count = filePaths.length;
			setImportStatus(t("library.addedAttachments", { count }));
			setTimeout(() => setImportStatus(null), 3000);
		} catch (e) {
			setImportStatus(t("library.failedAddAttachment", { error: String(e) }));
			setTimeout(() => setImportStatus(null), 5000);
		}
	};

	const handleImport = async (filePaths: string[]) => {
		try {
			const result = await importLocalFiles(filePaths);
			const importedCount = result.imported.length;
			const skippedCount = result.skipped.length;

			if (importedCount > 0 && skippedCount === 0) {
				setImportStatus(t("library.importedPapers", { count: importedCount }));
			} else if (importedCount > 0 && skippedCount > 0) {
				setImportStatus(
					t("library.importedAndSkipped", {
						imported: importedCount,
						skipped: skippedCount,
					}),
				);
			} else {
				setImportStatus(
					t("library.noPapersImported", { skipped: skippedCount }),
				);
			}
			setTimeout(() => setImportStatus(null), 4000);
		} catch (e) {
			setImportStatus(t("library.importFailed", { error: String(e) }));
			setTimeout(() => setImportStatus(null), 5000);
		}
	};

	return (
		<>
			{/* Import status toast */}
			{(importStatus || importing) && (
				<div className="fixed bottom-4 right-4 z-50 rounded-lg border bg-background px-4 py-3 shadow-lg">
					{importing ? (
						<div className="flex items-center gap-2">
							<svg
								className="h-4 w-4 animate-spin text-primary"
								viewBox="0 0 24 24"
								fill="none"
								role="img"
								aria-label="Loading"
							>
								<circle
									className="opacity-25"
									cx="12"
									cy="12"
									r="10"
									stroke="currentColor"
									strokeWidth="4"
								/>
								<path
									className="opacity-75"
									fill="currentColor"
									d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
								/>
							</svg>
							<span className="text-sm">{t("library.importingFiles")}</span>
						</div>
					) : (
						<span className="text-sm">{importStatus}</span>
					)}
				</div>
			)}
		</>
	);
}
