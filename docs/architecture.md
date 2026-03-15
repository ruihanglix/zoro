# Architecture Overview

Zoro is an AI-native literature management tool -- like Zotero, but built for the age of AI agents. It is a cross-platform desktop application built with [Tauri v2](https://v2.tauri.app/), combining a Rust backend with a React/TypeScript frontend.

## Monorepo Structure

The project is organized as a monorepo managed with **pnpm 10 workspaces** (for TypeScript) and **Cargo workspaces** (for Rust), orchestrated by **Turborepo**.

```
zoro/
  apps/
    desktop/                    # Tauri v2 desktop app
      src/                      #   React 19 + TypeScript frontend
      src-tauri/                #   Rust backend (Tauri commands, connector, storage)
    browser-extension/          # Chrome Manifest V3 extension
  packages/
    core/                       # Shared TypeScript types (types only, no runtime code)
  crates/
    zoro-core/              # Rust domain models, slug generation, BibTeX/RIS parsers
    zoro-db/                # SQLite database layer with FTS5 full-text search
    zoro-metadata/          # Metadata enrichment (CrossRef, Semantic Scholar, OpenAlex) + PDF extraction
    zoro-subscriptions/     # Subscription source trait + HuggingFace Daily Papers plugin
```

## Crate Dependency Graph

```
                  zoro-core
                    (domain models, slug utils,
                     BibTeX/RIS parsers, errors)
                   /       |        \
                  v        v         v
           zoro-db  zoro-   zoro-subscriptions
          (SQLite, FTS5, metadata    (SubscriptionSource trait,
           queries,     (CrossRef,    HuggingFace plugin)
           schema)      S2, OpenAlex,
                        PDF extract)
                  \        |        /
                   v       v       v
              zoro-desktop (Tauri app)
              (commands, connector, storage,
               subscription poller)
```

| Crate | Description | Key Dependencies |
|---|---|---|
| `zoro-core` | Domain models (`Paper`, `Author`, `Collection`, etc.), slug generation, BibTeX/RIS parsers, error types | `serde`, `serde_json`, `chrono`, `sha2`, `slug`, `thiserror` |
| `zoro-db` | SQLite database layer with schema creation, FTS5 full-text search, and query modules | `zoro-core`, `rusqlite` (bundled), `thiserror` |
| `zoro-metadata` | Metadata enrichment from external APIs (CrossRef, Semantic Scholar, OpenAlex), DOI content negotiation for citations, and local PDF metadata extraction (title, authors, DOI via `lopdf`) | `reqwest`, `thiserror`, `lopdf`, `regex` |
| `zoro-subscriptions` | Plugin-based subscription system with the `SubscriptionSource` trait and HuggingFace Daily Papers implementation | `zoro-core`, `reqwest`, `async-trait`, `tokio` |
| `zoro-desktop` | Tauri v2 application integrating all crates, providing IPC commands, HTTP connector server, and file storage | All crates above, `tauri`, `axum`, `tower-http`, `tracing` |

## Tauri v2 Architecture

Zoro uses the Tauri v2 architecture, which separates the application into two processes:

```
+---------------------------------------------------+
|                Desktop Application                  |
|                                                     |
|  +-----------+    Tauri IPC     +----------------+  |
|  |  WebView  | <=============> | Rust Backend    |  |
|  |           |   (invoke/      |                 |  |
|  |  React 19 |    commands)    | - AppState      |  |
|  |  Zustand  |                 | - Tauri cmds    |  |
|  |  shadcn   |                 | - Storage       |  |
|  |  Tailwind |                 | - Sub poller    |  |
|  +-----------+                 +----------------+  |
|                                       |             |
|                                       v             |
|                                +----------------+   |
|                                | SQLite DB      |   |
|                                | (library.db)   |   |
|                                +----------------+   |
+---------------------------------------------------+
```

**Frontend (WebView):**
- React 19 with TypeScript
- Zustand for state management (two stores: `libraryStore` for data, `uiStore` for UI state)
- shadcn/ui (Radix + Tailwind CSS) for components
- All Tauri IPC calls centralized in `src/lib/commands.ts`

**Backend (Rust):**
- `AppState` holds a `Mutex<Database>` and the data directory path
- Tauri commands (`#[tauri::command]`) handle all IPC from the frontend
- `std::sync::Mutex` is used (not tokio) because SQLite operations are fast
- `tokio::spawn` runs background tasks (connector server, subscription poller)
- Tauri plugins: `shell`, `dialog`, `fs`

## Connector HTTP Server

The connector is an embedded **axum** HTTP server that runs inside the desktop app on a configurable port (default **23120**, bound to `127.0.0.1`). It enables the browser extension and external tools to communicate with Zoro.

```
+-------------------+       HTTP        +---------------------+
| Browser Extension | ===============> | Connector Server     |
| (Chrome MV3)     |   localhost:23120 | (axum)               |
+-------------------+                  |                      |
                                       | Routes:              |
                                       |  GET  /connector/ping|
                                       |  POST /connector/    |
                                       |       saveItem       |
                                       |  POST /connector/    |
                                       |       saveHtml       |
                                       |  GET  /connector/    |
                                       |       status         |
                                       |  GET  /connector/    |
                                       |       collections    |
                                       +---------------------+
                                              |
                                              v
                                       +----------------+
                                       | AppState       |
                                       | (shared w/     |
                                       |  Tauri app)    |
                                       +----------------+
```

The connector server shares `AppState` with the Tauri application via `AppHandle`, meaning data saved through the connector is immediately visible in the desktop UI.

CORS is configured to allow any origin (since the browser extension makes cross-origin requests to `127.0.0.1`).

### Zotero Connector Compatibility Mode

Zoro runs a second HTTP server on port **23119** (Zotero's default connector port) by default, implementing the Zotero Connector protocol. This allows the official Zotero browser extension to save papers directly to Zoro instead of Zotero.

```
+-------------------+       HTTP        +---------------------+
| Zotero Browser    | ===============> | Zotero Compat Server |
| Extension         |   localhost:23119 | (axum)               |
+-------------------+                  |                      |
                                       | Zotero Protocol:     |
                                       |  POST /connector/    |
                                       |       ping           |
                                       |  POST /connector/    |
                                       |       saveItems      |
                                       |  POST /connector/    |
                                       |       saveAttachment |
                                       |  POST /connector/    |
                                       |       saveSnapshot   |
                                       |  ... (15+ endpoints) |
                                       +---------------------+
```

Key design decisions:
- **Enabled by default**: Can be disabled via Settings > Browser Connector > "Enable Zotero Connector compatibility". If the port is already in use (e.g. by Zotero), a warning is displayed in the settings panel
- **No translators needed**: The Zotero browser extension runs translators in the browser and sends structured item JSON. Zoro only receives and stores the parsed data.
- **Coexists with native connector**: Both servers (23120 for Zoro extension, 23119 for Zotero extension) run simultaneously
- **Session management**: An in-memory `SessionStore` tracks save sessions, attachment upload progress, and collection/tag assignments
- **Lifecycle management**: Uses `tokio_util::CancellationToken` for graceful start/stop without restarting the app
- **Data mapping**: Zotero item JSON is mapped to Zoro's `AddPaperInput`. Fields that don't map directly (journal, volume, issue, etc.) are preserved in `extra_json`. Zotero tags are stored as `extra_json.labels` (read-only metadata) — they do NOT create sidebar tags, which are user-curated only

The implementation lives in `apps/desktop/src-tauri/src/connector/zotero_compat/`:
- `mod.rs` — Server spawn/stop functions
- `server.rs` — axum router with all Zotero protocol routes
- `handlers.rs` — Endpoint implementations
- `types.rs` — Zotero protocol request/response types
- `session.rs` — Session tracking and attachment progress
- `mapping.rs` — Zotero item JSON to Zoro data model conversion

## Data Flow Diagrams

### Browser Extension Save Flow

```
  Browser (ArXiv/DOI page)
       |
       v
  Content Script (detector)
  - detectArxiv(), detectDoi(), detectGeneric()
  - Extracts: title, authors, DOI, ArXiv ID, abstract, PDF/HTML URLs
       |
       v
  Popup UI (confirm/edit metadata)
       |
       v
  POST /connector/saveItem  ---------> Connector Server (axum)
       |                                       |
       |                                       v
       |                                AppState.db.lock()
       |                                       |
       |                                       v
       |                                generate_paper_slug()
       |                                       |
       |                                       v
       |                                insert_paper() -> SQLite
       |                                       |
       |                                       v
       |                                create_paper_dir()
       |                                  ~/.zoro/library/papers/{slug}/
       |                                    metadata.json
       |                                    attachments/
       |                                    notes/
       v
  (Optional) POST /connector/saveHtml
  - Saves HTML snapshot to paper dir
```

### Subscription Feed Flow

```
  Subscription Poller (background task)
       |
       | (every poll_interval_minutes)
       v
  SubscriptionSource::fetch()
  (e.g., HuggingFaceDailyPapers)
       |
       | HTTP GET https://huggingface.co/api/daily_papers
       v
  Parse API response -> Vec<SubscriptionItem>
       |
       v
  Store in subscription_items table
       |
       v
  User clicks "Add to Library" in UI
       |
       v
  Create Paper + paper directory
```

### Local PDF Import Flow (Drag & Drop)

```
  User drags PDF file(s) onto the application window
       |
       v
  Tauri webview drag-drop event
  (onDragDropEvent in FileDropZone component)
       |
       | Filter for .pdf files
       v
  invoke("import_local_files", { filePaths })
       |
       v
  For each PDF file:
  1. extract_pdf_metadata() via lopdf
     - Read PDF Info dict (/Title, /Author, /Subject)
     - Regex scan first 3 pages for DOI (10.xxxx/...)
     - Regex scan for arXiv ID (arXiv:YYMM.NNNNN)
       |
       v
  2. generate_paper_slug() from title or filename
       |
       v
  3. create_paper_dir() + fs::copy(PDF)
       |
       v
  4. insert_paper() + insert_attachment()
       |
       v
  5. tokio::spawn background enrichment
     (if DOI or arXiv ID found)
     - CrossRef / Semantic Scholar / OpenAlex
     - Fill missing: abstract, authors, journal, dates, etc.
```

## Technology Stack

| Layer | Technology | Version |
|---|---|---|
| Desktop Framework | Tauri | v2 |
| Frontend | React + TypeScript | React 19 |
| UI Components | shadcn/ui (Radix + Tailwind CSS) | - |
| State Management | Zustand | v5 |
| Database | SQLite via rusqlite (bundled) | rusqlite 0.31 |
| Full-Text Search | SQLite FTS5 | (built-in) |
| HTTP Connector | axum | 0.7 |
| CORS | tower-http | 0.5 |
| HTTP Client | reqwest | 0.12 |
| Async Runtime | Tokio | 1.x (full features) |
| Error Handling | thiserror | 2.x |
| Serialization | serde + serde_json | 1.x |
| Logging | tracing + tracing-subscriber | 0.1 / 0.3 |
| Browser Extension | Chrome Manifest V3 | - |
| Build (JS) | pnpm workspaces + Turborepo | pnpm 10 |
| Build (Rust) | Cargo workspace | Edition 2021 |
| CI/CD | GitHub Actions | - |

## Directory Tree

```
zoro/
  .github/
    workflows/
      ci.yml                    # CI: format, clippy, test, type-check
      release.yml               # Release: tag-triggered multi-platform builds
  apps/
    desktop/
      src/                      # React frontend
        components/             #   UI components (PascalCase.tsx)
        lib/
          commands.ts           #   Tauri IPC command wrappers
        stores/                 #   Zustand stores
      src-tauri/
        src/
          lib.rs                #   App entry point, setup, state
          commands/              #   Tauri command handlers
            library.rs           #     Paper CRUD, collections, tags
            search.rs            #     Full-text search
            subscription.rs      #     Subscription management
            import_export.rs     #     BibTeX/RIS import/export
            connector.rs         #     Connector status
          connector/             #   HTTP connector server
            mod.rs               #     Server startup
            server.rs            #     axum router setup
            handlers.rs          #     HTTP endpoint handlers
          storage/               #   Filesystem operations
            mod.rs               #     Data dir initialization
            paper_dir.rs         #     Paper directory CRUD
            attachments.rs       #     Attachment management
          subscriptions/         #   Background subscription poller
        Cargo.toml
        tauri.conf.json
    browser-extension/
      manifest.json             # Chrome MV3 manifest
      src/
        content/
          detectors/
            arxiv.ts            #   ArXiv paper detector
            doi.ts              #   DOI paper detector
            generic.ts          #   Generic citation meta tag detector
        popup/                  #   Extension popup UI
        lib/
          types.ts              #   Shared types
  packages/
    core/                       # Shared TypeScript types
  crates/
    zoro-core/
      src/
        models.rs               # Domain models (Paper, Author, Collection, etc.)
        slug_utils.rs           # Paper slug generation
        bibtex.rs               # BibTeX parser
        ris.rs                  # RIS parser
        error.rs                # CoreError enum
    zoro-db/
      src/
        schema.rs               # DDL: tables, indexes, FTS5, triggers
        error.rs                # DbError enum
    zoro-subscriptions/
      src/
        source.rs               # SubscriptionSource trait definition
        huggingface.rs          # HuggingFace Daily Papers implementation
        error.rs                # SubscriptionError enum
    zoro-metadata/
      src/
        lib.rs                  # EnrichmentResult, enrich_paper() pipeline
        pdf_extract.rs          # Local PDF metadata extraction (title, DOI, arXiv)
        crossref.rs             # CrossRef API client
        semantic_scholar.rs     # Semantic Scholar API client
        openalex.rs             # OpenAlex API client
        doi_content_negotiation.rs  # DOI content negotiation for citations
        error.rs                # MetadataError enum
  Cargo.toml                    # Workspace root
  rustfmt.toml                  # Rust formatting config
```
