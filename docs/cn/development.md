# 开发指南

## 环境要求

| 依赖 | 最低版本 | 说明 |
|------|---------|------|
| Rust | stable (最新) | 通过 [rustup](https://rustup.rs/) 安装 |
| Node.js | 20+ | 推荐使用 LTS 版本 |
| pnpm | 10+ | 通过 `corepack enable && corepack prepare pnpm@latest` 安装 |
| Git | 2.0+ | 版本控制 |

### 各平台系统依赖

#### macOS

安装 Xcode 命令行工具：

```bash
xcode-select --install
```

无需额外系统库，macOS 自带 WebView (WKWebView)。

#### Linux (Ubuntu/Debian)

```bash
sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  libappindicator3-dev \
  librsvg2-dev \
  patchelf
```

| 包 | 用途 |
|---|------|
| `libwebkit2gtk-4.1-dev` | WebView 渲染引擎（Tauri 必需） |
| `libappindicator3-dev` | 系统托盘图标支持 |
| `librsvg2-dev` | SVG 图标渲染 |
| `patchelf` | 修改 ELF 二进制文件的 RPATH（AppImage 打包需要） |

#### Windows

- **WebView2**：Windows 11 已预装；Windows 10 需要[手动安装](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)
- **Visual Studio Build Tools**：安装时勾选 "C++ 桌面开发" 工作负载
- 或安装完整的 Visual Studio Community Edition

## 克隆和设置

```bash
# 克隆仓库
git clone https://github.com/AIClaw/zoro.git
cd zoro

# 安装前端依赖
pnpm install
```

Rust 依赖会在首次编译时自动下载。`rusqlite` 使用 `bundled` feature，会自动编译 SQLite，无需系统安装。

## 开发命令

### 日常开发

```bash
# 启动桌面应用开发模式（前端热重载 + Rust 自动重编译）
pnpm tauri dev

# 前端开发服务器运行在 http://localhost:1420
```

### Rust 相关

```bash
# 运行全部 Rust 测试
cargo test --all

# 运行单个 crate 的测试
cargo test -p zoro-core

# 运行匹配名称的测试
cargo test -p zoro-core slug

# 格式化代码
cargo fmt --all

# 格式检查（CI 强制）
cargo fmt --all -- --check

# Clippy 检查（CI 强制，-D warnings 视警告为错误）
cargo clippy --all-targets --all-features -- -D warnings
```

### TypeScript 相关

```bash
# 桌面前端类型检查
pnpm --filter @zoro/desktop type-check

# 桌面前端 Biome lint
pnpm --filter @zoro/desktop lint

# 构建浏览器扩展
pnpm --filter @zoro/browser-extension build

# 浏览器扩展 lint
pnpm --filter @zoro/browser-extension lint
```

### CI 等效完整检查

在提交 PR 前，运行以下命令确保通过所有 CI 检查：

```bash
cargo fmt --all -- --check && \
cargo clippy --all-targets --all-features -- -D warnings && \
cargo test --all && \
pnpm --filter @zoro/desktop type-check
```

### 构建发布版本

```bash
# 构建桌面应用
cd apps/desktop
pnpm tauri build

# 构建浏览器扩展
pnpm --filter @zoro/browser-extension build
# 产物在 apps/browser-extension/dist/
```

## 项目布局详解

### Rust Crate 架构

| Crate | 路径 | 职责 |
|-------|------|------|
| `zoro-core` | `crates/zoro-core/` | 领域模型（`Paper`、`Author` 等）、slug 生成算法、BibTeX/RIS 解析器、`CoreError` |
| `zoro-db` | `crates/zoro-db/` | SQLite DDL schema、CRUD 查询、FTS5 全文搜索、`DbError` |
| `zoro-subscriptions` | `crates/zoro-subscriptions/` | `SubscriptionSource` trait、HuggingFace Daily Papers 实现、`SubscriptionError` |
| `zoro-desktop` | `apps/desktop/src-tauri/` | Tauri 应用入口、命令处理器、Connector HTTP 服务器、文件存储管理 |

### 前端结构

| 目录 | 说明 |
|------|------|
| `apps/desktop/src/components/` | React UI 组件 |
| `apps/desktop/src/lib/commands.ts` | Tauri IPC 命令封装 |
| `apps/desktop/src/stores/` | Zustand 状态管理 |
| `packages/core/` | 共享 TypeScript 类型（纯类型，无运行时代码） |

### 浏览器扩展结构

| 目录 | 说明 |
|------|------|
| `apps/browser-extension/src/background/` | Service Worker（后台脚本） |
| `apps/browser-extension/src/content/` | Content Scripts（内容脚本） |
| `apps/browser-extension/src/content/detectors/` | 论文检测器（ArXiv、DOI、通用） |
| `apps/browser-extension/src/popup/` | 弹出窗口 UI |
| `apps/browser-extension/src/lib/` | API 客户端和类型定义 |

## 调试技巧

### Tauri DevTools

在开发模式下（`pnpm tauri dev`），WebView 会自动启用 DevTools。在应用窗口中右键点击 → "检查"即可打开浏览器开发者工具，可用于：

- 前端组件调试
- 网络请求监控
- Console 日志查看

### Rust 日志

Zoro 使用 `tracing` + `tracing-subscriber` 进行结构化日志记录。通过 `RUST_LOG` 环境变量控制日志级别：

```bash
# 显示所有 zoro 相关的 info 级别日志（默认）
pnpm tauri dev

# 显示 debug 级别日志
RUST_LOG=zoro=debug pnpm tauri dev

# 显示特定模块的 trace 日志
RUST_LOG=zoro_db=trace pnpm tauri dev

# 显示所有依赖库的日志
RUST_LOG=debug pnpm tauri dev
```

默认日志过滤器设置在 `lib.rs` 中：

```rust
tracing_subscriber::fmt()
    .with_env_filter(
        EnvFilter::from_default_env()
            .add_directive("zoro=info".parse().unwrap())
    )
    .init();
```

### 数据库调试

SQLite 数据库文件位于 `~/.zoro/library.db`，可以使用 `sqlite3` 命令行工具直接查询：

```bash
sqlite3 ~/.zoro/library.db

# 查看所有表
.tables

# 查看论文列表
SELECT id, slug, title FROM papers LIMIT 10;

# 全文搜索
SELECT * FROM papers_fts WHERE papers_fts MATCH 'attention';
```

### Connector 调试

Connector HTTP 服务器运行在 `127.0.0.1:23120`，可用 `curl` 测试：

```bash
# 测试连接
curl http://127.0.0.1:23120/connector/ping

# 查看状态
curl http://127.0.0.1:23120/connector/status

# 查看集合列表
curl http://127.0.0.1:23120/connector/collections
```

## 常见问题及解决方案

### Q: `pnpm tauri dev` 编译失败，提示找不到系统库

**A**: 确保安装了平台对应的系统依赖（见上文"各平台系统依赖"）。Linux 上最常缺少 `libwebkit2gtk-4.1-dev`。

### Q: 首次编译非常慢

**A**: 正常现象。首次编译需要编译所有 Rust 依赖（包括 bundled SQLite），后续增量编译会快得多。可以使用 `sccache` 加速：

```bash
cargo install sccache
export RUSTC_WRAPPER=sccache
```

### Q: 浏览器扩展无法连接到桌面应用

**A**: 检查以下几点：

1. 桌面应用是否正在运行
2. Connector 服务器是否启动（查看应用日志）
3. 端口 23120 是否被占用：`lsof -i :23120`（macOS/Linux）或 `netstat -ano | findstr :23120`（Windows）
4. 浏览器扩展的 `host_permissions` 是否包含 `http://127.0.0.1:23120/*`

### Q: 数据库迁移问题

**A**: 当前使用 `CREATE TABLE IF NOT EXISTS` 模式，不支持正式的迁移系统。如遇 schema 变更导致的问题，可以删除 `~/.zoro/library.db` 重建（会丢失数据）。

### Q: Windows 上编译失败提示找不到 C++ 工具链

**A**: 确保安装了 Visual Studio Build Tools 并勾选了 "C++ 桌面开发" 工作负载。或者安装完整的 Visual Studio Community Edition。

### Q: 如何重置应用数据

**A**: 删除 `~/.zoro/` 目录即可完全重置。下次启动应用会自动重建数据库和目录结构。

```bash
rm -rf ~/.zoro
```
