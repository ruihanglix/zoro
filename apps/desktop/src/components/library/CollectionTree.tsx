// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import {
	ContextMenu,
	ContextMenuContent,
	ContextMenuItem,
	ContextMenuSeparator,
	ContextMenuTrigger,
} from "@/components/ui/context-menu";
import type { CollectionResponse } from "@/lib/commands";
import * as commands from "@/lib/commands";
import { registerDropTarget, unregisterDropTarget } from "@/lib/dragState";
import { cn } from "@/lib/utils";
import { useLibraryStore } from "@/stores/libraryStore";
import {
	ChevronDown,
	ChevronRight,
	Folder,
	FolderOpen,
	FolderPlus,
	Pencil,
	Trash2,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

interface CollectionTreeNode {
	collection: CollectionResponse;
	children: CollectionTreeNode[];
}

/** Build a tree from a flat list of collections using parent_id. */
export function buildCollectionTree(
	collections: CollectionResponse[],
): CollectionTreeNode[] {
	const map = new Map<string, CollectionTreeNode>();
	const roots: CollectionTreeNode[] = [];

	for (const col of collections) {
		map.set(col.id, { collection: col, children: [] });
	}

	for (const col of collections) {
		const node = map.get(col.id)!;
		if (col.parent_id && map.has(col.parent_id)) {
			map.get(col.parent_id)!.children.push(node);
		} else {
			roots.push(node);
		}
	}

	return roots;
}

interface CollectionTreeItemProps {
	node: CollectionTreeNode;
	depth: number;
	selectedId: string | null;
	onSelect: (id: string) => void;
}

export function CollectionTreeItem({
	node,
	depth,
	selectedId,
	onSelect,
}: CollectionTreeItemProps) {
	const { t } = useTranslation();
	const [expanded, setExpanded] = useState(false);
	const [renaming, setRenaming] = useState(false);
	const [addingSub, setAddingSub] = useState(false);
	const hasChildren = node.children.length > 0 || addingSub;
	const isSelected = selectedId === node.collection.id;
	const updateCollection = useLibraryStore((s) => s.updateCollection);
	const deleteCollection = useLibraryStore((s) => s.deleteCollection);
	const createCollection = useLibraryStore((s) => s.createCollection);
	const renameInputRef = useRef<HTMLInputElement>(null);
	const subInputRef = useRef<HTMLInputElement>(null);
	const subBlurGuardRef = useRef(false);
	const renameBlurGuardRef = useRef(false);

	const handleRename = () => {
		setRenaming(true);
		renameBlurGuardRef.current = true;
		setTimeout(() => {
			renameInputRef.current?.focus();
			renameInputRef.current?.select();
			setTimeout(() => {
				renameBlurGuardRef.current = false;
			}, 150);
		}, 50);
	};

	const handleRenameSubmit = (value: string) => {
		setRenaming(false);
		if (value.trim() && value.trim() !== node.collection.name) {
			updateCollection(node.collection.id, { name: value.trim() });
		}
	};

	const handleDelete = () => {
		if (
			confirm(t("collectionTree.deleteConfirm", { name: node.collection.name }))
		) {
			deleteCollection(node.collection.id);
		}
	};

	const fetchPapers = useLibraryStore((s) => s.fetchPapers);
	const fetchCollections = useLibraryStore((s) => s.fetchCollections);
	const buttonRef = useRef<HTMLButtonElement>(null);

	// Register as drop target for paper drag
	useEffect(() => {
		const el = buttonRef.current;
		if (!el) return;
		const targetId = `collection-${node.collection.id}`;
		registerDropTarget({
			id: targetId,
			type: "collection",
			label: node.collection.name,
			element: el,
			onDrop: async (paperId: string) => {
				console.log(
					"[Drop] Collection:",
					node.collection.name,
					"paperId:",
					paperId,
				);
				try {
					await commands.addPaperToCollection(paperId, node.collection.id);
					console.log(
						"[Drop] Successfully added paper to collection:",
						node.collection.name,
					);
					await fetchPapers();
					await fetchCollections();
				} catch (err) {
					console.error("[Drop] Failed to add paper to collection:", err);
				}
			},
		});
		return () => unregisterDropTarget(targetId);
	}, [node.collection.id, node.collection.name, fetchPapers, fetchCollections]);

	const handleAddSubCollection = () => {
		setExpanded(true);
		setAddingSub(true);
		subBlurGuardRef.current = true;
		setTimeout(() => {
			subInputRef.current?.focus();
			setTimeout(() => {
				subBlurGuardRef.current = false;
			}, 150);
		}, 50);
	};

	const handleSubCollectionSubmit = (value: string) => {
		setAddingSub(false);
		if (value.trim()) {
			createCollection(value.trim(), node.collection.id);
		}
	};

	return (
		<div>
			<ContextMenu>
				<ContextMenuTrigger asChild>
					<button
						ref={buttonRef}
						type="button"
						className={cn(
							"flex w-full items-center gap-1 rounded-sm px-1.5 py-1 text-sm hover:bg-accent/50 transition-colors text-left",
							isSelected && "bg-accent text-accent-foreground",
						)}
						style={{ paddingLeft: `${depth * 12 + 6}px` }}
						onClick={() => onSelect(node.collection.id)}
					>
						{/* Expand/collapse chevron */}
						{hasChildren ? (
							<span
								className="shrink-0 p-0.5 hover:bg-accent rounded-sm"
								onClick={(e) => {
									e.stopPropagation();
									setExpanded(!expanded);
								}}
								onKeyDown={() => {}}
								role="button"
								tabIndex={-1}
							>
								{expanded ? (
									<ChevronDown className="h-3.5 w-3.5" />
								) : (
									<ChevronRight className="h-3.5 w-3.5" />
								)}
							</span>
						) : (
							<span className="w-[18px] shrink-0" />
						)}
						{expanded || isSelected ? (
							<FolderOpen className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
						) : (
							<Folder className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
						)}
						{renaming ? (
							<input
								ref={renameInputRef}
								type="text"
								defaultValue={node.collection.name}
								className="flex-1 min-w-0 bg-transparent text-sm border-b border-primary outline-none py-0.5"
								onClick={(e) => e.stopPropagation()}
								onKeyDown={(e) => {
									e.stopPropagation();
									if (e.key === "Enter") {
										handleRenameSubmit(e.currentTarget.value);
									} else if (e.key === "Escape") {
										setRenaming(false);
									}
								}}
								onBlur={(e) => {
									if (renameBlurGuardRef.current) {
										e.currentTarget.focus();
										return;
									}
									handleRenameSubmit(e.currentTarget.value);
								}}
							/>
						) : (
							<>
								<span className="truncate flex-1 min-w-0">
									{node.collection.name}
								</span>
								<span className="text-[10px] text-muted-foreground shrink-0 tabular-nums">
									{node.collection.paper_count}
								</span>
							</>
						)}
					</button>
				</ContextMenuTrigger>
				<ContextMenuContent className="w-48">
					<ContextMenuItem onSelect={handleRename}>
						<Pencil className="mr-2 h-4 w-4" />
						{t("collectionTree.rename")}
					</ContextMenuItem>
					<ContextMenuItem onSelect={handleAddSubCollection}>
						<FolderPlus className="mr-2 h-4 w-4" />
						{t("collectionTree.newSubcollection")}
					</ContextMenuItem>
					<ContextMenuSeparator />
					<ContextMenuItem
						onSelect={handleDelete}
						className="text-destructive focus:text-destructive"
					>
						<Trash2 className="mr-2 h-4 w-4" />
						{t("common.delete")}
					</ContextMenuItem>
				</ContextMenuContent>
			</ContextMenu>

			{/* Children + inline sub-collection input */}
			{expanded && (
				<>
					{node.children.map((child) => (
						<CollectionTreeItem
							key={child.collection.id}
							node={child}
							depth={depth + 1}
							selectedId={selectedId}
							onSelect={onSelect}
						/>
					))}
					{addingSub && (
						<div
							className="flex items-center gap-1 px-1.5 py-1"
							style={{ paddingLeft: `${(depth + 1) * 12 + 6}px` }}
						>
							<span className="w-[18px] shrink-0" />
							<Folder className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
							<input
								ref={subInputRef}
								type="text"
								className="flex-1 min-w-0 bg-transparent text-sm border-b border-primary outline-none py-0.5"
								placeholder="Sub-collection name"
								onKeyDown={(e) => {
									if (e.key === "Enter") {
										handleSubCollectionSubmit(e.currentTarget.value);
									} else if (e.key === "Escape") {
										setAddingSub(false);
									}
								}}
								onBlur={(e) => {
									if (subBlurGuardRef.current) {
										e.currentTarget.focus();
										return;
									}
									handleSubCollectionSubmit(e.currentTarget.value);
								}}
							/>
						</div>
					)}
				</>
			)}
		</div>
	);
}
