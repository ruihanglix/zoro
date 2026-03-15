// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { useLibraryStore } from "@/stores/libraryStore";
import { useMemo } from "react";

export function HighlightedText({
	text,
	className,
}: {
	text: string;
	className?: string;
}) {
	const searchQuery = useLibraryStore((s) => s.searchQuery);

	const parts = useMemo(() => {
		if (!searchQuery.trim()) return null;

		const words = searchQuery
			.trim()
			.split(/\s+/)
			.filter((w) => w.length > 0);
		if (words.length === 0) return null;

		const escaped = words.map((w) => w.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
		const pattern = new RegExp(`(${escaped.join("|")})`, "gi");
		return text.split(pattern);
	}, [text, searchQuery]);

	if (!parts) {
		return <span className={className}>{text}</span>;
	}

	const words = searchQuery
		.trim()
		.split(/\s+/)
		.map((w) => w.toLowerCase());

	return (
		<span className={className}>
			{parts.map((part, i) =>
				words.includes(part.toLowerCase()) ? (
					<mark
						key={i}
						className="bg-yellow-200 dark:bg-yellow-700/60 text-inherit rounded-sm px-0.5"
					>
						{part}
					</mark>
				) : (
					part
				),
			)}
		</span>
	);
}
