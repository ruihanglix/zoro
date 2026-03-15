// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use super::handlers;
use super::session::SessionStore;
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tauri::AppHandle;
use tokio_util::sync::CancellationToken;
use tower_http::cors::{Any, CorsLayer};

pub struct ZoteroCompatState {
    pub app: AppHandle,
    pub sessions: SessionStore,
}

pub async fn run_zotero_compat_server(
    app: AppHandle,
    port: u16,
    cancel: CancellationToken,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let state = Arc::new(ZoteroCompatState {
        app,
        sessions: SessionStore::new(),
    });

    // Spawn session cleanup task
    let sessions_clone = state.sessions.clone();
    let cancel_cleanup = cancel.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_secs(300)) => {
                    sessions_clone.cleanup_expired();
                }
                _ = cancel_cleanup.cancelled() => break,
            }
        }
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let router = Router::new()
        // Core endpoints
        .route("/connector/ping", post(handlers::ping))
        .route(
            "/connector/getSelectedCollection",
            post(handlers::get_selected_collection),
        )
        .route("/connector/saveItems", post(handlers::save_items))
        .route("/connector/saveSnapshot", post(handlers::save_snapshot))
        // Attachment endpoints
        .route("/connector/saveAttachment", post(handlers::save_attachment))
        .route(
            "/connector/saveStandaloneAttachment",
            post(handlers::save_standalone_attachment),
        )
        .route(
            "/connector/saveSingleFile",
            post(handlers::save_single_file),
        )
        // Session endpoints
        .route(
            "/connector/sessionProgress",
            post(handlers::session_progress),
        )
        .route("/connector/updateSession", post(handlers::update_session))
        // Import
        .route("/connector/import", post(handlers::import))
        // Stub endpoints
        .route("/connector/getTranslators", post(handlers::get_translators))
        .route(
            "/connector/getTranslatorCode",
            post(handlers::get_translator_code),
        )
        .route("/connector/delaySync", post(handlers::delay_sync))
        .route("/connector/proxies", get(handlers::proxies))
        .route(
            "/connector/getClientHostnames",
            get(handlers::get_client_hostnames),
        )
        .route(
            "/connector/getRecognizedItem",
            post(handlers::get_recognized_item),
        )
        .route(
            "/connector/hasAttachmentResolvers",
            post(handlers::has_attachment_resolvers),
        )
        .route("/connector/installStyle", post(handlers::install_style))
        .layer(cors)
        .with_state(state);

    let addr = format!("127.0.0.1:{}", port);
    tracing::info!("Starting Zotero compat server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            cancel.cancelled().await;
            tracing::info!("Zotero compat server shutting down");
        })
        .await?;

    Ok(())
}
