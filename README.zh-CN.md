<p align="center">
  <img src="docs/img/logo.png" width="128" alt="Zoro Logo" />
  <h1 align="center">Zoro</h1>
  <p align="center"><strong>用母语读论文，让 AI 帮你管论文。</strong></p>
  <p align="center">
    <a href="./README.md">English</a> · <a href="https://github.com/ruihanglix/zoro/releases">下载</a> · <a href="./docs/cn/development.md">开发指南</a>
  </p>
</p>

Zoro 是一款 AI 原生的文献管理工具，为非英语母语研究者和 AI Agent 时代而生。从第一行代码开始，**母语阅读体验**和 **AI Agent 协作**就是核心设计原则，而非事后追加的功能。

> 跨平台桌面应用（macOS / Windows / Linux），本地优先，数据完全属于你。

<p align="center">
  <img src="docs/img/home.png" width="800" alt="Zoro 主界面" />
</p>

---

## 下载安装

从 [Releases](https://github.com/ruihanglix/zoro/releases) 页面下载对应平台的安装包：

| 平台 | 文件 |
|---|---|
| macOS (Apple Silicon) | `Zoro_x.x.x_aarch64.dmg` |
| macOS (Intel) | `Zoro_x.x.x_x64.dmg` |
| Windows | `Zoro_x.x.x_x64-setup.exe` |
| Linux (Debian/Ubuntu) | `Zoro_x.x.x_amd64.deb` |
| Linux (AppImage) | `Zoro_x.x.x_amd64.AppImage` |

> 想从源码构建？请参阅[开发指南](./docs/cn/development.md)。

---

## 亮点功能

### 🌏 母语原生 — 让英文论文不再是障碍

大多数文献管理工具把翻译当成附加功能。Zoro 将母语阅读体验深度融入每一个界面。

**三种显示模式，一键切换** — 原文 / 双语 / 译文，论文列表、摘要、详情页全局生效。标题、摘要自动翻译，译文突出显示、原文辅助对照，打造沉浸式双语阅读体验。

<p align="center">
  <img src="docs/img/home.png" width="800" alt="三种显示模式" />
</p>

**双语 PDF 并排阅读** — 左侧原文 PDF，右侧译文 PDF，同步滚动。两侧均支持高亮、标注、手写笔记。

<p align="center">
  <img src="docs/img/pdf_reader.png" width="800" alt="双语 PDF 阅读器" />
</p>

**HTML 全文翻译** — ArXiv HTML 论文逐段落后台翻译，进度实时可见，边翻译边阅读。

<p align="center">
  <img src="docs/img/html_reader.png" width="800" alt="HTML 全文翻译" />
</p>

**订阅源也能双语浏览** — HuggingFace 每日论文等订阅内容同样支持双语显示，快速筛选感兴趣的论文。

<p align="center">
  <img src="docs/img/dailypaper.png" width="800" alt="双语每日论文" />
</p>

所有翻译结果本地缓存，再次打开即时显示，无需重复等待。

---

### 🤖 Agent 原生 — 你的论文库，AI 也能用

Zoro 从存储层到协议层，都为 AI Agent 而设计。

**内置 AI 助手** — 应用内嵌 Agent 面板，支持多轮对话。直接对当前论文提问、总结、翻译、分析。支持图片输入、工具调用、思维链展示。

**MCP Server** — 内置 [Model Context Protocol](https://modelcontextprotocol.io/) 服务器，提供约 35 个工具。Claude Desktop、Cursor、OpenCode 等 AI 工具可直接检索、浏览、管理你的论文。设置中一键开启。详见 [MCP Server 文档](./docs/cn/mcp-server.md)。

**文件系统即 API** — 每篇论文存储为独立目录，包含结构化 `metadata.json`。AI Agent 无需任何特殊 SDK，直接读取文件系统即可理解你的论文库。`attachments/` 目录开放写入，Agent 可自动生成摘要、翻译、分析报告。

```
~/.zoro/library/papers/
  2017-attention-is-all-you-need-a1b2c3d4/
    metadata.json          ← 结构化元数据，Agent 可直接读取
    paper.pdf              ← PDF 原文
    abs.html               ← HTML 全文
    attachments/           ← Agent 可写入
      summary.md           ← AI 生成的摘要
      translation-zh.md    ← AI 生成的翻译
    notes/                 ← 用户笔记
```

---

## 更多功能

- **Zotero 导入** — 导入已有的 Zotero 文献库，包括论文、分类、标签和元数据，无缝迁移、数据无损
- **WebDAV 同步** — 通过任意 WebDAV 服务在多设备间同步论文库，无冲突、可加密、服务器由你掌控
- **全功能 PDF 阅读器** — 高亮、下划线、便签、手写墨迹标注、大纲导航、引文悬停预览
- **浏览器扩展** — Chrome 扩展，一键从 ArXiv、DOI 页面、通用学术网站保存论文。[兼容 Zotero Connector](./docs/cn/browser-extension.md)
- **导入 / 导出** — BibTeX / RIS 格式双向支持，拖拽 PDF 导入，格式化引用输出（APA、IEEE、MLA、Chicago）
- **本地优先 & 隐私保护** — SQLite 数据库，数据完全本地存储，离线可用。论文数据不上传云端
- **跨平台** — 原生支持 macOS、Windows、Linux

---

## 许可证

[AGPL-3.0](./LICENSE)
