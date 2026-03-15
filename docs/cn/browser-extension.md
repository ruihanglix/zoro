# 浏览器扩展

## Manifest V3 架构概述

Zoro 浏览器扩展基于 Chrome **Manifest V3** 构建，用于从浏览器中识别并保存学术论文到 Zoro 桌面应用。

扩展通过本地 HTTP 连接（Connector）与桌面应用通信，不依赖任何云服务。

### Manifest 配置

```json
{
  "manifest_version": 3,
  "name": "Zoro",
  "version": "0.1.0",
  "description": "Save papers to Zoro - AI-native literature manager",
  "permissions": ["activeTab", "storage"],
  "host_permissions": [
    "http://127.0.0.1:23120/*",
    "http://127.0.0.1:23119/*"
  ],
  "action": {
    "default_popup": "src/popup/index.html"
  },
  "background": {
    "service_worker": "background.js",
    "type": "module"
  },
  "content_scripts": [
    {
      "matches": [
        "*://arxiv.org/*",
        "*://www.arxiv.org/*",
        "*://scholar.google.com/*",
        "*://doi.org/*"
      ],
      "js": ["content.js"],
      "run_at": "document_idle"
    }
  ]
}
```

关键配置：

- **permissions**: `activeTab`（访问当前标签页）、`storage`（存储扩展设置）
- **host_permissions**: 允许访问本地 Connector 服务器（端口 23120 和 23119）
- **content_scripts.matches**: 定义自动注入 Content Script 的站点列表
- **content_scripts.run_at**: `document_idle` — 等待页面加载完成后运行

## 组件分解

```
┌─────────────────────────────────────────────────┐
│                  浏览器扩展                       │
│                                                   │
│  ┌─────────────────────────────────────────────┐ │
│  │          Popup UI（弹出窗口）                 │ │
│  │  显示检测状态、保存按钮、集合选择              │ │
│  └──────────────────┬──────────────────────────┘ │
│                     │ chrome.runtime.sendMessage  │
│  ┌──────────────────▼──────────────────────────┐ │
│  │       Service Worker（后台脚本）              │ │
│  │  background/index.ts                         │ │
│  │  - 消息路由                                   │ │
│  │  - 与 Connector 通信（background/connector.ts）│ │
│  └──────────────────┬──────────────────────────┘ │
│                     │ chrome.tabs.sendMessage     │
│  ┌──────────────────▼──────────────────────────┐ │
│  │       Content Scripts（内容脚本）             │ │
│  │  content/index.ts                            │ │
│  │  - 检测当前页面是否包含论文                    │ │
│  │  - 调用检测器提取元数据                       │ │
│  │  - 检测器:                                    │ │
│  │    ├── detectors/arxiv.ts                     │ │
│  │    ├── detectors/doi.ts                       │ │
│  │    └── detectors/generic.ts                   │ │
│  └─────────────────────────────────────────────┘ │
│                                                   │
│  ┌─────────────────────────────────────────────┐ │
│  │          lib/（共享模块）                      │ │
│  │  ├── api.ts       # Connector API 客户端     │ │
│  │  └── types.ts     # 类型定义                  │ │
│  └─────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────┘
```

### Service Worker（后台脚本）

`background/index.ts` — 扩展的后台入口点：

- 处理来自 Popup 和 Content Script 的消息
- 通过 `background/connector.ts` 与桌面应用的 Connector 通信
- 管理扩展状态

### Content Scripts（内容脚本）

`content/index.ts` — 注入到匹配站点的脚本：

- 在页面加载完成后自动运行
- 按优先级依次调用检测器
- 提取论文元数据并发送到 Service Worker

### Popup UI（弹出窗口）

`popup/` — 点击扩展图标时显示的界面：

- 显示当前页面的论文检测状态
- 提供保存按钮
- 集合选择器

## 论文检测器系统

扩展内置三个论文检测器，按优先级依次执行：

### 1. ArXiv 检测器 (`detectors/arxiv.ts`)

匹配 ArXiv 页面并提取论文信息：

