# Plugins

**Status: evaluation. There is no plugin runtime in Folio, and this document
does not add one.** It records the options, the recommended staging, and the
boundaries a future plugin host would have to respect.

## Recommendation

Keep the default app free of any embedded plugin runtime.

1. **Themes remain the first-class extension point.** They are already JSON,
   validated token by token, and shipped without executable code.
2. **Add a manifest-based declarative layer next.** One manifest can describe a
   theme pack and, later, a small set of validated renderer options. No code,
   no new runtime, no startup cost.
3. **If code extensions become necessary, prefer an opt-in external subprocess
   host speaking JSON-RPC over stdio.** It adds no embedded JS or WASM runtime,
   isolates crashes, and lets a plugin be written in any language.
4. **Do not load JavaScript into the webview and do not load native dynamic
   libraries.** Both trade Folio's small, sealed shell for a large security and
   stability surface.

This is deliberately conservative. Folio's value is a quiet reader that starts
instantly and treats untrusted Markdown safely. A plugin system that weakens
either property is not worth the feature count.

## What Folio is optimizing for

| Constraint | Consequence |
| --- | --- |
| The default app is small (single Rust binary, system WebView) | No bundled browser, Node, JS engine, or WASM runtime. |
| Startup is part of the product | No plugin discovery, process spawn, or code compilation on the boot path. |
| Documents are untrusted input | Plugin output is data, never HTML or CSS injected into the page. |
| Rendering happens in Rust | A plugin cannot sit in the parser or highlighter without an explicit host boundary. |
| macOS distribution uses hardened runtime and notarization | Native libraries complicate signing, library validation, and crash ownership. |
| Small maintainer surface | Every extension mechanism is permanent API and security debt. |

## Today: themes are the extension point

A theme is a JSON file in
`~/Library/Application Support/com.folio.reader/themes/`. Folio validates every
colour, font stack, length, and range in Rust, then emits CSS custom properties.
A bad theme is skipped; it cannot execute code or add arbitrary CSS.

That is the model the rest of this proposal follows: **declarative data,
validated by the host, with a narrow contract.**

## Options

| Option | Runtime in the default app | Isolation | Startup / distribution cost | Fit for Folio | Verdict |
| --- | --- | --- | --- | --- | --- |
| **Declarative JSON** (themes, manifests) | None; `serde_json` is already present | Validation, not isolation | Zero on boot if loaded on demand | Excellent for themes and approved options; poor for computed behaviour | **Now** |
| **WASM plugins** (e.g. Wasmtime) | Several MB of linked runtime, even when unused; extra plugin toolchain | Strong: linear memory, capability-based WASI, traps instead of process crashes | Instantiation is fast, but runtime size and API work are not | Good sandbox, but large up-front cost and a second ABI to maintain | **Defer** |
| **External subprocess + JSON-RPC/stdio** | No embedded runtime; a process manager only | Process isolation from crashes; *not* a security sandbox unless the OS provides one | Tens of milliseconds per cold start; IPC copies text | Good: language-agnostic, keeps the parser in Rust, can be opt-in and lazy | **Preferred for code** |
| **JS in the webview** | No new runtime (the WebView exists) | None: plugin code shares the app origin, DOM, `localStorage`, and Tauri IPC | Minimal startup, but CSP and packaging change | Bad: CSP is `script-src 'self'`; parsing/sanitizing lives in Rust; a plugin cannot safely influence the render pipeline | **Reject** |
| **Native dynamic library** | None | None: same address space | Zero startup, fastest calls | Bad: unstable ABI, crashes the app, hardened runtime/library validation, notarization, architecture slices | **Reject** |

### Why not WASM now

Wasmtime is the reference-quality choice, but it is a large dependency with its
own release cadence, platform builds, and security surface. A minimal build may
be a few megabytes depending on features; the exact number has to be measured
before it is promised. Plugins would also need a Rust/C/AssemblyScript toolchain
and a Folio-specific ABI for Markdown, tokens, links, and export. The result is
a good sandbox with a high fixed cost — the opposite of "small by default."

