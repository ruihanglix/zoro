# Deployment & Release Guide

This document covers the CI/CD pipeline, release process, and build instructions for Zoro.

## CI Workflow

The CI pipeline (`.github/workflows/ci.yml`) runs on every push to `main` and on pull requests targeting `main`. It ensures code quality across both Rust and TypeScript.

### CI Jobs

#### `check` (ubuntu-22.04)

Runs the full Rust and TypeScript quality gate:

| Step | Command | Purpose |
|---|---|---|
| Rust format check | `cargo fmt --all -- --check` | Enforce consistent formatting |
| Rust clippy | `cargo clippy --all-targets --all-features -- -D warnings` | Lint with warnings-as-errors |
| Rust tests | `cargo test --all` | Run all unit and integration tests |
| TypeScript type check | `pnpm --filter @zoro/desktop type-check` | Verify TypeScript types |

#### `build-extension` (ubuntu-22.04)

Builds the browser extension to verify it compiles:

```
pnpm --filter @zoro/browser-extension build
```

### CI Requirements

The CI pipeline installs these dependencies:

- **pnpm 10** (via `pnpm/action-setup@v4`)
- **Node.js 20** (via `actions/setup-node@v4`)
- **Rust stable** with `clippy` and `rustfmt` components
- **System packages** (Ubuntu): `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`, `patchelf`

## Release Workflow

The release workflow (`.github/workflows/release.yml`) builds multi-platform desktop installers. It is triggered by:

- **Git tags** matching `v*` (e.g., `v0.1.0`)
- **Manual dispatch** (`workflow_dispatch`)

### Build Matrix

The release builds for 6 platform/architecture combinations:

| Platform | Target | Output Formats |
|---|---|---|
| macOS (Universal) | `universal-apple-darwin` | `.dmg` |
| macOS (Apple Silicon) | `aarch64-apple-darwin` | `.dmg` |
| macOS (Intel) | `x86_64-apple-darwin` | `.dmg` |
| Linux (x86_64) | default | `.deb`, `.AppImage` |
| Windows (x86_64) | default | `.exe`, `.msi` |
| Windows (ARM64) | `aarch64-pc-windows-msvc` | `.exe`, `.msi` |

### Version Synchronization

When triggered by a git tag, the release workflow automatically syncs the version from the tag to:
- `apps/desktop/package.json`
- `apps/desktop/src-tauri/tauri.conf.json`

```bash
VERSION="${GITHUB_REF#refs/tags/v}"
# Updates both JSON files with the extracted version
```

### Release Output

The workflow uses `tauri-apps/tauri-action@v0` which:
1. Builds the desktop app for each platform
2. Creates a **draft** GitHub Release
3. Uploads all platform artifacts
4. Generates bilingual (English + Chinese) release notes with download links

### Release Notes Format

Release notes are bilingual, with download links for each platform:

```markdown
## Download Guide

**Windows (x86_64)** -- Most Windows PCs
[Download .exe installer](...) | [Download .msi installer](...)

**macOS (Universal)** -- Supports both Intel & Apple Silicon
[Download .dmg](...)

**macOS (Apple Silicon)** -- M1/M2/M3/M4 Mac
[Download .dmg](...)

**macOS (Intel)** -- Intel Mac
[Download .dmg](...)

**Linux (x86_64)** -- Debian/Ubuntu or generic Linux
[Download .deb](...) | [Download .AppImage](...)

**Windows (ARM64)** -- ARM devices like Surface Pro X
[Download .exe installer](...) | [Download .msi installer](...)

---

## 下载指南

(Same links with Chinese descriptions)
```

## Manual Build Instructions

### Prerequisites

See [Development Guide](development.md) for prerequisites and system dependencies.

### macOS

```bash
# Universal binary (recommended)
cd apps/desktop
pnpm tauri build --target universal-apple-darwin

# Apple Silicon only
pnpm tauri build --target aarch64-apple-darwin

# Intel only
pnpm tauri build --target x86_64-apple-darwin
```

For Universal builds, both Rust targets must be installed:

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

Output: `apps/desktop/src-tauri/target/release/bundle/dmg/Zoro_*.dmg`

### Linux

```bash
# Install system dependencies (Ubuntu/Debian)
sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf

# Build
cd apps/desktop
pnpm tauri build
```

Output:
- `apps/desktop/src-tauri/target/release/bundle/deb/zoro_*_amd64.deb`
- `apps/desktop/src-tauri/target/release/bundle/appimage/Zoro_*_amd64.AppImage`

### Windows

```bash
# x86_64 (most PCs)
cd apps/desktop
pnpm tauri build

# ARM64
pnpm tauri build --target aarch64-pc-windows-msvc
```

For ARM64 builds:

```bash
rustup target add aarch64-pc-windows-msvc
```

Output:
- `apps/desktop/src-tauri/target/release/bundle/nsis/Zoro_*_x64-setup.exe`
- `apps/desktop/src-tauri/target/release/bundle/msi/Zoro_*_x64_en-US.msi`

## Browser Extension

### Building

```bash
pnpm --filter @zoro/browser-extension build
```

Output is in `apps/browser-extension/dist/`.

### Publishing to Chrome Web Store

1. **Prepare the zip**:
   ```bash
   cd apps/browser-extension/dist
   zip -r ../zoro-extension.zip .
   ```

2. **Upload** to the [Chrome Web Store Developer Dashboard](https://chrome.google.com/webstore/devconsole)

3. **Submit** for review

The extension uses `host_permissions` for `127.0.0.1` only, which is a local-only permission and generally passes Chrome Web Store review without issues.

### Version Management

Update the version in `apps/browser-extension/manifest.json` before publishing:

```json
{
  "version": "0.2.0"
}
```

## Version Management

### Desktop App

The canonical version is managed via **git tags**:

1. Create and push a tag:
   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```

2. The release workflow:
   - Extracts the version from the tag (`v0.2.0` -> `0.2.0`)
   - Updates `apps/desktop/package.json` and `apps/desktop/src-tauri/tauri.conf.json`
   - Builds and creates a draft release

3. Review the draft release on GitHub and publish when ready.

### Rust Crates

Rust crate versions are managed in their respective `Cargo.toml` files:
- `crates/zoro-core/Cargo.toml`
- `crates/zoro-db/Cargo.toml`
- `crates/zoro-subscriptions/Cargo.toml`
- `apps/desktop/src-tauri/Cargo.toml`

These are not currently published to crates.io.

## Creating a Release

Step-by-step release process:

1. **Ensure CI passes** on the `main` branch
2. **Update changelogs** (if applicable)
3. **Tag the release**:
   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```
4. **Wait for the release workflow** to complete (builds all platforms)
5. **Review the draft release** on GitHub
   - Verify all 6 platform builds succeeded
   - Check download links work
   - Edit release notes if needed
6. **Publish** the release (un-draft it)
7. **Build and publish** the browser extension separately if updated

## See Also

- [Development Guide](development.md) -- Local development setup
- [Contributing Guide](contributing.md) -- How to contribute
- [Architecture Overview](architecture.md) -- System design
