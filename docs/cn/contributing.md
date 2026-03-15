# 贡献指南

## 欢迎

感谢你对 Zoro 的关注！Zoro 是一个 AI 原生的文献管理工具，我们欢迎各种形式的贡献：代码、文档、Bug 报告、功能建议、测试等。

### 项目价值观

- **Agent 友好**：设计决策应考虑 AI Agent 的使用场景
- **简洁实用**：优先实现核心功能，避免过度抽象
- **跨平台**：保持 Windows、macOS、Linux 的兼容性
- **本地优先**：数据存储在用户本地，尊重隐私

## 快速开始

请先阅读[开发指南](development.md)完成环境搭建和项目编译。

简要步骤：

```bash
# 安装 Rust、Node.js 20+、pnpm 10+
# 安装平台系统依赖（详见开发指南）

git clone https://github.com/AIClaw/zoro.git
cd zoro
pnpm install
pnpm tauri dev
```

## 开发工作流

### 1. Fork 和克隆

```bash
# Fork 仓库后克隆你的 fork
git clone https://github.com/YOUR_USERNAME/zoro.git
cd zoro
git remote add upstream https://github.com/AIClaw/zoro.git
```

### 2. 创建功能分支

```bash
git checkout -b feat/your-feature-name
# 或
git checkout -b fix/your-bug-fix
```

分支命名约定：

| 前缀 | 用途 | 示例 |
|------|------|------|
| `feat/` | 新功能 | `feat/pubmed-detector` |
| `fix/` | Bug 修复 | `fix/fts-search-ranking` |
| `refactor/` | 重构 | `refactor/command-handlers` |
| `docs/` | 文档 | `docs/api-reference` |
| `test/` | 测试 | `test/bibtex-parser` |

### 3. 实现

编写代码并遵循本文档中的代码风格要求（见下文）。

### 4. 测试

```bash
# Rust 测试
cargo test --all

# TypeScript 类型检查
pnpm --filter @zoro/desktop type-check

# 完整 CI 检查
cargo fmt --all -- --check && \
cargo clippy --all-targets --all-features -- -D warnings && \
cargo test --all && \
pnpm --filter @zoro/desktop type-check
```

### 5. 提交 PR

```bash
git add .
git commit -m "feat: add PubMed paper detector"
git push origin feat/pubmed-detector
```

然后在 GitHub 上创建 Pull Request。

## 代码风格

### Rust

#### 格式化

代码必须通过 `cargo fmt --all -- --check`。格式化配置在 `rustfmt.toml`：

```toml
edition = "2021"
max_width = 100
tab_spaces = 4
use_field_init_shorthand = true
```

#### Clippy

代码必须通过 `cargo clippy --all-targets --all-features -- -D warnings`，即所有 clippy 警告被视为错误。

#### 错误处理

- 每个 crate 有自己的错误枚举，使用 `thiserror::Error` 派生
- 使用 `?` 操作符传播错误
- Tauri 命令返回 `Result<T, String>`，在边界处字符串化错误

```rust
// crate 内部
fn do_something() -> Result<(), DbError> {
    let data = query_db()?;  // 自动通过 #[from] 转换
    Ok(())
}

// Tauri 命令边界
#[tauri::command]
pub async fn my_command(state: State<'_, AppState>) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock error: {}", e))?;
    // ...
}
```

#### 命名

- 函数/字段：`snake_case` — `generate_paper_slug`, `abstract_text`
- 类型/枚举/变体：`PascalCase` — `Paper`, `ReadStatus`, `DbError::NotFound`
- Crate 名：`kebab-case` — `zoro-core`
- 后缀：`*Error` 错误枚举，`*Input` 输入结构体，`*Response` 响应类型，`*Row` 数据库行类型

#### Derive 和属性

```rust
// 数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paper { ... }

// 错误枚举
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("Not found: {0}")]
    NotFound(String),
}

// Tauri 命令输入
#[derive(Debug, serde::Deserialize)]
pub struct AddPaperInput { ... }
```

#### 测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_slug() {
        let slug = generate_paper_slug("Title", "id", None);
        assert!(!slug.is_empty());
    }

    #[tokio::test]
    #[ignore] // 网络测试
    async fn test_fetch_papers() {
        // ...
    }
}
```

### TypeScript

#### 格式化和 Lint

使用 [Biome](https://biomejs.dev/) 进行代码检查和格式化：

```bash
pnpm --filter @zoro/desktop lint
pnpm --filter @zoro/browser-extension lint
```

风格要点：
- 分号：始终使用
- 引号：双引号 (`"`)
- 缩进：2 空格
- 尾逗号：多行时使用

