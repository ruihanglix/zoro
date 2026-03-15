// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { cn } from "@/lib/utils";
import { ANNOTATION_COLORS } from "@/stores/annotationStore";

interface ColorPickerProps {
	selectedColor: string;
	onColorSelect: (color: string) => void;
	size?: "sm" | "md";
	layout?: "row" | "grid";
}

export function ColorPicker({
	selectedColor,
	onColorSelect,
	size = "sm",
	layout = "row",
}: ColorPickerProps) {
	const dotSize = size === "sm" ? "h-4 w-4" : "h-5 w-5";
	const ringSize =
		size === "sm" ? "ring-1 ring-offset-1" : "ring-2 ring-offset-2";

	return (
		<div
			className={cn(
				"flex items-center gap-1.5",
				layout === "grid" && "flex-wrap",
			)}
		>
			{ANNOTATION_COLORS.map((c) => (
				<button
					key={c.value}
					type="button"
					className={cn(
						"rounded-full transition-transform hover:scale-110",
						dotSize,
						selectedColor === c.value && `${ringSize} ring-foreground`,
					)}
					style={{ backgroundColor: c.value }}
					onClick={() => onColorSelect(c.value)}
					title={c.name}
				/>
			))}
		</div>
	);
}
