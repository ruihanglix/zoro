# AGENTS.md — Zoro

AI-native literature management tool (like Zotero for AI agents).
Tauri v2 desktop app: Rust backend + React/TypeScript frontend.

## Documentation

The `docs/` directory contains bilingual (English + `docs/cn/` Chinese) documentation:
`architecture`, `data-model`, `development`, `deployment`, `contributing`,
`agent-integration`, `browser-extension`, `subscription-plugins`.

When making changes that affect public APIs, data models, architecture, build/deploy
processes, or plugin interfaces, **update the corresponding docs under `docs/`** (and
their Chinese translations under `docs/cn/`) to keep them in sync with the code.

## Architecture

Monorepo managed with **pnpm 10** workspaces + **Turborepo**.

| Package | Path | Tech |
|---|---|---|
| Desktop app (frontend) | `apps/desktop/src/` | React 19, TypeScript, Zustand 5, Tailwind/shadcn-ui |
| Desktop app (backend) | `apps/desktop/src-tauri/` | Rust, Tauri v2, axum 0.7, rusqlite |
| Browser extension | `apps/browser-extension/` | React 19, Chrome Manifest V3 |
| Shared types | `packages/core/` | TypeScript (types only, no runtime code) |
| Rust core models | `crates/zoro-core/` | Rust — domain models, BibTeX/RIS parsers |
| Rust DB layer | `crates/zoro-db/` | Rust — SQLite + FTS5 via rusqlite |
| Rust storage | `crates/zoro-storage/` | Rust — shared file/sync logic (paper dirs, metadata sync, attachments) |
| Rust subscriptions | `crates/zoro-subscriptions/` | Rust — feed sources (HuggingFace Daily Papers) |
| MCP server | `crates/zoro-mcp/` | Rust — standalone MCP server binary (rmcp, stdio + HTTP) |
| MCP sidecar script | `scripts/build-mcp-sidecar.sh` | Builds `zoro-mcp` and copies to Tauri `externalBin` path |

## Build / Lint / Test Commands

```bash
# Install dependencies
pnpm install

# --- Rust ---
cargo fmt --all -- --check          # Format check (CI enforced)
cargo fmt --all                     # Auto-format
cargo clippy --all-targets --all-features -- -D warnings   # Lint (CI enforced)
cargo test --all                    # Run all Rust tests
cargo test -p zoro-core         # Run tests for a single crate
cargo test -p zoro-core slug    # Run tests matching "slug" in one crate

# --- TypeScript ---
pnpm --filter @zoro/desktop type-check       # Type check desktop (CI enforced)
pnpm --filter @zoro/desktop lint              # Biome lint desktop
pnpm --filter @zoro/browser-extension build   # Build extension (CI enforced)
pnpm --filter @zoro/browser-extension lint    # Biome lint extension

# --- Full CI equivalent ---
cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all && pnpm --filter @zoro/desktop type-check

# --- Dev ---
pnpm tauri dev                      # Run desktop app in dev mode (port 1420)
# NOTE: `pnpm tauri dev` automatically builds the zoro-mcp sidecar binary
# via beforeDevCommand (scripts/build-mcp-sidecar.sh). No manual step needed.

# --- MCP sidecar (manual, rarely needed) ---
bash scripts/build-mcp-sidecar.sh                  # Build MCP sidecar for host (debug)
bash scripts/build-mcp-sidecar.sh --release         # Build MCP sidecar for host (release)
bash scripts/build-mcp-sidecar.sh --target aarch64-apple-darwin  # Cross-compile
```

No JavaScript/TypeScript test framework is configured. There are no TS test files.

## Pre-commit Hook

A git pre-commit hook is available to catch CI failures **before** pushing.
It selectively runs checks based on the types of staged files:

- **`.rs` files staged** → `cargo fmt --check`, `cargo clippy`, `cargo test`
- **`apps/desktop/**/*.{ts,tsx}` staged** → `pnpm --filter @zoro/desktop type-check`
- **`apps/browser-extension/**` staged** → `pnpm --filter @zoro/browser-extension build`

### Setup

```bash
bash scripts/setup-hooks.sh
```

This copies `scripts/pre-commit` into `.git/hooks/pre-commit`.
Every contributor should run this once after cloning the repo.

To temporarily skip: `git commit --no-verify`

## Auto-formatting (REQUIRED)

