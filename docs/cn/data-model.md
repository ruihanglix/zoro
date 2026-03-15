# 数据模型与 API 参考

## SQLite 数据库 Schema

数据库文件位于 `~/.zoro/library.db`，使用 SQLite + WAL 模式 + 外键约束。所有建表语句定义在 `crates/zoro-db/src/schema.rs` 中。

### 表结构

#### papers — 论文主表

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

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | TEXT (PK) | UUID v4 |
| `slug` | TEXT (UNIQUE) | 论文目录名，格式 `{year}-{title-slug}-{8char-hash}` |
| `title` | TEXT | 论文标题 |
| `abstract_text` | TEXT | 摘要 |
| `doi` | TEXT | DOI 标识符 |
| `arxiv_id` | TEXT | ArXiv ID |
| `url` | TEXT | 论文页面 URL |
| `pdf_url` | TEXT | PDF 下载链接 |
| `html_url` | TEXT | HTML 版本链接 |
| `published_date` | TEXT | 发表日期 |
| `added_date` | TEXT | 添加到库的时间（RFC 3339） |
| `modified_date` | TEXT | 最后修改时间（RFC 3339） |
| `source` | TEXT | 来源：`"browser-extension"`, `"subscription"`, `"manual"`, `"import"` |
| `read_status` | TEXT | 阅读状态：`"unread"`, `"reading"`, `"read"` |
| `rating` | INTEGER | 评分（1-5） |
| `extra_json` | TEXT | JSON 格式的额外数据。包含 `labels` 数组用于存储只读元数据标签（如 arXiv 分类、Zotero 标签），与用户手动管理的侧边栏 tags 分开 |
| `dir_path` | TEXT | 论文目录相对路径，如 `papers/{slug}` |

#### authors — 作者表

```sql
CREATE TABLE IF NOT EXISTS authors (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    affiliation TEXT,
    orcid TEXT
);
```

#### paper_authors — 论文-作者关联表

```sql
CREATE TABLE IF NOT EXISTS paper_authors (
    paper_id TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
    author_id TEXT NOT NULL REFERENCES authors(id),
    position INTEGER NOT NULL,
    PRIMARY KEY (paper_id, author_id)
);
```

`position` 字段表示作者在论文中的排序位置。

#### collections — 集合（文件夹）表

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
-- 作用域唯一：不同父级下允许相同 slug
CREATE UNIQUE INDEX idx_collections_slug_parent
    ON collections(slug, COALESCE(parent_id, ''));
```

支持通过 `parent_id` 实现嵌套集合。不同父文件夹下可以有同名的子文件夹。

#### paper_collections — 论文-集合关联表

```sql
CREATE TABLE IF NOT EXISTS paper_collections (
    paper_id TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
    collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    added_date TEXT NOT NULL,
    PRIMARY KEY (paper_id, collection_id)
);
```

#### tags — 标签表

```sql
CREATE TABLE IF NOT EXISTS tags (
    id TEXT PRIMARY KEY,
    name TEXT UNIQUE NOT NULL,
    color TEXT
);
```

#### paper_tags — 论文-标签关联表

```sql
CREATE TABLE IF NOT EXISTS paper_tags (
    paper_id TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    source TEXT DEFAULT 'manual',
    PRIMARY KEY (paper_id, tag_id)
);
```

`source` 字段标记标签来源：`"manual"`（用户手动）。标签仅允许用户手动创建和管理 — 来自 Zotero Connector 或 arXiv 的外部标签存储在 `extra_json.labels` 中，不会自动创建为侧边栏标签。

#### attachments — 附件表

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

#### subscriptions — 订阅表

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

#### subscription_items — 订阅条目表

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

### FTS5 全文搜索

使用 SQLite FTS5 扩展对论文标题和摘要进行全文索引，支持 Porter 词干提取和 Unicode 分词：

```sql
CREATE VIRTUAL TABLE papers_fts USING fts5(
    title,
    abstract_text,
    content='papers',
    content_rowid='rowid',
    tokenize='porter unicode61'
);
```

配置说明：

- `content='papers'`：内容来自 `papers` 表（content-sync 模式）
- `content_rowid='rowid'`：使用 `papers` 表的 `rowid` 作为关联
- `tokenize='porter unicode61'`：使用 Porter 词干提取 + Unicode 分词器

### 触发器

自动同步 `papers` 表的变更到 FTS5 索引：

```sql
-- 插入后同步
CREATE TRIGGER papers_ai AFTER INSERT ON papers BEGIN
    INSERT INTO papers_fts(rowid, title, abstract_text)
    VALUES (new.rowid, new.title, new.abstract_text);
