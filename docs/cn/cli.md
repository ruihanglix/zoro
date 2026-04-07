# Zoro CLI 命令行工具

`zoro` CLI 让你直接在终端中管理 Zoro 文献库。它同时面向**人类用户**（彩色表格输出）和 **AI Agent**（通过 `--json` 输出 JSON）。与 MCP 服务器不同，CLI 不要求 Agent 支持 MCP 协议——任何能执行 shell 命令的 Agent 都可以使用。

## 快速开始

```bash
# 构建 CLI
cargo build --release -p zoro-cli

# 查看文献库状态
./target/release/zoro status

# 搜索论文
./target/release/zoro search "attention"

# 列出所有论文（JSON 输出，适合 Agent）
./target/release/zoro --json list

# 导出论文为 BibTeX
./target/release/zoro export <paper-slug> --format bibtex
```

## 安装

### 从源码构建

```bash
cargo build --release -p zoro-cli
```

二进制文件位于 `target/release/zoro`。

### 安装到系统 PATH

构建完成后（或从桌面应用 sidecar 二进制）：

```bash
# 自助安装：在 /usr/local/bin/zoro 创建符号链接
./target/release/zoro install-cli

# 验证
which zoro
zoro status
```

在 macOS/Linux 上会创建符号链接到 `/usr/local/bin/zoro`（可能需要 `sudo`）。在 Windows 上会复制到 `%LOCALAPPDATA%\Zoro\bin\`，并提示添加到 PATH。

### 桌面应用内置

CLI 二进制作为 sidecar 内置在 Zoro 桌面应用中。可以在桌面应用的 **设置 → Install CLI** 中安装，或直接运行 sidecar 二进制的 `install-cli` 子命令。

## 混合模式 Backend

CLI 会自动检测使用哪种后端：

1. **HTTP 连接器**（默认）：如果 Zoro 桌面应用正在运行，CLI 通过 `localhost:23120` 连接其 HTTP 连接器
2. **本地 SQLite**（回退）：如果应用未运行，CLI 直接打开 `~/.zoro/library.db`

使用 `--local` 标志强制本地模式：

```bash
# 始终使用直连 SQLite
zoro --local search "transformer"
```

查看当前模式：

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

## 全局参数

| 参数 | 说明 |
|------|------|
| `--json` | 输出 JSON 格式（供 Agent/脚本使用） |
| `--data-dir <path>` | 自定义数据目录（默认 `~/.zoro`，也可通过 `ZORO_DATA_DIR` 环境变量设置） |
| `--local` | 强制使用本地 SQLite 后端（跳过 HTTP 连接器检测） |

## 命令

### 论文

```bash
# 全文搜索（FTS5 + 作者名匹配）
zoro search "attention mechanism"
zoro search --limit 5 "transformer"

# 列出论文（支持过滤）
zoro list
zoro list --collection "Deep Learning"
zoro list --tag "NLP"
zoro list --status unread
zoro list --limit 100

# 获取论文详情（支持 slug 或 ID）
zoro get attention-is-all-you-need-a1b2c3d4

# 添加论文（支持 DOI、arXiv ID、URL 或本地 PDF）
zoro add "10.1038/s41586-025-09422-z"
zoro add "2301.12345"
zoro add "https://arxiv.org/abs/2301.12345"
zoro add ./paper.pdf

# 删除论文
zoro delete attention-is-all-you-need-a1b2c3d4
```

### 集合

```bash
# 列出所有集合
zoro collections list

# 创建集合
zoro collections create "Machine Learning"
zoro collections create "NLP" --description "自然语言处理相关论文"

# 添加/移除论文到集合
zoro collections add <paper-slug> "Machine Learning"
zoro collections remove <paper-slug> "Machine Learning"
```

### 标签

```bash
# 列出所有标签
zoro tags list

# 添加/移除标签
zoro tags add <paper-slug> "important"
zoro tags remove <paper-slug> "important"
```

### 笔记

```bash
# 列出论文的笔记
zoro notes list <paper-slug>

# 添加笔记
zoro notes add <paper-slug> "关键发现：self-attention 的计算复杂度为 O(n²)"

# 删除笔记
zoro notes delete <note-id>
```

### 导出

```bash
# 导出为 BibTeX（默认）
zoro export <paper-slug>
zoro export <paper-slug> --format bibtex

# 导出为 RIS
zoro export <paper-slug> --format ris

# 导出为 JSON
zoro export <paper-slug> --format json
```

### 状态

```bash
# 查看连接模式和文献库统计
zoro status
zoro --json status
```

## Agent 使用 JSON 输出

所有命令都支持 `--json` 输出机器可读的 JSON：

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

任何 AI Agent（Claude Code、Cursor 等）都可以通过执行 shell 命令并解析 JSON 输出来查询和管理文献库。

## Agent 集成示例

AI 编码代理无需任何特殊协议即可使用 CLI：

```bash
# Agent 搜索相关论文
papers=$(zoro --json search "reinforcement learning")

# Agent 获取第一个结果的详情
slug=$(echo "$papers" | jq -r '.[0].slug')
zoro --json get "$slug"

# Agent 添加标签
zoro tags add "$slug" "relevant-to-project"

# Agent 添加研究笔记
zoro notes add "$slug" "这篇论文的 reward shaping 技术可以应用到我们的优化器中"

# Agent 导出引用
zoro export "$slug" --format bibtex >> references.bib
```

## 架构

CLI 实现为 `zoro-cli` crate（`crates/zoro-cli/`）：

```
crates/zoro-cli/
├── Cargo.toml
└── src/
    ├── main.rs              # 入口，clap 命令定义
    ├── config.rs            # 数据目录解析
    ├── output.rs            # 输出格式化（表格/JSON）
    ├── backend/
    │   ├── mod.rs           # Backend trait + 自动检测
    │   ├── local.rs         # LocalBackend（直连 SQLite）
    │   └── http.rs          # HttpBackend（连接器 API）
    └── commands/
        ├── mod.rs
        ├── papers.rs        # search, list, get, add, delete
        ├── collections.rs   # list, create, add, remove
        ├── tags.rs          # list, add, remove
        ├── notes.rs         # list, add, delete
        ├── export.rs        # bibtex, ris, json
        ├── status.rs        # 连接状态 + 统计
        └── install_cli.rs   # 安装到系统 PATH
```

复用与桌面应用和 MCP 服务器相同的 Rust crate：
- `zoro-core` — 领域模型、BibTeX/RIS 生成
- `zoro-db` — SQLite 查询、FTS5 搜索
- `zoro-storage` — 论文目录管理
- `zoro-metadata` — 元数据富化 API

## 另请参阅

- [Agent 集成指南](agent-integration.md) — 基于文件系统的访问模式
- [MCP 服务器](mcp-server.md) — 支持 MCP 协议的 Agent 集成
- [架构概览](architecture.md) — 系统设计
