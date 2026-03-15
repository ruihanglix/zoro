// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { ping } from "../lib/api";

let isConnected = false;

export async function checkConnection(): Promise<boolean> {
  isConnected = await ping();
  return isConnected;
}

export function getConnectionStatus(): boolean {
  return isConnected;
}

// Periodically check connection
setInterval(async () => {
  await checkConnection();
}, 30000); // Every 30 seconds
