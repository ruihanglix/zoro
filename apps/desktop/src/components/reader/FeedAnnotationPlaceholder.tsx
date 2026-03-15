// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { BookOpen, Highlighter } from "lucide-react";
import { useTranslation } from "react-i18next";

export function FeedAnnotationPlaceholder() {
	const { t } = useTranslation();
	return (
		<div className="flex h-full flex-col border-r">
			<div className="flex items-center gap-2 border-b px-3 py-2">
				<Highlighter className="h-3.5 w-3.5 text-muted-foreground" />
				<span className="text-xs font-medium">{t("reader.annotations")}</span>
			</div>
			<div className="flex-1 flex items-center justify-center p-4">
				<div className="text-center">
					<BookOpen className="mx-auto mb-2 h-8 w-8 text-muted-foreground opacity-50" />
					<p className="text-[11px] text-muted-foreground">
						{t("reader.addToLibraryForAnnotations")}
					</p>
				</div>
			</div>
		</div>
	);
}