END;

-- 删除后同步
CREATE TRIGGER papers_ad AFTER DELETE ON papers BEGIN
    INSERT INTO papers_fts(papers_fts, rowid, title, abstract_text)
    VALUES ('delete', old.rowid, old.title, old.abstract_text);
END;

-- 更新后同步（先删旧记录，再插新记录）
CREATE TRIGGER papers_au AFTER UPDATE ON papers BEGIN
    INSERT INTO papers_fts(papers_fts, rowid, title, abstract_text)
    VALUES ('delete', old.rowid, old.title, old.abstract_text);
    INSERT INTO papers_fts(rowid, title, abstract_text)
    VALUES (new.rowid, new.title, new.abstract_text);
END;
```

### 索引

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

### 数据库初始化

数据库在 `Database::open()` 时自动初始化：

```rust
pub struct Database {
    pub conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, DbError> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self { conn };
        db.initialize()?; // 调用 schema::create_tables()
        Ok(db)
    }
}
```

## Rust 领域模型

定义在 `crates/zoro-core/src/models.rs` 中。

### Paper — 论文

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paper {
    pub id: String,                        // UUID v4
    pub slug: String,                      // 目录名 slug
    pub title: String,                     // 标题
    pub authors: Vec<Author>,              // 作者列表
    #[serde(rename = "abstract")]
    pub abstract_text: Option<String>,     // 摘要
    pub doi: Option<String>,              // DOI
    pub arxiv_id: Option<String>,         // ArXiv ID
    pub url: Option<String>,              // 论文页面 URL
    pub pdf_url: Option<String>,          // PDF 下载链接
    pub html_url: Option<String>,         // HTML 版本链接
    pub published_date: Option<String>,   // 发表日期
    pub added_date: String,               // 入库时间 (RFC 3339)
    pub modified_date: String,            // 修改时间 (RFC 3339)
    pub source: Option<String>,           // 来源
    pub tags: Vec<String>,                // 标签名列表
    pub collections: Vec<String>,         // 集合名列表
    pub attachments: Vec<AttachmentInfo>, // 附件信息
    pub notes: Vec<String>,              // 笔记列表
    pub read_status: ReadStatus,          // 阅读状态
    pub rating: Option<u8>,               // 评分 (1-5)
    pub extra: serde_json::Value,         // 额外 JSON 数据
}
```

### Author — 作者

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    pub name: String,                  // 姓名
    pub affiliation: Option<String>,   // 单位/机构
    pub orcid: Option<String>,         // ORCID 标识符
}
```

### AttachmentInfo — 附件简要信息

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentInfo {
    pub filename: String,              // 文件名
    #[serde(rename = "type")]
    pub attachment_type: String,       // 类型（如 "pdf", "html"）
    pub created: String,               // 创建时间
}
```

### ReadStatus — 阅读状态

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ReadStatus {
    Unread,
    Reading,
    Read,
}
```

JSON 序列化为小写字符串：`"unread"`、`"reading"`、`"read"`。

### Collection — 集合

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub id: String,                    // UUID v4
    pub name: String,                  // 集合名称
    pub slug: String,                  // URL-friendly slug
    pub parent_id: Option<String>,     // 父集合 ID（支持嵌套）
    pub position: i32,                 // 排序位置
    pub created_date: String,          // 创建时间
    pub description: Option<String>,   // 描述
}
```

### Tag — 标签

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: String,                    // UUID v4
    pub name: String,                  // 标签名
    pub color: Option<String>,         // 颜色（如 "#ff6b6b"）
}
```

### Attachment — 附件

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String,                     // UUID v4
    pub paper_id: String,              // 所属论文 ID
    pub filename: String,              // 文件名
    pub file_type: String,             // 类型
    pub mime_type: Option<String>,     // MIME 类型
    pub file_size: Option<i64>,        // 文件大小（字节）
    pub relative_path: String,         // 相对于论文目录的路径
    pub created_date: String,          // 创建时间
    pub modified_date: String,         // 修改时间
    pub source: String,                // 来源
    pub metadata: Option<serde_json::Value>, // 元数据 JSON
}
```

