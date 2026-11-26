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
# A local update artifact is opt-in: without TAURI_SIGNING_PRIVATE_KEY the
# build stays a plain DMG, which is what contributors want.
if [[ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ]]; then
  echo "→ signing the update artifact (TAURI_SIGNING_PRIVATE_KEY is set)"
  pnpm tauri build --target universal-apple-darwin --bundles app,dmg \
    --config '{"bundle":{"createUpdaterArtifacts":true}}'

  tarball=src-tauri/target/universal-apple-darwin/release/bundle/macos/Folio.app.tar.gz
  node scripts/release-manifest.mjs \
    --tag "v$(node -p "require('./src-tauri/tauri.conf.json').version")" \
    --tarball "$tarball" \
    --signature "$tarball.sig" \
    --out "$(dirname "$tarball")/latest.json"
else
  pnpm tauri build --target universal-apple-darwin --bundles app,dmg
fi

echo
echo "Folio.app:"
find src-tauri/target/universal-apple-darwin/release/bundle -maxdepth 3 -name "Folio.app" -print
echo "DMG:"
find src-tauri/target/universal-apple-darwin/release/bundle -maxdepth 3 -name "*.dmg" -print
