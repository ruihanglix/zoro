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
}

interface AgentUpdate {
	kind: string;
	session_id: string;
	text?: string;
	tool_call_id?: string;
	title?: string;
	status?: string;
	content_text?: string;
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
	messages: ChatMessage[];
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

export const useAgentStore = create<AgentState>((set, get) => ({
	agents: [],
	activeAgentName: null,
	sessionId: null,
	activeCwd: null,
	messages: [],
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
				// Same paper – just update cwd if it changed (paperDir loads async)
				const newCwd = cwd ?? null;
				if (get().activeCwd !== newCwd) {
					set({ activeCwd: newCwd });
				}
				return;
			}
		}

		chatStartLock = true;
		try {
			const { activeChatId, messages, chatConfigLoaded, activeAgentName } =
				get();

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
			const kept = switchingFromAcp
				? [
						...messages,
						{
							id: nextMsgId(),
							role: "separator" as const,
							text: "Switched to Chat mode",
							timestamp: Date.now(),
						},
					]
				: [];

			// Reuse existing chatId when switching agents within the same conversation;
			// only generate a new one if there is no active chat.
			const chatId = activeChatId ?? generateChatId();
			set({
				activeAgentName: CHAT_AGENT_NAME,
				sessionId: CHAT_AGENT_NAME,
				activeCwd: cwd ?? null,
				messages: kept,
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

		const { activeChatId, messages, activeAgentName } = get();

		// Preserve messages with a separator when switching modes
		const switchingMode =
			activeAgentName && activeAgentName !== agentName && messages.length > 0;
		const kept = switchingMode
			? [
					...messages,
					{
						id: nextMsgId(),
						role: "separator" as const,
						text: `Switched to ${agentName}`,
						timestamp: Date.now(),
					},
				]
			: [];

		// Reuse existing chatId when switching agents within the same conversation;
		// only generate a new one if there is no active chat.
		const chatId = activeChatId ?? generateChatId();
		set({
			activeAgentName: agentName,
			sessionId: null,
			activeCwd: cwd ?? null,
			messages: kept,
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
			messages,
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
		};
		set((s) => ({
			messages: [...s.messages, userMsg],
			streaming: true,
			error: null,
		}));

		scheduleSave(get);

		// Split messages at the last separator to get current-segment history
		// and optional prior context from a different mode/agent
		const lastSepIdx = findLastIndex(messages, (m) => m.role === "separator");
		const currentSegment =
			lastSepIdx >= 0 ? messages.slice(lastSepIdx + 1) : messages;
		const priorContext = lastSepIdx >= 0 ? messages.slice(0, lastSepIdx) : [];

		if (activeAgentName === CHAT_AGENT_NAME) {
			const historyMessages = currentSegment
				.filter((m) => m.role === "user" || m.role === "agent")
				.map((m) => ({
					role: m.role === "agent" ? "assistant" : "user",
					content: m.text,
				}));

			// Inject prior context as a prefixed assistant/user exchange
			if (priorContext.length > 0 && historyMessages.length === 0) {
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
					messages: historyMessages,
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
				set((s) => ({
					streaming: false,
					messages: [
						...s.messages,
						{
							id: nextMsgId(),
							role: "error" as const,
							text: String(e),
							timestamp: Date.now(),
						},
					],
				}));
			}
		} else {
			// For ACP agents, inject prior context in the first prompt of the segment
			let prompt = text;
			if (
				priorContext.length > 0 &&
				currentSegment.filter((m) => m.role === "user").length === 0
			) {
				const summary = buildContextSummary(priorContext);
				if (summary) {
					prompt = `[Previous conversation context]\n${summary}\n[End of context]\n\n${text}`;
				}
			}

			try {
				await commands.acpSendPrompt(activeAgentName, prompt, images);
			} catch (e) {
				set((s) => ({
					streaming: false,
					messages: [
						...s.messages,
						{
							id: nextMsgId(),
							role: "error" as const,
							text: String(e),
							timestamp: Date.now(),
						},
					],
				}));
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
		const { activeAgentName, messages, activeChatId } = get();
		if (activeChatId && messages.length > 0) {
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

	clearMessages: () => set({ messages: [] }),

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
		const { messages, activeChatId } = get();
		if (activeChatId && messages.length > 0) {
			get().saveCurrentChat();
		}
		set({
			activeChatId: generateChatId(),
			messages: [],
		});
	},

	switchChat: async (chatId: string) => {
		const { activeChatId, messages } = get();
		if (activeChatId && messages.length > 0) {
			await get().saveCurrentChat();
		}
		try {
			const session = await commands.acpLoadChatSession(chatId);
			set({
				activeChatId: chatId,
				activeAgentName: session.agentName,
				activeCwd: session.cwd ?? null,
				messages: session.messages as unknown as ChatMessage[],
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
				set({ activeChatId: null, messages: [] });
			}
			await get().fetchChatSessions();
		} catch (e) {
			console.error("Failed to delete chat session", e);
		}
	},

	saveCurrentChat: async () => {
		const { activeChatId, activeAgentName, activeCwd, messages } = get();
		if (!activeChatId || !activeAgentName || messages.length === 0) return;

		const existing = get().chatSessions.find((s) => s.id === activeChatId);
		const now = new Date().toISOString();
		const hasNewMessages =
			!existing || messages.length !== existing.messageCount;

		try {
			await commands.acpSaveChatSession({
				id: activeChatId,
				agentName: activeAgentName,
				title: chatTitle(messages),
				messages: messages as unknown as Record<string, unknown>[],
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
	switch (update.kind) {
		case "text_chunk": {
			set((s) => {
				const msgs = [...s.messages];
				const last = msgs[msgs.length - 1];
				if (last && last.role === "agent") {
					msgs[msgs.length - 1] = {
						...last,
						text: last.text + (update.text ?? ""),
					};
				} else {
					msgs.push({
						id: nextMsgId(),
						role: "agent",
						text: update.text ?? "",
						timestamp: Date.now(),
					});
				}
				return { messages: msgs };
			});
			break;
		}
		case "thought_chunk": {
			set((s) => {
				const msgs = [...s.messages];
				const last = msgs[msgs.length - 1];
				if (last && last.role === "thought") {
					msgs[msgs.length - 1] = {
						...last,
						text: last.text + (update.text ?? ""),
					};
				} else {
					msgs.push({
						id: nextMsgId(),
						role: "thought",
						text: update.text ?? "",
						timestamp: Date.now(),
					});
				}
				return { messages: msgs };
			});
			break;
		}
		case "tool_call": {
			set((s) => ({
				messages: [
					...s.messages,
					{
						id: nextMsgId(),
						role: "tool" as const,
						text: "",
						toolCallId: update.tool_call_id,
						toolTitle: update.title ?? "",
						toolStatus: update.status ?? "pending",
						timestamp: Date.now(),
					},
				],
			}));
			break;
		}
		case "tool_call_update": {
			set((s) => {
				const msgs = s.messages.map((m) =>
					m.toolCallId === update.tool_call_id
						? {
								...m,
								toolStatus: update.status ?? m.toolStatus,
								text: update.content_text ?? m.text,
							}
						: m,
				);
				return { messages: msgs };
			});
			break;
		}
		case "plan": {
			set((s) => ({
				messages: [
					...s.messages,
					{
						id: nextMsgId(),
						role: "plan" as const,
						text: "",
						planEntries: update.entries,
						timestamp: Date.now(),
					},
				],
			}));
			break;
		}
		case "config_options": {
			if (update.config_options) {
				set({ configOptions: update.config_options });
			}
			break;
		}
		case "prompt_done": {
			set({ streaming: false });
			scheduleSave(get);
			break;
		}
		case "error": {
			set((s) => ({
				streaming: false,
				messages: [
					...s.messages,
					{
						id: nextMsgId(),
						role: "error" as const,
						text: update.message ?? "Unknown error",
						timestamp: Date.now(),
					},
				],
			}));
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
				const msgs = [...s.messages];
				const last = msgs[msgs.length - 1];
				if (last && last.role === "agent") {
					msgs[msgs.length - 1] = {
						...last,
						text: last.text + (update.text ?? ""),
					};
				} else {
					msgs.push({
						id: nextMsgId(),
						role: "agent",
						text: update.text ?? "",
						timestamp: Date.now(),
					});
				}
				return { messages: msgs };
			});
			break;
		}
		case "tool_call": {
			set((s) => ({
				messages: [
					...s.messages,
					{
						id: nextMsgId(),
						role: "tool" as const,
						text: "",
						toolCallId: update.tool_call_id,
						toolTitle: update.name ?? "",
						toolStatus: update.needs_confirmation
							? "pending_confirmation"
							: "running",
						toolArguments: update.arguments,
						needsConfirmation: update.needs_confirmation,
						timestamp: Date.now(),
					},
				],
			}));
			break;
		}
		case "tool_result": {
			set((s) => {
				const msgs = s.messages.map((m) =>
					m.toolCallId === update.tool_call_id
						? {
								...m,
								toolStatus: update.is_error ? "error" : "completed",
								toolResult: update.result,
								needsConfirmation: false,
							}
						: m,
				);
				return { messages: msgs };
			});
			break;
		}
		case "done": {
			set({ streaming: false });
			scheduleSave(get);
			break;
		}
		case "error": {
			set((s) => ({
				streaming: false,
				messages: [
					...s.messages,
					{
						id: nextMsgId(),
						role: "error" as const,
						text: update.message ?? "Unknown error",
						timestamp: Date.now(),
					},
				],
			}));
			scheduleSave(get);
			break;
		}
	}
}
