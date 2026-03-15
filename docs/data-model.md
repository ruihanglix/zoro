# Data Model & API Reference

This document covers the SQLite database schema, Rust domain models, Tauri IPC commands, and the connector HTTP API.

## SQLite Database Schema

The database is stored at `~/.zoro/library.db`. All tables are created by the `create_tables` function in `crates/zoro-db/src/schema.rs`.

### Tables

#### `papers`

The central table storing paper metadata.

```sql
CREATE TABLE IF NOT EXISTS papers (
    id TEXT PRIMARY KEY,
    slug TEXT UNIQUE NOT NULL,
    title TEXT NOT NULL,
    abstract_text TEXT,
    doi TEXT,
    arxiv_id TEXT,
    url TEXT,
    pdf_url TEXT,
    html_url TEXT,
    published_date TEXT,
    added_date TEXT NOT NULL,
    modified_date TEXT NOT NULL,
    source TEXT,
    read_status TEXT DEFAULT 'unread',
    rating INTEGER,
    extra_json TEXT,
    dir_path TEXT NOT NULL
);
```

- `id`: UUID v4 string
- `slug`: Human-readable identifier (e.g., `2017-attention-is-all-you-need-a1b2c3d4`)
- `source`: One of `"browser-extension"`, `"subscription"`, `"manual"`, `"import"`
- `read_status`: One of `"unread"`, `"reading"`, `"read"`
- `extra_json`: Arbitrary JSON for extensibility. Contains a `labels` array for read-only metadata labels (e.g., arXiv categories, Zotero tags) — these are distinct from user-curated sidebar tags
- `dir_path`: Relative path within the library (e.g., `papers/2017-attention-...`)

#### `authors`

```sql
CREATE TABLE IF NOT EXISTS authors (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    affiliation TEXT,
    orcid TEXT
);
```

#### `paper_authors`

Many-to-many join table with position ordering.

```sql
CREATE TABLE IF NOT EXISTS paper_authors (
    paper_id TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
    author_id TEXT NOT NULL REFERENCES authors(id),
    position INTEGER NOT NULL,
    PRIMARY KEY (paper_id, author_id)
);
```

#### `collections`

Hierarchical collections (folders) for organizing papers.

```sql
CREATE TABLE IF NOT EXISTS collections (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    parent_id TEXT REFERENCES collections(id),
    position INTEGER DEFAULT 0,
    created_date TEXT NOT NULL,
    description TEXT
);
-- Scoped unique: same slug allowed under different parents
CREATE UNIQUE INDEX idx_collections_slug_parent
    ON collections(slug, COALESCE(parent_id, ''));
```

#### `paper_collections`

Many-to-many join between papers and collections.

```sql
CREATE TABLE IF NOT EXISTS paper_collections (
    paper_id TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
    collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    added_date TEXT NOT NULL,
    PRIMARY KEY (paper_id, collection_id)
);
```

#### `tags`

```sql
CREATE TABLE IF NOT EXISTS tags (
    id TEXT PRIMARY KEY,
    name TEXT UNIQUE NOT NULL,
    color TEXT
);
```

#### `paper_tags`

Many-to-many join between papers and tags.

```sql
CREATE TABLE IF NOT EXISTS paper_tags (
    paper_id TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    source TEXT DEFAULT 'manual',
    PRIMARY KEY (paper_id, tag_id)
);
```

The `source` column tracks how the tag was applied (`"manual"`). Tags are user-curated only — external labels from the Zotero Connector or arXiv are stored in `extra_json.labels` instead of being auto-created as tags.

#### `attachments`

Files associated with a paper (PDFs, HTML snapshots, summaries, etc.).

```sql
CREATE TABLE IF NOT EXISTS attachments (
    id TEXT PRIMARY KEY,
    paper_id TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
    filename TEXT NOT NULL,
    file_type TEXT NOT NULL,
    mime_type TEXT,
    file_size INTEGER,
    relative_path TEXT NOT NULL,
    created_date TEXT NOT NULL,
    modified_date TEXT NOT NULL,
    source TEXT DEFAULT 'manual',
    metadata_json TEXT
);
```

#### `subscriptions`

Feed source configurations.

```sql
CREATE TABLE IF NOT EXISTS subscriptions (
    id TEXT PRIMARY KEY,
    source_type TEXT NOT NULL,
    name TEXT NOT NULL,
    config_json TEXT,
    enabled INTEGER DEFAULT 1,
    poll_interval_minutes INTEGER DEFAULT 60,
    last_polled TEXT,
    created_date TEXT NOT NULL
);
```