WASM becomes interesting only if two things are true: subprocess startup is
measurably too slow for an interactive hook, and there is enough plugin demand
to justify the runtime. Neither is true today.

### Why not JS in the webview

It looks free because the WebView already exists. It is not:

- The CSP is `script-src 'self'`. Running third-party JS would require
  `unsafe-eval`, a separate origin, or shipping plugin code as app assets.
- Plugin JS would run beside the app's own DOM and Tauri IPC. It could read
  document content, call host commands, or alter chrome unless every command is
  re-authorized per plugin.
- The expensive work — Markdown parsing, sanitizing, highlighting — is in Rust.
  JS could only transform source before the parser or HTML after it. Post-render
  HTML is exactly the surface Folio is designed to keep closed.

A separate webview per plugin would restore isolation, but then we have a second
browser context, an IPC bridge, and a rendering pipeline that still cannot see
the Rust AST. That is more machinery than a subprocess host.

### Why not native dynamic libraries

`dlopen` is fast and gives a plugin the full Rust/C ABI. It also lets a plugin
segfault the reader, requires ABI-stable exports forever, and complicates the
hardened runtime, library validation, code signing, and notarization. A plugin
built for one architecture would need a universal slice. The failure mode is the
whole app, not one feature.

## Recommended path

### Stage 0 — themes (shipped)

- JSON files, validated in Rust, no executable content.
- Theme packs can be documented and shared as files.
- No change to binary size or startup.

### Stage 1 — declarative manifest (next)

Add one `extension.json` per installed extension. It is **data only**; the host
validates it and never treats it as code.

```json
{
  "schemaVersion": 1,
  "id": "com.example.field-notes",
  "name": "Field Notes",
  "version": "0.1.0",
  "kind": "declarative",
  "themes": ["themes/field-notes.json", "themes/field-notes-dark.json"],
  "render": {
    "smartPunctuation": true,
    "highlighting": true
  }
}
```

What Stage 1 can carry, with no new runtime:

| Field | Validation | Notes |
| --- | --- | --- |
| `themes[]` | Same validator as today; paths stay inside the extension directory | Bundles related light/dark themes. |
| `render.smartPunctuation` | Boolean | Maps to an existing renderer option; no new parser. |
| `render.highlighting` | Boolean | Maps to an existing renderer option. |
| `callouts[]` *(candidate)* | Fixed `kind` slug, label length, icon from a small allowlist, tone token | Only if the renderer gains a validated callout registry; not part of Stage 1. |
| `name`, `version`, `id` | Slug/semver/length limits | Shown in the Extensions list. |

Stage 1 deliberately has **no** `styles`, `css`, `html`, `script`, or `entry`
fields. If a manifest needs CSS or code, it is the wrong stage.

Loading is on demand: the app reads manifests when the Extensions pane is open
or when an enabled extension is used. It does not execute anything at boot.

### Stage 2 — opt-in external plugin host

Only after a real, repeated need for code extensions.

- A plugin is a directory with `plugin.json` and an executable entrypoint.
- The user installs it by placing it in
  `~/Library/Application Support/com.folio.reader/plugins/<id>/` and explicitly
  enabling it. There is no marketplace, auto-download, or auto-update.
- The host launches the entrypoint directly (no shell), with a scrubbed
  environment, a temporary working directory, and only stdin/stdout/stderr.
- The host speaks JSON-RPC 2.0. For the MVP, one JSON object per line
  (newline-delimited JSON), with an 8 MiB frame cap. If large documents make
  that awkward, a later protocol version can switch to `Content-Length`
  framing.
- Processes are lazy: spawn on the first hook invocation, keep alive for the
  document/session, and stop after an idle timeout.
- A crash kills one document's enrichment, not the reader.

The default app may contain the small process-manager code, but no embedded
runtime and no spawn on the startup path. If the measured binary delta is
undesirable, compile the host behind a non-default Cargo feature and measure
before deciding to ship it in the standard build.

## Hooks

Hooks are narrow on purpose. Each has a stable JSON shape, a timeout, and an
output that the Rust host re-validates. No hook may return raw HTML for display.

