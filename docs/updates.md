# Updates

Folio updates over the air from its own GitHub Releases. The updater runs only
when the reader chooses **Folio → Check for Updates…**; nothing is fetched at
launch, and nothing is installed without a second, explicit **Install**.

## What the reader sees

1. The menu item is chosen. Folio asks
   `https://github.com/blend-innovation/folio/releases/latest/download/latest.json`.
2. If the manifest version is newer than the running app, a native dialog offers
   the release notes and the version it would replace.
3. **Install** downloads `Folio.app.tar.gz`, verifies it against the public key
   built into the app, replaces the bundle in place, and relaunches.
4. **Later** closes the dialog and does nothing else. If the network is down or
   the endpoint is unreachable, the dialog says so and Folio keeps reading.

If `/Applications` is not writable by the user, macOS asks for an administrator
password to complete the swap; otherwise it is a rename on the same volume and
is effectively instant.

There is no update process left running afterwards, no scheduled check, and no
telemetry. The request is the only network traffic the shell ever originates.

## The pieces

| Piece | Where |
| --- | --- |
| Menu item and dialogs | `src-tauri/src/update.rs`, wired in `lib.rs` |
| Plugin and endpoint | `tauri-plugin-updater`, configured under `plugins.updater` in `tauri.conf.json` |
| Public key | `plugins.updater.pubkey` in `tauri.conf.json` |
| Manifest builder | `scripts/release-manifest.mjs` |
| Signing and release | `.github/workflows/macos.yml` |

The manifest is the same shape for both Mac architectures, because a universal
build serves both:

```json
{
  "version": "0.2.0",
  "notes": "...",
  "pub_date": "2026-01-01T00:00:00Z",
  "platforms": {
    "darwin-aarch64": { "signature": "...", "url": "https://github.com/.../Folio.app.tar.gz" },
    "darwin-x86_64": { "signature": "...", "url": "https://github.com/.../Folio.app.tar.gz" }
  }
}
```

The manifest is fetched over TLS but is not itself signed, so
`release-manifest.mjs` refuses to write one unless the tag, `tauri.conf.json`,
and `src-tauri/Cargo.toml` versions all agree. Only the artifact's signature is
cryptographic, and it covers exactly one tarball.

## Cutting a release

1. Bump the version in `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`
   (they must match; the manifest script enforces it).
2. Land it on `main`, then tag `v<version>` and push the tag.
3. CI builds the universal app, signs it, and — when
   `TAURI_SIGNING_PRIVATE_KEY` is set — emits `Folio.app.tar.gz` and
   `Folio.app.tar.gz.sig`, then writes `latest.json` and attaches all four
   assets (DMG, app zip, tarball, signature, manifest) to the release.
4. `releases/latest` must resolve to that release for the endpoint to work. Do
   not mark an older release as latest by hand, and do not publish a tag as a
   pre-release if it should ship.

A tagged build without the signing secret still publishes a DMG, but prints a
warning and never creates `latest.json`. That release is installable by hand and
invisible to the updater.

## Signing

Two independent signatures matter:

- **Update signature** (`TAURI_SIGNING_PRIVATE_KEY`): proves the tarball came
  from someone holding the private key. Required for updates to install at all;
  an unsigned or foreign-signed artifact is refused before anything is written.
- **Apple code signature** (`APPLE_CERTIFICATE`, `APPLE_SIGNING_IDENTITY`, plus
  notarization credentials): the updater replaces the files inside the bundle.
  For a signed app that breaks the signature and the replacement will not
  launch; for an unsigned one Gatekeeper quarantines it. Either way the download
  verifies and the install produces an app nobody can open, so the first release
  people may update *from* has to be signed and notarized.

The keys live outside the repository:

```sh
pnpm tauri signer generate -w ~/.tauri/folio.key
```

`~/.tauri/folio.key.pub` is pasted into `plugins.updater.pubkey`; the private
key becomes the repository secret `TAURI_SIGNING_PRIVATE_KEY`, with
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` if it is password protected. **Back both up
off the machine.** If the private key is lost, every installed copy of Folio is
permanently unable to update itself — the only recovery is a manual download.

Treat the key as a release credential: it can sign a build that any Folio
installation will trust. Rotating it requires shipping one final release signed
with the old key, whose bundle carries the new public key.

### Required secrets

| Secret | Purpose |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | Signs `Folio.app.tar.gz` |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Only if the key is password protected |
| `APPLE_CERTIFICATE` | Base64 Developer ID Application `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | Password for that `.p12` |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Name (TEAMID)` |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | Notarization |

All of them are optional for pull-request and fork builds, which produce an
unsigned DMG exactly as before.

## Limits, stated plainly

- **The first release with the updater is a bridge.** Builds published before it
  have no menu item and cannot be updated in place.
- **No staged rollout.** `releases/latest` is all-or-nothing; to pause a release,
  mark it as a pre-release or pull the tag's manifest asset.
- **No downgrades.** The updater ignores a manifest whose version is not newer
  than the running app.
- **Unsigned local builds are not part of the chain.** `pnpm tauri build` without
  the secrets produces a DMG, not an update artifact; debug builds report the
  available version and install nothing.
- **Verification is per file, not per release.** The trust anchor is the pubkey
  compiled into the app, so the update channel is exactly as trustworthy as the
  key that signed it.