#### `subscription_items`

Individual items fetched from subscription sources.

```sql
CREATE TABLE IF NOT EXISTS subscription_items (
    id TEXT PRIMARY KEY,
    subscription_id TEXT NOT NULL REFERENCES subscriptions(id) ON DELETE CASCADE,
    paper_id TEXT REFERENCES papers(id),
    external_id TEXT NOT NULL,
    title TEXT NOT NULL,
    data_json TEXT,
    fetched_date TEXT NOT NULL,
    added_to_library INTEGER DEFAULT 0
);
```

### Full-Text Search (FTS5)

A virtual FTS5 table enables full-text search across paper titles and abstracts:

```sql
CREATE VIRTUAL TABLE papers_fts USING fts5(
    title,
    abstract_text,
    content='papers',
    content_rowid='rowid',
    tokenize='porter unicode61'
);
```

- Uses **content-sync** mode (`content='papers'`) to mirror the `papers` table
- **Porter stemming** + **unicode61** tokenizer for multilingual support
- Kept in sync via triggers (see below)

### FTS5 Triggers

Three triggers keep the FTS index synchronized with the `papers` table:

```sql
-- After INSERT: index the new paper
CREATE TRIGGER papers_ai AFTER INSERT ON papers BEGIN
    INSERT INTO papers_fts(rowid, title, abstract_text)
    VALUES (new.rowid, new.title, new.abstract_text);
END;

-- After DELETE: remove from index
CREATE TRIGGER papers_ad AFTER DELETE ON papers BEGIN
    INSERT INTO papers_fts(papers_fts, rowid, title, abstract_text)
    VALUES ('delete', old.rowid, old.title, old.abstract_text);
END;

-- After UPDATE: remove old, insert new
CREATE TRIGGER papers_au AFTER UPDATE ON papers BEGIN
    INSERT INTO papers_fts(papers_fts, rowid, title, abstract_text)
    VALUES ('delete', old.rowid, old.title, old.abstract_text);
    INSERT INTO papers_fts(rowid, title, abstract_text)
    VALUES (new.rowid, new.title, new.abstract_text);
END;
```

### Indexes

```sql
CREATE INDEX IF NOT EXISTS idx_papers_slug ON papers(slug);
CREATE INDEX IF NOT EXISTS idx_papers_doi ON papers(doi);
CREATE INDEX IF NOT EXISTS idx_papers_arxiv_id ON papers(arxiv_id);
CREATE INDEX IF NOT EXISTS idx_papers_added_date ON papers(added_date);
CREATE INDEX IF NOT EXISTS idx_paper_authors_author ON paper_authors(author_id);
CREATE INDEX IF NOT EXISTS idx_paper_collections_collection ON paper_collections(collection_id);
CREATE INDEX IF NOT EXISTS idx_paper_tags_tag ON paper_tags(tag_id);
CREATE INDEX IF NOT EXISTS idx_attachments_paper ON attachments(paper_id);
CREATE INDEX IF NOT EXISTS idx_subscription_items_sub ON subscription_items(subscription_id);
CREATE INDEX IF NOT EXISTS idx_subscription_items_ext ON subscription_items(external_id);
```

## Rust Domain Models

Defined in `crates/zoro-core/src/models.rs`. These are the application-level models used throughout the Rust codebase.

### `Paper`

The primary domain model representing a paper in the library.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paper {
    pub id: String,                        // UUID v4
    pub slug: String,                      // e.g., "2017-attention-is-all-you-need-a1b2c3d4"
    pub title: String,
    pub authors: Vec<Author>,
    #[serde(rename = "abstract")]
    pub abstract_text: Option<String>,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    pub url: Option<String>,
    pub pdf_url: Option<String>,
    pub html_url: Option<String>,
    pub published_date: Option<String>,    // ISO 8601
    pub added_date: String,                // RFC 3339
    pub modified_date: String,             // RFC 3339
    pub source: Option<String>,            // "browser-extension", "subscription", "manual", "import"
    pub tags: Vec<String>,
    pub collections: Vec<String>,
    pub attachments: Vec<AttachmentInfo>,
    pub notes: Vec<String>,
    pub read_status: ReadStatus,
    pub rating: Option<u8>,
    pub extra: serde_json::Value,          // Arbitrary JSON
}
```

### `Author`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    pub name: String,
    pub affiliation: Option<String>,
    pub orcid: Option<String>,
}
```

### `AttachmentInfo`

