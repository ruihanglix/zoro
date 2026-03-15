// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { ZoteroImportDialog } from "@/components/library/ZoteroImportDialog";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import * as commands from "@/lib/commands";
import { supportedLanguages } from "@/lib/i18n";
import type { SupportedLanguage } from "@/lib/i18n";
import { useTranslationStore } from "@/stores/translationStore";
import { useUiStore } from "@/stores/uiStore";
import {
	BookOpen,
	Check,
	Download,
	FileText,
	Globe,
	Languages,
	Lightbulb,
	Loader2,
	MousePointerClick,
	Puzzle,
	Sparkles,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

const ONBOARDING_KEY = "zoro-onboarding-completed";

// Native language options (same as settings page)
const nativeLanguageOptions = [
	{ value: "zh", label: "简体中文" },
	{ value: "ja", label: "日本語" },
	{ value: "ko", label: "한국어" },
	{ value: "es", label: "Español" },
	{ value: "fr", label: "Français" },
	{ value: "de", label: "Deutsch" },
	{ value: "pt", label: "Português" },
	{ value: "ru", label: "Русский" },
	{ value: "ar", label: "العربية" },
	{ value: "it", label: "Italiano" },
	{ value: "vi", label: "Tiếng Việt" },
	{ value: "th", label: "ไทย" },
	{ value: "id", label: "Bahasa Indonesia" },
	{ value: "hi", label: "हिन्दी" },
	{ value: "en", label: "English" },
] as const;

// Infer native language from UI language code
function inferNativeLang(uiLang: string): string {
	// Map UI language codes to native language codes
	const map: Record<string, string> = {
		"zh-CN": "zh",
		ja: "ja",
		ko: "ko",
		es: "es",
		fr: "fr",
		de: "de",
		pt: "pt",
		ru: "ru",
		en: "",
	};
	return map[uiLang] ?? "";
}

export function isOnboardingCompleted(): boolean {
	return localStorage.getItem(ONBOARDING_KEY) === "true";
}

export function resetOnboarding(): void {
	localStorage.removeItem(ONBOARDING_KEY);
}

// Provider presets (static, shared between component and init logic)
const providerPresets = [
	{
		name: "OpenAI",
		baseUrl: "https://api.openai.com/v1",
		model: "gpt-4o-mini",
	},
	{
		name: "Custom",
		baseUrl: "",
		model: "",
	},
];

// Infer preset name from a base URL
function inferPresetFromUrl(baseUrl: string): string {
	const match = providerPresets.find(
		(p) => p.name !== "Custom" && p.baseUrl === baseUrl,
	);
	return match?.name ?? (baseUrl ? "Custom" : "OpenAI");
}

export function OnboardingOverlay({ onComplete }: { onComplete: () => void }) {
	const { t } = useTranslation();
	const language = useUiStore((s) => s.language);
	const setLanguage = useUiStore((s) => s.setLanguage);
	const aiConfig = useTranslationStore((s) => s.aiConfig);
	const fetchAiConfig = useTranslationStore((s) => s.fetchAiConfig);
	const displayModeStore = useTranslationStore((s) => s.displayMode);

	// Language state — use existing config if available, otherwise infer from system language
	const [nativeLang, setNativeLang] = useState(
		() => aiConfig?.nativeLang || inferNativeLang(language),
	);
	const [displayMode, setDisplayMode] = useState<
		"original" | "bilingual" | "translated"
	>(() => displayModeStore);

	// AI state — use existing config if available, otherwise use defaults
	const [aiBaseUrl, setAiBaseUrl] = useState(
		() => aiConfig?.baseUrl || "https://api.openai.com/v1",
	);
	const [aiApiKey, setAiApiKey] = useState("");
	const [aiModel, setAiModel] = useState(
		() => aiConfig?.model || "gpt-4o-mini",
	);
	const [aiTesting, setAiTesting] = useState(false);
	const [aiTestResult, setAiTestResult] = useState<{
		ok: boolean;
		msg: string;
	} | null>(null);
	const [zoteroImportOpen, setZoteroImportOpen] = useState(false);

	const [selectedPreset, setSelectedPreset] = useState(() =>
		inferPresetFromUrl(aiConfig?.baseUrl || "https://api.openai.com/v1"),
	);

	// Only auto-infer native lang from UI language on first-time onboarding (no existing config)
	useEffect(() => {
		if (!aiConfig?.nativeLang) {
			setNativeLang(inferNativeLang(language));
		}
	}, [language, aiConfig?.nativeLang]);

	const handlePresetChange = useCallback(
		(presetName: string) => {
			setSelectedPreset(presetName);
			const preset = providerPresets.find((p) => p.name === presetName);
			if (preset) {
				setAiBaseUrl(preset.baseUrl);
				setAiModel(preset.model);
				setAiTestResult(null);
			}
		},
		// providerPresets is a static const, safe to omit from deps
		// eslint-disable-next-line react-hooks/exhaustive-deps
		[],
	);

	const handleTestConnection = async () => {
		setAiTesting(true);
		setAiTestResult(null);
		try {
			// Save config first so backend can test
			await commands.updateAiConfig({
				provider: "",
				baseUrl: aiBaseUrl,
				apiKey: aiApiKey || undefined,
				model: aiModel,
			});
			const msg = await commands.testAiConnection();
			setAiTestResult({ ok: true, msg });
		} catch (err) {
			setAiTestResult({ ok: false, msg: String(err) });
		} finally {
			setAiTesting(false);
		}
	};

	const handleSkip = () => {
		localStorage.setItem(ONBOARDING_KEY, "true");
		onComplete();
	};

	const handleGetStarted = async () => {
		try {
			// Save language & AI preferences
			await commands.updateAiConfig({
				nativeLang: nativeLang,
				provider: "",
				baseUrl: aiBaseUrl,
				...(aiApiKey ? { apiKey: aiApiKey } : {}),
				model: aiModel,
			});
			await fetchAiConfig();
		} catch (err) {
			console.error("Failed to save onboarding config:", err);
		}

		localStorage.setItem(ONBOARDING_KEY, "true");
		onComplete();
	};

	return (
		<div className="fixed inset-0 z-[9999] flex items-center justify-center bg-background/80 backdrop-blur-sm">
			<div className="w-full max-w-lg mx-4 rounded-xl border bg-background shadow-2xl overflow-hidden">
				{/* Header */}
				<div className="px-6 pt-6 pb-4 text-center">
					<div className="inline-flex items-center justify-center w-12 h-12 rounded-full bg-primary/10 mb-3">
						<BookOpen className="h-6 w-6 text-primary" />
					</div>
					<h1 className="text-xl font-bold">{t("onboarding.welcomeTitle")}</h1>
					<p className="text-sm text-muted-foreground mt-1">
						{t("onboarding.welcomeSubtitle")}
					</p>
				</div>

				<div className="px-6 pb-6 space-y-4 max-h-[60vh] overflow-y-auto">
					{/* Language Section */}
					<div className="rounded-lg border p-4 space-y-3">
						<div className="flex items-center gap-2">
							<Globe className="h-4 w-4 text-primary" />
							<h2 className="text-sm font-semibold">
								{t("onboarding.languageSection")}
							</h2>
						</div>

						{/* Interface Language */}
						<div>
							<label
								htmlFor="ob-ui-lang"
								className="text-xs font-medium text-muted-foreground"
							>
								{t("onboarding.interfaceLanguage")}
							</label>
							<select
								id="ob-ui-lang"
								value={language}
								onChange={(e) =>
									setLanguage(e.target.value as SupportedLanguage)
								}
								className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm"
							>
								{supportedLanguages.map((lang) => (
									<option key={lang.code} value={lang.code}>
										{lang.label}
									</option>
								))}
							</select>
						</div>

						{/* Native Language */}
						<div>
							<label
								htmlFor="ob-native-lang"
								className="text-xs font-medium text-muted-foreground"
							>
								{t("onboarding.nativeLanguage")}
							</label>
							<select
								id="ob-native-lang"
								value={nativeLang}
								onChange={(e) => setNativeLang(e.target.value)}
								className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm"
							>
								<option value="">{t("settings.disabled")}</option>
								{nativeLanguageOptions.map((opt) => (
									<option key={opt.value} value={opt.value}>
										{opt.label}
									</option>
								))}
							</select>
							<p className="text-[11px] text-muted-foreground mt-0.5">
								{t("onboarding.nativeLanguageDesc")}
							</p>
						</div>

						{/* Display Mode */}
						<div>
							<span className="text-xs font-medium text-muted-foreground">
								{t("onboarding.displayMode")}
							</span>
							<div className="mt-1 flex gap-1">
								{(["original", "bilingual", "translated"] as const).map(
									(mode) => (
										<button
											key={mode}
											type="button"
											onClick={() => setDisplayMode(mode)}
											className={`flex-1 h-8 rounded-md text-xs font-medium transition-colors ${
												displayMode === mode
													? "bg-primary text-primary-foreground"
													: "bg-muted text-muted-foreground hover:bg-muted/80"
											}`}
										>
											{t(`displayMode.${mode}`)}
										</button>
									),
								)}
							</div>
						</div>
					</div>

					{/* AI Configuration Section */}
					<div className="rounded-lg border p-4 space-y-3">
						<div className="flex items-center gap-2">
							<Sparkles className="h-4 w-4 text-primary" />
							<h2 className="text-sm font-semibold">
								{t("onboarding.aiSection")}
							</h2>
							<span className="text-[10px] text-muted-foreground bg-muted px-1.5 py-0.5 rounded">
								{t("onboarding.aiSectionOptional")}
							</span>
						</div>

						{/* Provider Preset */}
						<div>
							<span className="text-xs font-medium text-muted-foreground">
								{t("onboarding.aiProvider")}
							</span>
							<div className="mt-1 flex gap-1">
								{providerPresets.map((preset) => (
									<button
										key={preset.name}
										type="button"
										onClick={() => handlePresetChange(preset.name)}
										className={`flex-1 h-8 rounded-md text-xs font-medium transition-colors ${
											selectedPreset === preset.name
												? "bg-primary text-primary-foreground"
												: "bg-muted text-muted-foreground hover:bg-muted/80"
										}`}
									>
										{preset.name}
									</button>
								))}
							</div>
						</div>

						{/* Base URL — shown for Custom */}
						{selectedPreset === "Custom" && (
							<div>
								<label
									htmlFor="ob-base-url"
									className="text-xs font-medium text-muted-foreground"
								>
									{t("onboarding.aiBaseUrl")}
								</label>
								<input
									id="ob-base-url"
									type="text"
									value={aiBaseUrl}
									onChange={(e) => {
										setAiBaseUrl(e.target.value);
										setAiTestResult(null);
									}}
									placeholder={t("onboarding.aiBaseUrlPlaceholder")}
									className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm"
								/>
							</div>
						)}

						{/* API Key */}
						<div>
							<label
								htmlFor="ob-api-key"
								className="text-xs font-medium text-muted-foreground"
							>
								{t("onboarding.aiApiKey")}
							</label>
							<input
								id="ob-api-key"
								type="password"
								value={aiApiKey}
								onChange={(e) => {
									setAiApiKey(e.target.value);
									setAiTestResult(null);
								}}
								placeholder={t("onboarding.aiApiKeyPlaceholder")}
								className="mt-1 h-8 w-full rounded-md border bg-transparent px-2 text-sm"
							/>
						</div>

						{/* Model */}
						<div>
							<label
								htmlFor="ob-model"
								className="text-xs font-medium text-muted-foreground"
							>
								{t("onboarding.aiModel")}
							</label>
							<div className="mt-1 flex gap-2">
								<input
									id="ob-model"
									type="text"
									value={aiModel}
									onChange={(e) => {
										setAiModel(e.target.value);
										setAiTestResult(null);
									}}
									className="h-8 flex-1 rounded-md border bg-transparent px-2 text-sm"
								/>
								<Button
									variant="outline"
									size="sm"
									onClick={handleTestConnection}
									disabled={aiTesting || !aiModel}
									className="h-8 shrink-0"
								>
									{aiTesting ? (
										<Loader2 className="h-3.5 w-3.5 animate-spin" />
									) : aiTestResult?.ok ? (
										<>
											<Check className="mr-1 h-3.5 w-3.5 text-green-600" />
											<span className="text-green-600">
												{t("onboarding.aiTestSuccess")}
											</span>
										</>
									) : (
										t("onboarding.aiTestConnection")
									)}
								</Button>
							</div>
							{aiTestResult && !aiTestResult.ok && (
								<p className="text-[11px] text-destructive mt-1">
									{aiTestResult.msg}
								</p>
							)}
						</div>
					</div>

					{/* Quick Tips Section */}
					<div className="rounded-lg border p-4 space-y-2">
						<div className="flex items-center gap-2 mb-1">
							<Lightbulb className="h-4 w-4 text-primary" />
							<h2 className="text-sm font-semibold">
								{t("onboarding.tipsSection")}
							</h2>
						</div>
						<div className="grid gap-1.5">
							<div className="flex items-start gap-2.5 text-xs text-muted-foreground">
								<Download className="h-3.5 w-3.5 mt-0.5 shrink-0 text-primary/70" />
								<span>{t("onboarding.tipDragDrop")}</span>
							</div>
							<div className="flex items-start gap-2.5 text-xs text-muted-foreground">
								<Puzzle className="h-3.5 w-3.5 mt-0.5 shrink-0 text-primary/70" />
								<span>{t("onboarding.tipBrowserExtension")}</span>
							</div>
							<div className="flex items-start gap-2.5 text-xs text-muted-foreground">
								<Languages className="h-3.5 w-3.5 mt-0.5 shrink-0 text-primary/70" />
								<span>{t("onboarding.tipBilingualReading")}</span>
							</div>
							<div className="flex items-start gap-2.5 text-xs text-muted-foreground">
								<MousePointerClick className="h-3.5 w-3.5 mt-0.5 shrink-0 text-primary/70" />
								<span>{t("onboarding.tipRightClick")}</span>
							</div>
							<div className="flex items-start gap-2.5 text-xs text-muted-foreground">
								<FileText className="h-3.5 w-3.5 mt-0.5 shrink-0 text-primary/70" />
								<button
									type="button"
									className="text-left text-primary hover:underline"
									onClick={() => setZoteroImportOpen(true)}
								>
									{t("onboarding.tipZoteroImport")}
								</button>
							</div>
						</div>
					</div>
				</div>

				<Separator />

				{/* Footer */}
				<div className="px-6 py-4 flex justify-between items-center">
					<Button variant="ghost" size="sm" onClick={handleSkip}>
						{t("onboarding.skip")}
					</Button>
					<Button size="sm" onClick={handleGetStarted}>
						{t("onboarding.getStarted")}
						<span className="ml-1">→</span>
					</Button>
				</div>
			</div>
			<ZoteroImportDialog
				open={zoteroImportOpen}
				onClose={() => setZoteroImportOpen(false)}
			/>
		</div>
	);
}
