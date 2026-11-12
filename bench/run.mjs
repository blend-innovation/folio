import { execFileSync, spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync, statSync, rmSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join, resolve } from "node:path";

const ROOT = process.cwd();
const CARGO = process.env.CARGO ?? resolve(homedir(), ".cargo/bin/cargo");
const BIN = resolve(ROOT, "target/release/folio");
const CORPUS = join(tmpdir(), "folio-corpus");
const OUT = "/dev/null";
const SIZE_OUT = join(tmpdir(), "folio-bench-size.html");

mkdirSync(CORPUS, { recursive: true });

function corpus(targetBytes) {
  let out =
    "# Benchmark Document\n\nA short opening paragraph for the measure.\n\n";
  let section = 0;
  while (out.length < targetBytes) {
    section += 1;
    out += `## Section ${section}\n\nProse with **strong emphasis**, *italic*, \`inline_code()\`, and a [link](https://example.com/${section}). A second sentence gives the paragraph enough length to wrap naturally at the reading measure.\n\n- first item with a little detail\n- second item with \`code\`\n- third item\n\n> A quiet blockquote about the shape of a page.\n\n> [!NOTE]\n> A note worth keeping in the margin.\n\n| Decision | Cost | Benefit |\n|:--|--:|:--|\n| Measure | low | high |\n| Line height | low | high |\n\n\`\`\`rust\nfn section_${section}(input: &str) -> usize {\n    let words = input.split_whitespace().count();\n    words + ${section}\n}\n\`\`\`\n\n`;
  }
  return out;
}

function median(values) {
  const sorted = [...values].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
}

function percentile(values, p) {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.ceil((p / 100) * sorted.length) - 1)];
}

function runOnce(file) {
  const start = process.hrtime.bigint();
  const result = spawnSync(BIN, ["render", file, "--out", OUT], {
    stdio: ["ignore", "ignore", "pipe"],
    encoding: "utf8",
    maxBuffer: 256 * 1024 * 1024,
  });
  const elapsed = Number(process.hrtime.bigint() - start) / 1e6;
  if (result.status !== 0) {
    throw new Error(result.stderr || `folio exited ${result.status}`);
  }
  return elapsed;
}

console.log("building folio-cli (release)…");
execFileSync(CARGO, ["build", "--release", "-q", "-p", "folio-cli"], {
  cwd: ROOT,
  stdio: "inherit",
});

// Process startup baseline.
const startup = [];
for (let i = 0; i < 12; i += 1) {
  const start = process.hrtime.bigint();
  execFileSync(BIN, ["--version"], { stdio: "ignore" });
  startup.push(Number(process.hrtime.bigint() - start) / 1e6);
}

const sizes = [
  ["8 KB", 8 * 1024, 30],
  ["100 KB", 100 * 1024, 20],
  ["1 MB", 1024 * 1024, 10],
  ["4 MB", 4 * 1024 * 1024, 5],
];

const results = [];
for (const [label, bytes, iterations] of sizes) {
  const file = join(CORPUS, `bench-${bytes}.md`);
  const source = corpus(bytes);
  writeFileSync(file, source);
  const markdownBytes = statSync(file).size;

  for (let i = 0; i < 2; i += 1) runOnce(file);

  const times = [];
  for (let i = 0; i < iterations; i += 1) times.push(runOnce(file));
  const sizeRun = spawnSync(BIN, ["render", file, "--out", SIZE_OUT], {
    stdio: ["ignore", "ignore", "pipe"],
    encoding: "utf8",
    maxBuffer: 256 * 1024 * 1024,
  });
  if (sizeRun.status !== 0) throw new Error(sizeRun.stderr || "size run failed");
  const htmlBytes = statSync(SIZE_OUT).size;
  const p50 = median(times);
  const p95 = percentile(times, 95);
  const row = {
    label,
    markdownBytes,
    htmlBytes,
    iterations,
    ms: {
      min: Math.min(...times),
      p50,
      p95,
      max: Math.max(...times),
      mean: times.reduce((a, b) => a + b, 0) / times.length,
    },
    throughputMbPerSec: markdownBytes / 1024 / 1024 / (p50 / 1000),
  };
  results.push(row);
  console.log(
    `${label.padEnd(7)} md ${(markdownBytes / 1024).toFixed(0).padStart(5)} KB · html ${(htmlBytes / 1024).toFixed(0).padStart(5)} KB · ` +
      `p50 ${p50.toFixed(1).padStart(6)} ms · p95 ${p95.toFixed(1).padStart(6)} ms · ${row.throughputMbPerSec.toFixed(1)} MB/s`,
  );
}

const output = {
  generatedAt: new Date().toISOString(),
  host: `${process.platform}/${process.arch}`,
  node: process.version,
  startupP50Ms: median(startup),
  startupP95Ms: percentile(startup, 95),
  results,
};
writeFileSync(resolve(ROOT, "bench/results.json"), JSON.stringify(output, null, 2));
console.log(`\nstartup baseline p50 ${output.startupP50Ms.toFixed(1)} ms (process spawn + CLI)`);
console.log("wrote bench/results.json");
rmSync(SIZE_OUT, { force: true });
