# Architecture

Folio is three small programs in one repository:

```text
┌────────────────────────────┐
│ Markdown file              │
└─────────────┬──────────────┘
              │
┌─────────────▼──────────────┐      ┌──────────────────────────────┐
│ folio-core (Rust)          │      │ folio (CLI)                  │
│ frontmatter + Markdown     │◀─────│ render / themes              │
│ highlighter + theme engine │      └──────────────────────────────┘
└─────────────┬──────────────┘
              │  HTML + metadata (one IPC value)
┌─────────────▼──────────────┐
│ folio-app (Tauri shell)    │
│ window · menu · dialogs    │
│ watcher · recents · images │
└─────────────┬──────────────┘
              │  invoke / events
┌─────────────▼──────────────┐
│ ui (vanilla TypeScript)    │
│ one article · theme tokens │
│ rail · find · chrome       │
└────────────────────────────┘
```

## Why these pieces

**Markdown belongs in Rust.** Parsing once in a native process keeps the
webview's job simple: receive a string, set it as HTML, and style it. There is
no Markdown parser, no syntax-highlighting bundle, and no framework in the UI.
A 1 MB document is rendered by `folio-core` in roughly 20 ms on the development
machine; the browser's DOM work is the remaining cost for very large files.

**The shell belongs to the OS.** Tauri gives Folio a real `.app` bundle, a
native menu, native file dialogs, drag and drop, and the system WebView. There
is no bundled Chromium and no Node runtime in the shipped app.

**The UI is one page.** A single HTML file, a single stylesheet, and a single
ES module. The article is replaced as one fragment. Everything around it —
rail, chrome, find, theme picker — is small, dependency-free DOM code.

## The rendering pipeline

`folio_core::render(source, options)`:

1. **Normalise line endings.** CRLF and lone CR become LF.
2. **Read frontmatter.** A leading `---` block is parsed into title,
   description, author, date, and tags. The block is removed from the body.
3. **Parse with pulldown-cmark** using tables, strikethrough, task lists,
   footnotes, GFM blockquote callouts, and smart punctuation.
4. **Render events into one `String`.** A small stack of buffers captures only
   what cannot be streamed:
   - headings (to compute a slug before opening the tag),
   - paragraphs (to detect an image-only paragraph and turn it into a figure),
   - links (to classify the destination),
   - images (to collect alt text),
   - code blocks (to highlight after the fence closes),
   - footnote definitions (to move them to the end of the document).
5. **Collect metadata** in the same pass: outline, word count, reading time,
   code/table/link/image counts, and a fallback description.
6. **Append footnotes** as a `<section class="footnotes">`, then return
   `{ html, meta }`.

Rendering is allocation-conscious: text is escaped directly into the output,
escape functions return `Cow::Borrowed` when there is nothing to escape, and
the highlighter writes spans into the same output. The renderer never builds an
intermediate AST.

### HTML contract

The UI relies on the renderer's markup:

| Markup | Meaning |
| --- | --- |
| `.doc-title[data-doc-title]` | First H1; the UI inserts the kicker and tags around it. |
| `p.lead` | First paragraph after the title. |
| `h1`–`h6[id]` + `.heading-anchor` | Stable outline targets. |
| `.code-block[data-lang]` | Header, language label, copy button, scrollable `pre`. |
| `.callout[data-kind]` | GFM alert. |
| `.document` figures: `figure.doc-figure > img + figcaption` | Standalone image. |
| `img[data-local-image]` | Local image the host should read and inline. |
| `a[data-external]` | Open in the system browser. |
| `a[data-open-path][data-link]` | Open a local document/file via the host. |
| `.footnotes > .footnote[id]` | Footnote section, collected at the end. |
| `.task-box[data-checked]` | Task list marker. |

Anything not in this table is ordinary prose markup and can be styled freely.

## Safety model

Folio reads files that an agent wrote. It treats them as untrusted input.

- **Raw HTML is never parsed as HTML.** pulldown-cmark's `Html` and
  `InlineHtml` events are escaped. HTML blocks are shown as source in a
  `<pre class="raw-html">`.
- **Links are classified.** `http`, `https`, `ftp`, `mailto`, and `tel` are
  allowed. `javascript:`, `vbscript:`, `data:`, `file:`, and unknown schemes
  become non-clickable `.link-blocked` spans. Whitespace and control characters
  are removed before the scheme check, so `java\nscript:` cannot slip through.
- **Attributes are escaped** with a dedicated function that also escapes
  quotes, newlines, CR, and tabs.
- **Images are classified separately.** Local paths become host-read data
  attributes; remote images must be http(s); data images are limited to raster
  formats and 8 MB; SVG data is rejected.
- **Local paths are normalised lexically** and only resolved when the desktop
  shell opts in. Traversal above the root is clamped.
- **Themes are validated token by token.** Colours are parsed as colours, font
  stacks reject CSS punctuation and `url(`, and lengths must match supported
  units and ranges. Theme CSS is generated by Rust, not concatenated from user
  input.
- **Local images are MIME-sniffed** in the shell; a file with a `.png`
  extension is not enough. Only PNG, JPEG, GIF, WebP, and AVIF are inlined, and
  the file is capped at 24 MB.

See `crates/folio-core/tests/render.rs` for the XSS regression suite.

## IPC contract

The webview talks to the shell through twelve commands and a handful of events.

