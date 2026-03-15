# 部署与发布指南

## CI 工作流概述

CI 工作流定义在 `.github/workflows/ci.yml` 中，在 `main` 分支的 push 和 PR 上自动触发。

### 触发条件

```yaml
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
```

### CI 检查项

CI 包含两个 job：

#### Job 1: `check`（核心检查）

运行在 `ubuntu-22.04` 上，执行以下步骤：

| 步骤 | 命令 | 说明 |
|------|------|------|
| 安装系统依赖 | `apt-get install libwebkit2gtk-4.1-dev ...` | Linux 编译所需 |
| 安装 npm 依赖 | `pnpm install` | 前端依赖 |
| Rust 格式检查 | `cargo fmt --all -- --check` | 代码风格一致性 |
| Rust Clippy | `cargo clippy --all-targets --all-features -- -D warnings` | 静态分析 |
| Rust 测试 | `cargo test --all` | 运行所有 Rust 测试 |
| TypeScript 类型检查 | `pnpm --filter @zoro/desktop type-check` | 前端类型安全 |

#### Job 2: `build-extension`（扩展构建）

运行在 `ubuntu-22.04` 上：

```bash
pnpm install
pnpm --filter @zoro/browser-extension build
```

确保浏览器扩展可以成功构建。

### CI 所需工具链

```yaml
- uses: pnpm/action-setup@v4
  with:
    version: 10

- uses: actions/setup-node@v4
  with:
    node-version: 20
    cache: pnpm

- uses: dtolnay/rust-toolchain@stable
  with:
    components: clippy, rustfmt
```

## Release 工作流

Release 工作流定义在 `.github/workflows/release.yml` 中，通过 git tag 触发多平台构建。

### 触发条件

```yaml
on:
  push:
    tags: ["v*"]       # 推送 v* 格式的 tag 时触发
  workflow_dispatch:    # 支持手动触发
```

### 构建矩阵

```yaml
strategy:
  fail-fast: false
  matrix:
    include:
      - platform: macos-latest
        args: --target universal-apple-darwin
        rust_targets: aarch64-apple-darwin,x86_64-apple-darwin
      - platform: macos-latest
        args: --target aarch64-apple-darwin
        rust_targets: aarch64-apple-darwin,x86_64-apple-darwin
      - platform: macos-latest
        args: --target x86_64-apple-darwin
        rust_targets: aarch64-apple-darwin,x86_64-apple-darwin
      - platform: ubuntu-22.04
        args: ""
        rust_targets: ""
      - platform: windows-latest
        args: ""
        rust_targets: ""
      - platform: windows-latest
        args: --target aarch64-pc-windows-msvc
        rust_targets: aarch64-pc-windows-msvc
```

### 构建矩阵详情

| 平台 | 架构 | 产物格式 | 文件名示例 |
|------|------|---------|-----------|
| macOS | Universal (Intel + Apple Silicon) | `.dmg` | `Zoro_{version}_universal.dmg` |
| macOS | aarch64 (Apple Silicon) | `.dmg` | `Zoro_{version}_aarch64.dmg` |
| macOS | x86_64 (Intel) | `.dmg` | `Zoro_{version}_x64.dmg` |
| Linux | x86_64 | `.deb` | `zoro_{version}_amd64.deb` |
| Linux | x86_64 | `.AppImage` | `Zoro_{version}_amd64.AppImage` |
| Windows | x86_64 | `.exe` | `Zoro_{version}_x64-setup.exe` |
| Windows | x86_64 | `.msi` | `Zoro_{version}_x64_en-US.msi` |
| Windows | aarch64 (ARM) | `.exe` | `Zoro_{version}_aarch64-setup.exe` |
| Windows | aarch64 (ARM) | `.msi` | `Zoro_{version}_aarch64_en-US.msi` |

### Release 流程

```
推送 tag (v0.2.0)
    │
    ▼
版本号同步 ──▶ 自动更新 package.json 和 tauri.conf.json 中的版本号
    │            node -e "..." 将 tag 中的版本写入配置文件
    │
    ▼
pnpm install ──▶ 安装前端依赖
    │
    ▼
tauri-apps/tauri-action@v0 ──▶ Tauri 官方 GitHub Action
    │                            自动执行 pnpm tauri build
    │                            根据平台生成对应安装包
    │
    ▼
创建 GitHub Release (Draft)
    ├── 上传构建产物
    └── 生成中英双语下载指南
```

### 版本号自动同步

Release 工作流会自动从 git tag 提取版本号并同步到配置文件：

```bash
VERSION="${GITHUB_REF#refs/tags/v}"
# 更新 apps/desktop/package.json 和 apps/desktop/src-tauri/tauri.conf.json
```

## 各平台手动构建

### macOS

