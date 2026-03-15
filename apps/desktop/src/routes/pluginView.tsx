// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

/**
 * PluginView — Full-page view for plugin sidebar_nav contributions.
 *
 * Displayed in the main content area (alongside Library, Feed, etc.)
 * when the user clicks the "Plugins" section in the left sidebar.
 */
import { createPluginSDK } from "@/plugins/PluginManager";
import { usePluginStore } from "@/plugins/pluginStore";
import { Puzzle } from "lucide-react";
import { useMemo } from "react";
import { useTranslation } from "react-i18next";

export function PluginView() {
	const { t } = useTranslation();
	const plugins = usePluginStore((s) => s.plugins);
	const loadedModules = usePluginStore((s) => s.loadedModules);
	const getContributions = usePluginStore((s) => s.getContributionsForSlot);

	const contributions = useMemo(() => {
		void plugins;
		void loadedModules;
		return getContributions("sidebar_nav");
	}, [plugins, loadedModules, getContributions]);

	// No plugins registered
	if (contributions.length === 0) {
		return (
			<div className="flex h-full items-center justify-center">
				<div className="text-center space-y-3">
					<Puzzle className="h-12 w-12 mx-auto text-muted-foreground/50" />
					<p className="text-sm text-muted-foreground">
						{t("plugins.noPluginViews")}
					</p>
				</div>
			</div>
		);
	}

	// Render the first plugin view as the main content.
	// If there are multiple plugins, we could add a tab bar here in the future.
	const contrib = contributions[0];
	const pluginInfo = plugins.find((p) => p.manifest.id === contrib.pluginId);
	if (!pluginInfo) return null;

	const Component = contrib.component;
	const sdk = createPluginSDK(pluginInfo);

	return (
		<div className="h-full overflow-hidden">
			<Component sdk={sdk} context={{}} />
		</div>
	);
}