```typescript
import type { DetectionResult } from "../../lib/types";

export function detectArxiv(url: string, doc: Document): DetectionResult {
  // 匹配 arxiv.org/abs/XXXX.XXXXX 或 arxiv.org/pdf/XXXX.XXXXX
  const absMatch = url.match(/arxiv\.org\/abs\/(\d+\.\d+)(v\d+)?/);
  const pdfMatch = url.match(/arxiv\.org\/pdf\/(\d+\.\d+)(v\d+)?/);
  const htmlMatch = url.match(/arxiv\.org\/html\/(\d+\.\d+)(v\d+)?/);

  const arxivId = absMatch?.[1] || pdfMatch?.[1] || htmlMatch?.[1];
  if (!arxivId) {
    return { detected: false, source: "arxiv" };
  }

  // 从页面提取标题
  const title =
    doc.querySelector("h1.title")?.textContent?.replace(/^Title:\s*/, "").trim() ||
    doc.querySelector('meta[name="citation_title"]')?.getAttribute("content") ||
    doc.title.replace(" - arXiv", "").trim() || "";

  // 提取作者
  const authorElements = doc.querySelectorAll(".authors a, meta[name='citation_author']");
  const authors: string[] = [];
  authorElements.forEach((el) => {
    const name = el.getAttribute("content") || el.textContent?.trim();
    if (name && !authors.includes(name)) {
      authors.push(name);
    }
  });

  // 提取摘要
  const abstractEl = doc.querySelector(".abstract");
  const abstract_text = abstractEl
    ? abstractEl.textContent?.replace(/^Abstract:\s*/, "").trim()
    : doc.querySelector('meta[name="citation_abstract"]')?.getAttribute("content") || undefined;

  return {
    detected: true,
    source: "arxiv",
    metadata: {
      title,
      authors,
      url: `https://arxiv.org/abs/${arxivId}`,
      arxiv_id: arxivId,
      doi,
      pdf_url: `https://arxiv.org/pdf/${arxivId}`,
      html_url: `https://arxiv.org/html/${arxivId}`,
      abstract_text,
    },
  };
}
```

提取字段：`title`、`authors`、`arxiv_id`、`doi`、`abstract_text`、`url`、`pdf_url`、`html_url`

### 2. DOI 检测器 (`detectors/doi.ts`)

匹配 DOI 页面和包含 DOI meta 标签的页面：

```typescript
export function detectDoi(url: string, doc: Document): DetectionResult {
  // 从 URL 匹配 DOI
  const doiUrlMatch = url.match(/doi\.org\/(10\.\d{4,}\/[^\s]+)/);

  // 从 meta 标签匹配 DOI
  const doiMeta =
    doc.querySelector('meta[name="citation_doi"]')?.getAttribute("content") ||
    doc.querySelector('meta[name="DC.identifier"]')?.getAttribute("content") ||
    doc.querySelector('meta[name="dc.identifier"]')?.getAttribute("content");

  const doi = doiUrlMatch?.[1] || (doiMeta?.startsWith("10.") ? doiMeta : undefined);
  // ...
}
```

支持的 meta 标签：`citation_doi`、`DC.identifier`、`dc.identifier`、`citation_title`、`DC.title`、`citation_author`、`DC.creator`、`citation_abstract`、`DC.description`、`citation_pdf_url`

### 3. 通用检测器 (`detectors/generic.ts`)

基于 Google Scholar / Highwire Press 格式的 citation meta 标签检测：

```typescript
export function detectGeneric(url: string, doc: Document): DetectionResult {
  const title = doc.querySelector('meta[name="citation_title"]')?.getAttribute("content");
  if (!title) {
    return { detected: false, source: "generic" };
  }
  // 提取 citation_author、citation_doi、citation_pdf_url、citation_abstract
  // ...
}
```

当 ArXiv 和 DOI 检测器都未匹配时，通用检测器作为兜底方案。

## 如何添加新的站点检测器

### 步骤 1：创建检测器文件

在 `apps/browser-extension/src/content/detectors/` 下创建新文件，如 `pubmed.ts`：

```typescript
import type { DetectionResult } from "../../lib/types";

