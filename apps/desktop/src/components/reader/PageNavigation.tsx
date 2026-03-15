// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { useAnnotationStore } from "@/stores/annotationStore";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

/**
 * Page navigation component showing current page / total pages
 * with input for jumping to a specific page.
 */
export function PageNavigation() {
	const { t } = useTranslation();
	const currentPage = useAnnotationStore((s) => s.currentPage);
	const totalPages = useAnnotationStore((s) => s.totalPages);
	const navigateToPage = useAnnotationStore((s) => s.navigateToPage);

	const [inputValue, setInputValue] = useState(String(currentPage));
	const [isEditing, setIsEditing] = useState(false);

	// Sync input value with current page when not editing
	useEffect(() => {
		if (!isEditing) {
			setInputValue(String(currentPage));
		}
	}, [currentPage, isEditing]);

	const handleSubmit = useCallback(() => {
		const page = Number.parseInt(inputValue, 10);
		if (!Number.isNaN(page) && page >= 1 && page <= totalPages) {
			navigateToPage(page);
		} else {
			setInputValue(String(currentPage));
		}
		setIsEditing(false);
	}, [inputValue, totalPages, currentPage, navigateToPage]);

	const handlePrev = () => {
		if (currentPage > 1) {
			navigateToPage(currentPage - 1);
		}
	};

	const handleNext = () => {
		if (currentPage < totalPages) {
			navigateToPage(currentPage + 1);
		}
	};

	if (totalPages === 0) return null;

	return (
		<div className="flex items-center gap-0.5">
			<button
				type="button"
				className="rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
				onClick={handlePrev}
				disabled={currentPage <= 1}
				title={t("reader.previousPage")}
			>
				<ChevronLeft className="h-4 w-4" />
			</button>

			<div className="flex items-center gap-1 text-xs">
				<input
					type="text"
					value={inputValue}
					onChange={(e) => {
						setInputValue(e.target.value);
						setIsEditing(true);
					}}
					onFocus={() => setIsEditing(true)}
					onBlur={handleSubmit}
					onKeyDown={(e) => {
						if (e.key === "Enter") {
							handleSubmit();
							(e.target as HTMLInputElement).blur();
						}
						if (e.key === "Escape") {
							setInputValue(String(currentPage));
							setIsEditing(false);
							(e.target as HTMLInputElement).blur();
						}
					}}
					className="w-8 rounded border bg-transparent px-1 py-0.5 text-center text-xs outline-none focus:border-primary"
				/>
				<span className="text-muted-foreground">/ {totalPages}</span>
			</div>

			<button
				type="button"
				className="rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
				onClick={handleNext}
				disabled={currentPage >= totalPages}
				title={t("reader.nextPage")}
			>
				<ChevronRight className="h-4 w-4" />
			</button>
		</div>
	);
}
