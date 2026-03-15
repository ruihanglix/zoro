// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

pub mod handlers;
pub mod server;
pub mod zotero_compat;

use tauri::AppHandle;

pub async fn start_server(
    app: AppHandle,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    server::run_server(app, port).await
}
