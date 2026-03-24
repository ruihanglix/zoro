// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import * as commands from "@/lib/commands";
import type {
	AgentInfoResponse,
	ChatSessionMeta,
	ConfigOptionInfo,
	ImageInput,
	ProviderInfo,
	SystemPromptPreset,
} from "@/lib/commands";
import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";

export const CHAT_AGENT_NAME = "__chat__";

export interface ChatMessage {
	id: string;
	role: "user" | "agent" | "thought" | "tool" | "plan" | "error" | "separator";
	text: string;
	images?: { base64Data: string; mimeType: string }[];
	toolCallId?: string;
	toolTitle?: string;
	toolStatus?: string;
	toolArguments?: string;
	toolResult?: string;
	needsConfirmation?: boolean;
	planEntries?: { content: string; status: string }[];
	timestamp: number;
	// Tree structure fields for conversation branching
	parentId: string | null;
	childrenIds: string[];
}

interface AgentUpdate {
	kind: string;
	session_id: string;
	text?: string;
	tool_call_id?: string;
	title?: string;
	status?: string;
	content_text?: string;
	raw_input?: string;
	raw_output?: string;
	stop_reason?: string;
	message?: string;
	entries?: { content: string; status: string }[];
	config_options?: ConfigOptionInfo[];
}

interface ChatUpdate {
	kind: string;
	text?: string;
	tool_call_id?: string;
	name?: string;
	arguments?: string;
	needs_confirmation?: boolean;
	result?: string;
	is_error?: boolean;
	stop_reason?: string;
	message?: string;
}

interface AgentState {
	agents: AgentInfoResponse[];
	activeAgentName: string | null;
	sessionId: string | null;
	activeCwd: string | null;
	// Tree-based message storage
	messageMap: Record<string, ChatMessage>;
	rootMessageIds: string[];
	activeLeafId: string | null;
	streaming: boolean;
	connecting: boolean;
	error: string | null;
	configOptions: ConfigOptionInfo[];

	chatSessions: ChatSessionMeta[];
	activeChatId: string | null;

	// Chat-mode specific
	chatPresets: SystemPromptPreset[];
	chatActivePreset: string;
	chatConfirmWrites: boolean;
	chatModel: string;
	chatProviderId: string | null;
	chatProviders: ProviderInfo[];
	chatConfigLoaded: boolean;
	chatPaperId: string | null;

	fetchAgents: () => Promise<void>;
	startSession: (agentName: string, cwd?: string) => Promise<void>;
	sendPrompt: (text: string, images?: ImageInput[]) => Promise<void>;
	cancelPrompt: () => Promise<void>;
	stopSession: () => Promise<void>;
	clearMessages: () => void;
	setConfigOption: (configId: string, value: string) => Promise<void>;

	fetchChatSessions: () => Promise<void>;
	newChat: () => void;
	switchChat: (chatId: string) => Promise<void>;
	deleteChat: (chatId: string) => Promise<void>;
	saveCurrentChat: () => Promise<void>;

	// Chat-mode actions
	startChat: (paperId?: string, cwd?: string) => Promise<void>;
	confirmTool: (approved: boolean) => Promise<void>;
	fetchChatConfig: () => Promise<void>;
	setChatActivePreset: (name: string) => void;
	setChatModel: (model: string) => void;
	setChatProvider: (providerId: string | null, model?: string) => void;

	// Branching actions
	editAndRegenerate: (
		messageId: string,
		newText: string,
		images?: ImageInput[],
	) => Promise<void>;
	regenerateLastResponse: () => Promise<void>;
	switchSibling: (messageId: string, direction: "prev" | "next") => void;

	// Computed: get active branch as flat array for rendering
	getActiveBranch: () => ChatMessage[];
	// Get sibling info for branch navigation UI
	getSiblingInfo: (
		messageId: string,
	) => { index: number; total: number } | null;
}

let eventUnlisten: (() => void) | null = null;
let chatStartLock = false;
let msgCounter = 0;
let saveTimer: ReturnType<typeof setTimeout> | null = null;

function nextMsgId(): string {
	return `msg-${++msgCounter}-${Date.now()}`;
}

