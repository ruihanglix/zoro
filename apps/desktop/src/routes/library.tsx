// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { AddPaperDialog } from "@/components/library/AddPaperDialog";
import { ImportDialog } from "@/components/library/ImportDialog";
import { PaperDetail } from "@/components/library/PaperDetail";
import { PaperList } from "@/components/library/PaperList";
import {
	ResizableHandle,
	ResizablePanel,
	ResizablePanelGroup,
} from "@/components/ui/resizable";
import { useLibraryStore } from "@/stores/libraryStore";
import { useUiStore } from "@/stores/uiStore";

export function Library() {
	const selectedPaper = useLibraryStore((s) => s.selectedPaper);
	const addPaperDialogOpen = useUiStore((s) => s.addPaperDialogOpen);
	const importDialogOpen = useUiStore((s) => s.importDialogOpen);

	return (
		<div className="h-full">
			<ResizablePanelGroup orientation="horizontal" autoSaveId="library-layout">
				<ResizablePanel
					id="paper-list"
					defaultSize={selectedPaper ? "60%" : "100%"}
					minSize="35%"
				>
					<PaperList />
				</ResizablePanel>
				{selectedPaper && (
					<>
						<ResizableHandle />
						<ResizablePanel
							id="paper-detail"
							defaultSize="40%"
							minSize="25%"
							maxSize="60%"
						>
							<PaperDetail paper={selectedPaper} />
						</ResizablePanel>
					</>
				)}
			</ResizablePanelGroup>
			{addPaperDialogOpen && <AddPaperDialog />}
			{importDialogOpen && <ImportDialog />}
		</div>
	);
}
