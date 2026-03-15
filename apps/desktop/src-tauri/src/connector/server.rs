// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use super::handlers;
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tauri::AppHandle;
use tower_http::cors::{Any, CorsLayer};

pub struct ConnectorState {
    pub app: AppHandle,
}

pub async fn run_server(
    app: AppHandle,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let state = Arc::new(ConnectorState { app });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let router = Router::new()
        .route("/connector/ping", get(handlers::ping))
        .route("/connector/saveItem", post(handlers::save_item))
        .route("/connector/saveHtml", post(handlers::save_html))
        .route("/connector/status", get(handlers::status))
        .route("/connector/collections", get(handlers::list_collections))
        .layer(cors)
        .with_state(state);

    let addr = format!("127.0.0.1:{}", port);
    tracing::info!("Starting connector server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