#### 命名

- 组件/类型：`PascalCase` — `PaperList`, `LibraryState`
- 函数/变量：`camelCase` — `fetchPapers`, `handleSubmit`
- 事件处理：`handle` 前缀 — `handleSearch`, `handleDelete`
- 后缀：`*Response`, `*Input`, `*State`, `*Props`

### Commit Message 规范

采用 [Conventional Commits](https://www.conventionalcommits.org/) 格式：

```
<type>: <description>

[optional body]
```

| 类型 | 用途 | 示例 |
|------|------|------|
| `feat` | 新功能 | `feat: add Semantic Scholar subscription source` |
| `fix` | Bug 修复 | `fix: handle empty abstract in FTS indexing` |
| `refactor` | 重构 | `refactor: extract slug generation to utility module` |
| `docs` | 文档 | `docs: add subscription plugin development guide` |
| `test` | 测试 | `test: add BibTeX parser edge case tests` |
| `chore` | 杂项 | `chore: update Rust dependencies` |
| `ci` | CI 相关 | `ci: add macOS ARM64 to build matrix` |

## 测试要求

### Rust 测试

所有 PR 必须通过 `cargo test --all`。

- 新功能应附带相应的测试
- 测试放在文件底部的 `#[cfg(test)] mod tests` 中
- 需要网络的测试标记为 `#[ignore]`
- 使用 `assert!()` 和 `assert_eq!()` 进行断言

### TypeScript 类型检查

所有 PR 必须通过 `pnpm --filter @zoro/desktop type-check`（等价于 `tsc --noEmit`）。

当前项目没有配置 JavaScript/TypeScript 测试框架。

## PR 流程和审查规范

### 提交 PR

1. 确保你的分支基于最新的 `main`：
   ```bash
   git fetch upstream
   git rebase upstream/main
   ```

2. 确保所有 CI 检查通过

3. PR 描述应包含：
   - 变更内容概述
   - 动机和背景
   - 测试方法
   - 截图（如有 UI 变更）

### 审查标准

- 代码风格符合项目规范
- 有适当的测试覆盖
- 错误处理完善
- 文档更新（如有 API 变更）
- 不引入不必要的依赖

## 可贡献的方向

### 新的订阅源插件

当前仅有 HuggingFace Daily Papers。欢迎实现更多数据源：

- Semantic Scholar Recommended Papers
- PaperswithCode
- OpenAlex
- bioRxiv/medRxiv
- DBLP

详见[订阅插件开发指南](subscription-plugins.md)。

### 更多浏览器检测器

当前支持 ArXiv、DOI 和通用 meta 标签。欢迎添加：

- PubMed
- IEEE Xplore
- ACM Digital Library
- Springer
- Nature
- Science

详见[浏览器扩展文档](browser-extension.md#如何添加新的站点检测器)。

### UI 改进

- 论文阅读器（内嵌 PDF/HTML 查看）
- 引文图谱可视化
- 批量操作界面
- 暗色主题
- 键盘快捷键
- 拖拽排序

### 国际化 (i18n)

- 中文界面翻译
- 日文界面翻译
- 多语言文档

### 移动端支持

Tauri v2 支持 iOS 和 Android 目标。移动端适配工作包括：

- 响应式 UI 布局
- 触摸友好的交互
- 移动端特有功能（分享菜单、系统文件选择器等）

### 其他

- 更完善的 BibTeX/RIS 解析器
- CSL (Citation Style Language) 支持
- WebDAV/Dropbox 同步
- AI 摘要/标签自动化流水线
- MCP (Model Context Protocol) 服务器集成

## 行为准则

参与 Zoro 项目的所有贡献者应遵循以下准则：

- **友善尊重**：对所有参与者保持礼貌和尊重
- **建设性沟通**：提供有建设性的反馈，接受合理的批评
- **包容多样**：欢迎不同背景和经验水平的贡献者
- **专注技术**：讨论聚焦于技术问题，避免人身攻击

不可接受的行为包括：骚扰、歧视性语言、人身攻击、发布他人隐私信息。

## 许可证

Zoro 采用 **Apache License 2.0** 开源许可证。

提交 PR 即表示你同意将你的贡献在 Apache-2.0 许可证下发布。

详见项目根目录的 [LICENSE](../../LICENSE) 文件。