export function detectPubmed(url: string, doc: Document): DetectionResult {
  // 匹配 PubMed URL
  const pmidMatch = url.match(/pubmed\.ncbi\.nlm\.nih\.gov\/(\d+)/);
  if (!pmidMatch) {
    return { detected: false, source: "pubmed" };
  }

  const pmid = pmidMatch[1];

  // 从页面提取元数据
  const title =
    doc.querySelector(".heading-title")?.textContent?.trim() ||
    doc.querySelector('meta[name="citation_title"]')?.getAttribute("content") ||
    "";

  const authorElements = doc.querySelectorAll(".authors-list .full-name");
  const authors: string[] = [];
  authorElements.forEach((el) => {
    const name = el.textContent?.trim();
    if (name) authors.push(name);
  });

  const abstract_text = doc.querySelector(".abstract-content")?.textContent?.trim();

  const doi =
    doc.querySelector('meta[name="citation_doi"]')?.getAttribute("content") || undefined;

  return {
    detected: true,
    source: "pubmed",
    metadata: {
      title,
      authors,
      url: `https://pubmed.ncbi.nlm.nih.gov/${pmid}/`,
      doi,
      abstract_text,
    },
  };
}
```

### 步骤 2：在 Content Script 中注册

编辑 `apps/browser-extension/src/content/index.ts`，导入并添加新检测器到检测链中：

```typescript
import { detectArxiv } from "./detectors/arxiv";
import { detectDoi } from "./detectors/doi";
import { detectPubmed } from "./detectors/pubmed"; // 新增
import { detectGeneric } from "./detectors/generic";

// 按优先级依次执行
const detectors = [detectArxiv, detectDoi, detectPubmed, detectGeneric];
```

### 步骤 3：更新 Manifest

在 `manifest.json` 的 `content_scripts.matches` 中添加新站点：

```json
{
  "content_scripts": [
    {
      "matches": [
        "*://arxiv.org/*",
        "*://www.arxiv.org/*",
        "*://scholar.google.com/*",
        "*://doi.org/*",
        "*://pubmed.ncbi.nlm.nih.gov/*"
      ],
      "js": ["content.js"],
      "run_at": "document_idle"
    }
  ]
}
```

### 步骤 4：测试

1. 构建扩展：`pnpm --filter @zoro/browser-extension build`
2. 在 Chrome 中加载未打包的扩展
3. 访问目标站点验证检测和保存功能

### 检测器类型定义

```typescript
// lib/types.ts

export interface PaperMetadata {
  title: string;
  authors: string[];
  url: string;
  doi?: string;
  arxiv_id?: string;
  pdf_url?: string;
  html_url?: string;
  abstract_text?: string;
  tags?: string[];
}

export interface SaveItemResponse {
  success: boolean;
  paper_id?: string;
  message: string;
}

export interface DetectionResult {
  detected: boolean;
  metadata?: PaperMetadata;
  source: string;
}
```

## Connector API 协议

浏览器扩展通过以下 HTTP API 与桌面应用通信。所有端点的基础 URL 为 `http://127.0.0.1:23120`。

### GET /connector/ping

测试桌面应用是否在线。

**请求**：无参数

**响应**：

```json
{
  "version": "0.1.0",
  "name": "Zoro"
}
```

**用途**：扩展启动时检测桌面应用是否运行。

### POST /connector/saveItem

保存论文到库。

**请求**：

