// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { Node, mergeAttributes } from "@tiptap/react";

export interface CitationAttributes {
	format: "pdf" | "html";
	page: number;
	position: string;
	text: string;
}

declare module "@tiptap/react" {
	interface Commands<ReturnType> {
		citation: {
			insertCitation: (attrs: CitationAttributes) => ReturnType;
		};
	}
}

export const Citation = Node.create({
	name: "citation",
	group: "block",
	content: "text*",
	atom: true,

	addAttributes() {
		return {
			format: { default: "pdf" },
			page: { default: 0 },
			position: { default: "" },
			text: { default: "" },
		};
	},

	parseHTML() {
		return [
			{
				tag: "blockquote[data-cite]",
				getAttrs: (dom) => {
					const el = dom as HTMLElement;
					return {
						format: el.getAttribute("data-format") || "pdf",
						page: Number.parseInt(el.getAttribute("data-page") || "0", 10),
						position: el.getAttribute("data-position") || "",
						text: el.textContent || "",
					};
				},
			},
		];
	},

	renderHTML({ HTMLAttributes }) {
		return [
			"blockquote",
			mergeAttributes(HTMLAttributes, {
				"data-cite": "",
				"data-format": HTMLAttributes.format,
				"data-page": String(HTMLAttributes.page),
				"data-position": HTMLAttributes.position,
			}),
			HTMLAttributes.text,
		];
	},

	addNodeView() {
		return ({ node, editor }) => {
			const wrapper = document.createElement("div");
			wrapper.className =
				"citation-block my-2 rounded-md border-l-4 bg-muted/30 p-3 text-sm relative group";
			wrapper.style.borderLeftColor =
				node.attrs.format === "pdf" ? "#3b82f6" : "#22c55e";
			wrapper.contentEditable = "false";

			const header = document.createElement("div");
			header.className =
				"flex items-center gap-1.5 text-[10px] text-muted-foreground mb-1.5";

			const icon = document.createElement("span");
			icon.innerHTML =
				node.attrs.format === "pdf"
					? '<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/></svg>'
					: '<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20"/><path d="M2 12h20"/></svg>';
			header.appendChild(icon);

			const label = document.createElement("span");
			label.textContent =
				node.attrs.format === "pdf" ? `PDF p.${node.attrs.page}` : "HTML";
			header.appendChild(label);

			const jumpBtn = document.createElement("button");
			jumpBtn.className =
				"ml-auto opacity-0 group-hover:opacity-100 text-primary hover:underline text-[10px] transition-opacity cursor-pointer";
			jumpBtn.textContent = "Jump to source";
			jumpBtn.type = "button";
			jumpBtn.addEventListener("click", (e) => {
				e.preventDefault();
				e.stopPropagation();
				const event = new CustomEvent("citation-jump", {
					bubbles: true,
					detail: {
						format: node.attrs.format,
						page: node.attrs.page,
						position: node.attrs.position,
					},
				});
				wrapper.dispatchEvent(event);
			});
			header.appendChild(jumpBtn);

			wrapper.appendChild(header);

			const quote = document.createElement("div");
			quote.className = "text-xs text-foreground/80 italic leading-relaxed";
			quote.textContent = node.attrs.text;
			wrapper.appendChild(quote);

			const deleteBtn = document.createElement("button");
			deleteBtn.className =
				"absolute top-1 right-1 opacity-0 group-hover:opacity-100 text-muted-foreground hover:text-destructive transition-opacity p-0.5 rounded";
			deleteBtn.type = "button";
			deleteBtn.innerHTML =
				'<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>';
			deleteBtn.addEventListener("click", (e) => {
				e.preventDefault();
				e.stopPropagation();
				const pos = editor.view.posAtDOM(wrapper, 0);
				editor
					.chain()
					.focus()
					.deleteRange({ from: pos, to: pos + node.nodeSize })
					.run();
			});
			wrapper.appendChild(deleteBtn);

			return { dom: wrapper };
		};
	},

	addCommands() {
		return {
			insertCitation:
				(attrs) =>
				({ commands }) => {
					return commands.insertContent({
						type: this.name,
						attrs,
					});
				},
		};
	},
});
