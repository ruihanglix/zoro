// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { Button } from "@/components/ui/button";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuSeparator,
	DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
	Tooltip,
	TooltipContent,
	TooltipProvider,
	TooltipTrigger,
} from "@/components/ui/tooltip";
import type { ConfigOptionInfo, ImageInput } from "@/lib/commands";
import { cn } from "@/lib/utils";
import {
	CHAT_AGENT_NAME,
	type ChatMessage,
	useAgentStore,
} from "@/stores/agentStore";
import { useLibraryStore } from "@/stores/libraryStore";
import { useTabStore } from "@/stores/tabStore";
import {
	Bot,
	Brain,
	Check,
	ChevronDown,
	ChevronRight,
	EllipsisVertical,
	FileText,
	FolderClosed,
	FolderOpen,
	History,
	Image,
	Loader2,
	MessageCircle,
	MessageSquarePlus,
	PanelLeft,
	PanelLeftClose,
	Send,
	Settings,
	Square,
	Trash2,
	Wrench,
	X,
	XCircle,
} from "lucide-react";
import React, { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";

interface AgentPanelProps {
	cwd?: string;
	paperId?: string;
}

export function AgentPanel({ cwd, paperId }: AgentPanelProps) {
	const { t } = useTranslation();
	const isGlobal = !cwd;

	const agents = useAgentStore((s) => s.agents);
	const activeAgentName = useAgentStore((s) => s.activeAgentName);
	const sessionId = useAgentStore((s) => s.sessionId);
	const messages = useAgentStore((s) => s.messages);
	const streaming = useAgentStore((s) => s.streaming);
	const connecting = useAgentStore((s) => s.connecting);
	const error = useAgentStore((s) => s.error);
	const fetchAgents = useAgentStore((s) => s.fetchAgents);
	const startSession = useAgentStore((s) => s.startSession);
	const sendPrompt = useAgentStore((s) => s.sendPrompt);
	const cancelPrompt = useAgentStore((s) => s.cancelPrompt);
	const stopSession = useAgentStore((s) => s.stopSession);
	const clearMessages = useAgentStore((s) => s.clearMessages);
	const configOptions = useAgentStore((s) => s.configOptions);
	const setConfigOption = useAgentStore((s) => s.setConfigOption);

	const chatSessions = useAgentStore((s) => s.chatSessions);
	const activeChatId = useAgentStore((s) => s.activeChatId);
	const fetchChatSessions = useAgentStore((s) => s.fetchChatSessions);
	const newChat = useAgentStore((s) => s.newChat);
	const switchChat = useAgentStore((s) => s.switchChat);
	const deleteChat = useAgentStore((s) => s.deleteChat);
	const startChat = useAgentStore((s) => s.startChat);
	const chatPresets = useAgentStore((s) => s.chatPresets);
	const chatActivePreset = useAgentStore((s) => s.chatActivePreset);
	const setChatActivePreset = useAgentStore((s) => s.setChatActivePreset);
	const chatModel = useAgentStore((s) => s.chatModel);
	const setChatModel = useAgentStore((s) => s.setChatModel);
	const chatProviderId = useAgentStore((s) => s.chatProviderId);
	const chatProviders = useAgentStore((s) => s.chatProviders);
	const setChatProvider = useAgentStore((s) => s.setChatProvider);
	const fetchChatConfig = useAgentStore((s) => s.fetchChatConfig);

	const chatPaperId = useAgentStore((s) => s.chatPaperId);
	const activeTabId = useTabStore((s) => s.activeTabId);
	const papers = useLibraryStore((s) => s.papers);

	const isChatMode = activeAgentName === CHAT_AGENT_NAME;

	const [inputText, setInputText] = useState("");
	const [showPresetPicker, setShowPresetPicker] = useState(false);
	const presetPickerRef = useRef<HTMLDivElement>(null);
	const [attachedImages, setAttachedImages] = useState<ImageInput[]>([]);
	const [showAgentPicker, setShowAgentPicker] = useState(false);
	const [sidebarOpen, setSidebarOpen] = useState(isGlobal);
	const [historyPopoverOpen, setHistoryPopoverOpen] = useState(false);
	const historyBtnRef = useRef<HTMLButtonElement>(null);
	const historyPopoverRef = useRef<HTMLDivElement>(null);
	const textareaRef = useRef<HTMLTextAreaElement>(null);
	const scrollRef = useRef<HTMLDivElement>(null);
	const fileInputRef = useRef<HTMLInputElement>(null);

	const paperFilteredSessions = isGlobal
		? chatSessions
		: chatSessions.filter((s) => s.cwd === cwd);

	useEffect(() => {
		if (!historyPopoverOpen) return;
		function handleClick(e: MouseEvent) {
			if (
				historyPopoverRef.current &&
				!historyPopoverRef.current.contains(e.target as Node) &&
				historyBtnRef.current &&
				!historyBtnRef.current.contains(e.target as Node)
			) {
				setHistoryPopoverOpen(false);
			}
		}
		document.addEventListener("mousedown", handleClick);
		return () => document.removeEventListener("mousedown", handleClick);
	}, [historyPopoverOpen]);

	useEffect(() => {
		fetchAgents();
		fetchChatSessions();
		fetchChatConfig();
	}, [fetchAgents, fetchChatSessions, fetchChatConfig]);

	// Paper panel: start chat when paperId is present (reader context).
	// Uses paperId (not isGlobal/cwd) because cwd (paperDir) is loaded async
	// and may still be undefined on first mount — which would make isGlobal
	// incorrectly true, skipping the startChat call entirely.
	// Re-runs when cwd becomes available so the session gets the correct dir.
	useEffect(() => {
		if (!paperId) return;
		startChat(paperId, cwd);
	}, [paperId, cwd]); // eslint-disable-line react-hooks/exhaustive-deps

	// Global panel: re-sync context when the global Agent tab becomes active,
	// in case the user was chatting inside a paper and then switched back.
	useEffect(() => {
		if (!isGlobal) return;
		if (activeTabId !== "agent") return;

		const expectedPaperId = paperId ?? null;
		if (!activeAgentName && !connecting) {
			startChat(paperId, cwd);
		} else if (
			activeAgentName === CHAT_AGENT_NAME &&
			chatPaperId !== expectedPaperId
		) {
			startChat(paperId, cwd);
		}
	}, [activeTabId]); // eslint-disable-line react-hooks/exhaustive-deps

	// Close preset picker on outside click
	useEffect(() => {
		if (!showPresetPicker) return;
		function handleClick(e: MouseEvent) {
			if (
				presetPickerRef.current &&
				!presetPickerRef.current.contains(e.target as Node)
			) {
				setShowPresetPicker(false);
			}
		}
		document.addEventListener("mousedown", handleClick);
		return () => document.removeEventListener("mousedown", handleClick);
	}, [showPresetPicker]);

	const scrollToBottom = useCallback(() => {
		if (scrollRef.current) {
			scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
		}
	}, []);

	const prevMessageCount = useRef(0);
	if (messages.length !== prevMessageCount.current) {
		prevMessageCount.current = messages.length;
		requestAnimationFrame(scrollToBottom);
	}

	const handleImageFile = useCallback((file: File) => {
		if (!file.type.startsWith("image/")) return;
		const reader = new FileReader();
		reader.onload = () => {
			const base64 = (reader.result as string).split(",")[1];
			if (base64) {
				setAttachedImages((prev) => [
					...prev,
					{ base64Data: base64, mimeType: file.type },
				]);
			}
		};
		reader.readAsDataURL(file);
	}, []);

	const handlePaste = useCallback(
		(e: React.ClipboardEvent) => {
			const items = e.clipboardData.items;
			for (const item of items) {
				if (item.type.startsWith("image/")) {
					e.preventDefault();
					const file = item.getAsFile();
					if (file) handleImageFile(file);
					return;
				}
			}
		},
		[handleImageFile],
	);

	const handleDrop = useCallback(
		(e: React.DragEvent) => {
			e.preventDefault();
			for (const file of e.dataTransfer.files) {
				handleImageFile(file);
			}
		},
		[handleImageFile],
	);

	const handleDragOver = useCallback((e: React.DragEvent) => {
		e.preventDefault();
	}, []);

	const handleSend = useCallback(() => {
		const text = inputText.trim();
		if (!text && attachedImages.length === 0) return;
		sendPrompt(text, attachedImages.length > 0 ? attachedImages : undefined);
		setInputText("");
		setAttachedImages([]);
		textareaRef.current?.focus();
	}, [inputText, attachedImages, sendPrompt]);

	const handleKeyDown = useCallback(
		(e: React.KeyboardEvent) => {
			if (e.key === "Enter" && !e.shiftKey) {
				e.preventDefault();
				handleSend();
			}
		},
		[handleSend],
	);

	const removeImage = useCallback((index: number) => {
		setAttachedImages((prev) => prev.filter((_, i) => i !== index));
	}, []);

	const activeAgent = agents.find((a) => a.name === activeAgentName);

	const chatArea = (
		<div className="flex h-full w-full flex-col">
			{/* Header: Agent selector + session controls */}
			<div className="px-4 py-3">
				<div className="flex items-center gap-1.5">
					{isGlobal && !sidebarOpen && (
						<TooltipProvider>
							<Tooltip>
								<TooltipTrigger asChild>
									<Button
										variant="ghost"
										size="icon"
										className="h-8 w-8 shrink-0"
										onClick={() => setSidebarOpen(true)}
									>
										<PanelLeft className="h-4 w-4" />
									</Button>
								</TooltipTrigger>
								<TooltipContent>Show history</TooltipContent>
							</Tooltip>
						</TooltipProvider>
					)}
					{!isGlobal && (
						<div className="relative">
							<TooltipProvider>
								<Tooltip>
									<TooltipTrigger asChild>
										<Button
											ref={historyBtnRef}
											variant="ghost"
											size="icon"
											className={cn(
												"h-8 w-8 shrink-0",
												historyPopoverOpen && "bg-accent",
											)}
											onClick={() => setHistoryPopoverOpen((v) => !v)}
										>
											<History className="h-4 w-4" />
										</Button>
									</TooltipTrigger>
									<TooltipContent>Chat history</TooltipContent>
								</Tooltip>
							</TooltipProvider>
							{historyPopoverOpen && (
								<HistoryPopover
									ref={historyPopoverRef}
									sessions={paperFilteredSessions}
									activeChatId={activeChatId}
									onNewChat={() => {
										newChat();
										setHistoryPopoverOpen(false);
									}}
									onSwitchChat={(id) => {
										switchChat(id);
										setHistoryPopoverOpen(false);
									}}
									onDeleteChat={deleteChat}
								/>
							)}
						</div>
					)}

					<div className="relative flex-1 min-w-0">
						<button
							type="button"
							className="flex w-full items-center justify-between rounded-md border px-3 py-1.5 text-sm hover:bg-accent/50 transition-colors"
							onClick={() => setShowAgentPicker(!showAgentPicker)}
						>
							<span className="truncate">
								{isChatMode
									? "Chat"
									: activeAgent
										? `${activeAgent.title}${connecting ? "" : sessionId ? " (connected)" : ""}`
										: "Chat"}
							</span>
							<ChevronDown className="h-3.5 w-3.5 shrink-0 ml-2 text-muted-foreground" />
						</button>

						{showAgentPicker && (
							<div className="absolute left-0 right-0 top-full z-50 mt-1 rounded-md border bg-popover p-1 shadow-md">
								{/* Built-in Chat option */}
								<button
									type="button"
									className={cn(
										"flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-sm hover:bg-accent/50 transition-colors text-left",
										isChatMode && "bg-accent",
									)}
									onClick={() => {
										startChat(paperId, cwd);
										setShowAgentPicker(false);
									}}
								>
									<MessageCircle className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
									<div className="flex-1 min-w-0">
										<div className="truncate font-medium">Chat</div>
										<div className="truncate text-xs text-muted-foreground">
											Direct LLM chat with library tools
										</div>
									</div>
								</button>

								{agents.length > 0 && <div className="my-1 border-t" />}

								{agents.map((agent) => (
									<button
										key={agent.name}
										type="button"
										className={cn(
											"flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-sm hover:bg-accent/50 transition-colors text-left",
											activeAgentName === agent.name && "bg-accent",
										)}
										onClick={() => {
											startSession(agent.name, cwd);
											setShowAgentPicker(false);
										}}
									>
										<Bot className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
										<div className="flex-1 min-w-0">
											<div className="truncate font-medium">{agent.title}</div>
											<div className="truncate text-xs text-muted-foreground">
												{agent.description}
											</div>
										</div>
										{agent.hasSession && (
											<span className="h-2 w-2 shrink-0 rounded-full bg-green-500" />
										)}
									</button>
								))}
							</div>
						)}
					</div>

					{/* Preset selector (chat mode only) — shown inline for global panel */}
					{isGlobal && isChatMode && (
						<div className="relative" ref={presetPickerRef}>
							<TooltipProvider>
								<Tooltip>
									<TooltipTrigger asChild>
										<Button
											variant="ghost"
											size="icon"
											className={cn(
												"h-8 w-8 shrink-0",
												showPresetPicker && "bg-accent",
											)}
											onClick={() => setShowPresetPicker((v) => !v)}
										>
											<Settings className="h-4 w-4" />
										</Button>
									</TooltipTrigger>
									<TooltipContent>{t("agent.systemPrompt")}</TooltipContent>
								</Tooltip>
							</TooltipProvider>
							{showPresetPicker && (
								<div className="absolute right-0 top-full z-50 mt-1 w-56 rounded-md border bg-popover p-1 shadow-md">
									{chatPresets.map((preset) => (
										<button
											key={preset.name}
											type="button"
											className={cn(
												"flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-xs text-left hover:bg-accent/50 transition-colors",
												chatActivePreset === preset.name && "bg-accent",
											)}
											onClick={() => {
												setChatActivePreset(preset.name);
												setShowPresetPicker(false);
											}}
										>
											<span className="flex-1 truncate">{preset.name}</span>
											{chatActivePreset === preset.name && (
												<Check className="h-3 w-3 shrink-0 text-primary" />
											)}
										</button>
									))}
									{chatPresets.length === 0 && (
										<p className="px-2 py-1.5 text-xs text-muted-foreground">
											No presets configured
										</p>
									)}
								</div>
							)}
						</div>
					)}

					{/* Global panel: inline buttons */}
					{isGlobal && sessionId && (
						<>
							<TooltipProvider>
								<Tooltip>
									<TooltipTrigger asChild>
										<Button
											variant="ghost"
											size="icon"
											className="h-8 w-8 shrink-0"
											onClick={clearMessages}
										>
											<Trash2 className="h-4 w-4" />
										</Button>
									</TooltipTrigger>
									<TooltipContent>{t("agent.clearChat")}</TooltipContent>
								</Tooltip>
							</TooltipProvider>
							<TooltipProvider>
								<Tooltip>
									<TooltipTrigger asChild>
										<Button
											variant="ghost"
											size="icon"
											className="h-8 w-8 shrink-0 text-destructive hover:text-destructive"
											onClick={stopSession}
										>
											<XCircle className="h-4 w-4" />
										</Button>
									</TooltipTrigger>
									<TooltipContent>{t("agent.disconnect")}</TooltipContent>
								</Tooltip>
							</TooltipProvider>
						</>
					)}

					{/* Reader sidebar: collapse actions into dropdown menu */}
					{!isGlobal && sessionId && (
						<DropdownMenu>
							<TooltipProvider>
								<Tooltip>
									<TooltipTrigger asChild>
										<DropdownMenuTrigger asChild>
											<Button
												variant="ghost"
												size="icon"
												className="h-8 w-8 shrink-0"
											>
												<EllipsisVertical className="h-4 w-4" />
											</Button>
										</DropdownMenuTrigger>
									</TooltipTrigger>
									<TooltipContent>{t("agent.moreActions")}</TooltipContent>
								</Tooltip>
							</TooltipProvider>
							<DropdownMenuContent align="end" className="w-48">
								{isChatMode && (
									<>
										<DropdownMenuItem
											onClick={() => setShowPresetPicker((v) => !v)}
										>
											<Settings className="h-4 w-4 mr-2" />
											{t("agent.systemPrompt")}
										</DropdownMenuItem>
										<DropdownMenuSeparator />
									</>
								)}
								<DropdownMenuItem onClick={clearMessages}>
									<Trash2 className="h-4 w-4 mr-2" />
									{t("agent.clearChat")}
								</DropdownMenuItem>
								<DropdownMenuItem
									className="text-destructive focus:text-destructive"
									onClick={stopSession}
								>
									<XCircle className="h-4 w-4 mr-2" />
									{t("agent.disconnect")}
								</DropdownMenuItem>
							</DropdownMenuContent>
						</DropdownMenu>
					)}
				</div>
				{error && <p className="mt-1 text-xs text-destructive">{error}</p>}
			</div>

			{/* Config option selectors (model, mode, etc.) */}
			{sessionId && configOptions.length > 0 && (
				<div className="flex flex-wrap items-center gap-1.5 px-4 pb-2">
					{configOptions.map((opt) => (
						<ConfigSelector
							key={opt.id}
							option={opt}
							onChange={(value) => setConfigOption(opt.id, value)}
						/>
					))}
				</div>
			)}

			{/* Chat mode: provider + model selector */}
			{isChatMode && (
				<div className="flex items-center gap-1.5 px-4 pb-2">
					{chatProviders.length > 1 && (
						<ProviderModelSelector
							providers={chatProviders}
							activeProviderId={chatProviderId}
							activeModel={chatModel}
							onSelect={(providerId, model) => {
								setChatProvider(providerId, model);
							}}
						/>
					)}
					<div className="flex items-center gap-1 rounded-md border px-2 py-0.5 text-xs">
						<span className="text-muted-foreground">Model:</span>
						<input
							className="bg-transparent outline-none max-w-[160px] font-medium"
							value={chatModel}
							onChange={(e) => setChatModel(e.target.value)}
							placeholder="gpt-4o"
						/>
					</div>
				</div>
			)}

			{/* Messages / empty state */}
			{connecting && messages.length === 0 ? (
				<div className="flex flex-1 flex-col items-center justify-center text-muted-foreground">
					<Loader2 className="h-8 w-8 mb-3 animate-spin opacity-60" />
					<p className="text-sm font-medium">
						Connecting to {activeAgent?.title ?? "agent"}...
					</p>
					<p className="text-xs mt-1 opacity-60">
						Starting session, please wait
					</p>
				</div>
			) : messages.length > 0 ? (
				<ScrollArea className="flex-1">
					<div ref={scrollRef} className="flex flex-col gap-2 px-4 pb-4">
						{messages.map((msg) => (
							<MessageBubble key={msg.id} message={msg} />
						))}
						{connecting && (
							<div className="flex items-center gap-1.5 text-xs text-muted-foreground px-1 py-1">
								<Loader2 className="h-3 w-3 animate-spin" />
								<span>Connecting to {activeAgent?.title ?? "agent"}...</span>
							</div>
						)}
						{streaming && (
							<div className="flex items-center gap-1 text-xs text-muted-foreground px-1">
								<Loader2 className="h-3 w-3 animate-spin" />
								<span>Thinking...</span>
							</div>
						)}
					</div>
				</ScrollArea>
			) : !sessionId ? (
				<div className="flex flex-1 flex-col items-center justify-center text-muted-foreground">
					<Loader2 className="h-6 w-6 animate-spin opacity-40" />
				</div>
			) : (
				<div className="flex flex-1 flex-col items-center justify-center text-muted-foreground">
					<Bot className="h-10 w-10 mb-3 opacity-40" />
					<p className="text-sm">Ask the agent anything</p>
					<p className="text-xs mt-1 opacity-60">
						Supports text and image inputs
					</p>
				</div>
			)}

			{/* Input area */}
			{sessionId && (
				<div
					className="border-t p-3"
					onDrop={handleDrop}
					onDragOver={handleDragOver}
				>
					{attachedImages.length > 0 && (
						<div className="mb-2 flex flex-wrap gap-2">
							{attachedImages.map((img, i) => (
								<div
									key={`${img.mimeType}-${i}`}
									className="relative h-16 w-16 rounded-md overflow-hidden border group"
								>
									<img
										src={`data:${img.mimeType};base64,${img.base64Data}`}
										alt="Attached"
										className="h-full w-full object-cover"
									/>
									<button
										type="button"
										className="absolute inset-0 flex items-center justify-center bg-black/50 opacity-0 group-hover:opacity-100 transition-opacity"
										onClick={() => removeImage(i)}
									>
										<X className="h-4 w-4 text-white" />
									</button>
								</div>
							))}
						</div>
					)}

					<div className="flex items-center gap-2">
						<div className="flex-1 relative">
							<textarea
								ref={textareaRef}
								className="w-full resize-none rounded-md border bg-muted/30 px-3 py-2 pr-8 text-sm placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring min-h-[38px] max-h-[120px]"
								placeholder="Ask the agent..."
								rows={1}
								value={inputText}
								onChange={(e) => {
									setInputText(e.target.value);
									const el = e.target;
									el.style.height = "auto";
									el.style.height = `${Math.min(el.scrollHeight, 120)}px`;
								}}
								onKeyDown={handleKeyDown}
								onPaste={handlePaste}
								disabled={streaming}
							/>
							<TooltipProvider>
								<Tooltip>
									<TooltipTrigger asChild>
										<button
											type="button"
											className="absolute right-2 inset-y-0 flex items-center text-muted-foreground hover:text-foreground transition-colors"
											onClick={() => fileInputRef.current?.click()}
											disabled={streaming}
										>
											<Image className="h-4 w-4" />
										</button>
									</TooltipTrigger>
									<TooltipContent>Attach image</TooltipContent>
								</Tooltip>
							</TooltipProvider>
							<input
								ref={fileInputRef}
								type="file"
								accept="image/*"
								multiple
								className="hidden"
								onChange={(e) => {
									if (e.target.files) {
										for (const file of e.target.files) {
											handleImageFile(file);
										}
										e.target.value = "";
									}
								}}
							/>
						</div>

						{streaming ? (
							<Button
								variant="destructive"
								size="icon"
								className="h-9 w-9 shrink-0"
								onClick={cancelPrompt}
							>
								<Square className="h-4 w-4" />
							</Button>
						) : (
							<Button
								size="icon"
								className="h-9 w-9 shrink-0"
								onClick={handleSend}
								disabled={!inputText.trim() && attachedImages.length === 0}
							>
								<Send className="h-4 w-4" />
							</Button>
						)}
					</div>
					<p className="mt-1 text-[10px] text-muted-foreground text-center">
						Paste, drag & drop, or click <Image className="inline h-3 w-3" /> to
						attach images
					</p>
				</div>
			)}
		</div>
	);

	if (!isGlobal) {
		return <div className="flex h-full w-full bg-background">{chatArea}</div>;
	}

	return (
		<div className="flex h-full w-full bg-background">
			{sidebarOpen && (
				<ChatSidebar
					sessions={chatSessions}
					activeChatId={activeChatId}
					onNewChat={newChat}
					onSwitchChat={switchChat}
					onDeleteChat={deleteChat}
					onCollapse={() => setSidebarOpen(false)}
					papers={papers}
				/>
			)}
			{chatArea}
		</div>
	);
}

// ── Provider + Model selector ────────────────────────────────────────────────

function ProviderModelSelector({
	providers,
	activeProviderId,
	activeModel,
	onSelect,
}: {
	providers: { id: string; name: string; models: string[] }[];
	activeProviderId: string | null;
	activeModel: string;
	onSelect: (providerId: string, model: string) => void;
}) {
	const [open, setOpen] = useState(false);
	const ref = useRef<HTMLDivElement>(null);

	useEffect(() => {
		if (!open) return;
		function handleClick(e: MouseEvent) {
			if (ref.current && !ref.current.contains(e.target as Node)) {
				setOpen(false);
			}
		}
		document.addEventListener("mousedown", handleClick);
		return () => document.removeEventListener("mousedown", handleClick);
	}, [open]);

	const activeProvider = providers.find((p) => p.id === activeProviderId);
	const label = activeProvider?.name ?? "Default";

	return (
		<div ref={ref} className="relative">
			<button
				type="button"
				className="flex items-center gap-1 rounded-md border px-2 py-0.5 text-xs hover:bg-accent/50 transition-colors"
				onClick={() => setOpen(!open)}
			>
				<span className="text-muted-foreground">Provider:</span>
				<span className="font-medium max-w-[120px] truncate">{label}</span>
				<ChevronDown className="h-3 w-3 shrink-0 text-muted-foreground" />
			</button>
			{open && (
				<div className="absolute left-0 top-full z-50 mt-1 max-h-72 min-w-[200px] overflow-y-auto rounded-md border bg-popover p-1 shadow-md">
					{providers.map((provider) => (
						<div key={provider.id}>
							{provider.models.length > 0 ? (
								provider.models.map((model) => (
									<button
										key={`${provider.id}-${model}`}
										type="button"
										className={cn(
											"flex w-full items-center gap-2 rounded-sm px-2 py-1 text-xs text-left hover:bg-accent/50 transition-colors",
											activeProviderId === provider.id &&
												activeModel === model &&
												"bg-accent",
										)}
										onClick={() => {
											onSelect(provider.id, model);
											setOpen(false);
										}}
									>
										<div className="flex-1 min-w-0">
											<div className="truncate font-medium">
												{model}{" "}
												<span className="font-normal text-muted-foreground">
													({provider.name})
												</span>
											</div>
										</div>
										{activeProviderId === provider.id &&
											activeModel === model && (
												<Check className="h-3 w-3 shrink-0 text-primary" />
											)}
									</button>
								))
							) : (
								<button
									type="button"
									className={cn(
										"flex w-full items-center gap-2 rounded-sm px-2 py-1 text-xs text-left hover:bg-accent/50 transition-colors",
										activeProviderId === provider.id && "bg-accent",
									)}
									onClick={() => {
										onSelect(provider.id, activeModel);
										setOpen(false);
									}}
								>
									<div className="flex-1 min-w-0">
										<div className="truncate font-medium">{provider.name}</div>
										<div className="truncate text-[10px] text-muted-foreground">
											Custom model input
										</div>
									</div>
									{activeProviderId === provider.id && (
										<Check className="h-3 w-3 shrink-0 text-primary" />
									)}
								</button>
							)}
						</div>
					))}
				</div>
			)}
		</div>
	);
}

// ── Config option selector ───────────────────────────────────────────────────

function ConfigSelector({
	option,
	onChange,
}: {
	option: ConfigOptionInfo;
	onChange: (value: string) => void;
}) {
	const [open, setOpen] = useState(false);
	const ref = useRef<HTMLDivElement>(null);

	useEffect(() => {
		if (!open) return;
		function handleClick(e: MouseEvent) {
			if (ref.current && !ref.current.contains(e.target as Node)) {
				setOpen(false);
			}
		}
		document.addEventListener("mousedown", handleClick);
		return () => document.removeEventListener("mousedown", handleClick);
	}, [open]);

	const selected = option.options.find((o) => o.value === option.current_value);
	const label = option.category === "model" ? "Model" : option.name;

	return (
		<div ref={ref} className="relative">
			<button
				type="button"
				className="flex items-center gap-1 rounded-md border px-2 py-0.5 text-xs hover:bg-accent/50 transition-colors"
				onClick={() => setOpen(!open)}
			>
				<span className="text-muted-foreground">{label}:</span>
				<span className="font-medium max-w-[140px] truncate">
					{selected?.name ?? option.current_value}
				</span>
				<ChevronDown className="h-3 w-3 shrink-0 text-muted-foreground" />
			</button>
			{open && (
				<div className="absolute left-0 top-full z-50 mt-1 max-h-64 min-w-[180px] overflow-y-auto rounded-md border bg-popover p-1 shadow-md">
					{option.options.map((opt) => (
						<button
							key={opt.value}
							type="button"
							className={cn(
								"flex w-full items-center gap-2 rounded-sm px-2 py-1 text-xs text-left hover:bg-accent/50 transition-colors",
								opt.value === option.current_value && "bg-accent",
							)}
							onClick={() => {
								onChange(opt.value);
								setOpen(false);
							}}
						>
							<div className="flex-1 min-w-0">
								<div className="truncate">{opt.name}</div>
								{opt.description && (
									<div className="truncate text-[10px] text-muted-foreground">
										{opt.description}
									</div>
								)}
							</div>
							{opt.value === option.current_value && (
								<Check className="h-3 w-3 shrink-0 text-primary" />
							)}
						</button>
					))}
				</div>
			)}
		</div>
	);
}

// ── Floating history popover (for item/paper view) ──────────────────────────

interface HistoryPopoverProps {
	sessions: {
		id: string;
		agentName: string;
		title: string;
		messageCount: number;
		updatedAt: string;
		cwd: string | null;
	}[];
	activeChatId: string | null;
	onNewChat: () => void;
	onSwitchChat: (id: string) => void;
	onDeleteChat: (id: string) => void;
}

const HistoryPopover = React.forwardRef<HTMLDivElement, HistoryPopoverProps>(
	({ sessions, activeChatId, onNewChat, onSwitchChat, onDeleteChat }, ref) => {
		return (
			<div
				ref={ref}
				className="absolute left-0 top-full z-50 mt-1 w-64 rounded-md border bg-popover shadow-lg"
			>
				<div className="flex items-center justify-between px-3 py-2 border-b">
					<span className="text-xs font-medium text-muted-foreground">
						History
					</span>
					<TooltipProvider>
						<Tooltip>
							<TooltipTrigger asChild>
								<Button
									variant="ghost"
									size="icon"
									className="h-6 w-6"
									onClick={onNewChat}
								>
									<MessageSquarePlus className="h-3.5 w-3.5" />
								</Button>
							</TooltipTrigger>
							<TooltipContent>New chat</TooltipContent>
						</Tooltip>
					</TooltipProvider>
				</div>
				<ScrollArea className="max-h-72">
					<div className="p-1">
						{sessions.length === 0 ? (
							<p className="px-2 py-4 text-center text-xs text-muted-foreground">
								No conversations yet
							</p>
						) : (
							sessions.map((session) => (
								<SessionItem
									key={session.id}
									session={session}
									active={activeChatId === session.id}
									onSwitch={() => onSwitchChat(session.id)}
									onDelete={() => onDeleteChat(session.id)}
								/>
							))
						)}
					</div>
				</ScrollArea>
			</div>
		);
	},
);
HistoryPopover.displayName = "HistoryPopover";

function formatTime(iso: string) {
	const d = new Date(iso);
	const now = new Date();
	const diffMs = now.getTime() - d.getTime();
	const diffMin = Math.floor(diffMs / 60000);
	if (diffMin < 1) return "just now";
	if (diffMin < 60) return `${diffMin}m`;
	const diffHr = Math.floor(diffMin / 60);
	if (diffHr < 24) return `${diffHr}h`;
	const diffDay = Math.floor(diffHr / 24);
	if (diffDay < 7) return `${diffDay}d`;
	return d.toLocaleDateString();
}

// ── Session item (shared by sidebar + popover) ──────────────────────────────

function SessionItem({
	session,
	active,
	onSwitch,
	onDelete,
}: {
	session: {
		id: string;
		agentName: string;
		title: string;
		updatedAt: string;
	};
	active: boolean;
	onSwitch: () => void;
	onDelete: () => void;
}) {
	return (
		<div
			className={cn(
				"group flex items-start gap-1 rounded-md px-2 py-1.5 text-xs transition-colors cursor-pointer mb-0.5",
				active ? "bg-accent text-accent-foreground" : "hover:bg-accent/50",
			)}
		>
			<button
				type="button"
				className="flex-1 min-w-0 text-left"
				onClick={onSwitch}
			>
				<div className="truncate font-medium leading-snug">{session.title}</div>
				<div className="flex items-center gap-1 text-[10px] text-muted-foreground mt-0.5">
					<span className="truncate">
						{session.agentName === CHAT_AGENT_NAME ? "Chat" : session.agentName}
					</span>
					<span>·</span>
					<span>{formatTime(session.updatedAt)}</span>
				</div>
			</button>
			<button
				type="button"
				className="shrink-0 mt-0.5 p-0.5 text-muted-foreground opacity-0 group-hover:opacity-100 hover:text-destructive transition-all"
				onClick={(e) => {
					e.stopPropagation();
					onDelete();
				}}
			>
				<Trash2 className="h-3 w-3" />
			</button>
		</div>
	);
}

// ── Left sidebar for chat history (global view only) ────────────────────────

interface SessionMeta {
	id: string;
	agentName: string;
	title: string;
	messageCount: number;
	updatedAt: string;
	cwd: string | null;
}

function ChatSidebar({
	sessions,
	activeChatId,
	onNewChat,
	onSwitchChat,
	onDeleteChat,
	onCollapse,
	papers,
}: {
	sessions: SessionMeta[];
	activeChatId: string | null;
	onNewChat: () => void;
	onSwitchChat: (id: string) => void;
	onDeleteChat: (id: string) => void;
	onCollapse: () => void;
	papers: { slug: string; title: string; short_title: string | null }[];
}) {
	const [paperFolderOpen, setPaperFolderOpen] = useState(false);
	const [expandedPapers, setExpandedPapers] = useState<Set<string>>(new Set());

	const generalSessions = sessions.filter((s) => !s.cwd);
	const paperSessions = sessions.filter((s) => !!s.cwd);

	const paperGroups: {
		cwd: string;
		title: string;
		fullTitle: string;
		sessions: SessionMeta[];
	}[] = [];
	const cwdMap = new Map<string, SessionMeta[]>();
	for (const s of paperSessions) {
		const key = s.cwd as string;
		if (!cwdMap.has(key)) cwdMap.set(key, []);
		cwdMap.get(key)!.push(s);
	}
	for (const [cwd, cwdSessions] of cwdMap) {
		const slug = cwd.split("/").filter(Boolean).pop() ?? cwd;
		const paper = papers.find((p) => p.slug === slug);
		const title = paper?.short_title ?? paper?.title ?? slug;
		const fullTitle = paper?.title ?? slug;
		paperGroups.push({ cwd, title, fullTitle, sessions: cwdSessions });
	}
	paperGroups.sort((a, b) => {
		const aMax = Math.max(
			...a.sessions.map((s) => new Date(s.updatedAt).getTime()),
		);
		const bMax = Math.max(
			...b.sessions.map((s) => new Date(s.updatedAt).getTime()),
		);
		return bMax - aMax;
	});

	useEffect(() => {
		if (!activeChatId) return;
		const active = sessions.find((s) => s.id === activeChatId);
		if (active?.cwd) {
			setPaperFolderOpen(true);
			setExpandedPapers((prev) => {
				if (prev.has(active.cwd as string)) return prev;
				const next = new Set(prev);
				next.add(active.cwd as string);
				return next;
			});
		}
	}, [activeChatId, sessions]);

	const togglePaperGroup = (cwd: string) => {
		setExpandedPapers((prev) => {
			const next = new Set(prev);
			if (next.has(cwd)) next.delete(cwd);
			else next.add(cwd);
			return next;
		});
	};

	return (
		<div className="flex h-full w-56 shrink-0 flex-col border-r bg-muted/20">
			<div className="flex items-center justify-between px-3 py-3">
				<div className="flex items-center gap-1">
					<TooltipProvider>
						<Tooltip>
							<TooltipTrigger asChild>
								<Button
									variant="ghost"
									size="icon"
									className="h-6 w-6"
									onClick={onCollapse}
								>
									<PanelLeftClose className="h-3.5 w-3.5" />
								</Button>
							</TooltipTrigger>
							<TooltipContent>Hide history</TooltipContent>
						</Tooltip>
					</TooltipProvider>
					<span className="text-xs font-medium text-muted-foreground">
						History
					</span>
				</div>
				<TooltipProvider>
					<Tooltip>
						<TooltipTrigger asChild>
							<Button
								variant="ghost"
								size="icon"
								className="h-6 w-6"
								onClick={onNewChat}
							>
								<MessageSquarePlus className="h-3.5 w-3.5" />
							</Button>
						</TooltipTrigger>
						<TooltipContent>New chat</TooltipContent>
					</Tooltip>
				</TooltipProvider>
			</div>

			<ScrollArea className="flex-1">
				<div className="px-2 pb-2">
					{generalSessions.length === 0 && paperSessions.length === 0 && (
						<p className="px-2 py-6 text-center text-xs text-muted-foreground">
							No conversations yet
						</p>
					)}

					{generalSessions.map((session) => (
						<SessionItem
							key={session.id}
							session={session}
							active={activeChatId === session.id}
							onSwitch={() => onSwitchChat(session.id)}
							onDelete={() => onDeleteChat(session.id)}
						/>
					))}

					{paperSessions.length > 0 && (
						<>
							{generalSessions.length > 0 && <div className="my-1.5" />}
							<button
								type="button"
								className="flex w-full items-center gap-1.5 rounded-md px-2 py-1.5 text-xs text-muted-foreground hover:bg-accent/50 transition-colors"
								onClick={() => setPaperFolderOpen((v) => !v)}
							>
								<ChevronRight
									className={cn(
										"h-3 w-3 shrink-0 transition-transform",
										paperFolderOpen && "rotate-90",
									)}
								/>
								{paperFolderOpen ? (
									<FolderOpen className="h-3.5 w-3.5 shrink-0" />
								) : (
									<FolderClosed className="h-3.5 w-3.5 shrink-0" />
								)}
								<span className="font-medium">Paper Chats</span>
								<span className="ml-auto text-[10px] opacity-60">
									{paperSessions.length}
								</span>
							</button>

							{paperFolderOpen && (
								<div className="ml-2 border-l pl-1">
									{paperGroups.map((group) => (
										<div key={group.cwd}>
											<button
												type="button"
												className="flex w-full items-center gap-1.5 rounded-md px-2 py-1 text-xs text-muted-foreground hover:bg-accent/50 transition-colors"
												onClick={() => togglePaperGroup(group.cwd)}
												title={group.fullTitle}
											>
												<ChevronRight
													className={cn(
														"h-2.5 w-2.5 shrink-0 transition-transform",
														expandedPapers.has(group.cwd) && "rotate-90",
													)}
												/>
												<FileText className="h-3 w-3 shrink-0" />
												<span className="flex-1 min-w-0 truncate font-medium text-left">
													{group.title}
												</span>
												<span className="shrink-0 text-[10px] opacity-60">
													{group.sessions.length}
												</span>
											</button>

											{expandedPapers.has(group.cwd) && (
												<div className="ml-2 border-l pl-1">
													{group.sessions.map((session) => (
														<SessionItem
															key={session.id}
															session={session}
															active={activeChatId === session.id}
															onSwitch={() => onSwitchChat(session.id)}
															onDelete={() => onDeleteChat(session.id)}
														/>
													))}
												</div>
											)}
										</div>
									))}
								</div>
							)}
						</>
					)}
				</div>
			</ScrollArea>
		</div>
	);
}

// ── Message bubble ──────────────────────────────────────────────────────────

function MessageBubble({ message }: { message: ChatMessage }) {
	if (message.role === "user") {
		return (
			<div className="flex justify-end">
				<div className="max-w-[85%] rounded-lg bg-primary px-3 py-2 text-primary-foreground">
					{message.images && message.images.length > 0 && (
						<div className="mb-2 flex flex-wrap gap-1">
							{message.images.map((img) => (
								<img
									key={`user-img-${img.mimeType}-${img.base64Data.slice(0, 16)}`}
									src={`data:${img.mimeType};base64,${img.base64Data}`}
									alt="Attached"
									className="h-20 w-auto rounded"
								/>
							))}
						</div>
					)}
					<p className="text-sm whitespace-pre-wrap">{message.text}</p>
				</div>
			</div>
		);
	}

	if (message.role === "agent") {
		return (
			<div className="flex justify-start">
				<div className="max-w-[85%] rounded-lg bg-muted px-3 py-2">
					<div
						className={cn(
							"prose prose-sm dark:prose-invert max-w-none break-words",
							"prose-p:my-1.5 prose-headings:my-2 prose-li:my-0.5",
							"prose-pre:rounded-md prose-pre:bg-black/20 [&_pre]:overflow-x-auto [&_pre]:max-w-full",
							"prose-code:bg-black/20 prose-code:px-1 prose-code:py-0.5 prose-code:rounded prose-code:text-[0.85em] prose-code:before:content-none prose-code:after:content-none",
							"prose-blockquote:border-primary/40",
							"prose-a:text-primary prose-a:no-underline hover:prose-a:underline",
							"prose-ol:pl-4 prose-ul:pl-4",
							"prose-img:rounded-md",
						)}
					>
						<Markdown remarkPlugins={[remarkGfm]}>{message.text}</Markdown>
					</div>
				</div>
			</div>
		);
	}

	if (message.role === "thought") {
		return (
			<div className="flex items-start gap-1.5 text-muted-foreground px-1">
				<Brain className="h-3.5 w-3.5 shrink-0 mt-0.5" />
				<p className="text-xs italic whitespace-pre-wrap">{message.text}</p>
			</div>
		);
	}

	if (message.role === "tool") {
		return <ToolCallBubble message={message} />;
	}

	if (message.role === "plan") {
		return (
			<div className="rounded-md border bg-muted/40 px-2 py-1.5 text-xs">
				<p className="font-medium mb-1">Plan</p>
				{message.planEntries?.map((entry) => (
					<div
						key={`plan-${entry.content}`}
						className="flex items-center gap-1.5 py-0.5"
					>
						{entry.status === "completed" ? (
							<Check className="h-3 w-3 text-green-500" />
						) : (
							<span className="h-3 w-3 rounded-full border shrink-0" />
						)}
						<span className="truncate">{entry.content}</span>
					</div>
				))}
			</div>
		);
	}

	if (message.role === "error") {
		return (
			<div className="flex items-start gap-1.5 rounded-md border border-destructive/30 bg-destructive/10 px-2 py-1.5 text-xs text-destructive">
				<XCircle className="h-3 w-3 shrink-0 mt-0.5" />
				<p className="whitespace-pre-wrap">{message.text}</p>
			</div>
		);
	}

	if (message.role === "separator") {
		return (
			<div className="flex items-center gap-2 py-1.5">
				<div className="flex-1 border-t border-dashed border-muted-foreground/30" />
				<span className="text-[10px] text-muted-foreground/60 shrink-0">
					{message.text}
				</span>
				<div className="flex-1 border-t border-dashed border-muted-foreground/30" />
			</div>
		);
	}

	return null;
}

// ── Tool call bubble with expandable details + confirmation ─────────────────

function ToolCallBubble({ message }: { message: ChatMessage }) {
	const [expanded, setExpanded] = useState(false);
	const confirmTool = useAgentStore((s) => s.confirmTool);

	const isPending = message.toolStatus === "pending_confirmation";
	const hasDetails = message.toolArguments || message.toolResult;

	const statusIcon =
		message.toolStatus === "completed" ? (
			<Check className="h-3 w-3 text-green-500" />
		) : message.toolStatus === "error" ? (
			<XCircle className="h-3 w-3 text-destructive" />
		) : isPending ? (
			<span className="h-3 w-3 rounded-full border-2 border-amber-500 shrink-0" />
		) : (
			<Loader2 className="h-3 w-3 animate-spin" />
		);

	return (
		<div className="rounded-md border bg-muted/40 text-xs">
			<div className="flex items-center gap-1.5 px-2 py-1.5">
				<Wrench className="h-3 w-3 shrink-0 text-muted-foreground" />
				<span className="flex-1 min-w-0 truncate">
					{message.toolTitle || "Tool call"}
				</span>
				{statusIcon}
				{hasDetails && (
					<button
						type="button"
						className="p-0.5 text-muted-foreground hover:text-foreground transition-colors"
						onClick={() => setExpanded((v) => !v)}
					>
						<ChevronRight
							className={cn(
								"h-3 w-3 transition-transform",
								expanded && "rotate-90",
							)}
						/>
					</button>
				)}
			</div>

			{isPending && (
				<div className="flex items-center gap-1.5 px-2 pb-1.5">
					<span className="text-amber-600 text-[10px]">
						Approve this action?
					</span>
					<Button
						variant="outline"
						size="sm"
						className="h-5 px-2 text-[10px]"
						onClick={() => confirmTool(true)}
					>
						<Check className="h-2.5 w-2.5 mr-0.5" />
						Allow
					</Button>
					<Button
						variant="ghost"
						size="sm"
						className="h-5 px-2 text-[10px] text-destructive hover:text-destructive"
						onClick={() => confirmTool(false)}
					>
						<X className="h-2.5 w-2.5 mr-0.5" />
						Reject
					</Button>
				</div>
			)}

			{expanded && hasDetails && (
				<div className="border-t px-2 py-1.5 space-y-1">
					{message.toolArguments && (
						<div>
							<p className="text-[10px] text-muted-foreground font-medium mb-0.5">
								Arguments
							</p>
							<pre className="text-[10px] bg-background rounded p-1 overflow-x-auto max-h-32 whitespace-pre-wrap">
								{formatJson(message.toolArguments)}
							</pre>
						</div>
					)}
					{message.toolResult && (
						<div>
							<p className="text-[10px] text-muted-foreground font-medium mb-0.5">
								Result
							</p>
							<pre className="text-[10px] bg-background rounded p-1 overflow-x-auto max-h-48 whitespace-pre-wrap">
								{formatJson(message.toolResult)}
							</pre>
						</div>
					)}
				</div>
			)}
		</div>
	);
}

function formatJson(text: string): string {
	try {
		return JSON.stringify(JSON.parse(text), null, 2);
	} catch {
		return text;
	}
}
