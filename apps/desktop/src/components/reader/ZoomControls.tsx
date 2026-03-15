// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { useAnnotationStore } from "@/stores/annotationStore";
import { Minus, Plus } from "lucide-react";
import { useTranslation } from "react-i18next";

const ZOOM_STEP = 0.1;

export function ZoomControls() {
	const { t } = useTranslation();
	const zoomLevel = useAnnotationStore((s) => s.zoomLevel);
	const setZoomLevel = useAnnotationStore((s) => s.setZoomLevel);

	const zoomOut = () =>
		setZoomLevel(Math.round((zoomLevel - ZOOM_STEP) * 10) / 10);
	const zoomIn = () =>
		setZoomLevel(Math.round((zoomLevel + ZOOM_STEP) * 10) / 10);
	const resetZoom = () => setZoomLevel(1);

	const pct = Math.round(zoomLevel * 100);

	return (
		<div className="flex items-center gap-0.5">
			<button
				type="button"
				className="rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
				onClick={zoomOut}
				disabled={zoomLevel <= 0.3}
				title={t("reader.zoomOut")}
			>
				<Minus className="h-3.5 w-3.5" />
			</button>

			<button
				type="button"
				className="min-w-[3rem] rounded px-1 py-0.5 text-xs text-center text-muted-foreground hover:bg-muted hover:text-foreground transition-colors tabular-nums"
				onClick={resetZoom}
				title={t("reader.resetZoom")}
			>
				{pct}%
			</button>

			<button
				type="button"
				className="rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
				onClick={zoomIn}
				disabled={zoomLevel >= 5}
				title={t("reader.zoomIn")}
			>
				<Plus className="h-3.5 w-3.5" />
			</button>
		</div>
	);
}
