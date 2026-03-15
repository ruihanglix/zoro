# 架构概览

## 项目概述

Zoro 是一款 **AI 原生的文献管理工具**，类似 Zotero，但专为 AI Agent 时代而设计。它采用 Tauri v2 构建跨平台桌面应用，配合 Chrome 浏览器扩展实现论文的一键采集。论文以人类可读、Agent 可访问的目录结构存储在本地文件系统中。

核心特性：

- 论文库管理：集合、标签、FTS5 全文搜索
- 浏览器扩展：从 ArXiv、DOI 页面等自动识别并保存论文
- 订阅源系统：插件化架构，内置 HuggingFace Daily Papers
- Agent 友好存储：`~/.zoro/library/papers/{slug}/metadata.json`
- 导入/导出：支持 BibTeX 和 RIS 格式
- 跨平台：Windows、macOS、Linux 桌面端，移动端（Tauri v2 iOS/Android）规划中

## Monorepo 结构

项目采用 **Cargo workspace**（Rust）+ **pnpm workspaces**（TypeScript）的双 Monorepo 架构，由 Turborepo 协调构建。

```
zoro/
├── Cargo.toml                    # Cargo workspace 根配置
├── package.json                  # pnpm workspace 根配置
├── apps/
│   ├── desktop/                  # Tauri v2 桌面应用
│   │   ├── src/                  #   React 前端源码
│   │   ├── src-tauri/            #   Rust 后端源码
│   │   └── package.json
│   └── browser-extension/        # Chrome Manifest V3 扩展
│       ├── src/
│       └── package.json
├── packages/
│   └── core/                     # 共享 TypeScript 类型（无运行时代码）
├── crates/
│   ├── zoro-core/            # Rust 领域模型、slug 生成、BibTeX/RIS 解析
│   ├── zoro-db/              # SQLite 数据库层（rusqlite + FTS5）
│   ├── zoro-metadata/        # 元数据富化（CrossRef、Semantic Scholar、OpenAlex）+ PDF 提取
│   └── zoro-subscriptions/   # 订阅源 trait + HuggingFace Daily Papers
└── .github/workflows/            # CI/CD 工作流
```

## Crate 依赖关系

```
┌─────────────────┐
│  zoro-core   │  领域模型、slug 生成、BibTeX/RIS 解析
│  (无外部 crate   │  依赖: serde, serde_json, chrono, uuid,
│   依赖)          │         thiserror, sha2, slug
└────────┬────────┘
         │
         ├──────────────┬──────────────────────────┐
         ▼              ▼                          ▼
┌─────────────────┐ ┌─────────────────┐  ┌──────────────────────┐
│  zoro-db     │ │zoro-metadata│  │ zoro-subscriptions│
│  SQLite 数据层   │ │元数据富化+PDF提取│  │ 订阅源插件系统        │
│  依赖: rusqlite  │ │依赖: reqwest,   │  │ 依赖: reqwest,        │
│                  │ │  lopdf, regex   │  │   async-trait, tokio   │
└────────┬────────┘ └────────┬────────┘  └──────────┬────────────┘
         │                   │                      │
         └───────────────────┼──────────────────────┘
                             ▼
         ┌──────────────────────┐
         │  zoro-desktop     │
         │  Tauri v2 桌面应用    │
         │  依赖: tauri, axum,   │
         │  tower-http, 全部     │
         │  workspace crate      │
         └──────────────────────┘
```

## Tauri v2 架构

Zoro 桌面应用基于 Tauri v2 构建，分为 Rust 后端和 React 前端两层：

```
┌─────────────────────────────────────────────────────────┐
│                    桌面应用窗口                           │
│  ┌───────────────────────────────────────────────────┐  │
│  │              WebView（React 前端）                  │  │
│  │  React 19 + TypeScript + Zustand + Tailwind/shadcn │  │
│  │                                                     │  │
│  │  commands.ts ────── invoke("命令名", {参数}) ──────┐ │  │
│  └─────────────────────────────────────────────────┐─┘  │
│                                                 IPC│     │
│  ┌─────────────────────────────────────────────────┴──┐  │
│  │              Rust 后端 (Tauri Core)                  │  │
│  │                                                      │  │
│  │  ┌──────────┐  ┌──────────┐  ┌────────────────────┐ │  │
│  │  │ Commands │  │ Storage  │  │   Subscriptions    │ │  │
│  │  │ (IPC     │  │ (文件系统│  │   (定时轮询)       │ │  │
│  │  │  处理器) │  │  管理)   │  │                    │ │  │
│  │  └────┬─────┘  └──────────┘  └────────────────────┘ │  │
│  │       │                                              │  │
│  │  ┌────▼─────────────────────────────────────────┐   │  │
│  │  │  AppState { db: Mutex<Database>, data_dir }   │   │  │
│  │  └──────────────────────────────────────────────┘   │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### IPC 通信机制

前端通过 `@tauri-apps/api/core` 的 `invoke()` 函数调用 Rust 后端命令：

- 前端（TypeScript）：`invoke<ReturnType>("snake_case_command", { snake_case_args })`
- 后端（Rust）：`#[tauri::command] pub async fn snake_case_command(...) -> Result<T, String>`
- 所有命令在 `commands.ts` 中集中定义，在 `lib.rs` 中通过 `generate_handler![]` 注册
- 命令首行统一获取数据库锁：`let db = state.db.lock().map_err(...)?;`