### Subscription — 订阅

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: String,                          // UUID v4
    pub source_type: String,                 // 源类型标识（如 "huggingface-daily"）
    pub name: String,                        // 显示名称
    pub config: Option<serde_json::Value>,   // 配置 JSON
    pub enabled: bool,                       // 是否启用
    pub poll_interval_minutes: i32,          // 轮询间隔（分钟）
    pub last_polled: Option<String>,         // 上次轮询时间
    pub created_date: String,                // 创建时间
}
```

### SubscriptionItem — 订阅条目

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionItem {
    pub id: String,                          // UUID v4
    pub subscription_id: String,             // 所属订阅 ID
    pub paper_id: Option<String>,            // 关联论文 ID
    pub external_id: String,                 // 外部标识（如 ArXiv ID）
    pub title: String,                       // 标题
    pub authors: Vec<Author>,                // 作者
    pub abstract_text: Option<String>,       // 摘要
    pub url: Option<String>,                 // URL
    pub pdf_url: Option<String>,             // PDF 链接
    pub html_url: Option<String>,            // HTML 链接
    pub upvotes: Option<i32>,                // 投票数
    pub data: Option<serde_json::Value>,     // 额外数据
    pub fetched_date: String,                // 抓取时间
    pub added_to_library: bool,              // 是否已添加到库
}
```

### PaperMetadata — 论文元数据文件

存储在 `~/.zoro/library/papers/{slug}/metadata.json` 中的结构：

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

### AppConfig — 应用配置

存储在 `~/.zoro/config.toml` 中：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub connector: ConnectorConfig,
    pub subscriptions: SubscriptionsConfig,
    pub ai: AiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub data_dir: String,          // 数据目录，默认 "~/.zoro"
    pub language: String,          // 语言，默认 "en"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorConfig {
    pub port: u16,                 // HTTP 端口，默认 23120
    pub enabled: bool,             // 是否启用，默认 true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionsConfig {
    pub poll_interval_minutes: i32, // 轮询间隔，默认 60 分钟
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub provider: String,          // AI 提供商
    pub api_key: String,           // API 密钥
    pub auto_summarize: bool,      // 自动摘要，默认 false
    pub auto_tag: bool,            // 自动标签，默认 false
}
```

## Tauri 命令 API 参考

所有 Tauri IPC 命令定义在 `apps/desktop/src/lib/commands.ts` 中，通过 `invoke()` 调用 Rust 后端。

### 论文管理

#### addPaper — 添加论文

```typescript
export const addPaper = (input: AddPaperInput) =>
  invoke<PaperResponse>("add_paper", { input });
```

**输入**：

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

**返回**：`PaperResponse`

#### getPaper — 获取论文详情

```typescript
export const getPaper = (id: string) =>
  invoke<PaperResponse>("get_paper", { id });
```

#### listPapers — 列出论文

```typescript
export const listPapers = (params?: {
  collectionId?: string;
  tagName?: string;
  readStatus?: string;
  sortBy?: string;
  sortOrder?: string;
  limit?: number;
  offset?: number;
}) => invoke<PaperResponse[]>("list_papers", { ... });
```

支持按集合、标签、阅读状态筛选，以及排序和分页。

#### deletePaper — 删除论文

```typescript
export const deletePaper = (id: string) =>
  invoke<void>("delete_paper", { id });
```

#### updatePaperStatus — 更新阅读状态

```typescript
export const updatePaperStatus = (id: string, readStatus: string) =>
  invoke<void>("update_paper_status", { id, read_status: readStatus });
```

`readStatus` 取值：`"unread"`, `"reading"`, `"read"`

#### updatePaperRating — 更新评分

```typescript
export const updatePaperRating = (id: string, rating: number | null) =>
  invoke<void>("update_paper_rating", { id, rating });
```

### 搜索

#### searchPapers — 全文搜索

```typescript
export const searchPapers = (query: string, limit?: number) =>
  invoke<PaperResponse[]>("search_papers", { query, limit: limit ?? null });
```

使用 FTS5 进行全文搜索，匹配标题和摘要。

### 集合管理

#### createCollection — 创建集合

```typescript
export const createCollection = (name: string, parentId?: string, description?: string) =>
  invoke<CollectionResponse>("create_collection", {
    name,
    parent_id: parentId ?? null,
    description: description ?? null,
  });
```

#### listCollections — 列出所有集合

```typescript
export const listCollections = () =>
  invoke<CollectionResponse[]>("list_collections");
```

#### deleteCollection — 删除集合

```typescript
export const deleteCollection = (id: string) =>
  invoke<void>("delete_collection", { id });
```

#### addPaperToCollection — 添加论文到集合

```typescript
export const addPaperToCollection = (paperId: string, collectionId: string) =>
  invoke<void>("add_paper_to_collection", { paper_id: paperId, collection_id: collectionId });
