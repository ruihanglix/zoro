// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { Node, mergeAttributes } from "@tiptap/react";

export interface PaperLinkAttributes {
	paperId: string;
	paperTitle: string;
	format?: string;
	page?: number;
	position?: string;
}

declare module "@tiptap/react" {
	interface Commands<ReturnType> {
		paperLink: {
			insertPaperLink: (attrs: PaperLinkAttributes) => ReturnType;
		};
	}
}

const FILE_ICON_SVG =
	'<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/></svg>';

const ANCHOR_ICON_SVG =
	'<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="5" r="3"/><line x1="12" x2="12" y1="22" y2="8"/><path d="M5 12H2a10 10 0 0 0 20 0h-3"/></svg>';

export const PaperLink = Node.create({
	name: "paperLink",
	group: "inline",
	inline: true,
	atom: true,

	addAttributes() {
		return {
			paperId: { default: "" },
			paperTitle: { default: "" },
			format: { default: null },
			page: { default: null },
			position: { default: null },
		};
	},

	parseHTML() {
		return [
			{
				tag: "span[data-paper-link]",
				getAttrs: (dom) => {
					const el = dom as HTMLElement;
					const page = el.getAttribute("data-page");
					return {
						paperId: el.getAttribute("data-paper-id") || "",
						paperTitle: el.textContent || "",
						format: el.getAttribute("data-format") || null,
						page: page ? Number.parseInt(page, 10) : null,
						position: el.getAttribute("data-position") || null,
					};
				},
			},
		];
	},

	renderHTML({ HTMLAttributes }) {
		const attrs: Record<string, string> = {
			"data-paper-link": "",
			"data-paper-id": HTMLAttributes.paperId,
		};
		if (HTMLAttributes.format) attrs["data-format"] = HTMLAttributes.format;
		if (HTMLAttributes.page != null)
			attrs["data-page"] = String(HTMLAttributes.page);
		if (HTMLAttributes.position)
			attrs["data-position"] = HTMLAttributes.position;

		return [
			"span",
			mergeAttributes(HTMLAttributes, attrs),
			HTMLAttributes.paperTitle,
		];
	},

	addNodeView() {
		return ({ node }) => {
			const chip = document.createElement("span");
			chip.className =
				"paper-link-chip inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-xs font-medium cursor-pointer transition-colors select-none";
			chip.style.cssText =
				"background: hsl(var(--primary) / 0.1); color: hsl(var(--primary)); border: 1px solid hsl(var(--primary) / 0.2);";
			chip.contentEditable = "false";

			chip.addEventListener("mouseenter", () => {
				chip.style.background = "hsl(var(--primary) / 0.2)";
				chip.style.borderColor = "hsl(var(--primary) / 0.4)";
			});
			chip.addEventListener("mouseleave", () => {
				chip.style.background = "hsl(var(--primary) / 0.1)";
				chip.style.borderColor = "hsl(var(--primary) / 0.2)";
			});

			const icon = document.createElement("span");
			icon.className = "shrink-0 flex items-center";
			icon.innerHTML = FILE_ICON_SVG;
			chip.appendChild(icon);

			const title = document.createElement("span");
			title.className = "truncate max-w-[200px]";
			title.textContent = node.attrs.paperTitle || "Untitled";
			chip.appendChild(title);

			const hasAnchor = node.attrs.position || node.attrs.page != null;
			if (hasAnchor) {
				const anchor = document.createElement("span");
				anchor.className = "shrink-0 flex items-center opacity-60";
				anchor.innerHTML = ANCHOR_ICON_SVG;
				anchor.title =
					node.attrs.format === "pdf"
						? `p.${node.attrs.page}`
						: "HTML position";
				chip.appendChild(anchor);
			}

			chip.addEventListener("click", (e) => {
				e.preventDefault();
				e.stopPropagation();
				const event = new CustomEvent("paper-link-click", {
					bubbles: true,
					detail: {
						paperId: node.attrs.paperId,
						paperTitle: node.attrs.paperTitle,
						format: node.attrs.format,
						page: node.attrs.page,
						position: node.attrs.position,
					},
				});
				chip.dispatchEvent(event);
			});

			return { dom: chip };
		};
	},

	addCommands() {
		return {
			insertPaperLink:
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
