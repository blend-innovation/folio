# Performance

Folio is built around one idea: do the expensive work once, in Rust, and give
the WebView a finished document.

## Environment

These numbers were measured on the development machine:

| | |
| --- | --- |
| CPU | Intel Core i5-8500, 6 cores / 6 threads, 3.0 GHz |
| Memory | 15 GB |
| OS | Ubuntu 24.04.5 (Linux 6.8) |
| Rust | stable 1.99, `--release`, thin LTO |
| Browser for UI measurements | Chrome for Testing 156 (headless, 1280×900 @2×) |

The workspace for this run lives on a FUSE-mounted 3 TB disk, so CLI benchmark
corpora are generated under the NVMe-backed system temp directory instead of the
repository. The core benchmark is entirely in memory.

The engine targets macOS, but the rendering, DOM, and UI code are identical.
macOS WebKit will differ in absolute numbers; the shape of the curve should not.
The native app's cold-launch time was not measured here because the bundle can
only be built on macOS. Build it with `scripts/build-macos.sh` and measure with
`open -W` / Instruments if you need that number.

## Markdown rendering (Rust)

Criterion, release build. Corpus is a mix of prose, headings, lists, callouts,
tables, and highlighted Rust code.

| Benchmark | Median | Throughput |
| --- | ---: | ---: |
| 8 KB document | 0.128 ms | 62 MiB/s |
| 100 KB document | 1.55 ms | 63 MiB/s |
| 1 MB document | 19.8 ms | 51 MiB/s |
| 1 MB, highlighting disabled | 18.0 ms | 55 MiB/s |
| Highlight 512 KB of Rust | 6.47 ms | 77 MiB/s |

A 1 MB Markdown file becomes a complete HTML fragment with an outline, word
count, syntax spans, callouts, and footnotes in about 20 ms. Syntax
highlighting is not the bottleneck: disabling it saves roughly 9%.

## Command line wall clock

`bench/run.mjs` measures the release CLI from process spawn to exit, including
writing the HTML file. This is deliberately pessimistic: the desktop app already
has the renderer warm in memory and does not spawn a process.

| Measurement | p50 |
| --- | ---: |
| 1 MB Markdown → 2.7 MB HTML | 55.4 ms |
| 4 MB Markdown → 10.9 MB HTML | 119.4 ms |
| small files (8–100 KB) | roughly 4–15 ms |
| `folio --version` process baseline | 3–7 ms |

The 1 MB and 4 MB rows are the meaningful ones: smaller runs are dominated by
process spawn and OS scheduling, which is why an 8 KB file can appear slower
than a 100 KB one. The desktop app does not spawn a process per document; these
numbers are an upper bound for the CLI path.

## The UI

The webview receives one HTML string and sets it as one fragment. The article is
not built node by node, and there is no Markdown parsing or highlighting in
JavaScript.

| Measurement | Median |
| --- | ---: |
| Module start → first document render (sample, ~5 KB) | 34 ms (p95 49 ms) |
| Sample document, render + paint | 32 ms |
| Render + paint, 120 KB Markdown (393 KB HTML) | 360 ms (p95 383 ms) |
| Render + paint, 700 KB Markdown (2.2 MB HTML) | 1.08 s (p95 1.26 s) |

These are medians of five runs for the sample boot and three for each injected
document, measured in headless Chrome on a busy development machine, with the
browser fully idle between runs. The sample is what most reading sessions look
like. 700 KB is a stress case — roughly the size of a small book — and the DOM,
not the parser, dominates it.

Two optimizations matter for very large documents:

1. **Deferred code enhancement.** Only code blocks within 700 px of the viewport
   are measured for the “show more lines” affordance. This removed about 700 ms
   from the 2.2 MB stress document.
2. **One fragment, one style pass.** The renderer emits final classes and spans;
   the UI does not walk or re-style code after insertion.

The outline for a 1,100-section document is built in about 10 ms.

## Bundle and app size

| Asset | Raw | Gzip |
| --- | ---: | ---: |
| `app.js` (minified ES module) | 31 KB | 10.3 KB |
| `app.css` (minified, all themes) | 36 KB | 8.2 KB |
| Manrope + Newsreader WOFF2 subsets | 504 KB | — |
| UI total before app binary | ~0.6 MB | ~18.5 KB JS+CSS |
| Release shell binary (Linux measurement) | 7.5 MB | — |
| Native webview | system WKWebView | — |

There is no bundled Chromium, no React, no Markdown parser, and no highlighter
in the frontend. The macOS app uses the system WKWebView. The release binary
figure is from the Linux validation build (`--release`, LTO, stripped); the
macOS binary will differ slightly but the dependencies and UI assets are the
same, so expect the same order of magnitude.

## Reproduce

```sh
# engine (criterion; ~1 minute)
pnpm bench

# release CLI wall clock; writes bench/results.json
pnpm bench:cli

# UI, accessibility, and large-document timing against real Chrome
pnpm test:e2e

# screenshots for visual review
pnpm screenshots
```

Raw criterion output is kept in `bench/criterion.txt`. CLI results are written
to `bench/results.json`; UI paint medians to `bench/ui.json`.

## Reading the numbers

- **Throughput over 50 MiB/s** for the core means a normal agent-generated note
  (2–50 KB) is parsed and highlighted in well under a millisecond.
- **Startup to first paint in double-digit milliseconds** is what makes Folio
  feel like a native app rather than a page load.
- **Extreme documents** are bounded by DOM insertion and layout. If you need to
  read a 2 MB Markdown file regularly, Folio remains usable, but the first paint
  is a second-scale operation. The renderer itself is not the limit.