Lightweight attachment reference (used in `Paper.attachments`).

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentInfo {
    pub filename: String,
    #[serde(rename = "type")]
    pub attachment_type: String,
    pub created: String,
}
```

### `ReadStatus`

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ReadStatus {
    Unread,
    Reading,
    Read,
}
```

### `Collection`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub parent_id: Option<String>,
    pub position: i32,
    pub created_date: String,
    pub description: Option<String>,
}
```

### `Tag`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
}
```

### `Attachment`

Full attachment record (database-backed).

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String,
    pub paper_id: String,
    pub filename: String,
    pub file_type: String,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
    pub relative_path: String,
    pub created_date: String,
    pub modified_date: String,
    pub source: String,
    pub metadata: Option<serde_json::Value>,
}
```

### `Subscription`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: String,
    pub source_type: String,
    pub name: String,
    pub config: Option<serde_json::Value>,
    pub enabled: bool,
    pub poll_interval_minutes: i32,
    pub last_polled: Option<String>,
    pub created_date: String,
}
```

### `SubscriptionItem`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionItem {
    pub id: String,
    pub subscription_id: String,
    pub paper_id: Option<String>,
    pub external_id: String,
    pub title: String,
    pub authors: Vec<Author>,
    pub abstract_text: Option<String>,
    pub url: Option<String>,
    pub pdf_url: Option<String>,
    pub html_url: Option<String>,
    pub upvotes: Option<i32>,
    pub data: Option<serde_json::Value>,
    pub fetched_date: String,
    pub added_to_library: bool,
}
```

### `PaperMetadata`

The JSON file written to each paper directory (`metadata.json`). Mirrors `Paper` without `modified_date`.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperMetadata {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub authors: Vec<Author>,
    #[serde(rename = "abstract")]
    pub abstract_text: Option<String>,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    pub url: Option<String>,
    pub pdf_url: Option<String>,
    pub html_url: Option<String>,
    pub published_date: Option<String>,
    pub added_date: String,
    pub source: Option<String>,
    pub tags: Vec<String>,
    pub collections: Vec<String>,
    pub attachments: Vec<AttachmentInfo>,
    pub notes: Vec<String>,
    pub read_status: ReadStatus,
    pub rating: Option<u8>,
    pub extra: serde_json::Value,
}
```

### `AppConfig`

