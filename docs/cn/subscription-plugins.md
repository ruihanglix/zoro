# 订阅插件开发

## 概述

Zoro 的订阅系统采用插件化架构，通过 `SubscriptionSource` trait 定义统一的插件接口。每个插件负责从特定数据源获取论文信息，应用会定时轮询已启用的订阅源并将结果呈现给用户。

目前内置的订阅源：

- **HuggingFace Daily Papers**：从 HuggingFace 社区获取每日 AI/ML 精选论文

订阅源插件代码位于 `crates/zoro-subscriptions/` crate 中。

## `SubscriptionSource` Trait 完整 API 参考

以下是 `source.rs` 中定义的完整 trait：

```rust
use async_trait::async_trait;
use zoro_core::models::SubscriptionItem;
use crate::error::SubscriptionError;

/// Trait that subscription source plugins implement
#[async_trait]
pub trait SubscriptionSource: Send + Sync {
    /// Unique identifier for this source type (e.g., "huggingface-daily")
    fn source_type(&self) -> &str;

    /// Human-readable display name
    fn display_name(&self) -> &str;

    /// Fetch new items, optionally since a given timestamp
    async fn fetch(
        &self,
        config: &serde_json::Value,
        since: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Vec<SubscriptionItem>, SubscriptionError>;

    /// Default configuration for this source
    fn default_config(&self) -> serde_json::Value;

    /// Description of what this source provides
    fn description(&self) -> &str;
}
```

### 方法说明

| 方法 | 返回类型 | 说明 |
|------|---------|------|
| `source_type()` | `&str` | 插件唯一标识符，用于数据库中的 `source_type` 字段。应使用 kebab-case 格式，如 `"huggingface-daily"`、`"semantic-scholar"` |
| `display_name()` | `&str` | 用户界面中显示的名称，如 `"HuggingFace Daily Papers"` |
| `fetch()` | `Result<Vec<SubscriptionItem>, SubscriptionError>` | 核心方法。获取新条目，`config` 是用户配置的 JSON，`since` 是上次轮询时间戳（可选） |
| `default_config()` | `serde_json::Value` | 返回该插件的默认配置 JSON，在创建新订阅时使用 |
| `description()` | `&str` | 插件的简短描述文本 |

### `SubscriptionItem` 结构

`fetch()` 方法返回的条目类型（定义在 `zoro-core/src/models.rs`）：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionItem {
    pub id: String,                    // UUID，由插件生成
    pub subscription_id: String,       // 留空，由调用方设置
    pub paper_id: Option<String>,      // 关联的论文 ID（尚未添加到库时为 None）
    pub external_id: String,           // 外部唯一标识（如 ArXiv ID）
    pub title: String,                 // 论文标题
    pub authors: Vec<Author>,          // 作者列表
    pub abstract_text: Option<String>, // 摘要
    pub url: Option<String>,           // 论文页面 URL
    pub pdf_url: Option<String>,       // PDF 下载链接
    pub html_url: Option<String>,      // HTML 版本链接
    pub upvotes: Option<i32>,          // 投票数/热度（可选）
    pub data: Option<serde_json::Value>, // 插件特定的额外数据
    pub fetched_date: String,          // 抓取时间（RFC 3339）
    pub added_to_library: bool,        // 始终设为 false
}
```

## 创建新订阅源插件：分步指南

以 Semantic Scholar 推荐论文为例，演示如何创建新的订阅源插件。

### 步骤 1：创建插件文件

在 `crates/zoro-subscriptions/src/` 下创建新文件：

```bash
touch crates/zoro-subscriptions/src/semantic_scholar.rs
```

### 步骤 2：实现 `SubscriptionSource` trait

```rust
use async_trait::async_trait;
use serde::Deserialize;
use tracing::info;
use zoro_core::models::{Author, SubscriptionItem};
use crate::error::SubscriptionError;
use crate::source::SubscriptionSource;

pub struct SemanticScholarRecommended {
    client: reqwest::Client,
}

