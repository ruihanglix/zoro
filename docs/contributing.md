# Contributing Guide

Thank you for your interest in contributing to Zoro! This project aims to be the go-to literature management tool for the AI age, and contributions of all kinds are welcome.

## Project Values

- **Agent-first design**: Every feature should consider how AI agents will interact with it
- **Simplicity**: Prefer simple, understandable solutions over clever abstractions
- **Cross-platform**: The app should work well on macOS, Linux, and Windows
- **Open formats**: Use standard, readable data formats (JSON, Markdown, SQLite)

## Getting Started

1. **Read the [Development Guide](development.md)** to set up your local environment
2. **Read the [Architecture Overview](architecture.md)** to understand how the system fits together
3. **Browse open issues** on GitHub for ideas, or create your own

## Development Workflow

### 1. Fork and Clone

```bash
git clone <your-fork-url> zoro
cd zoro
pnpm install
```

### 2. Create a Branch

```bash
git checkout -b feature/your-feature-name
# or
git checkout -b fix/issue-description
```

Branch naming conventions:
- `feature/` -- New features
- `fix/` -- Bug fixes
- `refactor/` -- Code refactoring
- `docs/` -- Documentation changes
- `test/` -- Test additions or fixes

### 3. Implement Your Changes

Follow the code style guidelines below. Write tests for new functionality.

### 4. Run Checks Locally

Before pushing, run the full CI equivalent:

```bash
# Rust checks
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all

# TypeScript checks
pnpm --filter @zoro/desktop type-check

# Browser extension (if modified)
pnpm --filter @zoro/browser-extension build
```

### 5. Commit and Push

```bash
git add .
git commit -m "feat: add ArXiv RSS subscription source"
git push origin feature/your-feature-name
```

### 6. Open a Pull Request

Open a PR against the `main` branch. Include:
- A clear description of the changes
- Motivation and context
- Testing steps
- Screenshots (for UI changes)

## Code Style

### Rust

**Formatting** is enforced by `rustfmt` with the project's `rustfmt.toml`:

```toml
edition = "2021"
max_width = 100
tab_spaces = 4
use_field_init_shorthand = true
```

Run `cargo fmt --all` to auto-format.

**Linting** is enforced by clippy with warnings treated as errors:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

**Error handling:**
- Use per-crate error enums with `thiserror::Error` (e.g., `CoreError`, `DbError`, `SubscriptionError`)
- Propagate with `?` operator; wrap external errors via `#[from]`
- Tauri commands return `Result<T, String>` -- stringify errors at the boundary

**Naming:**
- Functions/fields: `snake_case` (e.g., `generate_paper_slug`)
- Types/enums/variants: `PascalCase` (e.g., `Paper`, `ReadStatus`)
- Crate names: `kebab-case` (e.g., `zoro-core`)

**Imports:**
- Group order: external crates, std, crate-internal (`crate::`), workspace siblings
- No wildcard imports except `use super::*` in test modules

**Tests:**
- Inline `#[cfg(test)] mod tests` at the bottom of each file
- Test names prefixed with `test_`
- Network-dependent tests marked with `#[ignore]`

### TypeScript

**Formatting and linting** is handled by [Biome](https://biomejs.dev/):

- Semicolons: always
- Quotes: double (`"`)
- Indentation: 2 spaces
- Trailing commas: yes (multi-line)

Run `pnpm --filter @zoro/desktop lint` to lint.

**React patterns:**
- Functional components only, no `React.FC`
- Named exports (only `App.tsx` uses default export)
- Zustand stores for state (no prop drilling, no Context)
- `import type` for type-only imports

### Commit Messages

Follow the [Conventional Commits](https://www.conventionalcommits.org/) format:

```
<type>: <description>

[optional body]
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `refactor`: Code restructuring without behavior change
- `docs`: Documentation
- `test`: Adding or updating tests
- `chore`: Build, CI, dependency updates
- `style`: Formatting changes (no code logic change)

Examples:
```
feat: add ArXiv RSS subscription source
fix: handle duplicate slug on paper save
docs: add subscription plugin development guide
refactor: extract paper slug generation into core crate
test: add integration tests for BibTeX import
```

## Testing Requirements

All PRs must pass the CI checks:

1. **Rust format**: `cargo fmt --all -- --check`
2. **Rust lint**: `cargo clippy --all-targets --all-features -- -D warnings`
3. **Rust tests**: `cargo test --all`
4. **TypeScript types**: `pnpm --filter @zoro/desktop type-check` (equivalent to `tsc --noEmit`)
5. **Extension build**: `pnpm --filter @zoro/browser-extension build` (if extension is modified)

When adding new features:
- Add unit tests for new Rust functions
- Mark network-dependent tests with `#[ignore]`
- No JavaScript/TypeScript test framework is currently configured

## PR Process and Review

1. **All PRs require review** before merging
2. **CI must pass** -- all checks must be green
3. **Keep PRs focused** -- one feature or fix per PR
4. **Respond to feedback** promptly
5. **Squash merge** is the default merge strategy

## Areas Open for Contribution

### New Subscription Source Plugins

Implement the `SubscriptionSource` trait for additional paper feeds. See the [Subscription Plugin Development](subscription-plugins.md) guide. Ideas:

- ArXiv RSS/Atom feeds by category
- Semantic Scholar recommendations
- PubMed/PMC feeds
- OpenAlex trending papers
- Twitter/X academic paper threads
- Conference proceedings (NeurIPS, ICML, ACL, etc.)

### Additional Browser Detectors

Add paper detection for more websites. See the [Browser Extension](browser-extension.md) guide. Ideas:

- Semantic Scholar paper pages
- PubMed/PMC
- ACL Anthology
- IEEE Xplore
- Springer/Nature
- PMLR (Proceedings of Machine Learning Research)
- OpenReview

### UI Improvements

- Paper reading view (render PDF/HTML inline)
- Better tag management (color picker, hierarchy)
- Collection drag-and-drop
- Keyboard shortcuts
- Dark mode improvements
- Paper relationship graph visualization

### Internationalization

The app currently supports English. Adding i18n infrastructure and translations would help:

- Chinese (Simplified and Traditional)
- Japanese
- Korean
- European languages

### Mobile Support

Tauri v2 supports iOS and Android. Contributions toward a mobile version are welcome:

- Responsive layout for small screens
- Touch-friendly interactions
- Mobile-specific features (camera for paper scanning)

### Developer Experience

- Additional documentation
- Development tooling improvements
- Better error messages
- Performance profiling and optimization

## Code of Conduct

This project follows the [Contributor Covenant Code of Conduct](https://www.contributor-covenant.org/version/2/1/code_of_conduct/). By participating, you agree to maintain a respectful and inclusive environment.

In short:
- Be respectful and inclusive
- Welcome newcomers
- Focus on constructive feedback
- No harassment or discriminatory behavior

## License

Zoro is licensed under the **Apache License 2.0**. By contributing, you agree that your contributions will be licensed under the same license.

See the [LICENSE](../LICENSE) file for the full text.

## Questions?

- Open a GitHub issue for bugs or feature requests
- Start a discussion for questions or ideas
- Check existing issues before creating new ones