```

#### removePaperFromCollection — 从集合移除论文

```typescript
export const removePaperFromCollection = (paperId: string, collectionId: string) =>
  invoke<void>("remove_paper_from_collection", { paper_id: paperId, collection_id: collectionId });
```

### 标签管理

#### listTags — 列出所有标签

```typescript
export const listTags = () =>
  invoke<TagResponse[]>("list_tags");
```

#### addTagToPaper — 给论文添加标签

```typescript
export const addTagToPaper = (paperId: string, tagName: string) =>
  invoke<void>("add_tag_to_paper", { paper_id: paperId, tag_name: tagName });
```

#### removeTagFromPaper — 移除论文标签

```typescript
export const removeTagFromPaper = (paperId: string, tagName: string) =>
  invoke<void>("remove_tag_from_paper", { paper_id: paperId, tag_name: tagName });
```

### 订阅管理

#### listSubscriptions — 列出所有订阅

```typescript
export const listSubscriptions = () =>
  invoke<SubscriptionResponse[]>("list_subscriptions");
```

#### listFeedItems — 列出订阅条目

```typescript
export const listFeedItems = (subscriptionId: string, limit?: number, offset?: number) =>
  invoke<FeedItemResponse[]>("list_feed_items", {
    subscription_id: subscriptionId,
    limit: limit ?? null,
    offset: offset ?? null,
  });
```

#### addFeedItemToLibrary — 将订阅条目添加到论文库

```typescript
export const addFeedItemToLibrary = (itemId: string) =>
  invoke<string>("add_feed_item_to_library", { item_id: itemId });
```

返回新创建的论文 ID。

#### refreshSubscription — 刷新订阅

```typescript
export const refreshSubscription = (subscriptionId: string) =>
  invoke<number>("refresh_subscription", { subscription_id: subscriptionId });
```

返回获取到的新条目数量。

#### toggleSubscription — 启用/禁用订阅

```typescript
export const toggleSubscription = (id: string, enabled: boolean) =>
  invoke<void>("toggle_subscription", { id, enabled });
```

### 导入/导出

#### importBibtex — 导入 BibTeX

```typescript
export const importBibtex = (content: string) =>
  invoke<number>("import_bibtex", { content });
```

返回导入的论文数量。

#### exportBibtex — 导出 BibTeX

```typescript
export const exportBibtex = (paperIds?: string[]) =>
  invoke<string>("export_bibtex", { paper_ids: paperIds ?? null });
```

不传 `paperIds` 则导出全部。返回 BibTeX 格式字符串。

#### importRis — 导入 RIS

```typescript
export const importRis = (content: string) =>
  invoke<number>("import_ris", { content });
```

#### exportRis — 导出 RIS

```typescript
export const exportRis = (paperIds?: string[]) =>
  invoke<string>("export_ris", { paper_ids: paperIds ?? null });
```

### Connector 状态

#### getConnectorStatus — 获取 Connector 状态

```typescript
export const getConnectorStatus = () =>
  invoke<{ enabled: boolean; port: number; running: boolean }>("get_connector_status");
```

### 响应类型

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

export interface AuthorResponse {
  name: string;
  affiliation: string | null;
}

export interface TagResponse {
  id: string;
  name: string;
  color: string | null;
}

export interface AttachmentResponse {
  id: string;
  filename: string;
  file_type: string;
  file_size: number | null;
  source: string;
}

export interface CollectionResponse {
  id: string;
  name: string;
  slug: string;
  parent_id: string | null;
  paper_count: number;
  description: string | null;
}

export interface SubscriptionResponse {
  id: string;
  source_type: string;
  name: string;
  enabled: boolean;
  poll_interval_minutes: number;
  last_polled: string | null;
}

export interface FeedItemResponse {
  id: string;
  external_id: string;
  title: string;
  data: Record<string, unknown> | null;
  fetched_date: string;
  added_to_library: boolean;
}
```

## Connector HTTP 端点

详细的 Connector API 协议参见[浏览器扩展文档](browser-extension.md#connector-api-协议)。

| 方法 | 端点 | 说明 |
|------|------|------|
| GET | `/connector/ping` | 测试连接 |
| POST | `/connector/saveItem` | 保存论文 |
| POST | `/connector/saveHtml` | 保存 HTML 内容 |
| GET | `/connector/status` | 查询状态 |
| GET | `/connector/collections` | 获取集合列表 |