```json
{
  "title": "Attention Is All You Need",
  "authors": ["Ashish Vaswani", "Noam Shazeer"],
  "url": "https://arxiv.org/abs/1706.03762",
  "doi": "10.48550/arXiv.1706.03762",
  "arxiv_id": "1706.03762",
  "pdf_url": "https://arxiv.org/pdf/1706.03762",
  "html_url": "https://arxiv.org/html/1706.03762",
  "abstract_text": "The dominant sequence transduction models...",
  "tags": ["transformer", "attention"]
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `title` | `string` | 是 | 论文标题 |
| `authors` | `string[]` | 否 | 作者姓名列表 |
| `url` | `string` | 否 | 论文页面 URL |
| `doi` | `string` | 否 | DOI 标识符 |
| `arxiv_id` | `string` | 否 | ArXiv ID |
| `pdf_url` | `string` | 否 | PDF 下载链接 |
| `html_url` | `string` | 否 | HTML 版本链接 |
| `abstract_text` | `string` | 否 | 论文摘要 |
| `tags` | `string[]` | 否 | 标签列表 |

**响应**（成功）：

```json
{
  "success": true,
  "paper_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "message": "Paper saved successfully"
}
```

**响应**（失败）：

```json
{
  "success": false,
  "paper_id": null,
  "message": "Failed to save paper: UNIQUE constraint failed: papers.slug"
}
```

### POST /connector/saveHtml

保存论文的 HTML 内容。

**请求**：

```json
{
  "paper_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "html_content": "<!DOCTYPE html><html>..."
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `paper_id` | `string` | 是 | 论文 ID（来自 saveItem 的响应） |
| `html_content` | `string` | 是 | HTML 完整内容 |

**响应**：

```json
{
  "success": true,
  "paper_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "message": "HTML saved successfully"
}
```

HTML 文件保存在 `~/.zoro/library/papers/{slug}/abs.html`。

### GET /connector/status

查询 Connector 当前状态。

**响应**：

```json
{
  "status": "ready",
  "current_save": null
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `status` | `string` | `"ready"` 表示就绪 |
| `current_save` | `string \| null` | 当前正在保存的论文标题（`null` 表示空闲） |

### GET /connector/collections

获取所有集合列表。

**响应**：

```json
[
  { "id": "uuid-1", "name": "Machine Learning" },
  { "id": "uuid-2", "name": "NLP" }
]
```

## 开发构建

### 构建扩展

```bash
pnpm --filter @zoro/browser-extension build
```

构建产物位于 `apps/browser-extension/dist/` 目录。

### 在 Chrome 中加载未打包扩展

1. 打开 Chrome，访问 `chrome://extensions/`
2. 开启右上角的 **"开发者模式"**
3. 点击 **"加载已解压的扩展程序"**
4. 选择 `apps/browser-extension/dist/` 目录
5. 扩展图标会出现在工具栏

### 开发调试

- **Service Worker 日志**：`chrome://extensions/` → 点击扩展的 "Service Worker" 链接
- **Content Script 日志**：目标页面的 DevTools Console
- **Popup 日志**：右键点击扩展图标 → "审查弹出内容"

修改代码后需要：

1. 重新构建：`pnpm --filter @zoro/browser-extension build`
2. 在 `chrome://extensions/` 页面点击扩展的刷新按钮
3. 刷新目标页面

## 使用 Zotero 官方浏览器扩展

除了 Zoro 自带的浏览器扩展，你还可以使用 **Zotero 官方浏览器扩展** 将论文保存到 Zoro。这可以利用 Zotero 丰富的翻译器库，支持数千个学术出版商网站。

### 设置步骤

1. 安装 [Zotero Connector](https://www.zotero.org/download/connectors) 浏览器扩展
2. 在 Zoro 中，进入 **设置 > 浏览器连接器**
3. 启用 **"启用 Zotero Connector 兼容"**
4. 确保 Zotero 桌面端**未运行**（两个应用不能同时监听端口 23119）

### 工作原理

启用 Zotero Connector 兼容后，Zoro 在端口 **23119**（Zotero 默认端口）上启动第二个 HTTP 服务器，实现 Zotero Connector 协议：

1. Zotero 浏览器扩展使用内置翻译器检测论文（在浏览器中运行）
2. 点击 "Save to Zotero" 时，扩展将解析好的元数据发送到 `localhost:23119`
3. Zoro 接收结构化数据并保存到文献库
4. PDF 附件由扩展直接上传，存储在论文目录中

### 支持的功能

| 功能 | 状态 |
|---|---|
| 保存条目（论文、书籍等） | 支持 |
| 保存网页快照 | 支持 |
| PDF/EPUB 附件上传 | 支持 |
| SingleFile HTML 快照 | 支持 |
| 集合选择器 | 支持 |
| 标签分配 | 存储为元数据标签（不创建侧边栏标签） |
| RIS/BibTeX 导入 | 支持 |
| 会话进度跟踪 | 支持 |
| 翻译器同步 | 不需要（翻译器在浏览器中运行） |
| PDF 元数据识别 | 不支持 |
| 开放获取解析器 | 不支持 |
| Google Docs 集成 | 不支持 |

### 限制

- **不能与 Zotero 同时运行**：两个应用都监听端口 23119，启用兼容模式前请关闭 Zotero
- **无 PDF 识别**：直接保存的独立 PDF 不会自动识别元数据
- **Zotero 特有字段**：期刊卷号、期号、页码等字段保存在 `extra_json` 中，但不在 Zoro UI 中显示（数据不会丢失）
- **标签变为标签元数据**：来自 Zotero Connector 的标签存储为 `extra_json.labels` 中的只读标签，不会自动创建为侧边栏标签。Zoro 的侧边栏标签仅允许用户手动管理

## 发布到 Chrome 网上应用店

1. 确保 `manifest.json` 中的版本号已更新
2. 构建扩展：`pnpm --filter @zoro/browser-extension build`
3. 将 `dist/` 目录打包为 `.zip` 文件：
   ```bash
   cd apps/browser-extension/dist
   zip -r ../zoro-extension.zip .
   ```
4. 登录 [Chrome Web Store Developer Dashboard](https://chrome.google.com/webstore/devconsole)
5. 上传 `.zip` 文件
6. 填写商店信息（描述、截图、分类等）
7. 提交审核