Application configuration (`config.toml`).

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub general: GeneralConfig,       // data_dir, language
    pub connector: ConnectorConfig,   // port (default 23120), enabled
    pub subscriptions: SubscriptionsConfig,  // poll_interval_minutes (default 60)
    pub ai: AiConfig,                // provider, api_key, auto_summarize, auto_tag
}
```

## Tauri Commands API Reference

All Tauri IPC commands are defined in `apps/desktop/src-tauri/src/commands/` and exposed to the frontend via `apps/desktop/src/lib/commands.ts`. Commands follow the pattern: `invoke<ReturnType>("snake_case_command", { args })`.

### Library Commands

| Command | TypeScript Signature | Description |
|---|---|---|
| `add_paper` | `addPaper(input: AddPaperInput) => PaperResponse` | Add a paper to the library |
| `get_paper` | `getPaper(id: string) => PaperResponse` | Get a paper by ID |
| `list_papers` | `listPapers(params?) => PaperResponse[]` | List papers with optional filters |
| `delete_paper` | `deletePaper(id: string) => void` | Delete a paper and its directory |
| `update_paper_status` | `updatePaperStatus(id: string, readStatus: string) => void` | Set read status |
| `update_paper_rating` | `updatePaperRating(id: string, rating: number \| null) => void` | Set rating |

#### `AddPaperInput`

```typescript
export interface AddPaperInput {
  title: string;
  authors: { name: string; affiliation?: string }[];
  abstract_text?: string;
  doi?: string;
  arxiv_id?: string;
  url?: string;
  pdf_url?: string;
  html_url?: string;
  published_date?: string;
  source?: string;
  tags?: string[];
}
```

#### `listPapers` Filter Parameters

```typescript
listPapers(params?: {
  collectionId?: string;   // Filter by collection
  tagName?: string;        // Filter by tag name
  readStatus?: string;     // Filter by read status ("unread", "reading", "read")
  sortBy?: string;         // Sort field
  sortOrder?: string;      // "asc" or "desc"
  limit?: number;          // Pagination limit
  offset?: number;         // Pagination offset
})
```

#### `PaperResponse`

```typescript
export interface PaperResponse {
  id: string;
  slug: string;
  title: string;
  authors: AuthorResponse[];
  abstract_text: string | null;
  doi: string | null;
  arxiv_id: string | null;
  url: string | null;
  pdf_url: string | null;
  html_url: string | null;
  published_date: string | null;
  added_date: string;
  modified_date: string;
  source: string | null;
  read_status: string;
  rating: number | null;
  tags: TagResponse[];
  attachments: AttachmentResponse[];
  has_pdf: boolean;
  has_html: boolean;
}
```

### Search

| Command | TypeScript Signature | Description |
|---|---|---|
| `search_papers` | `searchPapers(query: string, limit?: number) => PaperResponse[]` | Full-text search via FTS5 |

### Collections

| Command | TypeScript Signature | Description |
|---|---|---|
| `create_collection` | `createCollection(name: string, parentId?: string, description?: string) => CollectionResponse` | Create a collection |
| `list_collections` | `listCollections() => CollectionResponse[]` | List all collections |
| `delete_collection` | `deleteCollection(id: string) => void` | Delete a collection |
| `add_paper_to_collection` | `addPaperToCollection(paperId: string, collectionId: string) => void` | Add paper to collection |
| `remove_paper_from_collection` | `removePaperFromCollection(paperId: string, collectionId: string) => void` | Remove paper from collection |

#### `CollectionResponse`

```typescript
export interface CollectionResponse {
  id: string;
  name: string;
  slug: string;
  parent_id: string | null;
  paper_count: number;
  description: string | null;
}
```

### Tags

| Command | TypeScript Signature | Description |
|---|---|---|
| `list_tags` | `listTags() => TagResponse[]` | List all tags |
| `add_tag_to_paper` | `addTagToPaper(paperId: string, tagName: string) => void` | Add tag (creates if new) |
| `remove_tag_from_paper` | `removeTagFromPaper(paperId: string, tagName: string) => void` | Remove tag from paper |

### Subscriptions

| Command | TypeScript Signature | Description |
|---|---|---|
| `list_subscriptions` | `listSubscriptions() => SubscriptionResponse[]` | List all subscriptions |
| `list_feed_items` | `listFeedItems(subscriptionId: string, limit?: number, offset?: number) => FeedItemResponse[]` | List items from a feed |
| `add_feed_item_to_library` | `addFeedItemToLibrary(itemId: string) => string` | Add feed item to library (returns paper ID) |
| `refresh_subscription` | `refreshSubscription(subscriptionId: string) => number` | Manually refresh a subscription (returns count) |
| `toggle_subscription` | `toggleSubscription(id: string, enabled: boolean) => void` | Enable/disable a subscription |

#### `SubscriptionResponse`

```typescript
export interface SubscriptionResponse {
  id: string;
  source_type: string;
  name: string;
  enabled: boolean;
  poll_interval_minutes: number;
  last_polled: string | null;
}
```

#### `FeedItemResponse`

```typescript
export interface FeedItemResponse {
  id: string;
  external_id: string;
  title: string;
  data: Record<string, unknown> | null;
  fetched_date: string;
  added_to_library: boolean;
}
```

### Import/Export

| Command | TypeScript Signature | Description |
|---|---|---|
| `import_bibtex` | `importBibtex(content: string) => number` | Import from BibTeX (returns count) |
| `export_bibtex` | `exportBibtex(paperIds?: string[]) => string` | Export to BibTeX |
| `import_ris` | `importRis(content: string) => number` | Import from RIS (returns count) |
| `export_ris` | `exportRis(paperIds?: string[]) => string` | Export to RIS |

### Connector Status

| Command | TypeScript Signature | Description |
|---|---|---|
| `get_connector_status` | `getConnectorStatus() => { enabled: boolean; port: number; running: boolean }` | Get connector server status |

## Connector HTTP Endpoints

The connector server (axum, default port 23120) provides HTTP endpoints for external tools. See [Browser Extension](browser-extension.md) for the complete API protocol with request/response examples.

| Method | Path | Description |
|---|---|---|
| `GET` | `/connector/ping` | Health check, returns `{ version, name }` |
| `POST` | `/connector/saveItem` | Save paper metadata to library |
| `POST` | `/connector/saveHtml` | Save HTML content for a paper |
| `GET` | `/connector/status` | Server status |
| `GET` | `/connector/collections` | List collections |

## See Also

- [Architecture Overview](architecture.md) -- System design
- [Agent Integration Guide](agent-integration.md) -- `metadata.json` schema and filesystem layout
- [Browser Extension](browser-extension.md) -- Connector API request/response details
