# Zoro CLI

The `zoro` CLI lets you manage your Zoro library directly from the terminal. It is designed for both **human users** (table-formatted output with colors) and **AI agents** (JSON output via `--json`). Unlike the MCP server which requires MCP protocol support, any agent that can run shell commands can use the CLI.

## Quick Start

```bash
# Build the CLI
cargo build --release -p zoro-cli

# Check library status
./target/release/zoro status

# Search papers
./target/release/zoro search "attention"

# List all papers (JSON output for agents)
./target/release/zoro --json list

# Export a paper as BibTeX
./target/release/zoro export <paper-slug> --format bibtex
```

## Installation

### From Source

```bash
cargo build --release -p zoro-cli
```

The binary is at `target/release/zoro`.

### Install to System PATH

After building (or from the bundled desktop app sidecar):

```bash
# Self-install: creates a symlink at /usr/local/bin/zoro
./target/release/zoro install-cli

# Verify
which zoro
zoro status
```

On macOS/Linux this creates a symlink to `/usr/local/bin/zoro` (may require `sudo`). On Windows it copies the binary to `%LOCALAPPDATA%\Zoro\bin\` and prints instructions to add it to PATH.

### Bundled with Desktop App

The CLI binary is bundled as a sidecar with the Zoro desktop app. Use **Settings → Install CLI** in the desktop app, or run the `install-cli` subcommand from the sidecar binary directly.

## Hybrid Backend

The CLI automatically detects which backend to use:

1. **HTTP connector** (default): If the Zoro desktop app is running, the CLI connects to its HTTP connector at `localhost:23120`
2. **Local SQLite** (fallback): If the app is not running, the CLI opens `~/.zoro/library.db` directly

You can force local mode with the `--local` flag:

```bash
# Always use direct SQLite access
zoro --local search "transformer"
```

Check which mode is active:

```bash
$ zoro status
Zoro Library Status
────────────────────────────────────────
  Mode: Local (direct DB)
  Data dir: /Users/you/.zoro
  Papers: 42
  Collections: 5
  Tags: 12
```

## Global Flags

| Flag | Description |
|------|-------------|
| `--json` | Output JSON instead of human-friendly tables (for agent/script use) |
| `--data-dir <path>` | Custom data directory (default: `~/.zoro`, or `ZORO_DATA_DIR` env) |
| `--local` | Force local SQLite backend (skip HTTP connector detection) |

## Commands

### Papers

```bash
# Full-text search (FTS5 + author name matching)
zoro search "attention mechanism"
zoro search --limit 5 "transformer"

# List papers with optional filters
zoro list
zoro list --collection "Deep Learning"
zoro list --tag "NLP"
zoro list --status unread
zoro list --limit 100

# Get paper details (by slug or ID)
zoro get attention-is-all-you-need-a1b2c3d4

# Add a paper (by DOI, arXiv ID, URL, or local PDF)
zoro add "10.1038/s41586-025-09422-z"
zoro add "2301.12345"
zoro add "https://arxiv.org/abs/2301.12345"
zoro add ./paper.pdf

# Delete a paper
zoro delete attention-is-all-you-need-a1b2c3d4
```

### Collections

```bash
# List all collections
zoro collections list

# Create a collection
zoro collections create "Machine Learning"
zoro collections create "NLP" --description "Natural Language Processing papers"

# Add/remove a paper to/from a collection
zoro collections add <paper-slug> "Machine Learning"
zoro collections remove <paper-slug> "Machine Learning"
```

### Tags

```bash
# List all tags
zoro tags list

# Add/remove tags
zoro tags add <paper-slug> "important"
zoro tags remove <paper-slug> "important"
```

### Notes

```bash
# List notes for a paper
zoro notes list <paper-slug>

# Add a note
zoro notes add <paper-slug> "Key insight: self-attention scales quadratically"

# Delete a note
zoro notes delete <note-id>
```

### Export

```bash
# Export as BibTeX (default)
zoro export <paper-slug>
zoro export <paper-slug> --format bibtex

# Export as RIS
zoro export <paper-slug> --format ris

# Export as JSON
zoro export <paper-slug> --format json
```

### Status

```bash
# Show connection mode and library statistics
zoro status
zoro --json status
```

## JSON Output for Agents

Every command supports `--json` for machine-readable output:

```bash
$ zoro --json search "attention"
[
  {
    "id": "a1b2c3d4-...",
    "slug": "2017-attention-is-all-you-need-a1b2c3d4",
    "title": "Attention Is All You Need",
    "authors_display": "Vaswani et al.",
    "doi": "10.48550/arXiv.1706.03762",
    "published_date": "2017-06-12",
    "read_status": "read",
    "tags": ["transformer", "nlp"],
    "collections": ["Deep Learning"],
    ...
  }
]

$ zoro --json status
{
  "mode": "Local (direct DB)",
  "data_dir": "/Users/you/.zoro",
  "paper_count": 42,
  "collection_count": 5,
  "tag_count": 12
}
```

This makes it easy for any AI agent (Claude Code, Cursor, etc.) to query and manage the library by running shell commands and parsing JSON output.

## Agent Integration Example

An AI coding agent can use the CLI without any special protocol support:

```bash
# Agent searches for relevant papers
papers=$(zoro --json search "reinforcement learning")

# Agent gets details of the top result
slug=$(echo "$papers" | jq -r '.[0].slug')
zoro --json get "$slug"

# Agent adds a tag
zoro tags add "$slug" "relevant-to-project"

# Agent adds a research note
zoro notes add "$slug" "This paper's reward shaping technique could apply to our optimizer"

# Agent exports citation for the paper
zoro export "$slug" --format bibtex >> references.bib
```

## Architecture

The CLI is implemented as the `zoro-cli` crate (`crates/zoro-cli/`):

```
crates/zoro-cli/
├── Cargo.toml
└── src/
    ├── main.rs              # Entry point, clap command definitions
    ├── config.rs            # Data directory resolution
    ├── output.rs            # Output formatting (table/JSON)
    ├── backend/
    │   ├── mod.rs           # Backend trait + auto-detection
    │   ├── local.rs         # LocalBackend (direct SQLite)
    │   └── http.rs          # HttpBackend (connector API)
    └── commands/
        ├── mod.rs
        ├── papers.rs        # search, list, get, add, delete
        ├── collections.rs   # list, create, add, remove
        ├── tags.rs          # list, add, remove
        ├── notes.rs         # list, add, delete
        ├── export.rs        # bibtex, ris, json
        ├── status.rs        # connection status + stats
        └── install_cli.rs   # install to system PATH
```

It reuses the same Rust crates as the desktop app and MCP server:
- `zoro-core` — domain models, BibTeX/RIS generation
- `zoro-db` — SQLite queries, FTS5 search
- `zoro-storage` — paper directory management
- `zoro-metadata` — metadata enrichment APIs

## See Also

- [Agent Integration Guide](agent-integration.md) — Filesystem-based access patterns
- [MCP Server](mcp-server.md) — MCP protocol integration for agents that support it
- [Architecture Overview](architecture.md) — System design