function generateChatId(): string {
	return `chat-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function chatTitle(messages: ChatMessage[]): string {
	const first = messages.find((m) => m.role === "user");
	if (!first) return "New chat";
	const text = first.text.trim();
	return text.length > 60 ? `${text.slice(0, 57)}...` : text;
}

function scheduleSave(get: () => AgentState) {
	if (saveTimer) clearTimeout(saveTimer);
	saveTimer = setTimeout(() => {
		get().saveCurrentChat();
	}, 1500);
}

function findLastIndex<T>(arr: T[], predicate: (item: T) => boolean): number {
	for (let i = arr.length - 1; i >= 0; i--) {
		if (predicate(arr[i])) return i;
	}
	return -1;
}

function buildContextSummary(messages: ChatMessage[]): string | null {
	const relevant = messages.filter(
		(m) => m.role === "user" || m.role === "agent",
	);
	if (relevant.length === 0) return null;
	return relevant
		.map((m) => {
			const role = m.role === "user" ? "User" : "Assistant";
			return `${role}: ${m.text}`;
		})
		.join("\n");
}

// ── Tree helper functions ────────────────────────────────────────────────────

/**
 * Walk from a given message up to the root, returning the path in root→leaf order.
 */
function getPathToRoot(
	messageMap: Record<string, ChatMessage>,
	leafId: string,
): ChatMessage[] {
	const path: ChatMessage[] = [];
	let currentId: string | null = leafId;
	while (currentId && messageMap[currentId]) {
		path.unshift(messageMap[currentId]);
		currentId = messageMap[currentId].parentId;
	}
	return path;
}

/**
 * Walk from root down following the "active" child at each level (last child = active).
 * This gives us the currently visible conversation branch.
 */
function getActiveBranchFromRoots(
	messageMap: Record<string, ChatMessage>,
	rootIds: string[],
	activeLeafId: string | null,
): ChatMessage[] {
	if (activeLeafId && messageMap[activeLeafId]) {
		return getPathToRoot(messageMap, activeLeafId);
	}
	// Fallback: follow last child from last root
	if (rootIds.length === 0) return [];
	const branch: ChatMessage[] = [];
	let currentId: string | null = rootIds[rootIds.length - 1];
	while (currentId && messageMap[currentId]) {
		const node: ChatMessage = messageMap[currentId];
		branch.push(node);
		if (node.childrenIds.length === 0) break;
		currentId = node.childrenIds[node.childrenIds.length - 1];
	}
	return branch;
}

/**
 * Add a new message to the tree, linking it as a child of parentId.
 * Returns the updated messageMap and rootMessageIds.
 */
function addMessageToTree(
	messageMap: Record<string, ChatMessage>,
	rootIds: string[],
	msg: ChatMessage,
): { messageMap: Record<string, ChatMessage>; rootMessageIds: string[] } {
	const newMap = { ...messageMap, [msg.id]: msg };
	let newRootIds = rootIds;

	if (msg.parentId && newMap[msg.parentId]) {
		const parent = newMap[msg.parentId];
		if (!parent.childrenIds.includes(msg.id)) {
			newMap[msg.parentId] = {
				...parent,
				childrenIds: [...parent.childrenIds, msg.id],
			};
		}
	} else if (!msg.parentId) {
		newRootIds = [...rootIds, msg.id];
	}

	return { messageMap: newMap, rootMessageIds: newRootIds };
}

/**
 * Convert a flat messages array (legacy format) into tree structure.
 * Each message becomes a child of the previous one (linear chain).
 */
function flatToTree(flatMessages: ChatMessage[]): {
	messageMap: Record<string, ChatMessage>;
	rootMessageIds: string[];
	activeLeafId: string | null;
} {
	const messageMap: Record<string, ChatMessage> = {};
	const rootMessageIds: string[] = [];
	let prevId: string | null = null;

	for (const msg of flatMessages) {
		const treeMsg: ChatMessage = {
			...msg,
			parentId: msg.parentId ?? prevId,
			childrenIds: msg.childrenIds ?? [],
		};
		messageMap[treeMsg.id] = treeMsg;

		if (!treeMsg.parentId) {
			rootMessageIds.push(treeMsg.id);
		} else if (messageMap[treeMsg.parentId]) {
			const parent = messageMap[treeMsg.parentId];
			if (!parent.childrenIds.includes(treeMsg.id)) {
				parent.childrenIds = [...parent.childrenIds, treeMsg.id];
			}
		}

		prevId = treeMsg.id;
	}

	const activeLeafId =
		flatMessages.length > 0 ? flatMessages[flatMessages.length - 1].id : null;

	return { messageMap, rootMessageIds, activeLeafId };
}

/**
 * Serialize tree into flat array (active branch first, then remaining messages).
 * This ensures backward compatibility when saving.
 */
function treeToFlat(
	messageMap: Record<string, ChatMessage>,
	_rootIds: string[],
	_activeLeafId: string | null,
): ChatMessage[] {
	// First, get all messages preserving tree fields
	const allMessages = Object.values(messageMap);
	// Sort by timestamp for stable ordering
	allMessages.sort((a, b) => a.timestamp - b.timestamp);
	return allMessages;
}

/**
 * Find the deepest leaf node starting from a given message, following the last child.
 */
function findDeepestLeaf(
	messageMap: Record<string, ChatMessage>,
	startId: string,
): string {
	let currentId = startId;
	while (messageMap[currentId]?.childrenIds.length > 0) {
		const children = messageMap[currentId].childrenIds;
		currentId = children[children.length - 1];
	}
	return currentId;
}

export const useAgentStore = create<AgentState>((set, get) => ({
	agents: [],
	activeAgentName: null,
	sessionId: null,
	activeCwd: null,
	messageMap: {},
	rootMessageIds: [],
	activeLeafId: null,
	streaming: false,
	connecting: false,
	error: null,
	configOptions: [],

	chatSessions: [],
	activeChatId: null,

	chatPresets: [],
	chatActivePreset: "",
	chatConfirmWrites: true,
	chatModel: "",
	chatProviderId: null,
	chatProviders: [],
	chatConfigLoaded: false,
	chatPaperId: null,

	// ── Computed: active branch for rendering ────────────────────────────────

	getActiveBranch: () => {
		const { messageMap, rootMessageIds, activeLeafId } = get();
		return getActiveBranchFromRoots(messageMap, rootMessageIds, activeLeafId);
	},

	getSiblingInfo: (messageId: string) => {
		const { messageMap } = get();
		const msg = messageMap[messageId];
		if (!msg) return null;

		if (!msg.parentId) {
			// Root-level message
			const { rootMessageIds } = get();
			const idx = rootMessageIds.indexOf(messageId);
			if (idx === -1 || rootMessageIds.length <= 1) return null;
			return { index: idx, total: rootMessageIds.length };
		}

		const parent = messageMap[msg.parentId];
		if (!parent || parent.childrenIds.length <= 1) return null;

		const idx = parent.childrenIds.indexOf(messageId);
		if (idx === -1) return null;
		return { index: idx, total: parent.childrenIds.length };
	},

	// ── Branch navigation ────────────────────────────────────────────────────

	switchSibling: (messageId: string, direction: "prev" | "next") => {
		const { messageMap, rootMessageIds } = get();
		const msg = messageMap[messageId];
		if (!msg) return;

		let siblingIds: string[];
		if (!msg.parentId) {
			siblingIds = rootMessageIds;
		} else {
			const parent = messageMap[msg.parentId];
			if (!parent) return;
			siblingIds = parent.childrenIds;
		}

		const currentIdx = siblingIds.indexOf(messageId);
		if (currentIdx === -1) return;

		const newIdx = direction === "prev" ? currentIdx - 1 : currentIdx + 1;
		if (newIdx < 0 || newIdx >= siblingIds.length) return;

		const newSiblingId = siblingIds[newIdx];
		// Navigate to the deepest leaf of the new sibling's subtree
		const newLeafId = findDeepestLeaf(messageMap, newSiblingId);
		set({ activeLeafId: newLeafId });
		scheduleSave(get);
	},

	// ── Edit and regenerate ──────────────────────────────────────────────────

	editAndRegenerate: async (
		messageId: string,
		newText: string,
		images?: ImageInput[],
	) => {
		const {
			activeAgentName,
			messageMap,
			rootMessageIds,
			chatPresets,
			chatActivePreset,
			chatPaperId,
			chatModel,
			chatProviderId,
			chatConfirmWrites,
		} = get();
		if (!activeAgentName) return;

		const originalMsg = messageMap[messageId];
		if (!originalMsg || originalMsg.role !== "user") return;

		// Create a new user message as sibling (same parent as original)
		const newUserMsg: ChatMessage = {
			id: nextMsgId(),
			role: "user",
			text: newText,
			images,
			timestamp: Date.now(),
			parentId: originalMsg.parentId,
			childrenIds: [],
		};

		// Add the new user message to the tree
		const { messageMap: updatedMap, rootMessageIds: updatedRoots } =
			addMessageToTree(messageMap, rootMessageIds, newUserMsg);

		set({
			messageMap: updatedMap,
			rootMessageIds: updatedRoots,
			activeLeafId: newUserMsg.id,
			streaming: true,
			error: null,
		});

		scheduleSave(get);

		// Build history from the branch leading to this new message
		const branchPath = getPathToRoot(updatedMap, newUserMsg.id);
		const lastSepIdx = findLastIndex(branchPath, (m) => m.role === "separator");
		const currentSegment =
			lastSepIdx >= 0 ? branchPath.slice(lastSepIdx + 1) : branchPath;
		const priorContext = lastSepIdx >= 0 ? branchPath.slice(0, lastSepIdx) : [];

		if (activeAgentName === CHAT_AGENT_NAME) {
			const historyMessages = currentSegment
				.filter((m) => m.role === "user" || m.role === "agent")
				.map((m) => ({
					role: m.role === "agent" ? "assistant" : "user",
					content: m.text,
				}));

			if (priorContext.length > 0 && historyMessages.length <= 1) {
				const summary = buildContextSummary(priorContext);
				if (summary) {
					historyMessages.unshift({
						role: "user",
						content: `[Previous conversation context]\n${summary}\n[End of context]`,
					});
					historyMessages.unshift({
						role: "assistant",
						content:
							"I have the context from your previous conversation. How can I help?",
					});
				}
			}

			try {
				await commands.chatSendMessage({
					messages: historyMessages.slice(0, -1), // exclude the last user msg (it's userMessage)
					userMessage: newText,
					images: images && images.length > 0 ? images : undefined,
					systemPrompt:
						chatPresets.find((p) => p.name === chatActivePreset)?.prompt ?? "",
					paperId: chatPaperId ?? null,
					model: chatModel || null,
					providerId: chatProviderId ?? null,
					confirmWrites: chatConfirmWrites,
				});
			} catch (e) {
				const errMsg: ChatMessage = {
					id: nextMsgId(),
					role: "error",
					text: String(e),
					timestamp: Date.now(),
					parentId: newUserMsg.id,
					childrenIds: [],
				};
				const result = addMessageToTree(
					get().messageMap,
					get().rootMessageIds,
					errMsg,
				);
				set({
					streaming: false,
					messageMap: result.messageMap,
					rootMessageIds: result.rootMessageIds,
					activeLeafId: errMsg.id,
				});
			}
		} else {
			// ACP agent
			let prompt = newText;
			if (
				priorContext.length > 0 &&
				currentSegment.filter((m) => m.role === "user").length <= 1
			) {
				const summary = buildContextSummary(priorContext);
				if (summary) {
					prompt = `[Previous conversation context]\n${summary}\n[End of context]\n\n${newText}`;
				}
			}

			try {
				await commands.acpSendPrompt(activeAgentName, prompt, images);
			} catch (e) {
				const errMsg: ChatMessage = {
					id: nextMsgId(),
					role: "error",
					text: String(e),
					timestamp: Date.now(),
					parentId: newUserMsg.id,
					childrenIds: [],
				};
				const result = addMessageToTree(
					get().messageMap,
					get().rootMessageIds,
					errMsg,
				);
				set({
					streaming: false,
					messageMap: result.messageMap,
					rootMessageIds: result.rootMessageIds,
					activeLeafId: errMsg.id,
				});
			}
		}
	},

	regenerateLastResponse: async () => {
		const {
			activeAgentName,
			messageMap,
			rootMessageIds,
			activeLeafId,
			chatPresets,
			chatActivePreset,
			chatPaperId,
			chatModel,
			chatProviderId,
			chatConfirmWrites,
		} = get();
		if (!activeAgentName || !activeLeafId) return;

		// Find the last agent message in the active branch, then find its parent (user message)
		const branch = getActiveBranchFromRoots(
			messageMap,
			rootMessageIds,
			activeLeafId,
		);
		// Find the last user message in the branch
		const lastUserIdx = findLastIndex(branch, (m) => m.role === "user");
		if (lastUserIdx === -1) return;

		const lastUserMsg = branch[lastUserIdx];

		// Create a placeholder to indicate we're regenerating from this user message
		// The event handler will create the new agent response as a new child of the user msg
		// We just need to set activeLeafId to the user message so new responses attach there
		set({
			activeLeafId: lastUserMsg.id,
			streaming: true,
			error: null,
		});

		// Build history up to (but not including) the user message that needs regeneration
		const branchToUser = getPathToRoot(messageMap, lastUserMsg.id);
		const lastSepIdx = findLastIndex(
			branchToUser,
			(m) => m.role === "separator",
		);
		const currentSegment =
			lastSepIdx >= 0 ? branchToUser.slice(lastSepIdx + 1) : branchToUser;
		const priorContext =
			lastSepIdx >= 0 ? branchToUser.slice(0, lastSepIdx) : [];

		if (activeAgentName === CHAT_AGENT_NAME) {
			const historyMessages = currentSegment
				.filter((m) => m.role === "user" || m.role === "agent")
				.map((m) => ({
					role: m.role === "agent" ? "assistant" : "user",
					content: m.text,
				}));

			if (priorContext.length > 0 && historyMessages.length <= 1) {
				const summary = buildContextSummary(priorContext);
				if (summary) {
					historyMessages.unshift({
						role: "user",
						content: `[Previous conversation context]\n${summary}\n[End of context]`,
					});
					historyMessages.unshift({
						role: "assistant",
						content:
							"I have the context from your previous conversation. How can I help?",
					});
				}
			}

			try {
				await commands.chatSendMessage({
					messages: historyMessages.slice(0, -1),
					userMessage: lastUserMsg.text,
					images:
						lastUserMsg.images && lastUserMsg.images.length > 0
							? lastUserMsg.images
							: undefined,
					systemPrompt:
						chatPresets.find((p) => p.name === chatActivePreset)?.prompt ?? "",
					paperId: chatPaperId ?? null,
					model: chatModel || null,
					providerId: chatProviderId ?? null,
					confirmWrites: chatConfirmWrites,
				});
			} catch (e) {
				const errMsg: ChatMessage = {
					id: nextMsgId(),
					role: "error",
					text: String(e),
					timestamp: Date.now(),
					parentId: lastUserMsg.id,
					childrenIds: [],
				};
				const result = addMessageToTree(
					get().messageMap,
					get().rootMessageIds,
					errMsg,
				);
				set({
					streaming: false,
					messageMap: result.messageMap,
					rootMessageIds: result.rootMessageIds,
					activeLeafId: errMsg.id,
				});
			}
		} else {
			// ACP agent
			let prompt = lastUserMsg.text;
			if (
				priorContext.length > 0 &&
				currentSegment.filter((m) => m.role === "user").length <= 1
			) {
				const summary = buildContextSummary(priorContext);
				if (summary) {
					prompt = `[Previous conversation context]\n${summary}\n[End of context]\n\n${lastUserMsg.text}`;
				}
			}

			try {
				await commands.acpSendPrompt(
					activeAgentName,
					prompt,
					lastUserMsg.images,
				);
			} catch (e) {
				const errMsg: ChatMessage = {
					id: nextMsgId(),
					role: "error",
					text: String(e),
					timestamp: Date.now(),
					parentId: lastUserMsg.id,
					childrenIds: [],
				};
				const result = addMessageToTree(
					get().messageMap,
					get().rootMessageIds,
					errMsg,
				);
				set({
					streaming: false,
					messageMap: result.messageMap,
					rootMessageIds: result.rootMessageIds,
					activeLeafId: errMsg.id,
				});
			}
		}
	},

	// ── Core actions ─────────────────────────────────────────────────────────

	fetchAgents: async () => {
		try {
			const agents = await commands.acpListAgents();
			set({ agents });
		} catch (e) {
			console.error("Failed to fetch agents", e);
		}
	},

	fetchChatConfig: async () => {
		try {
			const cfg = await commands.chatGetConfig();
			const providers = cfg.providers ?? [];
			set({
				chatPresets: cfg.presets,
				chatActivePreset: cfg.activePreset,
				chatConfirmWrites: cfg.confirmToolCalls,
				chatModel: cfg.defaultModel,
				chatProviders: providers,
				chatProviderId: providers.length > 0 ? providers[0].id : null,
				chatConfigLoaded: true,
			});
		} catch (e) {
			console.error("Failed to fetch chat config", e);
		}
	},

	startChat: async (paperId?: string, cwd?: string) => {
		const newPaperId = paperId ?? null;

		if (chatStartLock) return;

		if (get().activeAgentName === CHAT_AGENT_NAME && eventUnlisten) {
			if (get().chatPaperId === newPaperId) {
				const newCwd = cwd ?? null;
				if (get().activeCwd !== newCwd) {
					set({ activeCwd: newCwd });
				}
				return;
			}
		}

		chatStartLock = true;
		try {
			const {
				activeChatId,
				messageMap,
				rootMessageIds,
				activeLeafId,
				chatConfigLoaded,
				activeAgentName,
			} = get();

			if (!chatConfigLoaded) {
				await get().fetchChatConfig();
			}

			if (eventUnlisten) {
				eventUnlisten();
				eventUnlisten = null;
			}

			const unlisten = await listen<ChatUpdate>("chat-update", (event) => {
				handleChatUpdate(event.payload, set, get);
			});
			eventUnlisten = unlisten;

			// Preserve messages with a separator when switching from ACP to Chat
			const switchingFromAcp =
				activeAgentName && activeAgentName !== CHAT_AGENT_NAME;

			let newMap = switchingFromAcp ? { ...messageMap } : {};
			let newRoots = switchingFromAcp ? [...rootMessageIds] : [];
			let newLeaf = switchingFromAcp ? activeLeafId : null;

			if (switchingFromAcp) {
				const sepMsg: ChatMessage = {
					id: nextMsgId(),
					role: "separator",
					text: "Switched to Chat mode",
					timestamp: Date.now(),
					parentId: activeLeafId,
					childrenIds: [],
				};
				const result = addMessageToTree(newMap, newRoots, sepMsg);
				newMap = result.messageMap;
				newRoots = result.rootMessageIds;
				newLeaf = sepMsg.id;
			}

			const chatId = activeChatId ?? generateChatId();
			set({
				activeAgentName: CHAT_AGENT_NAME,
				sessionId: CHAT_AGENT_NAME,
				activeCwd: cwd ?? null,
				messageMap: newMap,
				rootMessageIds: newRoots,
				activeLeafId: newLeaf,
				connecting: false,
				error: null,
				configOptions: [],
				activeChatId: chatId,
				chatPaperId: paperId ?? null,
			});
		} finally {
			chatStartLock = false;
		}
	},

	startSession: async (agentName: string, cwd?: string) => {
		if (agentName === CHAT_AGENT_NAME) {
			return get().startChat(undefined, cwd);
		}

		const {
			activeChatId,
			messageMap,
			rootMessageIds,
			activeLeafId,
			activeAgentName,
		} = get();

		// Preserve messages with a separator when switching modes
		const switchingMode =
			activeAgentName &&
			activeAgentName !== agentName &&
			Object.keys(messageMap).length > 0;

		let newMap = switchingMode ? { ...messageMap } : {};
		let newRoots = switchingMode ? [...rootMessageIds] : [];
		let newLeaf = switchingMode ? activeLeafId : null;

		if (switchingMode) {
			const sepMsg: ChatMessage = {
				id: nextMsgId(),
				role: "separator",
				text: `Switched to ${agentName}`,
				timestamp: Date.now(),
				parentId: activeLeafId,
				childrenIds: [],
			};
			const result = addMessageToTree(newMap, newRoots, sepMsg);
			newMap = result.messageMap;
			newRoots = result.rootMessageIds;
			newLeaf = sepMsg.id;
		}

		const chatId = activeChatId ?? generateChatId();
		set({
			activeAgentName: agentName,
			sessionId: null,
			activeCwd: cwd ?? null,
			messageMap: newMap,
			rootMessageIds: newRoots,
			activeLeafId: newLeaf,
			connecting: true,
			error: null,
			configOptions: [],
			activeChatId: chatId,
			chatPaperId: null,
		});
		try {
			if (eventUnlisten) {
				eventUnlisten();
				eventUnlisten = null;
			}
			const unlisten = await listen<AgentUpdate>(
				"acp-session-update",
				(event) => {
					handleAgentUpdate(event.payload, set, get);
				},
			);
			eventUnlisten = unlisten;

			const sessionId = await commands.acpStartSession(agentName, cwd);
			set({ sessionId, connecting: false });
		} catch (e) {
			set({ connecting: false, error: String(e) });
		}
	},

	sendPrompt: async (text: string, images?: ImageInput[]) => {
		const {
			activeAgentName,
			messageMap,
			rootMessageIds,
			activeLeafId,
			chatPresets,
			chatActivePreset,
			chatPaperId,
			chatModel,
			chatProviderId,
			chatConfirmWrites,
		} = get();
		if (!activeAgentName) return;

		const userMsg: ChatMessage = {
			id: nextMsgId(),
			role: "user",
			text,
			images,
			timestamp: Date.now(),
			parentId: activeLeafId,
			childrenIds: [],
		};

		const { messageMap: newMap, rootMessageIds: newRoots } = addMessageToTree(
			messageMap,
			rootMessageIds,
			userMsg,
		);

		set({
			messageMap: newMap,
			rootMessageIds: newRoots,
			activeLeafId: userMsg.id,
			streaming: true,
			error: null,
		});

		scheduleSave(get);

		// Build history from the branch leading to this new message
		const branchPath = getPathToRoot(newMap, userMsg.id);
		const lastSepIdx = findLastIndex(branchPath, (m) => m.role === "separator");
		const currentSegment =
			lastSepIdx >= 0 ? branchPath.slice(lastSepIdx + 1) : branchPath;
		const priorContext = lastSepIdx >= 0 ? branchPath.slice(0, lastSepIdx) : [];

		if (activeAgentName === CHAT_AGENT_NAME) {
			const historyMessages = currentSegment
				.filter((m) => m.role === "user" || m.role === "agent")
				.map((m) => ({
					role: m.role === "agent" ? "assistant" : "user",
					content: m.text,
				}));

			// Inject prior context as a prefixed assistant/user exchange
			if (priorContext.length > 0 && historyMessages.length <= 1) {
				const summary = buildContextSummary(priorContext);
				if (summary) {
					historyMessages.unshift({
						role: "user",
						content: `[Previous conversation context]\n${summary}\n[End of context]`,
					});
					historyMessages.unshift({
						role: "assistant",
						content:
							"I have the context from your previous conversation. How can I help?",
					});
				}
			}

			try {
				await commands.chatSendMessage({
					messages: historyMessages.slice(0, -1),
					userMessage: text,
					images: images && images.length > 0 ? images : undefined,
					systemPrompt:
						chatPresets.find((p) => p.name === chatActivePreset)?.prompt ?? "",
					paperId: chatPaperId ?? null,
					model: chatModel || null,
					providerId: chatProviderId ?? null,
					confirmWrites: chatConfirmWrites,
				});
			} catch (e) {
				const errMsg: ChatMessage = {
					id: nextMsgId(),
					role: "error",
					text: String(e),
					timestamp: Date.now(),
					parentId: userMsg.id,
					childrenIds: [],
				};
				const result = addMessageToTree(
					get().messageMap,
					get().rootMessageIds,
					errMsg,
				);
				set({
					streaming: false,
					messageMap: result.messageMap,
					rootMessageIds: result.rootMessageIds,
					activeLeafId: errMsg.id,
				});
			}
		} else {
			// For ACP agents, inject prior context in the first prompt of the segment
			let prompt = text;
			if (
				priorContext.length > 0 &&
				currentSegment.filter((m) => m.role === "user").length <= 1
			) {
				const summary = buildContextSummary(priorContext);
				if (summary) {
					prompt = `[Previous conversation context]\n${summary}\n[End of context]\n\n${text}`;
				}
			}

			try {
				await commands.acpSendPrompt(activeAgentName, prompt, images);
			} catch (e) {
				const errMsg: ChatMessage = {
					id: nextMsgId(),
					role: "error",
					text: String(e),
					timestamp: Date.now(),
					parentId: userMsg.id,
					childrenIds: [],
				};
				const result = addMessageToTree(
					get().messageMap,
					get().rootMessageIds,
					errMsg,
				);
				set({
					streaming: false,
					messageMap: result.messageMap,
					rootMessageIds: result.rootMessageIds,
					activeLeafId: errMsg.id,
				});
			}
		}
	},

	cancelPrompt: async () => {
		const { activeAgentName } = get();
		if (!activeAgentName) return;
		try {
			if (activeAgentName === CHAT_AGENT_NAME) {
				await commands.chatCancel();
			} else {
				await commands.acpCancelPrompt(activeAgentName);
			}
		} catch (e) {
			console.error("Cancel failed", e);
		}
		set({ streaming: false });
	},

	confirmTool: async (approved: boolean) => {
		try {
			await commands.chatConfirmTool(approved);
		} catch (e) {
			console.error("Confirm tool failed", e);
		}
	},

	stopSession: async () => {
		const { activeAgentName, messageMap, activeChatId } = get();
		if (activeChatId && Object.keys(messageMap).length > 0) {
			await get().saveCurrentChat();
		}

		if (activeAgentName === CHAT_AGENT_NAME) {
			try {
				await commands.chatCancel();
			} catch (_) {
				// ignore
			}
		} else if (activeAgentName) {
			try {
				await commands.acpStopSession(activeAgentName);
			} catch (e) {
				console.error("Stop session failed", e);
			}
		}

		if (eventUnlisten) {
			eventUnlisten();
			eventUnlisten = null;
		}
		set({
			activeAgentName: null,
			sessionId: null,
			activeCwd: null,
			streaming: false,
			configOptions: [],
			activeChatId: null,
			chatPaperId: null,
		});
	},

	clearMessages: () =>
		set({ messageMap: {}, rootMessageIds: [], activeLeafId: null }),

	setConfigOption: async (configId: string, value: string) => {
		const { activeAgentName, configOptions } = get();
		if (!activeAgentName) return;

		const prev = configOptions;
		set({
			configOptions: configOptions.map((opt) =>
				opt.id === configId ? { ...opt, current_value: value } : opt,
			),
		});

		try {
			const updated = await commands.acpSetConfigOption(
				activeAgentName,
				configId,
				value,
			);
			set({ configOptions: updated });
		} catch (e) {
			set({ configOptions: prev, error: String(e) });
		}
	},

	setChatActivePreset: (name: string) => {
		set({ chatActivePreset: name });
	},

	setChatModel: (model: string) => {
		set({ chatModel: model });
	},

	setChatProvider: (providerId: string | null, model?: string) => {
		const update: Partial<AgentState> = { chatProviderId: providerId };
		if (model !== undefined) {
			update.chatModel = model;
		}
		set(update);
	},

	fetchChatSessions: async () => {
		try {
			const chatSessions = await commands.acpListChatSessions();
			set({ chatSessions });
		} catch (e) {
			console.error("Failed to fetch chat sessions", e);
		}
	},

	newChat: () => {
		const { messageMap, activeChatId } = get();
		if (activeChatId && Object.keys(messageMap).length > 0) {
			get().saveCurrentChat();
		}
		set({
			activeChatId: generateChatId(),
			messageMap: {},
			rootMessageIds: [],
			activeLeafId: null,
		});
	},

	switchChat: async (chatId: string) => {
		const { activeChatId, messageMap } = get();
		if (activeChatId && Object.keys(messageMap).length > 0) {
			await get().saveCurrentChat();
		}
		try {
			const session = await commands.acpLoadChatSession(chatId);
			const rawMessages = session.messages as unknown as ChatMessage[];

			// Detect if messages already have tree fields or are legacy flat format
			const hasTreeFields = rawMessages.some(
				(m) => m.parentId !== undefined || m.childrenIds !== undefined,
			);

			let messageMap: Record<string, ChatMessage>;
			let rootMessageIds: string[];
			let activeLeafId: string | null;

			if (hasTreeFields && rawMessages.some((m) => m.childrenIds?.length > 0)) {
				// Messages have tree structure — reconstruct from stored data
				messageMap = {};
				rootMessageIds = [];
				for (const m of rawMessages) {
					const msg: ChatMessage = {
						...m,
						parentId: m.parentId ?? null,
						childrenIds: m.childrenIds ?? [],
					};
					messageMap[msg.id] = msg;
					if (!msg.parentId) {
						rootMessageIds.push(msg.id);
					}
				}
				// Active leaf = deepest leaf following last children
				if (rootMessageIds.length > 0) {
					activeLeafId = findDeepestLeaf(
						messageMap,
						rootMessageIds[rootMessageIds.length - 1],
					);
				} else {
					activeLeafId = null;
				}
			} else {
				// Legacy flat format — convert to tree
				const tree = flatToTree(rawMessages);
				messageMap = tree.messageMap;
				rootMessageIds = tree.rootMessageIds;
				activeLeafId = tree.activeLeafId;
			}

			set({
				activeChatId: chatId,
				activeAgentName: session.agentName,
				activeCwd: session.cwd ?? null,
				messageMap,
				rootMessageIds,
				activeLeafId,
				sessionId:
					session.agentName === CHAT_AGENT_NAME ? CHAT_AGENT_NAME : null,
			});
		} catch (e) {
			console.error("Failed to load chat session", e);
		}
	},

	deleteChat: async (chatId: string) => {
		try {
			await commands.acpDeleteChatSession(chatId);
			const { activeChatId } = get();
			if (activeChatId === chatId) {
				set({
					activeChatId: null,
					messageMap: {},
					rootMessageIds: [],
					activeLeafId: null,
				});
			}
			await get().fetchChatSessions();
		} catch (e) {
			console.error("Failed to delete chat session", e);
		}
	},

	saveCurrentChat: async () => {
		const {
			activeChatId,
			activeAgentName,
			activeCwd,
			messageMap,
			rootMessageIds,
			activeLeafId,
		} = get();
		if (
			!activeChatId ||
			!activeAgentName ||
			Object.keys(messageMap).length === 0
		)
			return;

		const existing = get().chatSessions.find((s) => s.id === activeChatId);
		const now = new Date().toISOString();
		const msgCount = Object.keys(messageMap).length;
		const hasNewMessages = !existing || msgCount !== existing.messageCount;

		// Serialize: save all messages as flat array with tree fields preserved
		const allMessages = treeToFlat(messageMap, rootMessageIds, activeLeafId);
		const activeBranch = getActiveBranchFromRoots(
			messageMap,
			rootMessageIds,
			activeLeafId,
		);

		try {
			await commands.acpSaveChatSession({
				id: activeChatId,
				agentName: activeAgentName,
				title: chatTitle(activeBranch),
				messages: allMessages as unknown as Record<string, unknown>[],
				createdAt: existing?.createdAt ?? now,
				updatedAt: hasNewMessages ? now : (existing?.updatedAt ?? now),
				cwd: activeCwd,
			});
			await get().fetchChatSessions();
		} catch (e) {
			console.error("Failed to save chat session", e);
		}
	},
}));

// ── ACP agent event handler ─────────────────────────────────────────────────

function handleAgentUpdate(
	update: AgentUpdate,
	set: {
		(partial: Partial<AgentState>): void;
		(fn: (s: AgentState) => Partial<AgentState>): void;
	},
	get: () => AgentState,
) {
	// DEBUG: log raw ACP events to inspect tool_call fields
	console.log("[AgentUpdate]", update.kind, JSON.stringify(update));

	switch (update.kind) {
		case "text_chunk": {
			set((s) => {
				const branch = getActiveBranchFromRoots(
					s.messageMap,
					s.rootMessageIds,
					s.activeLeafId,
				);
				const last = branch[branch.length - 1];
				if (last && last.role === "agent") {
					// Append to existing agent message
					const updatedMsg = { ...last, text: last.text + (update.text ?? "") };
					return {
						messageMap: { ...s.messageMap, [last.id]: updatedMsg },
					};
				}
				// Create new agent message as child of activeLeafId
				const newMsg: ChatMessage = {
					id: nextMsgId(),
					role: "agent",
					text: update.text ?? "",
					timestamp: Date.now(),
					parentId: s.activeLeafId,
					childrenIds: [],
				};
				const result = addMessageToTree(s.messageMap, s.rootMessageIds, newMsg);
				return {
					messageMap: result.messageMap,
					rootMessageIds: result.rootMessageIds,
					activeLeafId: newMsg.id,
				};
			});
			break;
		}
		case "thought_chunk": {
			set((s) => {
				const branch = getActiveBranchFromRoots(
					s.messageMap,
					s.rootMessageIds,
					s.activeLeafId,
				);
				const last = branch[branch.length - 1];
				if (last && last.role === "thought") {
					const updatedMsg = { ...last, text: last.text + (update.text ?? "") };
					return {
						messageMap: { ...s.messageMap, [last.id]: updatedMsg },
					};
				}
				const newMsg: ChatMessage = {
					id: nextMsgId(),
					role: "thought",
					text: update.text ?? "",
					timestamp: Date.now(),
					parentId: s.activeLeafId,
					childrenIds: [],
				};
				const result = addMessageToTree(s.messageMap, s.rootMessageIds, newMsg);
				return {
					messageMap: result.messageMap,
					rootMessageIds: result.rootMessageIds,
					activeLeafId: newMsg.id,
				};
			});
			break;
		}
		case "tool_call": {
			set((s) => {
				const newMsg: ChatMessage = {
					id: nextMsgId(),
					role: "tool",
					text: "",
					toolCallId: update.tool_call_id,
					toolTitle: update.title ?? "",
					toolStatus: update.status ?? "pending",
					toolArguments: update.raw_input ?? undefined,
					toolResult: update.raw_output ?? undefined,
					timestamp: Date.now(),
					parentId: s.activeLeafId,
					childrenIds: [],
				};
				const result = addMessageToTree(s.messageMap, s.rootMessageIds, newMsg);
				return {
					messageMap: result.messageMap,
					rootMessageIds: result.rootMessageIds,
					activeLeafId: newMsg.id,
				};
			});
			break;
		}
		case "tool_call_update": {
			set((s) => {
				// Find the tool message by toolCallId in the map
				const toolMsgId = Object.keys(s.messageMap).find(
					(id) => s.messageMap[id].toolCallId === update.tool_call_id,
				);
				if (!toolMsgId) return {};
				const toolMsg = s.messageMap[toolMsgId];
				return {
					messageMap: {
						...s.messageMap,
						[toolMsgId]: {
							...toolMsg,
							toolStatus: update.status ?? toolMsg.toolStatus,
							text: update.content_text ?? toolMsg.text,
							toolArguments: update.raw_input ?? toolMsg.toolArguments,
							toolResult: update.raw_output ?? toolMsg.toolResult,
						},
					},
				};
			});
			break;
		}
		case "plan": {
			set((s) => {
				const newMsg: ChatMessage = {
					id: nextMsgId(),
					role: "plan",
					text: "",
					planEntries: update.entries,
					timestamp: Date.now(),
					parentId: s.activeLeafId,
					childrenIds: [],
				};
				const result = addMessageToTree(s.messageMap, s.rootMessageIds, newMsg);
				return {
					messageMap: result.messageMap,
					rootMessageIds: result.rootMessageIds,
					activeLeafId: newMsg.id,
				};
			});
			break;
		}
		case "config_options": {
			if (update.config_options) {
				set({ configOptions: update.config_options });
			}
			break;
		}
		case "prompt_done": {
			// Finalize any tool messages still in non-terminal state
			set((s) => {
				const updatedMap = { ...s.messageMap };
				for (const id of Object.keys(updatedMap)) {
					const msg = updatedMap[id];
					if (
						msg.role === "tool" &&
						msg.toolStatus !== "completed" &&
						msg.toolStatus !== "error"
					) {
						updatedMap[id] = { ...msg, toolStatus: "completed" };
					}
				}
				return { streaming: false, messageMap: updatedMap };
			});
			scheduleSave(get);
			break;
		}
		case "error": {
			set((s) => {
				const newMsg: ChatMessage = {
					id: nextMsgId(),
					role: "error",
					text: update.message ?? "Unknown error",
					timestamp: Date.now(),
					parentId: s.activeLeafId,
					childrenIds: [],
				};
				const result = addMessageToTree(s.messageMap, s.rootMessageIds, newMsg);
				return {
					streaming: false,
					messageMap: result.messageMap,
					rootMessageIds: result.rootMessageIds,
					activeLeafId: newMsg.id,
				};
			});
			scheduleSave(get);
			break;
		}
	}
}

// ── Chat event handler ──────────────────────────────────────────────────────

function handleChatUpdate(
	update: ChatUpdate,
	set: {
		(partial: Partial<AgentState>): void;
		(fn: (s: AgentState) => Partial<AgentState>): void;
	},
	get: () => AgentState,
) {
	switch (update.kind) {
		case "text_chunk": {
			set((s) => {
				const branch = getActiveBranchFromRoots(
					s.messageMap,
					s.rootMessageIds,
					s.activeLeafId,
				);
				const last = branch[branch.length - 1];
				if (last && last.role === "agent") {
					const updatedMsg = { ...last, text: last.text + (update.text ?? "") };
					return {
						messageMap: { ...s.messageMap, [last.id]: updatedMsg },
					};
				}
				const newMsg: ChatMessage = {
					id: nextMsgId(),
					role: "agent",
					text: update.text ?? "",
					timestamp: Date.now(),
					parentId: s.activeLeafId,
					childrenIds: [],
				};
				const result = addMessageToTree(s.messageMap, s.rootMessageIds, newMsg);
				return {
					messageMap: result.messageMap,
					rootMessageIds: result.rootMessageIds,
					activeLeafId: newMsg.id,
				};
			});
			break;
		}
		case "tool_call": {
			set((s) => {
				const newMsg: ChatMessage = {
					id: nextMsgId(),
					role: "tool",
					text: "",
					toolCallId: update.tool_call_id,
					toolTitle: update.name ?? "",
					toolStatus: update.needs_confirmation
						? "pending_confirmation"
						: "running",
					toolArguments: update.arguments,
					needsConfirmation: update.needs_confirmation,
					timestamp: Date.now(),
					parentId: s.activeLeafId,
					childrenIds: [],
				};
				const result = addMessageToTree(s.messageMap, s.rootMessageIds, newMsg);
				return {
					messageMap: result.messageMap,
					rootMessageIds: result.rootMessageIds,
					activeLeafId: newMsg.id,
				};
			});
			break;
		}
		case "tool_result": {
			set((s) => {
				const toolMsgId = Object.keys(s.messageMap).find(
					(id) => s.messageMap[id].toolCallId === update.tool_call_id,
				);
				if (!toolMsgId) return {};
				const toolMsg = s.messageMap[toolMsgId];
				return {
					messageMap: {
						...s.messageMap,
						[toolMsgId]: {
							...toolMsg,
							toolStatus: update.is_error ? "error" : "completed",
							toolResult: update.result,
							needsConfirmation: false,
						},
					},
				};
			});
			break;
		}
		case "done": {
			// Finalize any tool messages still in non-terminal state
			set((s) => {
				const updatedMap = { ...s.messageMap };
				for (const id of Object.keys(updatedMap)) {
					const msg = updatedMap[id];
					if (
						msg.role === "tool" &&
						msg.toolStatus !== "completed" &&
						msg.toolStatus !== "error"
					) {
						updatedMap[id] = { ...msg, toolStatus: "completed" };
					}
				}
				return { streaming: false, messageMap: updatedMap };
			});
			scheduleSave(get);
			break;
		}
		case "error": {
			set((s) => {
				const newMsg: ChatMessage = {
					id: nextMsgId(),
					role: "error",
					text: update.message ?? "Unknown error",
					timestamp: Date.now(),
					parentId: s.activeLeafId,
					childrenIds: [],
				};
				const result = addMessageToTree(s.messageMap, s.rootMessageIds, newMsg);
				return {
					streaming: false,
					messageMap: result.messageMap,
					rootMessageIds: result.rootMessageIds,
					activeLeafId: newMsg.id,
				};
			});
			scheduleSave(get);
			break;
		}
	}
}
