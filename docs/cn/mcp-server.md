# MCP 服务器

`zoro-mcp` 是一个独立的 [Model Context Protocol](https://modelcontextprotocol.io/) 服务器，让 AI 代理（Claude Desktop、Cursor、OpenCode 等）可以完整访问你的 Zoro 文献库。

## 快速开始

```bash
# 构建 MCP 服务器
cargo build --release -p zoro-mcp

# 使用 stdio 传输运行（默认）
./target/release/zoro-mcp

# 使用 HTTP 传输运行
./target/release/zoro-mcp --transport http --port 23121

# 指定自定义数据目录
./target/release/zoro-mcp --data-dir /path/to/data
```

数据目录默认为 `~/.zoro`。也可以设置 `ZORO_DATA_DIR` 环境变量。

## 桌面应用集成

Zoro 桌面应用可以在 **设置 → MCP Server** 中直接管理 MCP 服务器：

- **自动启动**：启用后，桌面应用启动时自动启动 MCP 服务器
- **传输方式**：选择 HTTP（Streamable）或 stdio
- **端口**：配置 HTTP 端口（默认 23121）
- **控制**：手动启动 / 停止 / 重启服务器

桌面应用将 `zoro-mcp` 作为子进程启动，共享相同的数据目录。MCP 二进制文件必须与桌面应用可执行文件位于同一目录（开发时两者都构建到同一个 `target/` 目录）。

配置保存在 `~/.zoro/config.toml` 的 `[mcp]` 部分：

```toml
[mcp]
enabled = true
transport = "http"
port = 23121
```

## 客户端配置

### Claude Desktop

添加到 `~/Library/Application Support/Claude/claude_desktop_config.json`（macOS）：

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

添加到 `.opencode/config.json`：

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

### HTTP 传输

对于支持 HTTP MCP（Streamable HTTP）的客户端：

```bash
./target/release/zoro-mcp --transport http --port 23121
```

服务器监听地址为 `http://127.0.0.1:23121/mcp`。

## 工具

MCP 服务器提供约 35 个工具，覆盖 Zoro 的完整功能。

### 论文

| 工具 | 描述 |
|---|---|
| `add_paper` | 添加新论文到文献库 |
| `get_paper` | 通过 ID 获取论文详细信息 |
| `list_papers` | 列出论文（支持按集合、标签、状态过滤，排序和分页） |
| `search_papers` | 全文搜索论文标题和摘要 |
| `update_paper` | 更新论文元数据 |
| `update_paper_status` | 设置论文阅读状态（未读/阅读中/已读） |
| `update_paper_rating` | 设置论文评分（1-5）或清除 |
| `delete_paper` | 从文献库删除论文 |
| `enrich_paper_metadata` | 从外部 API（CrossRef、Semantic Scholar、OpenAlex）丰富论文元数据 |

### 集合

| 工具 | 描述 |
|---|---|
| `create_collection` | 创建新集合用于组织论文 |
| `list_collections` | 列出所有集合及论文数量 |
| `update_collection` | 更新集合名称、父集合或描述 |
| `delete_collection` | 删除集合 |
| `add_paper_to_collection` | 将论文添加到集合 |
| `remove_paper_from_collection` | 从集合中移除论文 |
| `get_collections_for_paper` | 获取包含某论文的所有集合 |

### 标签

| 工具 | 描述 |
|---|---|
| `list_tags` | 列出所有标签 |
| `search_tags` | 按名称前缀搜索标签 |
| `add_tag_to_paper` | 为论文添加标签 |
| `remove_tag_from_paper` | 从论文移除标签 |
| `update_tag` | 更新标签名称或颜色 |
| `delete_tag` | 删除标签 |

### 笔记

| 工具 | 描述 |
|---|---|
| `add_note` | 为论文添加笔记 |
| `list_notes` | 列出论文的所有笔记 |
| `update_note` | 更新笔记内容 |
| `delete_note` | 删除笔记 |

### 标注

| 工具 | 描述 |
|---|---|
| `add_annotation` | 添加 PDF 标注 |
| `list_annotations` | 列出论文的所有标注 |
| `update_annotation` | 更新标注 |
| `delete_annotation` | 删除标注 |

### 导入/导出

| 工具 | 描述 |
|---|---|
| `import_bibtex` | 从 BibTeX 内容导入论文 |
| `export_bibtex` | 导出论文为 BibTeX（全部或按 ID） |
| `import_ris` | 从 RIS 内容导入论文 |
| `export_ris` | 导出论文为 RIS（全部或按 ID） |

### 引用

| 工具 | 描述 |
|---|---|
| `get_formatted_citation` | 获取格式化引用（apa、ieee、mla、chicago、bibtex、ris） |
| `get_paper_bibtex` | 获取论文的 BibTeX 条目 |

### 订阅

| 工具 | 描述 |
|---|---|
| `list_subscriptions` | 列出所有订阅源 |
| `list_feed_items` | 列出订阅的文章（支持分页） |
| `add_feed_item_to_library` | 将订阅文章添加到文献库 |
| `refresh_subscription` | 刷新订阅源获取新文章 |

## 资源

服务器提供两个 MCP 资源：

| URI | 描述 |
|---|---|
| `zoro://library-index` | 文献库所有论文的 JSON 索引（轻量摘要） |
| `zoro://paper/{paper_id}` | 特定论文的完整元数据 |

## 架构

MCP 服务器是一个独立二进制文件，直接打开 SQLite 数据库（不依赖 Tauri）。它复用与桌面应用相同的 Rust crate：

- `zoro-core` -- 领域模型、slug 生成、BibTeX/RIS 解析器
- `zoro-db` -- SQLite 查询
- `zoro-storage` -- 文件/同步逻辑（论文目录、元数据同步）
- `zoro-metadata` -- 外部 API 丰富（CrossRef、Semantic Scholar、OpenAlex）
- `zoro-subscriptions` -- 订阅源（HuggingFace Daily Papers）

服务器使用 `rmcp`（官方 Rust MCP SDK），通过 `#[tool_router]` 和 `#[tool_handler]` 宏注册工具。

### 并发

- SQLite 数据库由 `std::sync::Mutex`（非 tokio::Mutex）保护，因为 SQLite 操作很快
- 网络调用（丰富元数据、刷新订阅）在发起 HTTP 请求前释放数据库锁
- HTTP 传输为每个会话创建新的服务器处理器（有状态模式）

## 另请参阅

- [代理集成指南](agent-integration.md) -- 基于文件系统的访问模式
- [数据模型与 API 参考](data-model.md) -- 完整数据库架构
- [架构概览](architecture.md) -- 系统设计
