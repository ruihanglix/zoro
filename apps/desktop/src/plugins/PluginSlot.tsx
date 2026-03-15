// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { useMemo } from "react";
import { createPluginSDK } from "./PluginManager";
import { usePluginStore } from "./pluginStore";
import type { SlotLocation } from "./types";

interface PluginSlotProps {
	location: SlotLocation;
	context?: Record<string, unknown>;
	className?: string;
}

/**
 * Renders all plugin contributions for a given slot location.
 * Each plugin component receives the SDK and optional context.
 */
export function PluginSlot({ location, context, className }: PluginSlotProps) {
	const plugins = usePluginStore((s) => s.plugins);
	const loadedModules = usePluginStore((s) => s.loadedModules);
	const getContributions = usePluginStore((s) => s.getContributionsForSlot);

	// getContributions reads from plugins/loadedModules internally,
	// so we include them as deps to trigger re-computation.
	const contributions = useMemo(() => {
		// Reference plugins + loadedModules to trigger useMemo when they change
		void plugins;
		void loadedModules;
		return getContributions(location);
	}, [location, plugins, loadedModules, getContributions]);

	if (contributions.length === 0) return null;

	return (
		<div className={className}>
			{contributions.map((contrib) => {
				const Component = contrib.component;
				const pluginInfo = plugins.find(
					(p) => p.manifest.id === contrib.pluginId,
				);
				if (!pluginInfo) return null;

				const sdk = createPluginSDK(pluginInfo, {
					paperId: context?.paperId as string | undefined,
				});

				return (
					<Component
						key={`${contrib.pluginId}-${contrib.contribution.id}`}
						sdk={sdk}
						context={context}
					/>
				);
			})}
		</div>
	);
}
