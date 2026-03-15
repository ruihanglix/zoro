// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

mod resources;
mod server;
mod state;
mod tools;

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use rmcp::ServiceExt;
use tracing_subscriber::EnvFilter;

use server::ZoroMcpServer;
use state::AppState;

#[derive(Debug, Clone, clap::ValueEnum)]
enum Transport {
    Stdio,
    Http,
}

#[derive(Parser, Debug)]
#[command(
    name = "zoro-mcp",
    about = "Zoro MCP Server — AI-native literature management for agents",
    version
)]
struct Args {
    /// Transport type
    #[arg(long, value_enum, default_value = "stdio")]
    transport: Transport,

    /// HTTP port (only used with --transport http)
    #[arg(long, default_value = "23121")]
    port: u16,

    /// Zoro data directory
    #[arg(long)]
    data_dir: Option<String>,
}

fn resolve_data_dir(arg: Option<&str>) -> PathBuf {
    if let Some(dir) = arg {
        PathBuf::from(dir)
    } else if let Ok(dir) = std::env::var("ZORO_DATA_DIR") {
        PathBuf::from(dir)
    } else {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".zoro")
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Only enable tracing to stderr (stdout is reserved for MCP stdio transport)
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("zoro=info".parse().unwrap()))
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();
    let data_dir = resolve_data_dir(args.data_dir.as_deref());

    tracing::info!("Data directory: {:?}", data_dir);

    // Ensure data directory exists
    zoro_storage::init_data_dir(&data_dir)?;

    // Open database
    let db_path = data_dir.join("library.db");
    let db = zoro_db::Database::open(&db_path)
        .map_err(|e| format!("Failed to open database at {:?}: {}", db_path, e))?;

    tracing::info!("Database opened at {:?}", db_path);

    // Build initial library-index.json
    zoro_storage::sync::rebuild_library_index(&db, &data_dir);

    let state = Arc::new(AppState::new(db, data_dir));
    let server = ZoroMcpServer::new(state);

    match args.transport {
        Transport::Stdio => {
            tracing::info!("Starting MCP server on stdio");
            let service = server
                .serve(tokio::io::join(tokio::io::stdin(), tokio::io::stdout()))
                .await?;
            service.waiting().await?;
        }
        Transport::Http => {
            tracing::info!("Starting MCP server on HTTP port {}", args.port);

            let ct = tokio_util::sync::CancellationToken::new();
            let config = rmcp::transport::StreamableHttpServerConfig {
                stateful_mode: true,
                cancellation_token: ct.child_token(),
                ..Default::default()
            };

            let service = rmcp::transport::StreamableHttpService::<
                _,
                rmcp::transport::streamable_http_server::session::local::LocalSessionManager,
            >::new(move || Ok(server.clone()), Default::default(), config);

            let app = axum::Router::new().nest_service("/mcp", service);
            let tcp = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", args.port)).await?;
            tracing::info!(
                "HTTP server listening on http://127.0.0.1:{}/mcp",
                args.port
            );

            axum::serve(tcp, app)
                .with_graceful_shutdown(async move { ct.cancelled_owned().await })
                .await?;
        }
    }

    Ok(())
}
