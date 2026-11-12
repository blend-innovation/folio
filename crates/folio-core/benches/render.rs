use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use folio_core::highlight::highlight;
use folio_core::{render, RenderOptions};
use std::hint::black_box;
use std::time::Duration;

/// A realistic agent document: prose, headings, lists, callouts, tables and
/// code. Built once per size so the benchmark measures rendering, not string
/// construction.
fn corpus(target_bytes: usize) -> String {
    let mut out = String::with_capacity(target_bytes + 2048);
    out.push_str("# Benchmark Document\n\nA short opening paragraph for the measure.\n\n");
    let mut section = 0usize;
    while out.len() < target_bytes {
        section += 1;
        out.push_str(&format!(
            "## Section {section}\n\n\
             Prose with **strong emphasis**, *italic*, `inline_code()`, and a \
             [link](https://example.com/{section}). A second sentence gives the \
             paragraph enough length to wrap naturally at the reading measure.\n\n\
             - first item with a little detail\n\
             - second item with `code`\n\
             - third item\n\n\
             > A quiet blockquote about the shape of a page.\n\n\
             > [!NOTE]\n> A note worth keeping in the margin.\n\n\
             | Decision | Cost | Benefit |\n|:--|--:|:--|\n| Measure | low | high |\n| Line height | low | high |\n\n\
             ```rust\nfn section_{section}(input: &str) -> usize {{\n    let words = input.split_whitespace().count();\n    words + {section}\n}}\n```\n\n"
        ));
    }
    out
}

fn bench_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("render");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(20);
    for (label, size) in [
        ("8kb", 8 * 1024),
        ("100kb", 100 * 1024),
        ("1mb", 1024 * 1024),
    ] {
        let source = corpus(size);
        group.throughput(Throughput::Bytes(source.len() as u64));
        group.bench_function(label, |b| {
            b.iter(|| {
                let rendered = render(black_box(&source), &RenderOptions::default());
                black_box(rendered.html.len());
            })
        });
    }
    group.finish();
}

fn bench_highlight(c: &mut Criterion) {
    let code = corpus(100 * 1024);
    let rust = code
        .split("```rust\n")
        .nth(1)
        .and_then(|part| part.split("\n```").next())
        .unwrap_or("fn main() {}");
    // Make a larger sample so the measurement is stable.
    let mut sample = String::with_capacity(1024 * 1024);
    while sample.len() < 512 * 1024 {
        sample.push_str(rust);
        sample.push('\n');
    }
    let mut group = c.benchmark_group("highlight");
    group.throughput(Throughput::Bytes(sample.len() as u64));
    group.bench_function("rust_512kb", |b| {
        b.iter(|| {
            let html = highlight(black_box(&sample), "rust").unwrap();
            black_box(html.len());
        })
    });
    group.finish();
}

fn bench_no_highlight(c: &mut Criterion) {
    let source = corpus(1024 * 1024);
    let options = RenderOptions {
        highlight: false,
        ..RenderOptions::default()
    };
    let mut group = c.benchmark_group("render_1mb");
    group.sample_size(10);
    group.throughput(Throughput::Bytes(source.len() as u64));
    group.bench_function("no_highlight", |b| {
        b.iter(|| {
            let rendered = render(black_box(&source), &options);
            black_box(rendered.html.len());
        })
    });
    group.finish();
}

criterion_group!(benches, bench_sizes, bench_highlight, bench_no_highlight);
criterion_main!(benches);
