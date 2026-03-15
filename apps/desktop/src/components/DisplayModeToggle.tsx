// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import {
	Tooltip,
	TooltipContent,
	TooltipProvider,
	TooltipTrigger,
} from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import { useTranslationStore } from "@/stores/translationStore";
import { FileText, Languages, Split } from "lucide-react";
import { useTranslation } from "react-i18next";

/**
 * A compact 3-state toggle for switching between display modes:
 * - original: show only original text
 * - bilingual: show both original and translated (immersive-translation style)
 * - translated: show only translated text
 */
export function DisplayModeToggle() {
	const { t } = useTranslation();
	const displayMode = useTranslationStore((s) => s.displayMode);
	const setDisplayMode = useTranslationStore((s) => s.setDisplayMode);
	const aiConfig = useTranslationStore((s) => s.aiConfig);

	// Don't show toggle if translation is not configured
	if (!aiConfig?.nativeLang) {
		return null;
	}

	const itemClass = (value: string) =>
		cn(
			"px-2 h-7",
			displayMode === value &&
				"bg-primary/15 text-primary hover:bg-primary/20 hover:text-primary",
		);

	return (
		<TooltipProvider delayDuration={300}>
			<ToggleGroup
				type="single"
				value={displayMode}
				onValueChange={(v) => {
					if (v) setDisplayMode(v as "bilingual" | "translated" | "original");
				}}
				className="border rounded-md"
			>
				<Tooltip>
					<TooltipTrigger asChild>
						<ToggleGroupItem
							value="original"
							size="sm"
							className={itemClass("original")}
						>
							<FileText className="h-3.5 w-3.5" />
						</ToggleGroupItem>
					</TooltipTrigger>
					<TooltipContent>{t("reader.originalOnly")}</TooltipContent>
				</Tooltip>
				<Tooltip>
					<TooltipTrigger asChild>
						<ToggleGroupItem
							value="bilingual"
							size="sm"
							className={itemClass("bilingual")}
						>
							<Split className="h-3.5 w-3.5" />
						</ToggleGroupItem>
					</TooltipTrigger>
					<TooltipContent>{t("reader.bilingual")}</TooltipContent>
				</Tooltip>
				<Tooltip>
					<TooltipTrigger asChild>
						<ToggleGroupItem
							value="translated"
							size="sm"
							className={itemClass("translated")}
						>
							<Languages className="h-3.5 w-3.5" />
						</ToggleGroupItem>
					</TooltipTrigger>
					<TooltipContent>{t("reader.translatedOnly")}</TooltipContent>
				</Tooltip>
			</ToggleGroup>
		</TooltipProvider>
	);
}
