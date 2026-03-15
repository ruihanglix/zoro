# MCP Server

The `zoro-mcp` binary is a standalone [Model Context Protocol](https://modelcontextprotocol.io/) server that gives AI agents (Claude Desktop, Cursor, OpenCode, etc.) full access to your Zoro library.

## Quick Start

```bash
# Build the MCP server
cargo build --release -p zoro-mcp

# Run with stdio transport (default)
./target/release/zoro-mcp

# Run with HTTP transport
./target/release/zoro-mcp --transport http --port 23121

# Specify a custom data directory
./target/release/zoro-mcp --data-dir /path/to/data
```

The data directory defaults to `~/.zoro`. You can also set the `ZORO_DATA_DIR` environment variable.

## Desktop App Integration

The Zoro desktop app can manage the MCP server directly from **Settings → MCP Server**:

- **Auto-start**: Enable to start the MCP server automatically when the desktop app launches
- **Transport**: Choose between HTTP (Streamable) or stdio
- **Port**: Configure the HTTP port (default 23121)
- **Controls**: Start / Stop / Restart the server manually

The desktop app spawns `zoro-mcp` as a child process, sharing the same data directory. The MCP binary must be located next to the desktop app executable (both are built into the same `target/` directory during development).

Configuration is persisted in `~/.zoro/config.toml` under the `[mcp]` section:

```toml
[mcp]
enabled = true
transport = "http"
port = 23121
```

## Client Configuration

### Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS):

```json
{
  "mcpServers": {
    "zoro": {
      "command": "/path/to/zoro-mcp",
      "args": []
    }
  }
}
```

### OpenCode

Add to `.opencode/config.json`:

```json
{
  "mcpServers": {
    "zoro": {
      "type": "stdio",
      "command": "/path/to/zoro-mcp",
      "args": []
    }
  }
}
```

### HTTP Transport

For clients that support HTTP-based MCP (Streamable HTTP):

```bash
./target/release/zoro-mcp --transport http --port 23121
```

The server listens at `http://127.0.0.1:23121/mcp`.

## Tools

The MCP server exposes ~35 tools covering the full Zoro feature set.

### Papers

| Tool | Description |
|---|---|
| `add_paper` | Add a new paper to the library |
| `get_paper` | Get detailed information about a paper by ID |
| `list_papers` | List papers with optional filters (collection, tag, status, sort, pagination) |
| `search_papers` | Full-text search across paper titles and abstracts |
| `update_paper` | Update paper metadata |
| `update_paper_status` | Set paper read status (unread/reading/read) |
| `update_paper_rating` | Set paper rating (1-5) or null to clear |
| `delete_paper` | Delete a paper from the library |
| `enrich_paper_metadata` | Enrich paper metadata from external APIs (CrossRef, Semantic Scholar, OpenAlex) |

### Collections

| Tool | Description |
|---|---|
| `create_collection` | Create a new collection for organizing papers |
| `list_collections` | List all collections with paper counts |
| `update_collection` | Update a collection's name, parent, or description |
| `delete_collection` | Delete a collection |
| `add_paper_to_collection` | Add a paper to a collection |
| `remove_paper_from_collection` | Remove a paper from a collection |
| `get_collections_for_paper` | Get all collections containing a paper |

### Tags

| Tool | Description |
|---|---|
| `list_tags` | List all tags |
| `search_tags` | Search tags by name prefix |
| `add_tag_to_paper` | Add a tag to a paper |
| `remove_tag_from_paper` | Remove a tag from a paper |
| `update_tag` | Update a tag's name or color |
| `delete_tag` | Delete a tag |

### Notes

| Tool | Description |
|---|---|
| `add_note` | Add a note to a paper |
| `list_notes` | List all notes for a paper |
| `update_note` | Update a note's content |
| `delete_note` | Delete a note |

### Annotations

| Tool | Description |
|---|---|
| `add_annotation` | Add a PDF annotation |
| `list_annotations` | List all annotations for a paper |
| `update_annotation` | Update an annotation |
| `delete_annotation` | Delete an annotation |

### Import/Export

| Tool | Description |
|---|---|
| `import_bibtex` | Import papers from BibTeX content |
| `export_bibtex` | Export papers as BibTeX (all or by paper IDs) |
| `import_ris` | Import papers from RIS content |
| `export_ris` | Export papers as RIS (all or by paper IDs) |

### Citations

| Tool | Description |
|---|---|
| `get_formatted_citation` | Get a formatted citation (apa, ieee, mla, chicago, bibtex, ris) |
| `get_paper_bibtex` | Get BibTeX entry for a paper |

### Subscriptions

| Tool | Description |
|---|---|
| `list_subscriptions` | List all feed subscriptions |
| `list_feed_items` | List feed items from a subscription with pagination |
| `add_feed_item_to_library` | Add a feed item to the library as a paper |
| `refresh_subscription` | Refresh a subscription feed to fetch new items |

## Resources

The server exposes two MCP resources:

| URI | Description |
|---|---|
| `zoro://library-index` | JSON index of all papers in the library (lightweight summary) |
| `zoro://paper/{paper_id}` | Full metadata for a specific paper |

## Architecture

The MCP server is a standalone binary that opens the SQLite database directly (no Tauri dependency). It reuses the same Rust crates as the desktop app:

- `zoro-core` -- domain models, slug generation, BibTeX/RIS parsers
- `zoro-db` -- SQLite queries
- `zoro-storage` -- file/sync logic (paper directories, metadata sync)
- `zoro-metadata` -- external API enrichment (CrossRef, Semantic Scholar, OpenAlex)
- `zoro-subscriptions` -- feed sources (HuggingFace Daily Papers)

The server uses `rmcp` (the official Rust MCP SDK) with the `#[tool_router]` and `#[tool_handler]` macros for tool registration.

### Concurrency

- The SQLite database is protected by `std::sync::Mutex` (not tokio::Mutex) since SQLite operations are fast
- Network calls (enrichment, subscription refresh) release the DB lock before making HTTP requests
- The HTTP transport creates a new server handler per session (stateful mode)

## See Also

- [Agent Integration Guide](agent-integration.md) -- Filesystem-based access patterns
- [Data Model & API Reference](data-model.md) -- Full database schema
- [Architecture Overview](architecture.md) -- System design
