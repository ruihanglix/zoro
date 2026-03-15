// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { Button } from "@/components/ui/button";
import type { FeedItemResponse } from "@/lib/commands";
import * as commands from "@/lib/commands";
import { useTabStore } from "@/stores/tabStore";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-shell";
import {
	ArrowLeft,
	ArrowRight,
	ExternalLink,
	FileText,
	RefreshCw,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

let webviewCounter = 0;

interface WebviewProps {
	url: string;
	feedItem?: FeedItemResponse | null;
	isActive: boolean;
}

export function Webview({ url, feedItem, isActive }: WebviewProps) {
	const { t } = useTranslation();
	const openTab = useTabStore((s) => s.openTab);
	const containerRef = useRef<HTMLDivElement>(null);
	const labelRef = useRef(`browser-${++webviewCounter}`);
	const createdRef = useRef(false);
	const [currentUrl, setCurrentUrl] = useState(url);
	const [createError, setCreateError] = useState<string | null>(null);

	// Create native webview on mount, destroy on unmount
	useEffect(() => {
		const label = labelRef.current;

		const createWebview = () => {
			const container = containerRef.current;
			if (!container || createdRef.current) return;

			const rect = container.getBoundingClientRect();
			if (rect.width <= 0 || rect.height <= 0) {
				requestAnimationFrame(createWebview);
				return;
			}

			createdRef.current = true;
			commands
				.createBrowserWebview(
					label,
					url,
					rect.left,
					rect.top,
					rect.width,
					rect.height,
				)
				.catch((e) => {
					setCreateError(String(e));
					createdRef.current = false;
				});
		};

		requestAnimationFrame(createWebview);

		return () => {
			createdRef.current = false;
			commands.closeBrowserWebview(label).catch(() => {});
		};
	}, [url]);

	// Track container size changes and update native webview position
	useEffect(() => {
		const container = containerRef.current;
		if (!container) return;

		const observer = new ResizeObserver(() => {
			if (!createdRef.current) return;
			const rect = container.getBoundingClientRect();
			if (rect.width > 0 && rect.height > 0) {
				commands
					.resizeBrowserWebview(
						labelRef.current,
						rect.left,
						rect.top,
						rect.width,
						rect.height,
					)
					.catch(() => {});
			}
		});

		observer.observe(container);
		return () => observer.disconnect();
	}, []);

	// Show/hide native webview based on tab visibility
	useEffect(() => {
		if (!createdRef.current) return;
		const label = labelRef.current;
		if (isActive) {
			commands.showBrowserWebview(label).catch(() => {});
			// Re-sync position when becoming visible
			const container = containerRef.current;
			if (container) {
				const rect = container.getBoundingClientRect();
				if (rect.width > 0 && rect.height > 0) {
					commands
						.resizeBrowserWebview(
							label,
							rect.left,
							rect.top,
							rect.width,
							rect.height,
						)
						.catch(() => {});
				}
			}
		} else {
			commands.hideBrowserWebview(label).catch(() => {});
		}
	}, [isActive]);

	// Listen for URL change events from the native webview
	useEffect(() => {
		const label = labelRef.current;
		const unlisten = listen<{ label: string; url: string }>(
			"browser-navigation",
			(event) => {
				if (event.payload.label === label) {
					setCurrentUrl(event.payload.url);
				}
			},
		);
		return () => {
			unlisten.then((fn) => fn());
		};
	}, []);

	const handleGoBack = useCallback(() => {
		commands.browserGoBack(labelRef.current).catch(() => {});
	}, []);

	const handleGoForward = useCallback(() => {
		commands.browserGoForward(labelRef.current).catch(() => {});
	}, []);

	const handleReload = useCallback(() => {
		commands.browserReload(labelRef.current).catch(() => {});
	}, []);

	const handleOpenPdf = useCallback(() => {
		if (!feedItem) return;
		openTab({
			type: "reader",
			feedItem,
			readerMode: "pdf",
			title: feedItem.title,
		});
	}, [feedItem, openTab]);

	const handleOpenExternal = useCallback(() => {
		open(currentUrl);
	}, [currentUrl]);

	const hasPdf = feedItem?.pdf_url;

	return (
		<div className="flex h-full w-full flex-col">
			{/* Toolbar */}
			<div className="flex items-center gap-1 border-b px-3 py-1.5 bg-muted/30 select-none shrink-0">
				<Button
					variant="ghost"
					size="icon"
					className="h-7 w-7"
					onClick={handleGoBack}
					title={t("common.back")}
				>
					<ArrowLeft className="h-3.5 w-3.5" />
				</Button>
				<Button
					variant="ghost"
					size="icon"
					className="h-7 w-7"
					onClick={handleGoForward}
					title={t("common.forward")}
				>
					<ArrowRight className="h-3.5 w-3.5" />
				</Button>
				<Button
					variant="ghost"
					size="icon"
					className="h-7 w-7"
					onClick={handleReload}
					title={t("common.refresh")}
				>
					<RefreshCw className="h-3.5 w-3.5" />
				</Button>

				<div className="flex-1 min-w-0 rounded-md border bg-background px-3 py-1 text-xs text-muted-foreground truncate mx-1">
					{currentUrl}
				</div>

				{hasPdf && (
					<Button
						variant="default"
						size="sm"
						className="h-7 text-xs shrink-0"
						onClick={handleOpenPdf}
					>
						<FileText className="mr-1.5 h-3.5 w-3.5" />
						{t("contextMenu.openPdf")}
					</Button>
				)}

				<Button
					variant="ghost"
					size="icon"
					className="h-7 w-7 shrink-0"
					onClick={handleOpenExternal}
					title={t("common.openInBrowser")}
				>
					<ExternalLink className="h-3.5 w-3.5" />
				</Button>
			</div>

			{/* Native webview placeholder */}
			{createError ? (
				<div className="flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground">
					<p className="text-sm">{t("common.unableToLoadPage")}</p>
					<p className="text-xs max-w-md text-center">{createError}</p>
					<div className="flex gap-2">
						<Button variant="outline" size="sm" onClick={handleOpenExternal}>
							<ExternalLink className="mr-1.5 h-3.5 w-3.5" />
							{t("common.openInBrowser")}
						</Button>
						{hasPdf && (
							<Button variant="default" size="sm" onClick={handleOpenPdf}>
								<FileText className="mr-1.5 h-3.5 w-3.5" />
								{t("contextMenu.openPdf")}
							</Button>
						)}
					</div>
				</div>
			) : (
				<div ref={containerRef} className="flex-1 w-full" />
			)}
		</div>
	);
}
