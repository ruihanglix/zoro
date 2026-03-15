// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { cn } from "@/lib/utils";
import { useEffect, useRef, useState } from "react";

interface EditableFieldProps {
	value: string | null | undefined;
	onSave: (value: string | null) => void;
	placeholder?: string;
	readOnly?: boolean;
	multiline?: boolean;
	className?: string;
}

export function EditableField({
	value,
	onSave,
	placeholder = "",
	readOnly = false,
	multiline = false,
	className,
}: EditableFieldProps) {
	const [editing, setEditing] = useState(false);
	const [draft, setDraft] = useState(value ?? "");
	const inputRef = useRef<HTMLInputElement | HTMLTextAreaElement>(null);

	// Sync draft when value changes externally (e.g. after save + re-fetch)
	useEffect(() => {
		if (!editing) {
			setDraft(value ?? "");
		}
	}, [value, editing]);

	// Auto-focus when entering edit mode
	useEffect(() => {
		if (editing && inputRef.current) {
			inputRef.current.focus();
			// Place cursor at end
			const len = inputRef.current.value.length;
			inputRef.current.setSelectionRange(len, len);
		}
	}, [editing]);

	const handleClick = () => {
		if (readOnly) return;
		setDraft(value ?? "");
		setEditing(true);
	};

	const handleSave = () => {
		setEditing(false);
		const trimmed = draft.trim();
		const newValue = trimmed === "" ? null : trimmed;
		const oldValue = value ?? null;
		if (newValue !== oldValue) {
			onSave(newValue);
		}
	};

	const handleCancel = () => {
		setEditing(false);
		setDraft(value ?? "");
	};

	const handleKeyDown = (e: React.KeyboardEvent) => {
		if (e.key === "Escape") {
			e.preventDefault();
			handleCancel();
		}
		if (e.key === "Enter" && !multiline) {
			e.preventDefault();
			handleSave();
		}
		// For multiline, Ctrl/Cmd+Enter saves
		if (e.key === "Enter" && multiline && (e.metaKey || e.ctrlKey)) {
			e.preventDefault();
			handleSave();
		}
	};

	if (editing) {
		const inputClasses = cn(
			"w-full rounded-sm border border-ring bg-background px-1.5 py-0.5 text-xs",
			"outline-none focus:ring-1 focus:ring-ring",
			className,
		);

		if (multiline) {
			return (
				<textarea
					ref={inputRef as React.RefObject<HTMLTextAreaElement>}
					value={draft}
					onChange={(e) => setDraft(e.target.value)}
					onBlur={handleSave}
					onKeyDown={handleKeyDown}
					className={cn(inputClasses, "min-h-[60px] resize-y")}
					rows={3}
				/>
			);
		}

		return (
			<input
				ref={inputRef as React.RefObject<HTMLInputElement>}
				type="text"
				value={draft}
				onChange={(e) => setDraft(e.target.value)}
				onBlur={handleSave}
				onKeyDown={handleKeyDown}
				className={inputClasses}
			/>
		);
	}

	// Display mode
	const displayValue = value ?? null;
	const isEmpty = displayValue === null || displayValue === "";

	return (
		<span
			onClick={handleClick}
			onKeyDown={(e) => {
				if (!readOnly && (e.key === "Enter" || e.key === " ")) {
					e.preventDefault();
					handleClick();
				}
			}}
			tabIndex={readOnly ? undefined : 0}
			role={readOnly ? undefined : "button"}
			className={cn(
				"text-xs",
				!readOnly &&
					"cursor-text rounded-sm px-1.5 py-0.5 -mx-1.5 -my-0.5 hover:bg-accent",
				isEmpty && !readOnly && "text-muted-foreground/40 italic",
				isEmpty && readOnly && "text-muted-foreground/30",
				className,
			)}
		>
			{isEmpty ? (readOnly ? "" : placeholder) : displayValue}
		</span>
	);
}
