// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { cn } from "@/lib/utils";
import {
	type BackgroundTask,
	scheduleAutoDismiss,
	useBackgroundTaskStore,
} from "@/stores/backgroundTaskStore";
import { useUiStore } from "@/stores/uiStore";
import { listen } from "@tauri-apps/api/event";
import {
	Activity,
	AlertCircle,
	CheckCircle2,
	Download,
	Languages,
	Loader2,
	Search,
	X,
} from "lucide-react";
import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

interface TaskEventPayload {
	task_id: string;
	paper_id: string;
	paper_title: string;
	task_type: string;
	status: string;
	message: string | null;
}

const TASK_TYPE_KEYS: Record<string, string> = {
	enrichment: "backgroundTasks.enrichment",
	"pdf-download": "backgroundTasks.pdfDownload",
	"html-download": "backgroundTasks.htmlDownload",
	"html-translation": "backgroundTasks.htmlTranslation",
	"pdf-translation": "backgroundTasks.pdfTranslation",
};

function TaskStatusIcon({ task }: { task: BackgroundTask }) {
	if (task.status === "running") {
		return (
			<Loader2 className="h-3.5 w-3.5 animate-spin text-primary shrink-0" />
		);
	}
	if (task.status === "completed") {
		return <CheckCircle2 className="h-3.5 w-3.5 text-green-500 shrink-0" />;
	}
	return <AlertCircle className="h-3.5 w-3.5 text-red-500 shrink-0" />;
}

function TaskTypeIcon({ taskType }: { taskType: string }) {
	if (taskType === "pdf-download" || taskType === "html-download") {
		return <Download className="h-3 w-3 text-muted-foreground shrink-0" />;
	}
	if (taskType === "html-translation" || taskType === "pdf-translation") {
		return <Languages className="h-3 w-3 text-muted-foreground shrink-0" />;
	}
	return <Search className="h-3 w-3 text-muted-foreground shrink-0" />;
}

export function BackgroundTaskPanel() {
	const { t } = useTranslation();
	const showBackgroundTasks = useUiStore((s) => s.showBackgroundTasks);
	const tasks = useBackgroundTaskStore((s) => s.tasks);
	const expanded = useBackgroundTaskStore((s) => s.expanded);
	const upsertTask = useBackgroundTaskStore((s) => s.upsertTask);
	const dismissTask = useBackgroundTaskStore((s) => s.dismissTask);
	const clearCompleted = useBackgroundTaskStore((s) => s.clearCompleted);
	const setExpanded = useBackgroundTaskStore((s) => s.setExpanded);
	const panelRef = useRef<HTMLDivElement>(null);

	// Listen for background-task events from Rust
	useEffect(() => {
		const unlisten = listen<TaskEventPayload>("background-task", (event) => {
			const p = event.payload;
			upsertTask({
				taskId: p.task_id,
				paperId: p.paper_id,
				paperTitle: p.paper_title,
				taskType: p.task_type,
				status: p.status as "running" | "completed" | "failed",
				message: p.message,
				updatedAt: Date.now(),
			});
			if (p.status === "completed" || p.status === "failed") {
				scheduleAutoDismiss(p.task_id);
			}
		});
		return () => {
			unlisten.then((fn) => fn());
		};
	}, [upsertTask]);

	// Close popover on outside click
	useEffect(() => {
		if (!expanded) return;
		const handleClick = (e: MouseEvent) => {
			if (panelRef.current && !panelRef.current.contains(e.target as Node)) {
				setExpanded(false);
			}
		};
		document.addEventListener("mousedown", handleClick);
		return () => document.removeEventListener("mousedown", handleClick);
	}, [expanded, setExpanded]);

	const runningCount = tasks.filter((t) => t.status === "running").length;
	const hasRunning = runningCount > 0;
	const hasTasks = tasks.length > 0;

	if (!showBackgroundTasks) return null;

	return (
		<div ref={panelRef} className="fixed bottom-4 left-4 z-50">
			{/* Popover panel — opens upward */}
			{expanded && (
				<div className="mb-2 w-80 rounded-lg border bg-background shadow-xl animate-in fade-in slide-in-from-bottom-2 duration-150">
					{/* Header */}
					<div className="flex items-center justify-between border-b px-3 py-2">
						<span className="text-xs font-semibold text-muted-foreground">
							{t("backgroundTasks.title")}
						</span>
						<div className="flex items-center gap-1">
							{tasks.some((t) => t.status !== "running") && (
								<button
									type="button"
									className="px-1.5 py-0.5 rounded text-[10px] text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
									onClick={clearCompleted}
								>
									{t("backgroundTasks.clearDone")}
								</button>
							)}
						</div>
					</div>

					{/* Task list */}
					<div className="max-h-[240px] overflow-auto">
						{tasks.length === 0 ? (
							<div className="flex items-center justify-center py-8 text-xs text-muted-foreground">
								{t("backgroundTasks.noTasks")}
							</div>
						) : (
							tasks.map((task) => (
								<div
									key={task.taskId}
									className={cn(
										"flex items-start gap-2 px-3 py-2 text-xs border-b last:border-b-0",
										task.status === "failed" && "bg-red-500/5",
									)}
								>
									<TaskStatusIcon task={task} />
									<div className="flex-1 min-w-0">
										<div className="flex items-center gap-1.5">
											<TaskTypeIcon taskType={task.taskType} />
											<span className="text-muted-foreground">
												{t(TASK_TYPE_KEYS[task.taskType] ?? task.taskType)}
											</span>
										</div>
										<div className="truncate text-foreground mt-0.5">
											{task.paperTitle}
										</div>
										{task.message && task.status === "failed" && (
											<div
												className="text-red-500 mt-0.5 truncate"
												title={task.message}
											>
												{task.message}
											</div>
										)}
									</div>
									<button
										type="button"
										className="shrink-0 text-muted-foreground hover:text-foreground p-0.5 rounded hover:bg-accent transition-colors mt-0.5"
										onClick={() => dismissTask(task.taskId)}
									>
										<X className="h-3 w-3" />
									</button>
								</div>
							))
						)}
					</div>
				</div>
			)}

			{/* Persistent floating button */}
			<button
				type="button"
				className={cn(
					"relative flex h-9 w-9 items-center justify-center rounded-full border shadow-md transition-all",
					"hover:shadow-lg hover:scale-105 active:scale-95",
					hasRunning
						? "bg-primary text-primary-foreground border-primary/50"
						: "bg-background text-muted-foreground border-border hover:text-foreground",
				)}
				onClick={() => setExpanded(!expanded)}
				title={t("backgroundTasks.title")}
			>
				{hasRunning ? (
					<Loader2 className="h-4 w-4 animate-spin" />
				) : (
					<Activity className="h-4 w-4" />
				)}

				{/* Badge */}
				{hasTasks && (
					<span
						className={cn(
							"absolute -top-1 -right-1 flex h-4 min-w-4 items-center justify-center rounded-full px-1 text-[10px] font-medium",
							hasRunning
								? "bg-primary-foreground text-primary"
								: "bg-muted-foreground text-background",
						)}
					>
						{tasks.length}
					</span>
				)}
			</button>
		</div>
	);
}