After modifying any Rust files, **always run `cargo fmt --all`** before finishing the task.
This prevents CI failures from `cargo fmt --all -- --check`.

After modifying any TypeScript/React files, **always run `pnpm --filter @zoro/desktop lint --fix`**
to auto-fix Biome lint/format issues.

## Code Style — Rust

### Formatting (`rustfmt.toml`)
- Edition 2021, max width 100, 4-space indentation
- `use_field_init_shorthand = true`

### Error Handling
- **Per-crate error enums** using `thiserror::Error` — no `anyhow`
- Crate errors: `CoreError`, `DbError`, `SubscriptionError`
- Propagate with `?` operator; external errors wrapped via `#[from]`
- **Tauri commands return `Result<T, String>`** — stringify errors at boundary:
  ```rust
  .map_err(|e| format!("DB lock error: {}", e))?
  ```
- Use `let _ = ...` for non-critical fire-and-forget results

### Naming
- Functions/fields: `snake_case` — `generate_paper_slug`, `abstract_text`
- Types/enums/variants: `PascalCase` — `Paper`, `ReadStatus`, `DbError::NotFound`
- Crate names: `kebab-case` — `zoro-core`
- Suffixes: `*Error` for error enums, `*Input` for input structs, `*Response` for API types, `*Row` for DB row types

### Module Organization
- `pub mod` declarations in `lib.rs`, selective `pub use` re-exports
- Tauri app uses `mod.rs` pattern for `commands/` directory
- No `prelude` modules, no custom macros

### Imports
- Group order: external crates, std, crate-internal (`crate::`), workspace siblings
- No wildcard imports except `use super::*` in test modules
- `serde_json::Value` and `serde_json::json!()` used path-qualified

### Derives
- Data structs: `#[derive(Debug, Clone, Serialize, Deserialize)]`
- Error enums: `#[derive(Debug, Error)]` with `#[error("...")]` on variants
- Tauri command inputs: `#[derive(Debug, serde::Deserialize)]`
- Tauri command responses: `#[derive(Debug, serde::Serialize)]`
- Serde attributes: `#[serde(rename = "abstract")]`, `#[serde(rename_all = "lowercase")]`

### Patterns
- All Tauri commands are `pub async fn` annotated with `#[tauri::command]`
- First line of every command: `let db = state.db.lock().map_err(...)? ;`
- `std::sync::Mutex` (not tokio) for DB lock — SQLite ops are fast
- `tokio::spawn` for background work (downloads), errors silently ignored
- `async_trait` for the `SubscriptionSource` trait
- `uuid::Uuid::new_v4().to_string()` for IDs, `chrono::Utc::now().to_rfc3339()` for timestamps
- Strings for IDs/dates/statuses — no newtype wrappers
- Most struct fields are `Option<T>` (papers have incomplete metadata)
- No `unsafe` code anywhere

### Tests
- Inline `#[cfg(test)] mod tests` at bottom of file, `use super::*`
- Test names prefixed with `test_`: `test_generate_slug`
- Assert with `assert!()` and `assert_eq!()`
- Network-dependent tests marked `#[ignore]`

## Code Style — TypeScript / React

### Formatting (Biome defaults, no config file)
- Semicolons: always
- Quotes: double (`"`)
- Indentation: 2 spaces
- Trailing commas: yes (multi-line)
- Arrow function params: always parenthesized — `(s) => s.view`

### Types
- `interface` for data models, props, store state; `type` only for simple unions
- Response types use `| null` for nullable fields; input types use `?` for optional
- `Record<string, unknown>` for dynamic objects — never `any`
- `import type` for type-only imports: `import type { PaperResponse } from "..."`
- `export interface` / `export type` for cross-module types

### Naming
- Components/types/interfaces: `PascalCase` — `PaperList`, `LibraryState`
- Functions/variables: `camelCase` — `fetchPapers`, `handleSubmit`
- Event handlers: `handle` prefix — `handleSearch`, `handleDelete`
- Boolean state: descriptive — `loading`, `saving`, `sidebarOpen`
- Suffixes: `*Response`, `*Input`, `*State`, `*Props`
- Files: components = `PascalCase.tsx`; UI primitives = `lowercase.tsx` (shadcn); stores = `camelCaseStore.ts`; routes = `lowercase.tsx`; utils = `camelCase.ts`