| Hook | Input | Output | Host handling |
| --- | --- | --- | --- |
| `markdown.transform` | Markdown source, document metadata, plugin options | Markdown source | Re-parsed and sanitized by `folio-core` exactly like user input. |
| `code.highlight` | Language, code text | Ordered token spans (`kind`, `start`, `end`) | Host escapes text and emits spans; no HTML from the plugin. |
| `link.resolve` | Link target, source document path | `{ kind: "external" \| "local" \| "blocked", value }` | Host re-runs scheme/path classification; plugin cannot widen the allowlist. |
| `export.render` | Rendered HTML, metadata, theme tokens, target format | Bytes plus media type/suggested name | Used only for an explicit export action; host writes the file after confirmation. Never displayed in the reading view. |

Explicitly **not** offered: `html.postprocess`, `ui.inject`, `theme.css`, and
anything that returns script. Those are the exact escape hatches Folio's safety
model forbids.

Hook timing matters more than hook count. `markdown.transform` runs once per
render. `code.highlight` runs per code block and has the tightest budget. A
plugin that needs to process the whole document on every scroll should be
rejected by design.

## Manifest v2 (executable plugins)

```json
{
  "schemaVersion": 2,
  "id": "com.example.callouts",
  "name": "Custom callouts",
  "version": "0.1.0",
  "kind": "process",
  "entrypoint": "bin/callouts",
  "protocol": "jsonrpc-stdio/1",
  "hooks": ["markdown.transform"],
  "permissions": ["document.source"],
  "limits": {
    "startupMs": 1500,
    "requestMs": 2000,
    "documentMs": 5000,
    "maxOutputBytes": 8388608
  },
  "platforms": ["macos"]
}
```

Validation rules:

- `id`: reverse-DNS slug, 3–128 characters, unique per install.
- `version`: semver; upgrades are manual in Stage 2.
- `entrypoint`: relative path only; resolved inside the plugin directory; no
  symlink escape; must be executable; launched without a shell.
- `protocol`: exact supported version. Unknown versions are disabled, not
  guessed at.
- `hooks`: must be a subset of hooks the host implements.
- `permissions`: only `document.source`, `document.metadata`, and
  `export.output` exist. The host grants no network permission and does not
  proxy filesystem access.
- `platforms`: the host refuses to launch a plugin on an unlisted platform.

## Security boundaries

The honest version: **a subprocess is isolation from crashes, not a full
sandbox.** The plugin runs as the same macOS user. It can read files the user can
read and, without OS-level restrictions, open network connections. That is why
installation and enabling are explicit, and why the docs must say so.

What the host does enforce:

- **No shell.** `Command::new(entrypoint)` with an argument vector; no command
  string, no shell expansion.
- **Path containment.** Manifest paths are canonicalized and must remain inside
  the plugin directory. Symlinks that escape are rejected.
- **Scrubbed environment.** No inherited secrets; a minimal environment with
  `HOME` pointing at the plugin data directory.
- **Protocol-only input.** The plugin receives only the document data each hook
  is granted, not the Tauri IPC surface.
- **Output caps.** Frame size, total stdout, and wall-clock time are bounded.
  Output beyond the cap is discarded and the plugin is stopped.
- **Timeouts.** Initialize 1.5 s, per request 2 s, per document 5 s. A hung
  plugin is killed; the reading view continues without its enrichment.
- **Circuit breaker.** Two consecutive failures or crashes disable the plugin
  for the session and surface one quiet message with a link to the Extensions
  pane.
- **Re-validation.** Markdown is re-parsed; links are re-classified; tokens are
  escaped by Rust; export bytes are written only after an explicit save action.
- **No injection surface.** Plugin output never becomes CSS, DOM nodes, Tauri
  commands, or executable code.

What it does **not** enforce, until a later OS-sandbox design:

- Network denial, arbitrary filesystem reads/writes, or memory limits that are
  stronger than best-effort monitoring. macOS does not offer a portable,
  unprivileged per-child sandbox through Rust's standard process API. Do not
  claim otherwise in the UI.
- Protection from a malicious plugin the user deliberately installed. The
  boundary is informed consent plus crash isolation, not malware defense.

