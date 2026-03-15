# Agent 集成指南

## 概述：为什么 Zoro 对 AI Agent 友好

Zoro 从设计之初就考虑了 AI Agent 的使用场景。与传统文献管理工具使用封闭的数据库格式不同，Zoro 将论文存储在**人类可读、Agent 可访问**的目录结构中：

- 每篇论文对应一个独立目录
- 元数据以标准 JSON 格式存储（`metadata.json`）
- 目录名使用语义化的 slug 格式，便于浏览和检索
- 预设的 `attachments/` 和 `notes/` 目录，Agent 可直接写入
- 无需通过 API 或数据库连接即可读写论文数据

这使得 Claude Code、OpenClaw 等 AI Agent 能够直接与论文库交互：读取元数据、生成摘要、翻译论文、批量处理等。

## `~/.zoro/` 文件结构

```
~/.zoro/
├── config.toml                              # 应用配置文件
├── library.db                               # SQLite 数据库
│
└── library/
    └── papers/
        ├── 2017-attention-is-all-you-need-a1b2c3d4/
        │   ├── metadata.json                # 论文元数据（JSON）
        │   ├── paper.pdf                    # PDF 版本
        │   ├── abs.html                   # HTML 版本
        │   ├── attachments/                 # Agent 可写目录
        │   │   ├── summary.md               # AI 生成的摘要
        │   │   ├── translation-zh.md        # AI 生成的中文翻译
        │   │   └── key-findings.md          # AI 提取的关键发现
        │   └── notes/                       # 用户和 Agent 笔记
        │       ├── reading-notes.md         # 阅读笔记
        │       └── review.md                # 评审意见
        │
        ├── 2024-scaling-laws-for-neural-b7e8f9a0/
        │   ├── metadata.json
        │   ├── paper.pdf
        │   ├── attachments/
        │   └── notes/
        │
        └── ...
```

### 关键目录和文件

| 路径 | 说明 | 权限 |
|------|------|------|
| `config.toml` | 应用全局配置 | 读写 |
| `library.db` | SQLite 数据库 | 应用管理 |
| `library/papers/` | 论文库根目录 | 只读浏览 |
| `library/papers/{slug}/` | 单篇论文目录 | 只读 |
| `library/papers/{slug}/metadata.json` | 论文元数据 | 只读 |
| `library/papers/{slug}/paper.pdf` | PDF 文件 | 只读 |
| `library/papers/{slug}/abs.html` | HTML 文件 | 只读 |
| `library/papers/{slug}/attachments/` | Agent 可写附件目录 | **读写** |
| `library/papers/{slug}/notes/` | 笔记目录 | **读写** |

## Paper Slug 格式

每篇论文的目录名（slug）由以下部分组成：

```
{year}-{title-slug}-{8字符-sha256-hash}
```

**生成规则**（来自 `crates/zoro-core/src/slug_utils.rs`）：

```rust
pub fn generate_paper_slug(title: &str, identifier: &str, year: Option<&str>) -> String {
    let year_str = year
        .map(|y| y[..4].to_string())
        .unwrap_or_else(|| chrono::Utc::now().format("%Y").to_string());

    let title_slug = slugify(title);
    let truncated = truncate_on_word_boundary(&title_slug, 40);

    let mut hasher = Sha256::new();
    hasher.update(identifier.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    let short_hash = &hash[..8];

    format!("{}-{}-{}", year_str, truncated, short_hash)
}
```

| 部分 | 说明 | 示例 |
|------|------|------|
| `year` | 4 位年份（来自发表日期，缺省用当前年份） | `2017` |
| `title-slug` | 标题的 URL-safe slug（最多 40 字符，在词边界截断） | `attention-is-all-you-need` |
| `hash` | `identifier`（DOI 或 ArXiv ID）的 SHA-256 前 8 个十六进制字符 | `a1b2c3d4` |

**示例**：

```
2017-attention-is-all-you-need-a1b2c3d4
2024-scaling-laws-for-neural-language-model-b7e8f9a0
2025-a-very-long-title-that-should-be-truncated-on-9f8e7d6c
```

## `metadata.json` Schema

每个论文目录下的 `metadata.json` 遵循 `PaperMetadata` 结构（定义在 `crates/zoro-core/src/models.rs`）：

