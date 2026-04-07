// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

mod backend;
mod commands;
mod config;
mod output;

use std::process;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "zoro",
    about = "Zoro CLI — AI-native literature management from the command line",
    version
)]
struct Cli {
    /// Output JSON instead of human-friendly tables (for agent/script use)
    #[arg(long, global = true)]
    json: bool,

    /// Custom data directory (default: ~/.zoro, or ZORO_DATA_DIR env)
    #[arg(long, global = true)]
    data_dir: Option<String>,

    /// Force local SQLite backend (skip HTTP connector detection)
    #[arg(long, global = true)]
    local: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Search papers (full-text search)
    Search {
        /// Search query
        query: String,
        /// Maximum results
        #[arg(long, default_value = "20")]
        limit: i64,
    },

    /// List papers with optional filters
    List {
        /// Filter by collection name or ID
        #[arg(long)]
        collection: Option<String>,
        /// Filter by tag name
        #[arg(long)]
        tag: Option<String>,
        /// Filter by read status (unread, reading, read)
        #[arg(long)]
        status: Option<String>,
        /// Maximum results
        #[arg(long, default_value = "50")]
        limit: i64,
    },

    /// Get paper details by ID or slug
    Get {
        /// Paper ID or slug
        paper: String,
    },

    /// Add a paper (by PDF path, URL, DOI, or arXiv ID)
    Add {
        /// PDF file path, URL, DOI, or arXiv ID
        source: String,
    },

    /// Open a paper in the desktop app
    Open {
        /// Paper ID or slug
        paper: String,
    },

    /// Delete a paper
    Delete {
        /// Paper ID or slug
        paper: String,
    },

    /// Manage collections
    Collections {
        #[command(subcommand)]
        action: CollectionCommands,
    },

    /// Manage tags
    Tags {
        #[command(subcommand)]
        action: TagCommands,
    },

    /// Manage notes
    Notes {
        #[command(subcommand)]
        action: NoteCommands,
    },

    /// Export paper citations
    Export {
        /// Paper ID or slug
        paper: String,
        /// Export format
        #[arg(long, default_value = "bibtex")]
        format: String,
    },

    /// Show connection status and library statistics
    Status,

    /// Install the zoro CLI command to system PATH
    InstallCli,

    /// Uninstall the zoro CLI command from system PATH
    UninstallCli,
}

#[derive(Subcommand)]
enum CollectionCommands {
    /// List all collections
    List,
    /// Create a new collection
    Create {
        /// Collection name
        name: String,
        /// Optional description
        #[arg(long)]
        description: Option<String>,
    },
    /// Add a paper to a collection
    Add {
        /// Paper ID or slug
        paper: String,
        /// Collection name or ID
        collection: String,
    },
    /// Remove a paper from a collection
    Remove {
        /// Paper ID or slug
        paper: String,
        /// Collection name or ID
        collection: String,
    },
}

#[derive(Subcommand)]
enum TagCommands {
    /// List all tags
    List,
    /// Add a tag to a paper
    Add {
        /// Paper ID or slug
        paper: String,
        /// Tag name
        tag: String,
    },
    /// Remove a tag from a paper
    Remove {
        /// Paper ID or slug
        paper: String,
        /// Tag name
        tag: String,
    },
}

#[derive(Subcommand)]
enum NoteCommands {
    /// List notes for a paper
    List {
        /// Paper ID or slug
        paper: String,
    },
    /// Add a note to a paper
    Add {
        /// Paper ID or slug
        paper: String,
        /// Note content
        content: String,
    },
    /// Delete a note
    Delete {
        /// Note ID
        note_id: String,
    },
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("zoro=warn".parse().unwrap()),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let data_dir = config::resolve_data_dir(cli.data_dir.as_deref());

    let result = run(cli, data_dir).await;
    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

async fn run(
    cli: Cli,
    data_dir: std::path::PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let json = cli.json;

    let backend = backend::connect(&data_dir, cli.local).await?;

    match cli.command {
        Commands::Search { query, limit } => {
            commands::papers::search(&*backend, &query, limit, json)?;
        }
        Commands::List {
            collection,
            tag,
            status,
            limit,
        } => {
            commands::papers::list(
                &*backend,
                collection.as_deref(),
                tag.as_deref(),
                status.as_deref(),
                limit,
                json,
            )?;
        }
        Commands::Get { paper } => {
            commands::papers::get(&*backend, &paper, json)?;
        }
        Commands::Add { source } => {
            commands::papers::add(&*backend, &source, json)?;
        }
        Commands::Open { paper } => {
            commands::papers::open(&*backend, &paper)?;
        }
        Commands::Delete { paper } => {
            commands::papers::delete(&*backend, &paper, json)?;
        }
        Commands::Collections { action } => match action {
            CollectionCommands::List => {
                commands::collections::list(&*backend, json)?;
            }
            CollectionCommands::Create { name, description } => {
                commands::collections::create(&*backend, &name, description.as_deref(), json)?;
            }
            CollectionCommands::Add { paper, collection } => {
                commands::collections::add_paper(&*backend, &paper, &collection, json)?;
            }
            CollectionCommands::Remove { paper, collection } => {
                commands::collections::remove_paper(&*backend, &paper, &collection, json)?;
            }
        },
        Commands::Tags { action } => match action {
            TagCommands::List => {
                commands::tags::list(&*backend, json)?;
            }
            TagCommands::Add { paper, tag } => {
                commands::tags::add(&*backend, &paper, &tag, json)?;
            }
            TagCommands::Remove { paper, tag } => {
                commands::tags::remove(&*backend, &paper, &tag, json)?;
            }
        },
        Commands::Notes { action } => match action {
            NoteCommands::List { paper } => {
                commands::notes::list(&*backend, &paper, json)?;
            }
            NoteCommands::Add { paper, content } => {
                commands::notes::add(&*backend, &paper, &content, json)?;
            }
            NoteCommands::Delete { note_id } => {
                commands::notes::delete(&*backend, &note_id, json)?;
            }
        },
        Commands::Export { paper, format } => {
            commands::export::export(&*backend, &paper, &format)?;
        }
        Commands::Status => {
            commands::status::status(&*backend, json)?;
        }
        Commands::InstallCli => {
            commands::install_cli::install_cli()?;
        }
        Commands::UninstallCli => {
            commands::install_cli::uninstall_cli()?;
        }
    }

    Ok(())
}
