// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import {
	ResizableHandle,
	ResizablePanel,
	ResizablePanelGroup,
} from "@/components/ui/resizable";
import { useUiStore } from "@/stores/uiStore";
import type React from "react";
import { Sidebar } from "./Sidebar";
import { TopBar } from "./TopBar";

export function MainLayout({ children }: { children: React.ReactNode }) {
	const sidebarOpen = useUiStore((s) => s.sidebarOpen);

	return (
		<div className="h-full w-full overflow-hidden bg-background">
			<ResizablePanelGroup orientation="horizontal" autoSaveId="main-layout">
				{sidebarOpen && (
					<>
						<ResizablePanel
							id="sidebar"
							defaultSize="18%"
							minSize="12%"
							maxSize="28%"
						>
							<Sidebar />
						</ResizablePanel>
						<ResizableHandle />
					</>
				)}
				<ResizablePanel id="content" defaultSize="82%" minSize="50%">
					<div className="flex h-full flex-col overflow-hidden">
						<TopBar />
						<main className="flex-1 overflow-hidden">{children}</main>
					</div>
				</ResizablePanel>
			</ResizablePanelGroup>
		</div>
	);
}
