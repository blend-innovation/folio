#!/usr/bin/env node
// Writes the update manifest the app reads when the reader picks
// "Check for Updates…".
//
// Tauri's updater compares the manifest version with the running app, then
// downloads the artifact and checks it against the public key in
// `tauri.conf.json`. The manifest itself is not signed, so it must describe the
// exact artifact that was signed: the tag, the bundle version, and the
// signature all have to belong to the same build. This script is the guard that
// keeps them together.
//
//   node scripts/release-manifest.mjs \
//     --tag v0.2.0 \
//     --tarball target/universal-apple-darwin/release/bundle/macos/Folio.app.tar.gz \
//     --signature target/universal-apple-darwin/release/bundle/macos/Folio.app.tar.gz.sig \
//     --out target/universal-apple-darwin/release/latest.json
//
// The universal tarball serves both Mac architectures, so the manifest lists
// it under `darwin-aarch64` and `darwin-x86_64`. Use `--out -` to print the
// manifest instead of writing it.

import { readFileSync, writeFileSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const DEFAULT_REPO = "blend-innovation/folio";
const DEFAULT_APP = "Folio";

/** Every Mac target the universal bundle can update. */
const TARGETS = ["darwin-aarch64", "darwin-x86_64"];

function fail(message) {
  console.error(`release-manifest: ${message}`);
  process.exit(1);
}

function parse(argv) {
  const flags = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) fail(`unexpected argument ${arg}`);
    const key = arg.slice(2);
    const value = argv[index + 1];
    if (value === undefined || value.startsWith("--")) fail(`${arg} needs a value`);
    flags[key] = value;
    index += 1;
  }
  return flags;
}

function versionFrom(relative) {
  const text = readFileSync(join(root, relative), "utf8");
  if (relative.endsWith(".json")) {
    return JSON.parse(text).version;
  }
  const match = text.match(/^version\s*=\s*"([^"]+)"/m);
  if (!match) fail(`no version in ${relative}`);
  return match[1];
}

const flags = parse(process.argv.slice(2));
const { tag, tarball, signature, out } = flags;
const repo = flags.repo ?? DEFAULT_REPO;
const app = flags.app ?? DEFAULT_APP;
const notes = flags.notes ?? "";

if (!tag) fail("--tag is required, e.g. --tag v0.2.0");
if (!tarball) fail("--tarball is required (the .app.tar.gz the build produced)");
if (!signature) fail("--signature is required (the .sig next to the tarball)");
if (!out) fail("--out is required (path for latest.json, or - for stdout)");

// The updater reads its version from the bundle, so a tag that disagrees with
// the bundle would announce a release that installs as something else.
const version = versionFrom("src-tauri/tauri.conf.json");
const crateVersion = versionFrom("src-tauri/Cargo.toml");
if (tag !== `v${version}`) {
  fail(`tag ${tag} does not match the bundle version v${version}`);
}
if (crateVersion !== version) {
  fail(
    `src-tauri/Cargo.toml is ${crateVersion} but src-tauri/tauri.conf.json is ${version}`,
  );
}

const artifact = basename(tarball);
const sig = readFileSync(signature, "utf8").trim();
if (!sig) fail(`${signature} is empty`);

const url = `https://github.com/${repo}/releases/download/${tag}/${artifact}`;
const pubDate = flags["pub-date"] ?? new Date().toISOString();
const manifest = {
  version,
  notes,
  pub_date: pubDate,
  platforms: Object.fromEntries(TARGETS.map((target) => [target, { signature: sig, url }])),
};

const text = `${JSON.stringify(manifest, null, 2)}\n`;
if (out === "-") {
  process.stdout.write(text);
} else {
  writeFileSync(out, text);
  console.log(`latest.json: ${version} → ${url}`);
}