## Connector HTTP 服务器

Connector 是嵌入在桌面应用中的 HTTP 服务器，基于 axum 0.7 构建，用于接收浏览器扩展发送的论文数据。

```
┌──────────────────┐    HTTP (127.0.0.1:23120)    ┌──────────────────┐
│  浏览器扩展       │ ──────────────────────────▶  │  Connector 服务器 │
│  (Chrome)         │                              │  (axum)           │
│                   │ ◀──────────────────────────  │                   │
│  Content Script   │         JSON 响应            │  路由:             │
│  ↓ 检测论文       │                              │  /connector/ping  │
│  ↓ 提取元数据     │                              │  /connector/      │
│  Background SW    │                              │    saveItem       │
│  ↓ 发送到 Conn.  │                              │  /connector/      │
└──────────────────┘                              │    saveHtml       │
                                                   │  /connector/      │
                                                   │    status         │
                                                   │  /connector/      │
                                                   │    collections    │
                                                   └────────┬─────────┘
                                                            │
                                                   ┌────────▼─────────┐
                                                   │  AppState         │
                                                   │  ├─ Database      │
                                                   │  └─ 文件系统      │
                                                   └──────────────────┘
```

- 默认端口：**23120**（可在 `config.toml` 中配置）
- 仅监听 `127.0.0.1`，不对外暴露
- CORS 策略：允许所有来源（因为浏览器扩展需要跨域访问）
- 在 Tauri `setup` 阶段通过 `tokio::spawn` 启动

### Zotero Connector 兼容模式

Zoro 默认在端口 **23119**（Zotero 默认端口）上运行第二个 HTTP 服务器，实现 Zotero Connector 协议。这允许 Zotero 官方浏览器扩展直接将论文保存到 Zoro。

关键设计：
- **默认开启**：可在设置 > 浏览器连接器 > "启用 Zotero Connector 兼容" 中关闭。如果端口已被占用（例如 Zotero 正在运行），设置面板中会显示警告提示
- **无需集成翻译器**：Zotero 浏览器扩展在浏览器端运行翻译器，发送结构化 JSON 数据，Zoro 只需接收和存储
- **与原生连接器共存**：两个服务器（23120 用于 Zoro 扩展，23119 用于 Zotero 扩展）同时运行
- **会话管理**：内存中的 `SessionStore` 跟踪保存会话、附件上传进度和集合分配
- **生命周期管理**：使用 `tokio_util::CancellationToken` 实现优雅启停，无需重启应用
- **数据映射**：Zotero 标签存储为 `extra_json.labels`（只读元数据标签），不会自动创建侧边栏标签。侧边栏标签仅允许用户手动管理

实现位于 `apps/desktop/src-tauri/src/connector/zotero_compat/`：
- `mod.rs` — 服务器启动/停止函数
- `server.rs` — axum 路由定义
- `handlers.rs` — 端点实现（15+ 个 Zotero 协议端点）
- `types.rs` — Zotero 协议请求/响应类型
- `session.rs` — 会话跟踪和附件进度
- `mapping.rs` — Zotero item JSON 到 Zoro 数据模型的转换

## 数据流

### 浏览器扩展保存论文

```
浏览器页面 ──▶ Content Script ──▶ 检测器（ArXiv/DOI/通用）
                                      │
                                      ▼ 提取 PaperMetadata
                                 Background SW
                                      │
                                      ▼ POST /connector/saveItem
                                 Connector 服务器
                                      │
                                      ├──▶ 生成 slug
                                      ├──▶ 写入 SQLite 数据库
                                      └──▶ 创建论文目录
                                           ~/.zoro/library/papers/{slug}/
                                           ├── metadata.json
                                           ├── paper.pdf  (后续下载)
                                           └── abs.html (后续保存)
```

### 订阅源拉取论文

```
定时轮询器 ──▶ SubscriptionSource.fetch()
                    │
                    ▼ 获取 Vec<SubscriptionItem>
              存入 subscription_items 表
                    │
                    ▼ 用户选择添加到库
              创建 Paper + 论文目录
```

### 本地 PDF 导入（拖拽）

```
用户拖拽 PDF 文件到应用窗口
     │
     ▼
Tauri webview 拖拽事件
（FileDropZone 组件中 onDragDropEvent）
     │
     │ 过滤 .pdf 文件
     ▼
invoke("import_local_files", { filePaths })
     │
     ▼
每个 PDF 文件:
1. extract_pdf_metadata()（使用 lopdf）
   - 读取 PDF Info 字典 (/Title, /Author, /Subject)
   - 正则扫描前 3 页提取 DOI (10.xxxx/...)
   - 正则扫描提取 arXiv ID (arXiv:YYMM.NNNNN)
     │
     ▼
2. generate_paper_slug()（从标题或文件名）
     │
     ▼
3. create_paper_dir() + fs::copy(PDF)
     │
     ▼
4. insert_paper() + insert_attachment()
     │
     ▼
5. tokio::spawn 后台元数据富化
   （如找到 DOI 或 arXiv ID）
   - CrossRef / Semantic Scholar / OpenAlex
   - 补全: 摘要、作者、期刊、日期等
```