impl SemanticScholarRecommended {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Default for SemanticScholarRecommended {
    fn default() -> Self {
        Self::new()
    }
}

// API 响应类型
#[derive(Debug, Deserialize)]
struct S2Paper {
    #[serde(rename = "paperId")]
    paper_id: String,
    title: Option<String>,
    #[serde(rename = "abstract")]
    abstract_text: Option<String>,
    authors: Option<Vec<S2Author>>,
    year: Option<i32>,
    #[serde(rename = "externalIds")]
    external_ids: Option<S2ExternalIds>,
}

#[derive(Debug, Deserialize)]
struct S2Author {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct S2ExternalIds {
    #[serde(rename = "ArXiv")]
    arxiv: Option<String>,
    #[serde(rename = "DOI")]
    doi: Option<String>,
}

#[async_trait]
impl SubscriptionSource for SemanticScholarRecommended {
    fn source_type(&self) -> &str {
        "semantic-scholar"
    }

    fn display_name(&self) -> &str {
        "Semantic Scholar Recommended"
    }

    fn description(&self) -> &str {
        "Recommended papers from Semantic Scholar API"
    }

    fn default_config(&self) -> serde_json::Value {
        serde_json::json!({
            "fields_of_study": ["Computer Science"],
            "limit": 20
        })
    }

