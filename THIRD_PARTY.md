# Third-party notices

## Typefaces

Folio bundles subsets of the following fonts as WOFF2. Both are licensed under
the SIL Open Font License 1.1; the full texts are included next to the font
files in `ui/assets/licenses/`.

- **Manrope** — Copyright 2018 The Manrope Project Authors
  (https://github.com/sharanda/manrope)
- **Newsreader** — Copyright 2020 The Newsreader Project Authors
  (https://github.com/productiontype/Newsreader)

## Design language

The default Blend theme adapts the visual tokens of the Blend project: cream,
ink, sage, terracotta, Newsreader, and Manrope. It is an independent
implementation for document reading, not a redistribution of Blend's code.

## Rust and JavaScript dependencies

- [pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark) — Markdown
  parsing (MIT).
- [Tauri](https://tauri.app) and its official plugins (MIT / Apache-2.0).
- [notify](https://github.com/notify-rs/notify) — file events (MIT/CC0).
- [serde](https://serde.rs), [percent-encoding](https://github.com/servo/rust-url),
  [base64](https://github.com/marshallpierce/rust-base64) (MIT/Apache-2.0).
- [toml](https://github.com/toml-rs/toml) — configuration file parsing
  (MIT/Apache-2.0).
- [Mermaid](https://mermaid.js.org) — diagram rendering (MIT). Mermaid is
  loaded on demand and bundles its own dependency tree, including D3 (ISC),
  dagre-d3-es (MIT), DOMPurify (MPL-2.0/Apache-2.0), KaTeX (MIT), Cytoscape
  (MIT), marked (MIT), and roughjs (MIT). Exact resolved versions are recorded
  in `pnpm-lock.yaml`.
- [esbuild](https://esbuild.github.io), [TypeScript](https://www.typescriptlang.org),
  [Puppeteer](https://pptr.dev), [axe-core](https://github.com/dequelabs/axe-core)
  — development and verification only (MIT/Apache-2.0/MPL-2.0).

The core reader UI is a single compiled ES module plus a small preview page.
Mermaid runs in the webview only after a document contains a diagram and the
block approaches the viewport. Its MIT license is reproduced below.

## Mermaid license

Copyright (c) 2014-present, Knut Sveidqvist

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
