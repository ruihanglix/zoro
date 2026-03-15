// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { cn } from "@/lib/utils";
import { useAnnotationStore } from "@/stores/annotationStore";
import { Loader2 } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

/**
 * Renders page thumbnails from the PDF document.
 * Uses pdf.js to render each page to a small canvas.
 * Lazy-loads thumbnails using IntersectionObserver.
 */
export function ThumbnailPanel() {
	const pdfDocument = useAnnotationStore((s) => s.pdfDocument);
	const totalPages = useAnnotationStore((s) => s.totalPages);
	const currentPage = useAnnotationStore((s) => s.currentPage);
	const navigateToPage = useAnnotationStore((s) => s.navigateToPage);

	if (!pdfDocument || totalPages === 0) {
		return (
			<div className="flex h-full items-center justify-center text-muted-foreground">
				<Loader2 className="h-5 w-5 animate-spin" />
			</div>
		);
	}

	return (
		<div className="flex flex-col gap-2 p-3 overflow-y-auto h-full">
			{Array.from({ length: totalPages }, (_, i) => i + 1).map((pageNum) => (
				<ThumbnailItem
					key={pageNum}
					pdfDocument={pdfDocument}
					pageNumber={pageNum}
					isActive={pageNum === currentPage}
					onClick={() => navigateToPage(pageNum)}
				/>
			))}
		</div>
	);
}

interface ThumbnailItemProps {
	pdfDocument: import("pdfjs-dist").PDFDocumentProxy;
	pageNumber: number;
	isActive: boolean;
	onClick: () => void;
}

function ThumbnailItem({
	pdfDocument,
	pageNumber,
	isActive,
	onClick,
}: ThumbnailItemProps) {
	const canvasRef = useRef<HTMLCanvasElement>(null);
	const containerRef = useRef<HTMLButtonElement>(null);
	const [rendered, setRendered] = useState(false);
	const renderingRef = useRef(false);

	const renderThumbnail = useCallback(async () => {
		if (renderingRef.current || rendered) return;
		renderingRef.current = true;

		try {
			const page = await pdfDocument.getPage(pageNumber);
			const canvas = canvasRef.current;
			if (!canvas) return;

			// Render at a small scale for thumbnails
			const thumbnailWidth = 140;
			const viewport = page.getViewport({ scale: 1 });
			const scale = thumbnailWidth / viewport.width;
			const scaledViewport = page.getViewport({ scale });

			canvas.width = scaledViewport.width;
			canvas.height = scaledViewport.height;

			const ctx = canvas.getContext("2d");
			if (!ctx) return;

			await page.render({
				canvasContext: ctx,
				viewport: scaledViewport,
			}).promise;

			setRendered(true);
		} catch (e) {
			console.error(`Failed to render thumbnail for page ${pageNumber}:`, e);
		} finally {
			renderingRef.current = false;
		}
	}, [pdfDocument, pageNumber, rendered]);

	// Lazy-load using IntersectionObserver
	useEffect(() => {
		const container = containerRef.current;
		if (!container) return;

		let idleHandle: number | null = null;

		const observer = new IntersectionObserver(
			(entries) => {
				if (entries[0].isIntersecting) {
					// 推迟到浏览器空闲时渲染，避免快速滚动时大量并发渲染阻塞主线程
					idleHandle = requestIdleCallback(
						() => {
							renderThumbnail();
							idleHandle = null;
						},
						{ timeout: 1000 }, // 最多延迟1秒保证最终渲染
					);
				} else if (idleHandle !== null) {
					// 离开视口时取消未执行的渲染任务
					cancelIdleCallback(idleHandle);
					idleHandle = null;
				}
			},
			{ threshold: 0.1 },
		);

		observer.observe(container);
		return () => {
			observer.disconnect();
			if (idleHandle !== null) {
				cancelIdleCallback(idleHandle);
			}
		};
	}, [renderThumbnail]);

	return (
		<button
			ref={containerRef}
			type="button"
			className={cn(
				"group flex flex-col items-center gap-1 rounded-md p-1 transition-colors",
				isActive ? "bg-primary/10 ring-2 ring-primary" : "hover:bg-muted/50",
			)}
			onClick={onClick}
		>
			<div className="relative border rounded bg-white shadow-sm overflow-hidden">
				<canvas
					ref={canvasRef}
					className="block"
					style={{ width: 140, height: "auto" }}
				/>
				{!rendered && (
					<div
						className="flex items-center justify-center bg-muted/30"
						style={{ width: 140, height: 180 }}
					>
						<Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
					</div>
				)}
			</div>
			<span
				className={cn(
					"text-[10px]",
					isActive ? "font-medium text-primary" : "text-muted-foreground",
				)}
			>
				{pageNumber}
			</span>
		</button>
	);
}
