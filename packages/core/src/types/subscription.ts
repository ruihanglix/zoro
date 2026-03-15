// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

export interface SubscriptionSourceInfo {
  source_type: string;
  display_name: string;
  description: string;
  default_config: Record<string, unknown>;
}
