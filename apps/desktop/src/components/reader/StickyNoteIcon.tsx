// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { cn } from "@/lib/utils";

interface StickyNoteIconProps {
	color: string;
	size?: number;
	className?: string;
	onClick?: () => void;
}

/**
 * A sticky note icon rendered as an SVG, similar to Zotero's yellow folded-corner note.
 */
export function StickyNoteIcon({
	color,
	size = 24,
	className,
	onClick,
}: StickyNoteIconProps) {
	return (
		<svg
			width={size}
			height={size}
			viewBox="0 0 24 24"
			className={cn("cursor-pointer drop-shadow-sm", className)}
			onClick={onClick}
		>
			{/* Note body */}
			<path
				d="M4 4h16v12l-6 6H4V4z"
				fill={color}
				stroke="rgba(0,0,0,0.2)"
				strokeWidth="0.5"
			/>
			{/* Folded corner */}
			<path d="M14 16v6l6-6h-6z" fill="rgba(0,0,0,0.15)" />
			{/* Corner fold highlight */}
			<path d="M14 16l6-0.5v0.5h-6z" fill="rgba(0,0,0,0.05)" />
		</svg>
	);
}
