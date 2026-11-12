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
- [esbuild](https://esbuild.github.io), [TypeScript](https://www.typescriptlang.org),
  [Puppeteer](https://pptr.dev), [axe-core](https://github.com/dequelabs/axe-core)
  — development and verification only (MIT/Apache-2.0/MPL-2.0).

No third-party code runs in the webview at runtime; the shipped UI is a single
ES module compiled from the repository's TypeScript.