```json
{
  "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "slug": "2017-attention-is-all-you-need-a1b2c3d4",
  "title": "Attention Is All You Need",
  "authors": [
    {
      "name": "Ashish Vaswani",
      "affiliation": "Google Brain",
      "orcid": null
    },
    {
      "name": "Noam Shazeer",
      "affiliation": "Google Brain",
      "orcid": null
    }
  ],
  "abstract": "The dominant sequence transduction models are based on complex recurrent or convolutional neural networks...",
  "doi": "10.48550/arXiv.1706.03762",
  "arxiv_id": "1706.03762",
  "url": "https://arxiv.org/abs/1706.03762",
  "pdf_url": "https://arxiv.org/pdf/1706.03762",
  "html_url": "https://arxiv.org/html/1706.03762",
  "published_date": "2017-06-12T00:00:00Z",
  "added_date": "2024-01-15T10:30:00Z",
  "source": "browser-extension",
  "tags": ["transformer", "attention", "NLP"],
  "collections": ["Deep Learning Fundamentals"],
  "attachments": [
    {
      "filename": "paper.pdf",
      "type": "pdf",
      "created": "2024-01-15T10:30:05Z"
    }
  ],
  "notes": [],
  "read_status": "read",
  "rating": 5,
  "extra": {
    "labels": ["cs.AI", "Machine Learning"]
  }
}
```

### 字段参考

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | `string` | UUID v4 唯一标识 |
| `slug` | `string` | 目录名 |
| `title` | `string` | 论文标题 |
| `authors` | `Author[]` | 作者列表 |
| `abstract` | `string?` | 摘要（JSON 中的键名是 `"abstract"`） |
| `doi` | `string?` | DOI |
| `arxiv_id` | `string?` | ArXiv ID |
| `url` | `string?` | 论文 URL |
| `pdf_url` | `string?` | PDF 链接 |
| `html_url` | `string?` | HTML 链接 |
| `published_date` | `string?` | 发表日期 |
| `added_date` | `string` | 入库时间 |
| `source` | `string?` | 来源 |
| `tags` | `string[]` | 标签列表 |
| `collections` | `string[]` | 所属集合 |
| `attachments` | `AttachmentInfo[]` | 附件信息 |
| `notes` | `string[]` | 笔记 |
| `read_status` | `string` | `"unread"` / `"reading"` / `"read"` |
| `rating` | `number?` | 1-5 评分 |
| `extra` | `object` | 额外数据。可包含 `labels: string[]` 用于只读元数据标签（arXiv 分类、Zotero 标签等） |

## attachments 目录

`attachments/` 目录是 **Agent 可写**的目录，用于存放 AI Agent 生成的各种内容：

| 文件 | 用途 | 格式 |
|------|------|------|
| `summary.md` | AI 生成的论文摘要 | Markdown |
| `translation-zh.md` | 中文翻译 | Markdown |
| `translation-{lang}.md` | 其他语言翻译 | Markdown |
| `key-findings.md` | 关键发现提取 | Markdown |
| `related-works.md` | 相关工作分析 | Markdown |
| `critique.md` | 论文评述 | Markdown |
| `implementation-notes.md` | 复现笔记 | Markdown |

Agent 可以自由在此目录下创建任何文件，不会影响应用的正常运行。建议使用 Markdown 格式并遵循上述命名约定。

## notes 目录

`notes/` 目录供用户和 Agent 存放笔记：

- 用户手写的阅读笔记
- Agent 生成的研究笔记
- 文献综述草稿

## 示例工作流

### 1. Claude Code / OpenClaw 读取论文元数据

```bash
# 列出所有论文
ls ~/.zoro/library/papers/

# 读取特定论文的元数据
cat ~/.zoro/library/papers/2017-attention-is-all-you-need-a1b2c3d4/metadata.json | jq .

# 搜索包含特定关键词的论文
grep -rl "transformer" ~/.zoro/library/papers/*/metadata.json

# 读取所有论文的标题
for dir in ~/.zoro/library/papers/*/; do
  jq -r '.title' "$dir/metadata.json" 2>/dev/null
done
```

Agent 可以通过简单的文件系统操作来浏览和查询论文库，无需连接数据库。

### 2. 自动摘要生成流水线

