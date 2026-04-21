// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { useTabStore } from "@/stores/tabStore";
import { useEffect, useState } from "react";

const IDLE_TIMEOUT = 5 * 60 * 1000; // 5 minutes
const CHECK_INTERVAL = 30_000; // 30 seconds

/**
 * Returns a Set of tab IDs that should remain mounted.
 * The active tab is always mounted; idle tabs (inactive > 5 min) are evicted.
 */
export function useMountedTabs(activeTabId: string): Set<string> {
	const [mountedIds, setMountedIds] = useState<Set<string>>(
		() => new Set([activeTabId]),
	);

	useEffect(() => {
		// Active tab is always mounted
		setMountedIds((prev) => {
			if (prev.has(activeTabId)) return prev;
			const next = new Set(prev);
			next.add(activeTabId);
			return next;
		});

		// Periodically evict idle tabs
		const timer = setInterval(() => {
			const now = Date.now();
			const { tabs: currentTabs, activeTabId: currentActiveId } =
				useTabStore.getState();
			setMountedIds((prev) => {
				const next = new Set<string>();
				for (const id of prev) {
					if (id === currentActiveId) {
						next.add(id);
						continue;
					}
					const tab = currentTabs.find((t) => t.id === id);
					if (
						tab?.lastActiveAt &&
						now - tab.lastActiveAt < IDLE_TIMEOUT
					) {
						next.add(id);
					}
				}
				// Ensure current active is always present
				next.add(currentActiveId);
				return next;
			});
		}, CHECK_INTERVAL);

		return () => clearInterval(timer);
	}, [activeTabId]);

	return mountedIds;
}