### Imports
Order: react/react-dom, external packages, `@/` alias imports, relative imports.
Use `import * as commands from "@/lib/commands"` for the Tauri IPC module.
Path alias: `@/` maps to `./src/*`.

### React Patterns
- Functional components only, no `React.FC` — use named `export function`
- `React.forwardRef` only for shadcn/ui primitives (with `displayName`)
- Only default export: `App.tsx`; everything else is named exports
- State: Zustand stores accessed directly in components (no prop drilling, no Context)
- Zustand selectors: `useLibraryStore((s) => s.papers)` — parameter always `s`, select individually
- Views switched via Zustand state, no router library
- Styling: Tailwind utilities + `cn()` helper (clsx + tailwind-merge), shadcn/ui components

### Tauri IPC (`lib/commands.ts`)
- All commands centralized in one file as exported arrow functions
- Wrap `invoke<ReturnType>("snake_case_command", { camelCaseArgs })`
- Tauri v2 applies `rename_all = "camelCase"` to command args automatically,
  so frontend invoke keys must be **camelCase**: `{ readStatus }` (not `read_status`)
- Input structs passed as `{ input }` keep their own serde rules (snake_case fields
  inside the struct are fine since the Rust structs have no `rename_all`)
- Optional params default to `null` via `?? null`
- No error handling at this layer — errors propagate to stores/components

### Zustand Stores
- `create<StateType>()` with unified state + actions interface
- Async actions: `set({ loading: true })`, try/catch, `set({ loading: false })`
- Errors stored as strings: `set({ error: String(e) })`
- After mutations, always re-fetch from backend (no optimistic updates)
- Two stores: `libraryStore` (data/domain), `uiStore` (UI state)

### Error Handling
- Store actions: try/catch with `String(e)` stored in state
- Component handlers: try/catch with `console.error()`
- Destructive actions: `confirm()` dialog before proceeding
- No toast system, no error boundaries

## Internationalization (I18N)

All user-facing text in the frontend **must** use the i18n system — no hardcoded English strings.

### Desktop App (`apps/desktop/`)

- **Library**: `i18next` + `react-i18next`
- **Config**: `src/lib/i18n.ts` — initializes i18next with language detection and all locale resources
- **Hook**: `const { t } = useTranslation()` in every component with visible text
- **Locale files**: `src/locales/{en,zh-CN,ja,ko,es,fr,de,pt,ru}.json`
- **Fallback language**: English (`en`)

### Browser Extension (`apps/browser-extension/`)

- **Lightweight module**: `src/popup/i18n.ts` — no external dependency
- **Function**: `t(key, params?)` with inline translations for all 9 languages

### Translation Key Conventions

- **Namespaced by feature**: `"section.keyName"` (e.g. `reader.highlight`, `settings.appearance`)
- **Common keys**: `common.save`, `common.cancel`, `common.delete`, `common.loading`, etc.
- **Naming**: `camelCase` for keys — `confirmBeforeDeleting`, `noModelsConfigured`
- **Interpolation**: `{{variable}}` syntax — `"clearFeedCache": "Clear Feed Cache ({{count}} items)"`
- **Plurals**: `_other` suffix for plural forms — `nPapers` / `nPapers_other`

### Adding New Text

1. Add the English key to `src/locales/en.json` under the appropriate section
2. Add translations to all 8 other locale files (use English as fallback if translation unavailable)
3. Use `t("section.keyName")` in the component (import `useTranslation` from `react-i18next`)
4. For components with `labelKey` patterns (e.g. nav items, options), store the key and call `t()` at render time

### Supported Languages (9)

| Code | Language |
|------|----------|
| `en` | English |
| `zh-CN` | 简体中文 |
| `ja` | 日本語 |
| `ko` | 한국어 |
| `es` | Español |
| `fr` | Français |
| `de` | Deutsch |
| `pt` | Português |
| `ru` | Русский |

### Rules

- **Never** hardcode user-visible text in components — always use `t()`
- Dynamic data (model names, file paths, URLs) does NOT need translation
- Section headers like `<p className="text-xs font-medium">` must use `{t("key")}`
- Placeholders, titles, tooltips, button labels, error messages — all must be translated
- When adding a new language, add to: `src/lib/i18n.ts` (supportedLanguages + resources), create locale JSON file, and update `apps/browser-extension/src/popup/i18n.ts`

