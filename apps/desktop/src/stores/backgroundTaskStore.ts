// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { create } from "zustand";

export type TaskStatus = "running" | "completed" | "failed";

export interface BackgroundTask {
	taskId: string;
	paperId: string;
	paperTitle: string;
	taskType: string;
	status: TaskStatus;
	message: string | null;
	updatedAt: number;
}

interface BackgroundTaskState {
	tasks: BackgroundTask[];
	expanded: boolean;

	upsertTask: (task: BackgroundTask) => void;
	dismissTask: (taskId: string) => void;
	clearCompleted: () => void;
	setExpanded: (expanded: boolean) => void;
}

const COMPLETED_TTL_MS = 8000;

export const useBackgroundTaskStore = create<BackgroundTaskState>((set) => ({
	tasks: [],
	expanded: false,

	upsertTask: (task) =>
		set((s) => {
			const idx = s.tasks.findIndex((t) => t.taskId === task.taskId);
			const updated = { ...task, updatedAt: Date.now() };
			if (idx >= 0) {
				const next = [...s.tasks];
				next[idx] = updated;
				return { tasks: next };
			}
			return { tasks: [...s.tasks, updated] };
		}),

	dismissTask: (taskId) =>
		set((s) => ({
			tasks: s.tasks.filter((t) => t.taskId !== taskId),
		})),

	clearCompleted: () =>
		set((s) => ({
			tasks: s.tasks.filter((t) => t.status === "running"),
		})),

	setExpanded: (expanded) => set({ expanded }),
}));

/**
 * Auto-dismiss completed/failed tasks after a delay.
 * Call once from the component that listens to events.
 */
export function scheduleAutoDismiss(taskId: string) {
	setTimeout(() => {
		const store = useBackgroundTaskStore.getState();
		const task = store.tasks.find((t) => t.taskId === taskId);
		if (task && task.status !== "running") {
			store.dismissTask(taskId);
		}
	}, COMPLETED_TTL_MS);
}
