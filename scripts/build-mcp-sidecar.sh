#!/usr/bin/env bash

# Copyright (c) 2026 Ruihang Li and the Zoro Team
# Licensed under the AGPL-3.0 license.
# See LICENSE file in the project root for full license information.

set -euo pipefail

# Build the zoro-mcp sidecar binary and copy it to src-tauri/binaries/
# with the Tauri-required target-triple suffix.
#
# Usage:
#   ./scripts/build-mcp-sidecar.sh [--release] [--target <triple>]

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TAURI_DIR="$WORKSPACE_ROOT/apps/desktop/src-tauri"
BINARIES_DIR="$TAURI_DIR/binaries"

RELEASE_FLAG=""
TARGET_TRIPLE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release) RELEASE_FLAG="--release"; shift ;;
    --target)  TARGET_TRIPLE="$2"; shift 2 ;;
    *)         shift ;;
  esac
done

# Auto-detect host triple if not specified.
if [ -z "$TARGET_TRIPLE" ]; then
  TARGET_TRIPLE="$(rustc -vV | awk '/^host:/ { print $2 }')"
fi

# Platform-specific binary name and extension.
if [[ "$TARGET_TRIPLE" == *windows* ]]; then
  BIN_NAME="zoro-mcp.exe"
  EXT=".exe"
else
  BIN_NAME="zoro-mcp"
  EXT=""
fi

DEST="$BINARIES_DIR/zoro-mcp-${TARGET_TRIPLE}${EXT}"

# Skip build if sidecar binary already exists (e.g. pre-built by CI).
if [ -f "$DEST" ]; then
  echo "Sidecar already exists at $DEST — skipping build."
  exit 0
fi

# Handle universal-apple-darwin: merge arm64 + x86_64 via lipo instead of cargo build.
if [ "$TARGET_TRIPLE" = "universal-apple-darwin" ]; then
  ARM_BIN="$BINARIES_DIR/zoro-mcp-aarch64-apple-darwin"
  X86_BIN="$BINARIES_DIR/zoro-mcp-x86_64-apple-darwin"

  # Build missing arch binaries first.
  if [ ! -f "$ARM_BIN" ]; then
    bash "$0" $RELEASE_FLAG --target aarch64-apple-darwin
  fi
  if [ ! -f "$X86_BIN" ]; then
    bash "$0" $RELEASE_FLAG --target x86_64-apple-darwin
  fi

  mkdir -p "$BINARIES_DIR"
  lipo -create "$ARM_BIN" "$X86_BIN" -output "$DEST"
  echo "Created universal sidecar at $DEST"
  exit 0
fi

echo "Building zoro-mcp for $TARGET_TRIPLE ..."
cargo build -p zoro-mcp $RELEASE_FLAG --target "$TARGET_TRIPLE"

# Determine profile directory.
if [ -n "$RELEASE_FLAG" ]; then
  PROFILE_DIR="release"
else
  PROFILE_DIR="debug"
fi

SRC_BIN="$WORKSPACE_ROOT/target/$TARGET_TRIPLE/$PROFILE_DIR/$BIN_NAME"

if [ ! -f "$SRC_BIN" ]; then
  echo "ERROR: Built binary not found at $SRC_BIN"
  exit 1
fi

# Copy to Tauri externalBin directory with triple suffix.
mkdir -p "$BINARIES_DIR"
cp "$SRC_BIN" "$DEST"

echo "Sidecar copied to $DEST"