```python
import json
import os
from pathlib import Path

PAPERS_DIR = Path.home() / ".zoro" / "library" / "papers"

def generate_summary(paper_dir: Path) -> str:
    """读取论文元数据和内容，生成摘要"""
    metadata_path = paper_dir / "metadata.json"
    with open(metadata_path) as f:
        metadata = json.load(f)

    title = metadata["title"]
    abstract_text = metadata.get("abstract", "")

    # 如果有 HTML 版本，读取全文
    html_path = paper_dir / "abs.html"
    full_text = ""
    if html_path.exists():
        full_text = html_path.read_text()

    # 调用 LLM 生成摘要（伪代码）
    summary = llm.summarize(title=title, abstract=abstract_text, full_text=full_text)

    return summary

def process_all_papers():
    """遍历所有论文，生成缺失的摘要"""
    for paper_dir in PAPERS_DIR.iterdir():
        if not paper_dir.is_dir():
            continue

        summary_path = paper_dir / "attachments" / "summary.md"
        if summary_path.exists():
            continue  # 已有摘要，跳过

        print(f"Processing: {paper_dir.name}")

        summary = generate_summary(paper_dir)

        # 确保 attachments 目录存在
        summary_path.parent.mkdir(exist_ok=True)
        summary_path.write_text(summary)
        print(f"  Summary saved to: {summary_path}")

process_all_papers()
```

### 3. 批量处理论文

```bash
#!/bin/bash
# batch-translate.sh — 批量翻译论文摘要到中文

PAPERS_DIR="$HOME/.zoro/library/papers"

for paper_dir in "$PAPERS_DIR"/*/; do
    slug=$(basename "$paper_dir")
    translation_file="$paper_dir/attachments/translation-zh.md"

    # 跳过已翻译的论文
    if [ -f "$translation_file" ]; then
        echo "Skipping $slug (already translated)"
        continue
    fi

    # 读取元数据
    title=$(jq -r '.title' "$paper_dir/metadata.json")
    abstract=$(jq -r '.abstract // empty' "$paper_dir/metadata.json")

    if [ -z "$abstract" ]; then
        echo "Skipping $slug (no abstract)"
        continue
    fi

    echo "Translating: $title"

    # 调用翻译 API（示例使用 curl）
    mkdir -p "$paper_dir/attachments"

    # 你的翻译逻辑...
    echo "# $title (中文翻译)" > "$translation_file"
    echo "" >> "$translation_file"
    echo "## 摘要" >> "$translation_file"
    # 追加翻译内容...
done
```

### 4. 与 Claude Code 集成

Claude Code 可以直接访问 `~/.zoro/` 目录：

```
用户: 帮我总结 ~/.zoro/library/papers/ 下所有关于 transformer 的论文

Claude Code:
1. 遍历 papers/ 目录，读取每个 metadata.json
2. 筛选标题或标签包含 "transformer" 的论文
3. 读取论文的 abstract 和 abs.html（如有）
4. 生成摘要并写入各论文的 attachments/summary.md
5. 生成一份汇总报告
```

## Agent 集成最佳实践

### 读取数据

1. **优先读取 `metadata.json`** — 包含结构化的论文元数据，解析简单
2. **按需读取 `abs.html`** — HTML 版本比 PDF 更容易解析
3. **使用 slug 目录名定位** — slug 格式 `{year}-{title}-{hash}` 具有语义信息
4. **批量操作使用目录遍历** — `ls` + `jq` 即可实现简单的查询

### 写入数据

1. **只写入 `attachments/` 和 `notes/` 目录** — 不要修改 `metadata.json` 或其他应用管理的文件
2. **使用 Markdown 格式** — 最通用且可读性最佳
3. **遵循命名约定** — `summary.md`、`translation-{lang}.md` 等
4. **确保目录存在** — 写入前先 `mkdir -p`
5. **使用 UTF-8 编码** — 所有文本文件使用 UTF-8

### 查询优化

1. **简单查询用文件系统** — `grep`、`find`、`jq` 等工具
2. **复杂查询用 SQLite** — 直接读取 `library.db`（只读模式）

```bash
# 查询评分 >= 4 的论文
sqlite3 ~/.zoro/library.db "SELECT slug, title, rating FROM papers WHERE rating >= 4"

# 全文搜索
sqlite3 ~/.zoro/library.db "SELECT slug, title FROM papers WHERE rowid IN (SELECT rowid FROM papers_fts WHERE papers_fts MATCH 'deep learning')"
```

### 安全注意事项

- **不要修改 `library.db`** — 数据库由应用管理，直接修改可能导致数据不一致
- **不要删除或重命名论文目录** — 会导致数据库引用失效
- **不要修改 `metadata.json`** — 应用会在特定操作后重新生成该文件
- **`attachments/` 和 `notes/` 是安全的写入区域** — 不影响应用核心数据
