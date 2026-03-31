// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import * as commands from "@/lib/commands";
import type { AnnotationResponse } from "@/lib/commands";
import { logger } from "@/lib/logger";
import type {
	AnnotationType,
	InkAnnotationData,
	ZoroHighlight,
} from "@/stores/annotationStore";
import { useCallback, useState } from "react";
import type { Content, ScaledPosition } from "react-pdf-highlighter";

function responseToHighlight(resp: AnnotationResponse): ZoroHighlight {
	const position: ScaledPosition = JSON.parse(resp.position_json);
	return {
		id: resp.id,
		type: resp.type as AnnotationType,
		color: resp.color,
		paperId: resp.paper_id,
		selectedText: resp.selected_text,
		imageData: resp.image_data,
		pageNumber: resp.page_number,
		createdDate: resp.created_date,
		modifiedDate: resp.modified_date,
		position,
		content: {
			text: resp.selected_text ?? undefined,
			image: resp.image_data ?? undefined,
		},
		comment: {
			text: resp.comment ?? "",
			emoji: "",
		},
	};
}

/**
 * Standalone annotation state management for a PDF viewer that doesn't
 * share state with the global annotationStore. Used by the secondary
 * viewer in bilingual mode.
 */
export function useLocalAnnotations(
	paperId: string | null,
	sourceFile: string,
) {
	const [annotations, setAnnotations] = useState<ZoroHighlight[]>([]);
	const [loading, setLoading] = useState(false);

	const fetchAnnotations = useCallback(async () => {
		if (!paperId) return;
		setLoading(true);
		try {
			const resp = await commands.listAnnotations(paperId, sourceFile);
			const highlights = resp.map(responseToHighlight);
			logger.debug("annotation", `[${sourceFile}] fetched ${highlights.length} annotations (types: ${highlights.map((h) => h.type).join(", ") || "none"})`);
			setAnnotations(highlights);
		} catch (e) {
			logger.error("annotation", "Local fetch failed", e);
		} finally {
			setLoading(false);
		}
	}, [paperId, sourceFile]);

	const addAnnotation = useCallback(
		async (
			type: AnnotationType,
			color: string,
			position: ScaledPosition,
			content: Content,
		): Promise<ZoroHighlight | null> => {
			if (!paperId) return null;
			try {
				const resp = await commands.addAnnotation(
					paperId,
					type,
					color,
					JSON.stringify(position),
					position.pageNumber,
					undefined,
					content.text ?? null,
					content.image ?? null,
					sourceFile,
				);
				const highlight = responseToHighlight(resp);
				setAnnotations((prev) => [...prev, highlight]);
				return highlight;
			} catch (e) {
				logger.error("annotation", "Local add failed", e);
				return null;
			}
		},
		[paperId, sourceFile],
	);

	const addInkAnnotation = useCallback(
		async (
			color: string,
			pageNumber: number,
			inkData: InkAnnotationData,
		): Promise<ZoroHighlight | null> => {
			if (!paperId) return null;
			try {
				const positionJson = JSON.stringify({
					pageNumber,
					boundingRect: {
						x1: inkData.boundingRect.x1,
						y1: inkData.boundingRect.y1,
						x2: inkData.boundingRect.x2,
						y2: inkData.boundingRect.y2,
						width: 100,
						height: 100,
						pageNumber,
					},
					rects: [],
					usePdfCoordinates: false,
					inkStrokes: inkData.strokes,
				});
				const resp = await commands.addAnnotation(
					paperId,
					"ink",
					color,
					positionJson,
					pageNumber,
					undefined,
					undefined,
					undefined,
					sourceFile,
				);
				const highlight = responseToHighlight(resp);
				setAnnotations((prev) => [...prev, highlight]);
				return highlight;
			} catch (e) {
				logger.error("annotation", "Local add ink failed", e);
				return null;
			}
		},
		[paperId, sourceFile],
	);

	const updateAnnotation = useCallback(
		async (id: string, color?: string | null, comment?: string | null) => {
			try {
				const resp = await commands.updateAnnotation(id, color, comment);
				const updated = responseToHighlight(resp);
				setAnnotations((prev) => prev.map((a) => (a.id === id ? updated : a)));
			} catch (e) {
			logger.error("annotation", "Local update failed", e);
			}
		},
		[],
	);

	const deleteAnnotation = useCallback(async (id: string) => {
		try {
			await commands.deleteAnnotation(id);
			setAnnotations((prev) => prev.filter((a) => a.id !== id));
		} catch (e) {
			logger.error("annotation", "Local delete failed", e);
		}
	}, []);

	const updateAnnotationType = useCallback(
		async (id: string, newType: AnnotationType) => {
			try {
				const resp = await commands.updateAnnotationType(id, newType);
				const updated = responseToHighlight(resp);
				setAnnotations((prev) => prev.map((a) => (a.id === id ? updated : a)));
			} catch (e) {
			logger.error("annotation", "Local update type failed", e);
			}
		},
		[],
	);

	return {
		annotations,
		loading,
		fetchAnnotations,
		addAnnotation,
		addInkAnnotation,
		updateAnnotation,
		deleteAnnotation,
		updateAnnotationType,
	};
}