    async fn fetch(
        &self,
        config: &serde_json::Value,
        _since: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Vec<SubscriptionItem>, SubscriptionError> {
        info!("Fetching Semantic Scholar recommended papers...");

        let limit = config.get("limit")
            .and_then(|v| v.as_i64())
            .unwrap_or(20);

        let url = format!(
            "https://api.semanticscholar.org/graph/v1/paper/search?query=machine+learning&limit={}&fields=title,abstract,authors,year,externalIds",
            limit
        );

        let response = self.client
            .get(&url)
            .header("User-Agent", "Zoro/0.1.0")
            .send()
            .await?;

        // 解析响应并转换为 SubscriptionItem
        let data: serde_json::Value = response.json().await?;
        let papers: Vec<S2Paper> = serde_json::from_value(
            data.get("data").cloned().unwrap_or_default()
        ).unwrap_or_default();

        info!("Fetched {} papers from Semantic Scholar", papers.len());

        let items = papers.into_iter().filter_map(|p| {
            let title = p.title?;
            let external_id = p.paper_id.clone();

            let authors = p.authors.unwrap_or_default()
                .into_iter()
                .filter_map(|a| a.name.map(|name| Author {
                    name,
                    affiliation: None,
                    orcid: None,
                }))
                .collect();

            let arxiv_id = p.external_ids.as_ref()
                .and_then(|ids| ids.arxiv.clone());

            let url = arxiv_id.as_ref()
                .map(|id| format!("https://arxiv.org/abs/{}", id))
                .or(Some(format!("https://www.semanticscholar.org/paper/{}", p.paper_id)));

            let now = chrono::Utc::now().to_rfc3339();

            Some(SubscriptionItem {
                id: uuid::Uuid::new_v4().to_string(),
                subscription_id: String::new(),
                paper_id: None,
                external_id,
                title,
                authors,
                abstract_text: p.abstract_text,
                url,
                pdf_url: arxiv_id.as_ref().map(|id| format!("https://arxiv.org/pdf/{}", id)),
                html_url: arxiv_id.as_ref().map(|id| format!("https://arxiv.org/html/{}", id)),
                upvotes: None,
                data: Some(serde_json::json!({
                    "year": p.year,
                    "semantic_scholar_id": p.paper_id,
                })),
                fetched_date: now,
                added_to_library: false,
            })
        }).collect();

        Ok(items)
    }
}
```

### 步骤 3：在 crate 中注册模块

编辑 `crates/zoro-subscriptions/src/lib.rs`：

```rust
pub mod source;
pub mod huggingface;
pub mod semantic_scholar;  // 新增
pub mod error;

pub use source::SubscriptionSource;
pub use huggingface::HuggingFaceDailyPapers;
pub use semantic_scholar::SemanticScholarRecommended;  // 新增
pub use error::SubscriptionError;
```

### 步骤 4：在桌面应用中注册

编辑 `apps/desktop/src-tauri/src/lib.rs` 的 `ensure_default_subscriptions` 函数，添加默认订阅（可选）：

```rust
fn ensure_default_subscriptions(db: &zoro_db::Database) {
    use zoro_db::queries::subscriptions;
    let subs = subscriptions::list_subscriptions(&db.conn).unwrap_or_default();

    // 已有的 HuggingFace 订阅
    if !subs.iter().any(|s| s.source_type == "huggingface-daily") {
        let _ = subscriptions::create_subscription(
            &db.conn,
            "huggingface-daily",
            "HuggingFace Daily Papers",
            Some(r#"{"auto_download_pdf": false}"#),
            60,
        );
    }

    // 新增：Semantic Scholar 订阅
    if !subs.iter().any(|s| s.source_type == "semantic-scholar") {
        let _ = subscriptions::create_subscription(
            &db.conn,
            "semantic-scholar",
            "Semantic Scholar Recommended",
            Some(r#"{"fields_of_study": ["Computer Science"], "limit": 20}"#),
            120,
        );
    }
}
```

同时需要在订阅轮询器中注册新源（`subscriptions/` 模块），使其能够被调度执行。

### 步骤 5：添加测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // 需要网络访问
    async fn test_fetch_semantic_scholar() {
        let source = SemanticScholarRecommended::new();
        let config = source.default_config();
        let items = source.fetch(&config, None).await.unwrap();
        assert!(!items.is_empty(), "Should fetch at least one paper");
        for item in &items {
            assert!(!item.title.is_empty(), "Paper should have a title");
            assert!(!item.external_id.is_empty(), "Paper should have an ID");
        }
    }
}
```

## HuggingFace Daily Papers：参考实现

内置的 HuggingFace Daily Papers 是理想的参考实现，完整代码在 `crates/zoro-subscriptions/src/huggingface.rs`。

### 关键实现要点

**1. 结构体定义**

```rust
pub struct HuggingFaceDailyPapers {
    client: reqwest::Client,
}

impl HuggingFaceDailyPapers {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}
```

保持 `reqwest::Client` 作为成员变量，以复用连接池。

**2. API 响应类型**

为外部 API 的 JSON 响应定义专用的反序列化结构体：

```rust
#[derive(Debug, Deserialize)]
struct DailyPaperResponse {
    paper: HfPaper,
    title: Option<String>,
    #[serde(rename = "publishedAt")]
    published_at: Option<String>,
    upvotes: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct HfPaper {
    id: String,          // ArXiv ID like "2603.05706"
    title: Option<String>,
    summary: Option<String>,
    authors: Option<Vec<HfAuthor>>,
}
```

**3. Fetch 实现**

```rust
async fn fetch(
    &self,
    _config: &serde_json::Value,
    _since: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<Vec<SubscriptionItem>, SubscriptionError> {
    info!("Fetching HuggingFace Daily Papers...");

    let url = "https://huggingface.co/api/daily_papers";
    let response = self.client
        .get(url)
        .header("User-Agent", "Zoro/0.1.0")
        .send()
        .await?;

    let papers: Vec<DailyPaperResponse> = response.json().await?;
    info!("Fetched {} papers from HuggingFace", papers.len());

    let items = papers.into_iter().map(|dp| {
        let arxiv_id = dp.paper.id.clone();
        // ... 转换逻辑
        SubscriptionItem {
            id: uuid::Uuid::new_v4().to_string(),
            subscription_id: String::new(), // 由调用方设置
            external_id: arxiv_id,
            // ...
            fetched_date: chrono::Utc::now().to_rfc3339(),
            added_to_library: false,
        }
    }).collect();

    Ok(items)
}
```

## 配置 Schema 设计模式

每个订阅源的配置以 `serde_json::Value`（JSON）形式存储在数据库的 `subscriptions.config_json` 字段中。推荐的配置 schema 设计：

```json
{
  "auto_download_pdf": false,
  "auto_download_html": false,
  "fields_of_study": ["Computer Science", "Mathematics"],
  "limit": 20,
  "api_key": "optional-if-needed"
}
```

在 `default_config()` 方法中提供合理的默认值：

```rust
fn default_config(&self) -> serde_json::Value {
    serde_json::json!({
        "auto_download_pdf": false,
        "auto_download_html": false
    })
}
```

在 `fetch()` 方法中读取配置：

```rust
async fn fetch(
    &self,
    config: &serde_json::Value,
    since: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<Vec<SubscriptionItem>, SubscriptionError> {
    let limit = config.get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(20);

    let api_key = config.get("api_key")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // 使用配置进行 API 调用...
}
```

## Fetch 生命周期和错误处理

### 生命周期

```
订阅轮询器定时触发
    │
    ▼
检查订阅是否启用 (enabled == true)
    │
    ▼
计算 since 时间戳（来自 last_polled 字段）
    │
    ▼
调用 source.fetch(config, since)
    │
    ├── 成功 → 返回 Vec<SubscriptionItem>
    │           │
    │           ▼ 存入 subscription_items 表
    │           │
    │           ▼ 更新 last_polled 时间戳
    │
    └── 失败 → 记录错误日志，不更新 last_polled
              下次轮询重试
```

### 错误处理

`SubscriptionError` 定义在 `crates/zoro-subscriptions/src/error.rs` 中，使用 `thiserror` 派生：

```rust
#[derive(Debug, thiserror::Error)]
pub enum SubscriptionError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Source error: {0}")]
    Source(String),
}
```

在 `fetch()` 中使用 `?` 操作符传播错误，`reqwest::Error` 和 `serde_json::Error` 会通过 `#[from]` 自动转换：

```rust
// reqwest 错误自动转为 SubscriptionError::Http
let response = self.client.get(url).send().await?;

// JSON 解析错误自动转为 SubscriptionError::Json
let papers: Vec<ApiResponse> = response.json().await?;

// 自定义错误
if papers.is_empty() {
    return Err(SubscriptionError::Source("No papers returned".into()));
}
```

## 如何在应用中注册新的订阅源

注册新源需要修改桌面应用的订阅轮询器模块，确保它知道如何根据 `source_type` 字符串实例化对应的插件：

1. 在 `crates/zoro-subscriptions/src/lib.rs` 中导出新类型
2. 在桌面应用的 `subscriptions/` 模块中，将 `source_type` 字符串映射到具体实现
3. 可选：在 `ensure_default_subscriptions()` 中创建默认订阅记录

## 测试订阅插件

### 单元测试

对于不依赖网络的逻辑（如数据转换），编写常规单元测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_type() {
        let source = MySource::new();
        assert_eq!(source.source_type(), "my-source");
    }

    #[test]
    fn test_default_config() {
        let source = MySource::new();
        let config = source.default_config();
        assert!(config.get("limit").is_some());
    }
}
```

### 集成测试

对需要网络的 `fetch()` 方法，使用 `#[ignore]` 标记：

```rust
#[tokio::test]
#[ignore] // 需要网络访问
async fn test_fetch() {
    let source = MySource::new();
    let config = source.default_config();
    let items = source.fetch(&config, None).await.unwrap();
    assert!(!items.is_empty());
}
```

运行被忽略的测试：

```bash
# 运行全部测试（包括 ignored）
cargo test -p zoro-subscriptions -- --ignored

# 只运行特定的 ignored 测试
cargo test -p zoro-subscriptions test_fetch -- --ignored
```

### 检查清单

- [ ] `source_type()` 返回唯一的 kebab-case 标识符
- [ ] `display_name()` 返回用户友好的名称
- [ ] `description()` 返回简短描述
- [ ] `default_config()` 返回合理的默认配置
- [ ] `fetch()` 正确处理空结果
- [ ] `fetch()` 正确处理网络错误
- [ ] `fetch()` 生成有效的 `SubscriptionItem`（`id` 使用 UUID，`subscription_id` 留空）
- [ ] `external_id` 在同一源内唯一，用于去重
- [ ] 网络测试标记为 `#[ignore]`
- [ ] 在 `lib.rs` 中注册了 `pub mod` 和 `pub use`
