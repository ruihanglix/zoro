// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

export { PluginSlot } from "./PluginSlot";
export { usePluginStore } from "./pluginStore";
export { injectPluginSharedDeps } from "./PluginSharedDeps";
export { createPluginSDK, loadPluginModule } from "./PluginManager";
export type {
	PluginComponentProps,
	LoadedPluginModule,
	SlotLocation,
	ContributionWithPlugin,
	PluginSDKInstance,
} from "./types";
