# Development Guide

This guide walks you through setting up a local development environment for Zoro.

## Prerequisites

| Tool | Version | Purpose |
|---|---|---|
| [Rust](https://rustup.rs/) | stable (latest) | Backend compilation |
| [Node.js](https://nodejs.org/) | 20+ | Frontend build toolchain |
| [pnpm](https://pnpm.io/) | 10+ | Package management |
| Git | any recent | Version control |

## System Dependencies

Tauri v2 requires platform-specific system libraries for the WebView and other native features.

### macOS

Install Xcode Command Line Tools:

```bash
xcode-select --install
```

No additional libraries are required -- macOS ships with WebKit (WKWebView).

### Linux (Ubuntu / Debian)

```bash
sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  libappindicator3-dev \
  librsvg2-dev \
  patchelf
```

| Package | Purpose |
|---|---|
| `libwebkit2gtk-4.1-dev` | WebView rendering engine |
| `libappindicator3-dev` | System tray support |
| `librsvg2-dev` | SVG icon rendering |
| `patchelf` | Binary patching for AppImage builds |

For other distributions, install the equivalent packages from your package manager.

### Windows

- **WebView2**: Pre-installed on Windows 10 (version 1803+) and Windows 11. For older systems, download from [Microsoft](https://developer.microsoft.com/en-us/microsoft-edge/webview2/).
- **Visual Studio Build Tools**: Install the "Desktop development with C++" workload, or install the full Visual Studio Community edition.

## Clone and Setup

```bash
# Clone the repository
git clone <repo-url> zoro
cd zoro

# Install JavaScript dependencies
pnpm install
```

Rust dependencies are automatically fetched on first build.

## Development Commands

### Run the Desktop App (Dev Mode)

```bash
pnpm tauri dev
```

This starts the Tauri development server with hot-reload for the frontend (port 1420) and recompiles the Rust backend on changes. The connector server starts automatically on port 23120.

### Rust Commands

```bash
# Run all Rust tests
cargo test --all

# Run tests for a single crate
cargo test -p zoro-core

# Run tests matching a specific name
cargo test -p zoro-core slug

# Format check (CI enforced)
cargo fmt --all -- --check

# Auto-format
cargo fmt --all

# Lint (CI enforced, treats warnings as errors)
cargo clippy --all-targets --all-features -- -D warnings
```

### TypeScript Commands

```bash
# Type check the desktop app (CI enforced)
pnpm --filter @zoro/desktop type-check

# Lint the desktop app (Biome)
pnpm --filter @zoro/desktop lint

# Build the browser extension
pnpm --filter @zoro/browser-extension build

# Lint the browser extension
pnpm --filter @zoro/browser-extension lint
```

### Full CI Equivalent

Run all checks locally before pushing:

```bash
cargo fmt --all -- --check && \
cargo clippy --all-targets --all-features -- -D warnings && \
cargo test --all && \
pnpm --filter @zoro/desktop type-check
```

### Build for Production

```bash
# Desktop app (creates platform-specific installers)
cd apps/desktop
pnpm tauri build

# Browser extension (output in dist/)
pnpm --filter @zoro/browser-extension build
```

## Project Layout

```
zoro/
  Cargo.toml              # Cargo workspace root (4 members)
  rustfmt.toml            # Rust formatting: edition 2021, 100 width, 4-space indent
  pnpm-workspace.yaml     # pnpm workspace config
  turbo.json              # Turborepo task definitions

  crates/
    zoro-core/        # Domain models (Paper, Author, etc.), slug generation,
                          #   BibTeX/RIS parsers, error types. No I/O, no database.
    zoro-db/          # SQLite via rusqlite (bundled). Schema creation with
                          #   FTS5 full-text search, indexes, triggers. Query modules.
    zoro-subscriptions/  # SubscriptionSource trait + HuggingFace plugin.
                          #   Uses reqwest for HTTP, async-trait for the plugin interface.

  apps/
    desktop/
      src/                # React 19 + TypeScript frontend
        components/       #   PascalCase.tsx component files
        lib/commands.ts   #   All Tauri IPC wrappers (invoke calls)
        stores/           #   Zustand stores (libraryStore, uiStore)
      src-tauri/          # Rust Tauri backend
        src/lib.rs        #   App entry: state init, plugin setup, handler registration
        src/commands/     #   Tauri command modules (library, search, subscription, etc.)
        src/connector/    #   axum HTTP server for browser extension communication
        src/storage/      #   Filesystem operations (paper dirs, metadata.json)

    browser-extension/    # Chrome Manifest V3 extension
      manifest.json       #   Permissions, content script config, service worker
      src/content/        #   Content scripts with paper detectors
      src/popup/          #   Extension popup UI
      src/lib/types.ts    #   Shared TypeScript types
```

## Debugging

### Tauri DevTools

In development mode (`pnpm tauri dev`), right-click in the app window and select "Inspect Element" to open the WebView DevTools. This provides the standard Chrome DevTools experience for debugging the frontend.

### Rust Logging

The backend uses `tracing` with `tracing-subscriber`. Control log levels via the `RUST_LOG` environment variable:

```bash
# Default: zoro crates log at info level
RUST_LOG=zoro=debug pnpm tauri dev

# See all SQL queries
RUST_LOG=zoro_db=trace pnpm tauri dev

# See HTTP connector requests
RUST_LOG=zoro=debug,tower_http=debug pnpm tauri dev

# Full verbose logging
RUST_LOG=debug pnpm tauri dev
```

The default filter is `zoro=info`, configured in `apps/desktop/src-tauri/src/lib.rs`.

### Database Inspection

The SQLite database is stored at `~/.zoro/library.db`. You can inspect it directly:

```bash
sqlite3 ~/.zoro/library.db
.tables          -- List all tables
.schema papers   -- Show table schema
SELECT * FROM papers LIMIT 5;
```

### Connector Server Testing

Test the connector server with curl while the app is running:

```bash
# Check if the server is running
curl http://127.0.0.1:23120/connector/ping

# Check server status
curl http://127.0.0.1:23120/connector/status

# List collections
curl http://127.0.0.1:23120/connector/collections
```

## Common Issues and Solutions

### `pnpm tauri dev` fails with missing system dependencies

**Linux**: Ensure all required packages are installed (see [System Dependencies](#linux-ubuntu--debian) above). The most common missing package is `libwebkit2gtk-4.1-dev`.

**macOS**: Run `xcode-select --install` to install CLI tools.

### Cargo build fails with linking errors

This usually means system libraries are missing. Check the error message for the specific library and install it.

### Port 23120 already in use

The connector server binds to `127.0.0.1:23120`. If another instance is running, the connector will fail to start (logged as an error but does not crash the app). Kill the other instance or change the port in `~/.zoro/config.toml`:

```toml
[connector]
port = 23121
enabled = true
```

### Database migration issues

The schema is created with `CREATE TABLE IF NOT EXISTS`, so tables are only created if they do not already exist. If you need to reset the database, delete `~/.zoro/library.db` and restart the app.

### FTS5 search not returning results

The FTS5 virtual table is synchronized via triggers on the `papers` table. If you manually modify the database outside of Zoro, the FTS index may become stale. Rebuild it:

```sql
INSERT INTO papers_fts(papers_fts) VALUES('rebuild');
```

### Browser extension cannot connect

1. Ensure the desktop app is running (the connector starts automatically).
2. Check that the connector port matches the extension's `host_permissions` in `manifest.json` (default: `23120`).
3. Verify with `curl http://127.0.0.1:23120/connector/ping`.

## Next Steps

- [Architecture Overview](architecture.md) -- Understand the system design
- [Data Model & API Reference](data-model.md) -- Database schema and command API
- [Browser Extension](browser-extension.md) -- Extension development
- [Contributing Guide](contributing.md) -- How to contribute
