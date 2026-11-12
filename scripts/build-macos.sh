#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "Folio's app bundle must be built on macOS (the shell is a native macOS binary)." >&2
  echo "The core engine and tests run anywhere: cargo test -p folio-core -p folio-cli" >&2
  exit 1
fi

for command in node pnpm cargo rustup; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "Missing $command. See README.md for prerequisites." >&2
    exit 1
  fi
done

if ! xcode-select -p >/dev/null 2>&1; then
  echo "Xcode Command Line Tools are required: xcode-select --install" >&2
  exit 1
fi

echo "→ installing UI dependencies"
pnpm install --frozen-lockfile

echo "→ adding Rust targets"
rustup target add aarch64-apple-darwin x86_64-apple-darwin

echo "→ building the UI"
pnpm build:ui

echo "→ building Folio for Apple silicon and Intel"
pnpm tauri build --target universal-apple-darwin --bundles app,dmg

echo
echo "Folio.app:"
find src-tauri/target/universal-apple-darwin/release/bundle -maxdepth 3 -name "Folio.app" -print
echo "DMG:"
find src-tauri/target/universal-apple-darwin/release/bundle -maxdepth 3 -name "*.dmg" -print