## Failure behaviour

| Failure | User-visible result | Host action |
| --- | --- | --- |
| Manifest invalid | Extension listed as unavailable with a short reason | Ignore it; do not execute. |
| Entrypoint missing/not executable | Same | Disable it. |
| Handshake timeout or wrong protocol | Same | Kill process, disable for session. |
| Hook times out | Document renders without that hook | Kill process; count failure. |
| Hook crashes | Document renders without that hook | Restart at most once; then disable for session. |
| Output exceeds cap or is malformed | Document renders without that hook | Kill process; log a bounded reason. |
| Plugin removed while running | Document continues | Stop process on next render. |

Every failure is local. There is no modal dialog and no blocked reading view.

## Why this does not grow the default app or slow startup

- **Stage 0** is already shipped. Themes add no runtime.
- **Stage 1** adds a manifest parser and validator built on `serde_json`, which
  is already in the binary. Manifests are read on demand, not scanned at boot.
  There is no executable content and no process to start.
- **Stage 2** adds a process manager, not an embedded runtime. No Wasmtime, no
  QuickJS, no Node, no Chromium. The manager is a few hundred lines of Rust and
  a process handle, not a multi-megabyte runtime; its binary delta should still
  be measured before shipping. Processes are spawned lazily only when an
  enabled plugin's hook is invoked. The default app still boots without reading
  plugin directories or launching anything. If the measured delta matters, the
  host can be a non-default Cargo feature; the default app remains plugin-free
  either way.

The wrong version of this feature would link a WASM runtime or a JS engine into
every copy of a Markdown reader. Folio should not do that.

## Effort and decision

| Stage / option | Scope | Rough effort | Default runtime | Decision |
| --- | --- | --- | --- | --- |
| Themes | Already implemented | — | None | Shipped; keep first-class. |
| Manifest v1 | Schema, validation, Extensions pane, theme packs | 3–5 engineer-days | None | **Do next**, only when theme packs need metadata. |
| Subprocess host v1 | Protocol, lifecycle, limits, install/enable UI, tests, docs | 2–4 engineer-weeks | No embedded runtime; lazy process only | **Preferred if code extensions are needed.** |
| WASM host | Runtime integration, ABI, plugin SDK, cross-platform builds | 4–8 engineer-weeks | Several MB linked runtime | **Defer** until subprocess latency is proven insufficient. |
| JS/webview plugins | CSP rewrite, per-plugin IPC authorization, separate context | 2–4 engineer-weeks | WebView only | **Reject** on security and architecture. |
| Native dylib | ABI, signing, notarization, crash containment | 3–6 engineer-weeks | None | **Reject.** |

## Non-goals

- No plugin runtime in the default app.
- No JavaScript evaluation in the reader WebView.
- No native dynamic libraries.
- No arbitrary CSS, HTML, or DOM injection from a plugin.
- No plugin marketplace, remote install, auto-update, or telemetry.
- No plugin-defined menu items or UI panels in the first host.
- No replacement of the Markdown parser, sanitizer, or core highlighter.
- No network or filesystem permissions granted by the host. (The plugin still
  runs with the user's OS rights; see the security boundary above.)
- No hot reload of plugin code mid-document.
- No Windows/Linux plugin API commitment. macOS is the first target.

## Open questions

1. Is there a concrete extension request that themes and manifests cannot
   satisfy? Without one, Stage 2 should not start.
2. Should hook budgets be global per document or per plugin? A global budget
   avoids one plugin starving another; per-plugin is easier to explain.
3. Is an OS-level sandbox available without breaking unsigned local builds? If
   not, keep the "isolation, not sandbox" wording.
4. Should export hooks run only from an explicit Save/Export action? Yes, but
   the UI and file-writing path need design.
5. Does the manifest layer need signatures before sharing theme packs? Probably
   not; validated data is low risk. Executable plugins should get a warning
   until signing exists.

## Related

- [Themes](themes.md) — the current, validated extension format.
- [Architecture](architecture.md) — the render pipeline and safety model.
- [Design](design.md) — why the reader stays quiet and small.