```bash
# 安装依赖
pnpm install

# Universal 构建（同时包含 Intel 和 Apple Silicon）
cd apps/desktop
pnpm tauri build --target universal-apple-darwin

# 仅 Apple Silicon
pnpm tauri build --target aarch64-apple-darwin

# 仅 Intel
pnpm tauri build --target x86_64-apple-darwin
```

产物位于 `apps/desktop/src-tauri/target/{target}/release/bundle/`。

**注意**：Universal 构建需要同时安装两个 Rust target：

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

### Linux (Ubuntu/Debian)

```bash
# 安装系统依赖
sudo apt-get update
sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf

# 安装前端依赖
pnpm install

# 构建
cd apps/desktop
pnpm tauri build
```

产物：

- `.deb` 包：`target/release/bundle/deb/zoro_{version}_amd64.deb`
- `.AppImage`：`target/release/bundle/appimage/Zoro_{version}_amd64.AppImage`

### Windows

```bash
# 确保已安装 Visual Studio Build Tools 和 WebView2

# 安装前端依赖
pnpm install

# x86_64 构建
cd apps/desktop
pnpm tauri build

# ARM64 构建（交叉编译）
rustup target add aarch64-pc-windows-msvc
pnpm tauri build --target aarch64-pc-windows-msvc
```

产物：

- `.exe` 安装包：`target/release/bundle/nsis/Zoro_{version}_x64-setup.exe`
- `.msi` 安装包：`target/release/bundle/msi/Zoro_{version}_x64_en-US.msi`

## 浏览器扩展构建与发布

### 构建

```bash
pnpm --filter @zoro/browser-extension build
```

产物位于 `apps/browser-extension/dist/`。

### 发布到 Chrome 网上应用店

1. 更新 `apps/browser-extension/manifest.json` 中的 `version`
2. 构建扩展
3. 打包：
   ```bash
   cd apps/browser-extension/dist
   zip -r ../zoro-extension-v{version}.zip .
   ```
4. 在 [Chrome Web Store Developer Dashboard](https://chrome.google.com/webstore/devconsole) 上传并提交审核

详见[浏览器扩展文档](browser-extension.md#发布到-chrome-网上应用店)。

## 版本管理

### Git Tag 驱动

Zoro 使用 git tag 触发自动发布：

```bash
# 创建新版本
git tag v0.2.0
git push origin v0.2.0
```

这会自动触发 Release 工作流，在 GitHub Actions 上进行多平台构建。

### 版本号同步机制

版本号存在以下位置（Release 工作流会自动同步）：

| 文件 | 字段 | 说明 |
|------|------|------|
| `apps/desktop/package.json` | `version` | 前端版本 |
| `apps/desktop/src-tauri/tauri.conf.json` | `version` | Tauri 应用版本 |
| `apps/browser-extension/manifest.json` | `version` | 扩展版本（需手动更新） |
| `crates/*/Cargo.toml` | `version` | Rust crate 版本（需手动更新） |

### 版本发布检查清单

- [ ] 所有 CI 检查通过
- [ ] 更新 Cargo.toml 中的 crate 版本号（如需）
- [ ] 更新浏览器扩展 manifest.json 版本号（如需）
- [ ] 创建 git tag 并推送
- [ ] 等待 GitHub Actions 构建完成
- [ ] 审查 Release Draft
- [ ] 补充发布说明
- [ ] 发布 Release

## 发布说明格式

Release 工作流自动生成中英双语下载指南，格式如下：

```markdown
## Download Guide

**Windows (x86_64)** — Most Windows PCs
[Download .exe installer](...) | [Download .msi installer](...)

**macOS (Universal)** — Supports both Intel & Apple Silicon
[Download .dmg](...)

**macOS (Apple Silicon)** — M1/M2/M3/M4 Mac
[Download .dmg](...)

**macOS (Intel)** — Intel Mac
[Download .dmg](...)

**Linux (x86_64)** — Debian/Ubuntu or generic Linux
[Download .deb](...) | [Download .AppImage](...)

**Windows (ARM64)** — ARM devices like Surface Pro X
[Download .exe installer](...) | [Download .msi installer](...)

---

## 下载指南

**Windows (x86_64)** — 大多数 Windows 电脑
[下载 .exe 安装包](...) | [下载 .msi 安装包](...)

**macOS (Universal)** — 同时支持 Intel 与 Apple Silicon
[下载 .dmg](...)

**macOS (Apple Silicon)** — M1/M2/M3/M4 芯片 Mac
[下载 .dmg](...)

**macOS (Intel)** — Intel 芯片 Mac
[下载 .dmg](...)

**Linux (x86_64)** — Debian/Ubuntu 或通用 Linux
[下载 .deb](...) | [下载 .AppImage](...)

**Windows (ARM64)** — Surface Pro X 等 ARM 设备
[下载 .exe 安装包](...) | [下载 .msi 安装包](...)
```

建议在发布说明中额外添加以下内容：

- 新功能列表
- Bug 修复
- 破坏性变更（如有）
- 已知问题
