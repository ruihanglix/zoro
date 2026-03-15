// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import * as commands from "@/lib/commands";
import type {
	ZoteroImportProgress,
	ZoteroImportResult,
	ZoteroScanResult,
} from "@/lib/commands";
import { useLibraryStore } from "@/stores/libraryStore";
import { listen } from "@tauri-apps/api/event";
import {
	AlertCircle,
	Check,
	CheckCircle2,
	ChevronRight,
	CloudOff,
	FileText,
	FolderOpen,
	FolderSearch,
	Loader2,
	X,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

type Step = "select" | "preview" | "importing" | "done";

interface Props {
	open: boolean;
	onClose: () => void;
}

export function ZoteroImportDialog({ open, onClose }: Props) {
	const { t } = useTranslation();
	const fetchPapers = useLibraryStore((s) => s.fetchPapers);
	const fetchCollections = useLibraryStore((s) => s.fetchCollections);
	const fetchTags = useLibraryStore((s) => s.fetchTags);

	const [step, setStep] = useState<Step>("select");
	const [zoteroDir, setZoteroDir] = useState("");
	const [detecting, setDetecting] = useState(false);
	const [validating, setValidating] = useState(false);
	const [validationError, setValidationError] = useState<string | null>(null);
	const [scanResult, setScanResult] = useState<ZoteroScanResult | null>(null);
	const [importResult, setImportResult] = useState<ZoteroImportResult | null>(
		null,
	);
	const [progress, setProgress] = useState<ZoteroImportProgress | null>(null);

	// Import options
	const [importCollections, setImportCollections] = useState(true);
	const [importNotes, setImportNotes] = useState(true);
	const [importAttachments, setImportAttachments] = useState(true);
	const [importAnnotations, setImportAnnotations] = useState(true);

	// Whether the user has acknowledged the cloud-PDF warning
	const [cloudWarningAcknowledged, setCloudWarningAcknowledged] =
		useState(false);

	const progressLogRef = useRef<HTMLDivElement>(null);

	// Auto-detect Zotero directory on open
	useEffect(() => {
		if (!open) return;
		setStep("select");
		setScanResult(null);
		setImportResult(null);
		setProgress(null);
		setValidationError(null);
		setCloudWarningAcknowledged(false);

		(async () => {
			setDetecting(true);
			try {
				const detected = await commands.detectZoteroDir();
				if (detected) {
					setZoteroDir(detected);
				}
			} catch {
				// Ignore detection errors
			} finally {
				setDetecting(false);
			}
		})();
	}, [open]);

	// Listen for progress events
	useEffect(() => {
		if (!open) return;
		const unlisten = listen<ZoteroImportProgress>(
			"zotero-import-progress",
			(event) => {
				setProgress(event.payload);
			},
		);
		return () => {
			unlisten.then((fn) => fn());
		};
	}, [open]);

	// Auto-scroll progress log
	useEffect(() => {
		if (progressLogRef.current) {
			progressLogRef.current.scrollTop = progressLogRef.current.scrollHeight;
		}
	}, [progress]);

	const handleBrowse = useCallback(async () => {
		try {
			const { open: openDialog } = await import("@tauri-apps/plugin-dialog");
			const selected = await openDialog({
				directory: true,
				multiple: false,
				title: t("zoteroImport.selectDirectory"),
			});
			if (selected) {
				setZoteroDir(selected as string);
				setValidationError(null);
				setScanResult(null);
			}
		} catch {
			// User cancelled
		}
	}, [t]);

	const handleValidateAndScan = useCallback(async () => {
		if (!zoteroDir.trim()) return;
		setValidating(true);
		setValidationError(null);
		setScanResult(null);
		try {
			await commands.validateZoteroDir(zoteroDir);
			const result = await commands.scanZoteroLibrary(zoteroDir);
			setScanResult(result);
			setStep("preview");
		} catch (err) {
			setValidationError(String(err));
		} finally {
			setValidating(false);
		}
	}, [zoteroDir]);

	const handleImport = useCallback(async () => {
		setStep("importing");
		setProgress(null);
		try {
			const result = await commands.importZoteroLibrary({
				zoteroDir,
				importCollections,
				importNotes,
				importAttachments,
				importAnnotations,
			});
			setImportResult(result);
			setStep("done");
			// Refresh sidebar: papers, collections, and tags
			await Promise.all([fetchPapers(), fetchCollections(), fetchTags()]);
		} catch (err) {
			setImportResult({
				papersImported: 0,
				papersSkipped: 0,
				collectionsImported: 0,
				notesImported: 0,
				attachmentsCopied: 0,
				attachmentsMissing: 0,
				annotationsImported: 0,
				errors: [String(err)],
			});
			setStep("done");
		}
	}, [
		zoteroDir,
		importCollections,
		importNotes,
		importAttachments,
		importAnnotations,
		fetchPapers,
		fetchCollections,
		fetchTags,
	]);

	if (!open) return null;

	return (
		<div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
			<div className="w-full max-w-lg rounded-lg border bg-background shadow-lg overflow-hidden">
				{/* Header */}
				<div className="flex items-center justify-between px-6 py-4">
					<h2 className="text-lg font-semibold flex items-center gap-2">
						<FileText className="h-5 w-5 text-primary" />
						{t("zoteroImport.title")}
					</h2>
					<Button
						variant="ghost"
						size="icon"
						onClick={onClose}
						disabled={step === "importing"}
					>
						<X className="h-4 w-4" />
					</Button>
				</div>

				<Separator />

				<div className="px-6 py-4 space-y-4 max-h-[70vh] overflow-y-auto">
					{/* Step 1: Select Zotero directory */}
					{step === "select" && (
						<>
							<p className="text-sm text-muted-foreground">
								{t("zoteroImport.description")}
							</p>

							<div className="space-y-2">
								<label className="text-xs font-medium text-muted-foreground">
									{t("zoteroImport.dataDirectory")}
								</label>
								<div className="flex gap-2">
									<input
										type="text"
										value={zoteroDir}
										onChange={(e) => {
											setZoteroDir(e.target.value);
											setValidationError(null);
											setScanResult(null);
										}}
										placeholder={
											detecting
												? t("zoteroImport.detecting")
												: t("zoteroImport.directoryPlaceholder")
										}
										disabled={detecting}
										className="h-8 flex-1 rounded-md border bg-transparent px-2 text-sm"
									/>
									<Button
										variant="outline"
										size="sm"
										onClick={handleBrowse}
										className="h-8 shrink-0"
									>
										<FolderOpen className="mr-1 h-3.5 w-3.5" />
										{t("zoteroImport.browse")}
									</Button>
								</div>

								{validationError && (
									<div className="flex items-start gap-2 text-destructive text-xs">
										<AlertCircle className="h-3.5 w-3.5 mt-0.5 shrink-0" />
										<span>{validationError}</span>
									</div>
								)}
							</div>

							<div className="flex justify-end">
								<Button
									size="sm"
									onClick={handleValidateAndScan}
									disabled={!zoteroDir.trim() || validating}
								>
									{validating ? (
										<Loader2 className="mr-1 h-3.5 w-3.5 animate-spin" />
									) : (
										<FolderSearch className="mr-1 h-3.5 w-3.5" />
									)}
									{validating
										? t("zoteroImport.scanning")
										: t("zoteroImport.scanLibrary")}
								</Button>
							</div>
						</>
					)}

					{/* Step 2: Preview scan results */}
					{step === "preview" && scanResult && (
						<>
							<div className="rounded-lg border p-4 space-y-3">
								<div className="flex items-center gap-2">
									<CheckCircle2 className="h-4 w-4 text-green-600" />
									<span className="text-sm font-medium">
										{t("zoteroImport.validLibrary")}
									</span>
								</div>

								<div className="grid grid-cols-3 gap-2 text-xs">
									<div className="rounded-md bg-muted p-2">
										<p className="text-muted-foreground">
											{t("zoteroImport.papers")}
										</p>
										<p className="text-lg font-semibold">
											{scanResult.totalItems}
										</p>
									</div>
									<div className="rounded-md bg-muted p-2">
										<p className="text-muted-foreground">
											{t("zoteroImport.collections")}
										</p>
										<p className="text-lg font-semibold">
											{scanResult.totalCollections}
										</p>
									</div>
									<div className="rounded-md bg-muted p-2">
										<p className="text-muted-foreground">
											{t("zoteroImport.tags")}
										</p>
										<p className="text-lg font-semibold">
											{scanResult.totalTags}
										</p>
									</div>
									<div className="rounded-md bg-muted p-2">
										<p className="text-muted-foreground">
											{t("zoteroImport.pdfs")}
										</p>
										<p className="text-lg font-semibold">
											{scanResult.totalAttachments}
										</p>
									</div>
									<div className="rounded-md bg-muted p-2">
										<p className="text-muted-foreground">
											{t("zoteroImport.notes")}
										</p>
										<p className="text-lg font-semibold">
											{scanResult.totalNotes}
										</p>
									</div>
									<div className="rounded-md bg-muted p-2">
										<p className="text-muted-foreground">
											{t("zoteroImport.annotations")}
										</p>
										<p className="text-lg font-semibold">
											{scanResult.totalAnnotations}
										</p>
									</div>
								</div>
							</div>

							{/* Cloud PDF warning */}
							{scanResult.cloudAttachments > 0 && (
								<div className="rounded-lg border border-amber-200 bg-amber-50 dark:border-amber-800 dark:bg-amber-950/30 p-3 space-y-2">
									<p className="text-xs font-medium text-amber-700 dark:text-amber-400 flex items-center gap-1.5">
										<CloudOff className="h-3.5 w-3.5 shrink-0" />
										{t("zoteroImport.cloudWarningTitle", {
											count: scanResult.cloudAttachments,
										})}
									</p>
									<p className="text-[11px] text-amber-600 dark:text-amber-300">
										{t("zoteroImport.cloudWarningDesc")}
									</p>
									<label className="flex items-center gap-2 text-xs text-amber-700 dark:text-amber-400 cursor-pointer pt-1">
										<input
											type="checkbox"
											checked={cloudWarningAcknowledged}
											onChange={(e) =>
												setCloudWarningAcknowledged(e.target.checked)
											}
											className="rounded"
										/>
										{t("zoteroImport.cloudWarningAck")}
									</label>
								</div>
							)}

							{/* Import options */}
							<div className="space-y-2">
								<h3 className="text-sm font-medium">
									{t("zoteroImport.importOptions")}
								</h3>
								<div className="space-y-1.5">
									{[
										{
											label: t("zoteroImport.optCollections"),
											checked: importCollections,
											onChange: setImportCollections,
											count: scanResult.totalCollections,
										},
										{
											label: t("zoteroImport.optAttachments"),
											checked: importAttachments,
											onChange: setImportAttachments,
											count: scanResult.totalAttachments,
										},
										{
											label: t("zoteroImport.optNotes"),
											checked: importNotes,
											onChange: setImportNotes,
											count: scanResult.totalNotes,
										},
										{
											label: t("zoteroImport.optAnnotations"),
											checked: importAnnotations,
											onChange: setImportAnnotations,
											count: scanResult.totalAnnotations,
										},
									].map((opt) => (
										<label
											key={opt.label}
											className="flex items-center gap-2 text-xs cursor-pointer"
										>
											<input
												type="checkbox"
												checked={opt.checked}
												onChange={(e) => opt.onChange(e.target.checked)}
												className="rounded"
											/>
											<span>
												{opt.label}{" "}
												<span className="text-muted-foreground">
													({opt.count})
												</span>
											</span>
										</label>
									))}
								</div>
								<p className="text-[11px] text-muted-foreground">
									{t("zoteroImport.papersAlwaysImported")}
								</p>
							</div>

							<div className="flex justify-between">
								<Button
									variant="outline"
									size="sm"
									onClick={() => setStep("select")}
								>
									{t("common.back")}
								</Button>
								<Button
									size="sm"
									onClick={handleImport}
									disabled={
										scanResult.cloudAttachments > 0 && !cloudWarningAcknowledged
									}
								>
									<ChevronRight className="mr-1 h-3.5 w-3.5" />
									{t("zoteroImport.startImport")}
								</Button>
							</div>
						</>
					)}

					{/* Step 3: Importing */}
					{step === "importing" && (
						<div className="space-y-4">
							<div className="flex items-center gap-2">
								<Loader2 className="h-4 w-4 animate-spin text-primary" />
								<span className="text-sm font-medium">
									{t("zoteroImport.importing")}
								</span>
							</div>

							{progress && (
								<>
									<div className="w-full bg-muted rounded-full h-2">
										<div
											className="bg-primary h-2 rounded-full transition-all duration-300"
											style={{
												width: `${progress.total > 0 ? (progress.current / progress.total) * 100 : 0}%`,
											}}
										/>
									</div>
									<div
										ref={progressLogRef}
										className="rounded-md bg-muted p-3 text-xs font-mono max-h-32 overflow-y-auto"
									>
										<p>{progress.message}</p>
									</div>
								</>
							)}
						</div>
					)}

					{/* Step 4: Done */}
					{step === "done" && importResult && (
						<div className="space-y-4">
							<div className="flex items-center gap-2">
								{importResult.errors.length === 0 ? (
									<Check className="h-5 w-5 text-green-600" />
								) : (
									<AlertCircle className="h-5 w-5 text-yellow-600" />
								)}
								<span className="text-sm font-medium">
									{t("zoteroImport.importComplete")}
								</span>
							</div>

							<div className="grid grid-cols-3 gap-2 text-xs">
								<div className="rounded-md bg-muted p-2">
									<p className="text-muted-foreground">
										{t("zoteroImport.papersImported")}
									</p>
									<p className="text-lg font-semibold">
										{importResult.papersImported}
									</p>
								</div>
								<div className="rounded-md bg-muted p-2">
									<p className="text-muted-foreground">
										{t("zoteroImport.papersSkipped")}
									</p>
									<p className="text-lg font-semibold">
										{importResult.papersSkipped}
									</p>
								</div>
								<div className="rounded-md bg-muted p-2">
									<p className="text-muted-foreground">
										{t("zoteroImport.collectionsResult")}
									</p>
									<p className="text-lg font-semibold">
										{importResult.collectionsImported}
									</p>
								</div>
								<div className="rounded-md bg-muted p-2">
									<p className="text-muted-foreground">
										{t("zoteroImport.attachmentsResult")}
									</p>
									<p className="text-lg font-semibold">
										{importResult.attachmentsCopied}
									</p>
								</div>
								<div className="rounded-md bg-muted p-2">
									<p className="text-muted-foreground">
										{t("zoteroImport.notesResult")}
									</p>
									<p className="text-lg font-semibold">
										{importResult.notesImported}
									</p>
								</div>
								<div className="rounded-md bg-muted p-2">
									<p className="text-muted-foreground">
										{t("zoteroImport.annotationsResult")}
									</p>
									<p className="text-lg font-semibold">
										{importResult.annotationsImported}
									</p>
								</div>
							</div>

							{importResult.attachmentsMissing > 0 && (
								<div className="rounded-lg border border-blue-200 bg-blue-50 dark:border-blue-800 dark:bg-blue-950/30 p-3 space-y-1">
									<p className="text-xs font-medium text-blue-700 dark:text-blue-400 flex items-center gap-1.5">
										<AlertCircle className="h-3.5 w-3.5" />
										{t("zoteroImport.missingAttachmentsTitle", {
											count: importResult.attachmentsMissing,
										})}
									</p>
									<p className="text-[11px] text-blue-600 dark:text-blue-300">
										{t("zoteroImport.missingAttachmentsDesc")}
									</p>
								</div>
							)}

							{importResult.errors.length > 0 && (
								<div className="space-y-1">
									<p className="text-xs font-medium text-destructive">
										{t("zoteroImport.errorsCount", {
											count: importResult.errors.length,
										})}
									</p>
									<div className="rounded-md bg-muted p-2 max-h-24 overflow-y-auto text-[11px] font-mono text-muted-foreground">
										{importResult.errors.slice(0, 20).map((err, i) => (
											<p key={`err-${i}`}>{err}</p>
										))}
										{importResult.errors.length > 20 && (
											<p>
												...{t("common.more")} {importResult.errors.length - 20}
											</p>
										)}
									</div>
								</div>
							)}

							<div className="flex justify-end">
								<Button size="sm" onClick={onClose}>
									{t("common.close")}
								</Button>
							</div>
						</div>
					)}
				</div>
			</div>
		</div>
	);
}