## 技术栈

| 层级 | 技术 | 说明 |
|------|------|------|
| 桌面框架 | Tauri v2 | Rust 后端 + 系统 WebView |
| 前端 | React 19 + TypeScript | WebView 中运行 |
| UI 组件 | shadcn/ui (Radix + Tailwind CSS) | 可定制的组件库 |
| 状态管理 | Zustand 5 | 轻量级状态管理 |
| 数据库 | SQLite (rusqlite, bundled) | 内嵌 FTS5 全文搜索 |
| HTTP 服务器 | axum 0.7 + tower-http 0.5 | Connector 服务 |
| HTTP 客户端 | reqwest 0.12 | 订阅源数据拉取 |
| 序列化 | serde + serde_json | JSON 序列化 |
| 异步运行时 | tokio | 异步任务执行 |
| 日志 | tracing + tracing-subscriber | 结构化日志 |
| 错误处理 | thiserror 2 | 类型化错误枚举 |
| 浏览器扩展 | Chrome Manifest V3 | React + TypeScript |
| 构建工具 | pnpm 10 + Turborepo + Cargo | 双 Monorepo 构建 |
| CI/CD | GitHub Actions | 多平台构建发布 |

## 目录树概览

```
zoro/
├── .github/
│   └── workflows/
│       ├── ci.yml                    # CI: 格式、lint、测试、类型检查
│       └── release.yml               # Release: 多平台构建发布
├── apps/
│   ├── desktop/
│   │   ├── src/                      # React 前端
│   │   │   ├── components/           #   UI 组件
│   │   │   ├── lib/
│   │   │   │   └── commands.ts       #   Tauri IPC 命令层
│   │   │   ├── stores/               #   Zustand 状态管理
│   │   │   └── App.tsx               #   应用入口
│   │   ├── src-tauri/
│   │   │   └── src/
│   │   │       ├── lib.rs            #   应用入口、状态初始化
│   │   │       ├── commands/         #   Tauri 命令处理器
│   │   │       │   ├── library.rs    #     论文/集合/标签 CRUD
│   │   │       │   ├── search.rs     #     全文搜索
│   │   │       │   ├── subscription.rs#    订阅管理
│   │   │       │   ├── import_export.rs#   BibTeX/RIS 导入导出
│   │   │       │   └── connector.rs  #     Connector 状态
│   │   │       ├── connector/        #   HTTP 服务器
│   │   │       │   ├── server.rs     #     axum 路由配置
│   │   │       │   └── handlers.rs   #     请求处理器
│   │   │       ├── storage/          #   文件系统管理
│   │   │       └── subscriptions/    #   订阅轮询器
│   │   └── package.json
│   └── browser-extension/
│       ├── src/
│       │   ├── background/           #   Service Worker
│       │   ├── content/              #   Content Scripts
│       │   │   └── detectors/        #     论文检测器
│       │   │       ├── arxiv.ts      #       ArXiv 检测
│       │   │       ├── doi.ts        #       DOI 检测
│       │   │       └── generic.ts    #       通用 meta 标签检测
│       │   ├── popup/                #   弹出窗口 UI
│       │   └── lib/
│       │       ├── api.ts            #     Connector API 客户端
│       │       └── types.ts          #     类型定义
│       └── manifest.json
├── packages/
│   └── core/                         # 共享 TypeScript 类型
├── crates/
│   ├── zoro-core/
│   │   └── src/
│   │       ├── models.rs             #   领域模型
│   │       ├── slug_utils.rs         #   Slug 生成
│   │       ├── bibtex.rs             #   BibTeX 解析
│   │       ├── ris.rs                #   RIS 解析
│   │       └── error.rs              #   CoreError
│   ├── zoro-db/
│   │   └── src/
│   │       ├── schema.rs             #   DDL 建表语句
│   │       ├── queries/              #   SQL 查询
│   │       └── error.rs              #   DbError
│   └── zoro-subscriptions/
│       └── src/
│           ├── source.rs             #   SubscriptionSource trait
│           ├── huggingface.rs        #   HuggingFace Daily Papers
│           └── error.rs              #   SubscriptionError
│   └── zoro-metadata/
│       └── src/
│           ├── lib.rs                #   EnrichmentResult, enrich_paper() 管道
│           ├── pdf_extract.rs        #   本地 PDF 元数据提取（标题、DOI、arXiv）
│           ├── crossref.rs           #   CrossRef API 客户端
│           ├── semantic_scholar.rs   #   Semantic Scholar API 客户端
│           ├── openalex.rs           #   OpenAlex API 客户端
│           ├── doi_content_negotiation.rs  # DOI 内容协商（引用格式）
│           └── error.rs              #   MetadataError
├── Cargo.toml                        # Cargo workspace 配置
├── rustfmt.toml                      # Rust 格式化配置
└── README.md
```
