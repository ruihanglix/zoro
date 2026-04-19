// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
	type ResolvedKeyBinding,
	findConflicts,
	findCrossScopeCollisions,
	formatForDisplay,
	keyboardEventToAccelerator,
} from "@/lib/keybindings";
import { cn } from "@/lib/utils";
import { useKeybindingStore } from "@/stores/keybindingStore";
import { AlertCircle, Info, RotateCcw, Search, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

export function KeyboardShortcutsSettings() {
	const { t } = useTranslation();
	const bindings = useKeybindingStore((s) => s.bindings);
	const rebind = useKeybindingStore((s) => s.rebind);
	const resetBinding = useKeybindingStore((s) => s.resetBinding);
	const resetAll = useKeybindingStore((s) => s.resetAll);
	const [filter, setFilter] = useState("");
	const [recordingId, setRecordingId] = useState<string | null>(null);
	const [confirmResetAll, setConfirmResetAll] = useState(false);
	const [pendingConflict, setPendingConflict] = useState<{
		id: string;
		newKey: string;
		conflictBinding: ResolvedKeyBinding;
	} | null>(null);

	// Group bindings by category
	const grouped = useMemo(() => {
		const q = filter.toLowerCase();
		const filtered = bindings.filter((b) => {
			if (!q) return true;
			const label = t(b.labelKey).toLowerCase();
			const keyDisplay = formatForDisplay(b.key).toLowerCase();
			return label.includes(q) || keyDisplay.includes(q);
		});
		const map = new Map<string, ResolvedKeyBinding[]>();
		for (const b of filtered) {
			const list = map.get(b.category) || [];
			list.push(b);
			map.set(b.category, list);
		}
		return map;
	}, [bindings, filter, t]);

	const handleRecord = useCallback(
		(id: string, accelerator: string | null) => {
			if (!accelerator) {
				// Backspace was pressed → unbind
				rebind(id, null);
				setRecordingId(null);
				return;
			}

			// Check for same-scope conflict
			const conflict = findConflicts(bindings, id, accelerator);
			if (conflict) {
				setPendingConflict({
					id,
					newKey: accelerator,
					conflictBinding: conflict,
				});
				return;
			}

			rebind(id, accelerator);
			setRecordingId(null);
		},
		[bindings, rebind],
	);

	const handleConfirmConflict = useCallback(() => {
		if (!pendingConflict) return;
		// Unbind the conflicting binding, then apply the new one
		rebind(pendingConflict.conflictBinding.id, null);
		rebind(pendingConflict.id, pendingConflict.newKey);
		setPendingConflict(null);
		setRecordingId(null);
	}, [pendingConflict, rebind]);

	const hasOverrides = bindings.some((b) => b.isOverridden);

	return (
		<div className="space-y-6">
			<div className="flex items-center justify-between">
				<h2 className="text-base font-semibold">
					{t("keybindings.title")}
				</h2>
				{hasOverrides && (
					<div className="relative">
						{confirmResetAll ? (
							<div className="flex items-center gap-2">
								<span className="text-xs text-muted-foreground">
									{t("keybindings.confirmResetAll")}
								</span>
								<Button
									size="sm"
									variant="destructive"
									onClick={() => {
										resetAll();
										setConfirmResetAll(false);
									}}
								>
									{t("common.confirm")}
								</Button>
								<Button
									size="sm"
									variant="ghost"
									onClick={() => setConfirmResetAll(false)}
								>
									{t("common.cancel")}
								</Button>
							</div>
						) : (
							<Button
								size="sm"
								variant="outline"
								onClick={() => setConfirmResetAll(true)}
							>
								<RotateCcw className="mr-1.5 h-3 w-3" />
								{t("keybindings.resetAll")}
							</Button>
						)}
					</div>
				)}
			</div>

			{/* Search */}
			<div className="relative">
				<Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground pointer-events-none" />
				<Input
					placeholder={t("keybindings.searchPlaceholder")}
					className="pl-8"
					value={filter}
					onChange={(e) => setFilter(e.target.value)}
				/>
				{filter && (
					<button
						type="button"
						className="absolute right-2.5 top-2.5 text-muted-foreground hover:text-foreground"
						onClick={() => setFilter("")}
					>
						<X className="h-4 w-4" />
					</button>
				)}
			</div>

			{/* Conflict dialog */}
			{pendingConflict && (
				<div className="rounded-md border border-destructive/50 bg-destructive/5 p-3 space-y-2">
					<div className="flex items-center gap-2 text-sm text-destructive">
						<AlertCircle className="h-4 w-4 shrink-0" />
						<span>
							{t("keybindings.conflictWarning", {
								key: formatForDisplay(pendingConflict.newKey),
								binding: t(pendingConflict.conflictBinding.labelKey),
							})}
						</span>
					</div>
					<div className="flex gap-2">
						<Button
							size="sm"
							variant="destructive"
							onClick={handleConfirmConflict}
						>
							{t("keybindings.overwrite")}
						</Button>
						<Button
							size="sm"
							variant="ghost"
							onClick={() => {
								setPendingConflict(null);
								setRecordingId(null);
							}}
						>
							{t("common.cancel")}
						</Button>
					</div>
				</div>
			)}

			{/* Binding categories */}
			<div className="grid grid-cols-1 lg:grid-cols-2 2xl:grid-cols-3 gap-4 items-start">
				{[...grouped.entries()].map(([category, items]) => (
					<div
						key={category}
						className="rounded-lg border bg-card/50 p-3 space-y-1"
					>
						<h3 className="text-xs font-medium text-muted-foreground uppercase tracking-wider mb-2">
							{t(category)}
						</h3>
						<div className="space-y-0.5">
							{items.map((binding) => (
								<BindingRow
									key={binding.id}
									binding={binding}
									isRecording={recordingId === binding.id}
									onStartRecord={() => {
										setRecordingId(binding.id);
										setPendingConflict(null);
									}}
									onCancelRecord={() => setRecordingId(null)}
									onRecord={(acc) => handleRecord(binding.id, acc)}
									onReset={() => resetBinding(binding.id)}
									allBindings={bindings}
								/>
							))}
						</div>
					</div>
				))}
			</div>

			{grouped.size === 0 && (
				<p className="text-sm text-muted-foreground text-center py-8">
					{t("keybindings.noResults")}
				</p>
			)}
		</div>
	);
}

function BindingRow({
	binding,
	isRecording,
	onStartRecord,
	onCancelRecord,
	onRecord,
	onReset,
	allBindings,
}: {
	binding: ResolvedKeyBinding;
	isRecording: boolean;
	onStartRecord: () => void;
	onCancelRecord: () => void;
	onRecord: (accelerator: string | null) => void;
	onReset: () => void;
	allBindings: ResolvedKeyBinding[];
}) {
	const { t } = useTranslation();
	const recordRef = useRef<HTMLButtonElement>(null);

	// Cross-scope collision info
	const crossScopeCollisions = useMemo(() => {
		if (!binding.key) return [];
		return findCrossScopeCollisions(allBindings, binding.id, binding.key);
	}, [allBindings, binding.id, binding.key]);

	// Capture keyboard events in recording mode
	useEffect(() => {
		if (!isRecording) return;
		const handleKeyDown = (e: KeyboardEvent) => {
			e.preventDefault();
			e.stopPropagation();

			if (e.key === "Escape") {
				onCancelRecord();
				return;
			}
			if (e.key === "Backspace" || e.key === "Delete") {
				onRecord(null); // Unbind
				return;
			}

			const acc = keyboardEventToAccelerator(e);
			if (acc) {
				onRecord(acc);
			}
		};
		window.addEventListener("keydown", handleKeyDown, true);
		return () => window.removeEventListener("keydown", handleKeyDown, true);
	}, [isRecording, onCancelRecord, onRecord]);

	// Auto-focus the record button when entering recording mode
	useEffect(() => {
		if (isRecording) {
			recordRef.current?.focus();
		}
	}, [isRecording]);

	return (
		<div className="flex items-center gap-2 py-1.5 px-2 rounded-md hover:bg-muted/50 group">
			{/* Label */}
			<span className="flex-1 text-sm min-w-0 truncate">
				{t(binding.labelKey)}
			</span>

			{/* Cross-scope info */}
			{crossScopeCollisions.length > 0 && !isRecording && (
				<span title={crossScopeCollisions.map(c => `${t(c.labelKey)} (${c.scope})`).join(", ")}>
					<Info className="h-3.5 w-3.5 text-muted-foreground" />
				</span>
			)}

			{/* Key display or recording state */}
			{isRecording ? (
				<button
					ref={recordRef}
					type="button"
					className="inline-flex items-center justify-center rounded-md border border-primary bg-primary/5 px-3 py-1 text-xs font-medium text-primary min-w-[120px] animate-pulse"
					onClick={onCancelRecord}
				>
					{t("keybindings.pressShortcut")}
				</button>
			) : (
				<button
					type="button"
					className={cn(
						"inline-flex items-center justify-center rounded-md border px-3 py-1 text-xs font-medium min-w-[80px] transition-colors",
						binding.key
							? "border-border bg-muted/50 text-foreground hover:bg-muted"
							: "border-dashed border-muted-foreground/30 text-muted-foreground hover:border-muted-foreground/50",
					)}
					onClick={onStartRecord}
					title={t("keybindings.clickToRecord")}
				>
					{binding.key ? (
						<KeyComboDisplay accelerator={binding.key} />
					) : (
						t("keybindings.notSet")
					)}
				</button>
			)}

			{/* Reset button (only if overridden) */}
			{binding.isOverridden && !isRecording && (
				<button
					type="button"
					className="rounded p-1 text-muted-foreground hover:text-foreground hover:bg-muted transition-colors opacity-0 group-hover:opacity-100"
					onClick={onReset}
					title={t("keybindings.reset")}
				>
					<RotateCcw className="h-3.5 w-3.5" />
				</button>
			)}
		</div>
	);
}

function KeyComboDisplay({ accelerator }: { accelerator: string }) {
	const parts = formatForDisplay(accelerator);
	return <span className="font-mono tracking-wider">{parts}</span>;
}
