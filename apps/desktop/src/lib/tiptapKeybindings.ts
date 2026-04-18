// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { toProseMirrorKey } from "@/lib/keybindings";
import { useKeybindingStore } from "@/stores/keybindingStore";
import { Extension } from "@tiptap/react";

/**
 * Tiptap extension that bridges the keybinding registry into ProseMirror keymaps.
 *
 * For bindings that need to interact with React state (insertLink, insertImage),
 * we dispatch CustomEvents on the editor's DOM element which NoteEditor listens for.
 */
export const ZoroKeymap = Extension.create({
	name: "zoroKeymap",

	addKeyboardShortcuts() {
		const bindings = useKeybindingStore.getState().bindings;
		const editorBindings = bindings.filter(
			(b) => b.scope === "editor" && b.key !== null,
		);

		const shortcuts: Record<
			string,
			(props: { editor: unknown }) => boolean
		> = {};

		for (const binding of editorBindings) {
			const pmKey = toProseMirrorKey(binding.key!);
			const actionId = binding.id;

			const headingMatch = actionId.match(/^editor\.heading(\d)$/);
			if (headingMatch) {
				const level = Number.parseInt(headingMatch[1], 10) as
					| 1
					| 2
					| 3
					| 4
					| 5
					| 6;
				// eslint-disable-next-line @typescript-eslint/no-explicit-any
				shortcuts[pmKey] = ({ editor }: { editor: any }) =>
					editor.chain().focus().toggleHeading({ level }).run();
			} else if (actionId === "editor.paragraph") {
				// eslint-disable-next-line @typescript-eslint/no-explicit-any
				shortcuts[pmKey] = ({ editor }: { editor: any }) =>
					editor.chain().focus().setParagraph().run();
			} else if (actionId === "editor.codeBlock") {
				// eslint-disable-next-line @typescript-eslint/no-explicit-any
				shortcuts[pmKey] = ({ editor }: { editor: any }) =>
					editor.chain().focus().toggleCodeBlock().run();
			} else if (actionId === "editor.blockquote") {
				// eslint-disable-next-line @typescript-eslint/no-explicit-any
				shortcuts[pmKey] = ({ editor }: { editor: any }) =>
					editor.chain().focus().toggleBlockquote().run();
			} else if (actionId === "editor.horizontalRule") {
				// eslint-disable-next-line @typescript-eslint/no-explicit-any
				shortcuts[pmKey] = ({ editor }: { editor: any }) =>
					editor.chain().focus().setHorizontalRule().run();
			} else if (actionId === "editor.insertLink") {
				// eslint-disable-next-line @typescript-eslint/no-explicit-any
				shortcuts[pmKey] = ({ editor }: { editor: any }) => {
					editor.view.dom.dispatchEvent(
						new CustomEvent("zoro-insert-link", { bubbles: true }),
					);
					return true;
				};
			} else if (actionId === "editor.insertImage") {
				// eslint-disable-next-line @typescript-eslint/no-explicit-any
				shortcuts[pmKey] = ({ editor }: { editor: any }) => {
					editor.view.dom.dispatchEvent(
						new CustomEvent("zoro-insert-image", { bubbles: true }),
					);
					return true;
				};
			}
		}

		return shortcuts;
	},
});
