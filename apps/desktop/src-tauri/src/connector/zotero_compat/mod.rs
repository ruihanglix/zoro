// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

pub mod handlers;
pub mod mapping;
pub mod server;
pub mod session;
pub mod types;

use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;

/// Spawn the Zotero-compatible connector server as a background task.
/// Returns the CancellationToken that can be used to stop it.
pub fn spawn_zotero_compat_server(app: AppHandle, port: u16) {
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    // Store the cancel token in AppState and clear any previous error
    let app_state: tauri::State<crate::AppState> = app.state();
    if let Ok(mut guard) = app_state.zotero_compat_cancel.lock() {
        if let Some(old) = guard.take() {
            old.cancel();
        }
        *guard = Some(cancel.clone());
    }
    let _ = app_state.zotero_compat_error.lock().map(|mut g| *g = None);

    let app_for_event = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = server::run_zotero_compat_server(app, port, cancel_clone).await {
            let err_str = e.to_string();
            let is_port_conflict = err_str.contains("Address already in use")
                || err_str.contains("address already in use")
                || err_str.contains("AddrInUse");

            let message = if is_port_conflict {
                format!(
                    "Port {} is already in use. Another application (e.g. Zotero) \
                     may be occupying this port. The Zotero Connector compatibility \
                     server could not start.",
                    port
                )
            } else {
                format!("Zotero compat server failed to start: {}", e)
            };

            tracing::error!("{}", message);
            let _ = app_for_event.emit("zotero-compat-error", &message);

            // Store error for status queries
            {
                let state = app_for_event.state::<crate::AppState>();
                let _ = state
                    .zotero_compat_error
                    .lock()
                    .map(|mut g| *g = Some(message));
            }
            // Clear the cancel token so status correctly shows "Stopped"
            {
                let state = app_for_event.state::<crate::AppState>();
                let _ = state.zotero_compat_cancel.lock().map(|mut g| {
                    g.take();
                });
            }
        }
    });

    tracing::info!("Zotero compat server spawned on port {}", port);
}

/// Stop the Zotero-compatible connector server if running.
pub fn stop_zotero_compat_server(app: &AppHandle) {
    let app_state: tauri::State<crate::AppState> = app.state();
    if let Ok(mut guard) = app_state.zotero_compat_cancel.lock() {
        if let Some(cancel) = guard.take() {
            cancel.cancel();
            tracing::info!("Zotero compat server stopped");
        }
    };
}