| Command | Result |
| --- | --- |
| `bootstrap` | Theme catalog + CSS, recents, version, initial file path, themes folder, platform. |
| `render_document { path }` | `DocumentPayload` for a file; starts the watcher and records the recent. |
| `open_file_dialog` | Shows the native picker; renders the selected file or returns `null`. |
| `open_sample` | Renders the bundled sample document without a watcher. |
| `read_image { path }` | A raster image as a `data:` URL, or an error. |
| `open_external { url }` | Hands an allowed URL to the OS. |
| `open_path { path }` | Opens a local file in its default app. |
| `reveal_themes` | Opens the user theme folder. |
| `markdown_default_status` | Whether the bundled app is currently the `.md` handler. |
| `set_markdown_default` | Makes Folio the LaunchServices handler for `.md` / `.markdown`. |
| `set_title { title }` | Updates the native window title. |
| `show_window` | Reveals the window after the first paint. |

| Event | Direction | Payload |
| --- | --- | --- |
| `folio:document-update` | shell → UI | Full `DocumentPayload` after a watched file changes. |
| `folio:document-error` | shell → UI | `{ message, path? }`. |
| `folio:open-document` | shell → UI | `{ path }` from Finder/Open With/file association. |
| `folio:menu` | shell → UI | `{ action }` from a native menu item. |
| `tauri://drag-enter` / `drag-leave` / `drag-drop` | Tauri → UI | Drag state and dropped paths. |

All payloads are serde `camelCase` structs mirrored by `ui/src/types.ts`.

### Default Markdown app

On the first real Markdown file in a session, the UI asks whether Folio should
become the default `.md` reader. The shell resolves the LaunchServices UTI for
the extension, compares its role handler to the bundle identifier, and offers a
theme-aware three-button prompt only when the app is running from a real `.app`
bundle and is not already the handler.

- **Yes** calls `LSSetDefaultRoleHandlerForContentType` for `.md` and makes a
  best-effort attempt for `.markdown`; the decision is remembered.
- **No** dismisses the prompt for this session; it may return next launch.
- **Never show again** is stored in the webview's `localStorage` and suppresses
  later offers.

No prompt appears for `.txt`, the built-in sample, or a `cargo run` development
binary. Users can always change the association in Finder → Get Info.

### Document payload

```ts
interface DocumentPayload {
  path: string;        // canonical absolute path, "" for the sample
  name: string;
  html: string;        // the article fragment
  meta: DocumentMeta;  // title, description, outline, stats, frontmatter
  size: number;        // bytes
  modifiedMs: number;  // file mtime, 0 for the sample
  sample: boolean;
}
```

## File watching

The shell watches the **parent directory**, not the file. Editors that save
atomically write a temporary file and rename it over the original, which would
break a watch on the file itself. Events are filtered to the current document,
debounced for 160 ms, given 60 ms to settle, and then re-rendered. A failed read
is retried once after 90 ms. A new document replaces the watcher; dropping the
watcher closes its event channel and stops the thread.

The UI preserves scroll position by ratio when the same path is re-rendered.
Opening a sample stops the watcher.

## Window and chrome

- macOS title bar is `Overlay` with a hidden title and traffic lights at
  `(18, 22)`. The rail reserves the top 50 px for the lights and drag region.
- The top bar is a translucent cream surface with a blur fallback; a hairline
  appears only after the document scrolls.
- Window geometry is persisted by `tauri-plugin-window-state`; visibility is
  not, so the first render always controls when the window appears.
- On macOS, closing the window hides it. Clicking the Dock icon or choosing a
  menu action brings it back; quit from the app menu.
- The window is created hidden and revealed by `show_window` after the first
  render. A 1.4 s fallback shows it even if the frontend fails to boot.
- On narrow windows the rail becomes a drawer with a scrim; on wide windows it
  can be collapsed. The top bar gains extra left padding under the traffic
  lights on macOS.

## Preferences and storage

The app stores only:

```text
~/Library/Application Support/com.folio.reader/
├── recents.json       # last 10 file paths, pruned if missing
├── themes/            # user theme JSON
└── window-state.json  # managed by the window-state plugin
```

The webview stores theme choice, automatic-appearance preference, and text size
in `localStorage`. Document content is never copied into app storage.

## CLI

`folio-cli` links `folio-core` and exposes the same rendering path:

```sh
folio render FILE [--json] [--out FILE] [--no-highlight] [--localize]
folio themes list [--dir DIR]
folio themes json [--dir DIR]
folio themes scaffold ID
```

This is how the end-to-end fixtures are generated, and it keeps the engine
usable without the app.

## Testing

| Layer | What is covered |
| --- | --- |
| `folio-core` unit tests | escaping, slugs, frontmatter, path classification, theme validation, highlighter families |
| `folio-core` integration | headings, callouts, tables, tasks, figures, footnotes, links, raw HTML, XSS, stats, Unicode, CRLF |
| `folio-cli` integration | stdout, JSON, `--out`, stdin, errors, theme listing/scaffolding |
| `tests/e2e` | real Chrome: render, outline, scroll spy, themes, auto appearance, find, clipboard, text size, default-app prompt, responsive rail, axe, large-document timing |
| criterion | render and highlight throughput across 8 KB/100 KB/1 MB plus no-highlight |
| `bench/run.mjs` | release CLI wall clock and HTML growth, including process startup |
