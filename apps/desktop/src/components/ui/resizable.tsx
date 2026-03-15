// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { cn } from "@/lib/utils";
import { GripVertical } from "lucide-react";
import {
	Group,
	type GroupProps,
	Panel,
	Separator,
	type SeparatorProps,
	useDefaultLayout,
} from "react-resizable-panels";

/**
 * Resizable panel group with optional localStorage persistence.
 * Wraps react-resizable-panels v4 Group with a layout-saving hook.
 */
function ResizablePanelGroup({
	className,
	orientation = "horizontal",
	autoSaveId,
	children,
	...props
}: Omit<GroupProps, "orientation"> & {
	orientation?: "horizontal" | "vertical";
	autoSaveId?: string;
}) {
	const layoutProps = autoSaveId
		? useDefaultLayout({ id: autoSaveId, storage: localStorage })
		: { defaultLayout: undefined, onLayoutChanged: undefined };

	return (
		<Group
			className={cn(
				"flex h-full w-full data-[orientation=vertical]:flex-col",
				className,
			)}
			orientation={orientation}
			defaultLayout={layoutProps.defaultLayout}
			onLayoutChanged={layoutProps.onLayoutChanged}
			{...props}
		>
			{children}
		</Group>
	);
}

const ResizablePanel = Panel;

function ResizableHandle({
	withHandle,
	className,
	...props
}: Omit<SeparatorProps, "children"> & {
	withHandle?: boolean;
}) {
	return (
		<Separator
			className={cn(
				"relative flex w-px items-center justify-center bg-border after:absolute after:inset-y-0 after:left-1/2 after:w-1 after:-translate-x-1/2 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring focus-visible:ring-offset-1 [data-orientation=vertical]&:h-px [data-orientation=vertical]&:w-full [data-orientation=vertical]&:after:left-0 [data-orientation=vertical]&:after:h-1 [data-orientation=vertical]&:after:w-full [data-orientation=vertical]&:after:-translate-y-1/2 [data-orientation=vertical]&:after:translate-x-0 data-[separator-active]:bg-ring",
				className,
			)}
			{...props}
		>
			{withHandle && (
				<div className="z-10 flex h-4 w-3 items-center justify-center rounded-sm border bg-border">
					<GripVertical className="h-2.5 w-2.5" />
				</div>
			)}
		</Separator>
	);
}

export { ResizablePanelGroup, ResizablePanel, ResizableHandle };
