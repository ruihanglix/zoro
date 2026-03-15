#!/usr/bin/env bash

# Copyright (c) 2026 Ruihang Li and the Zoro Team
# Licensed under the AGPL-3.0 license.
# See LICENSE file in the project root for full license information.

# Setup git hooks for the zoteclaw project.
# Usage: bash scripts/setup-hooks.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HOOKS_DIR="$REPO_ROOT/.git/hooks"

echo "Installing git hooks..."

# Install pre-commit hook
cp "$REPO_ROOT/scripts/pre-commit" "$HOOKS_DIR/pre-commit"
chmod +x "$HOOKS_DIR/pre-commit"

echo "✓ pre-commit hook installed at .git/hooks/pre-commit"
echo ""
echo "Done! The following checks will run automatically before each commit:"
echo "  • Rust: cargo fmt --check, clippy, tests   (when .rs files are staged)"
echo "  • TypeScript: type-check                    (when desktop TS files are staged)"
echo "  • Extension: build check                    (when extension files are staged)"
echo ""
echo "To skip hooks temporarily: git commit --no-verify"
