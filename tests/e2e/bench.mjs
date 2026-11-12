import { execFileSync } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { launch, openApp, startServer } from "./helpers.mjs";

const ROOT = process.cwd();

execFileSync(process.execPath, ["scripts/build-ui.mjs"], { stdio: "inherit" });
execFileSync(process.execPath, ["scripts/fixtures.mjs"], { stdio: "inherit" });

const server = await startServer();
const { browser, page, errors } = await launch();
await page.setViewport({ width: 1280, height: 900, deviceScaleFactor: 1 });
await page.emulateMediaFeatures([
  { name: "prefers-reduced-motion", value: "reduce" },
  { name: "prefers-color-scheme", value: "light" },
]);

function median(values) {
  const sorted = [...values].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
}

function percentile(values, p) {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.ceil((p / 100) * sorted.length) - 1)];
}

async function breathe() {
  await page.evaluate(
    () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))),
  );
}

async function renderPaints(payload) {
  await page.evaluate(() => window.__folio.welcome());
  await breathe();
  return page.evaluate(async (doc) => {
    const start = performance.now();
    window.__folio.render(doc);
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
    return performance.now() - start;
  }, payload);
}

const samples = [];
const boot = [];

await openApp(page, server.url);
for (let i = 0; i < 5; i += 1) {
  await page.reload({ waitUntil: "networkidle0" });
  await page.waitForSelector('.app[data-state="doc"]');
  await breathe();
  const entry = await page.evaluate(() => {
    const measure = performance.getEntriesByName("folio:boot")[0];
    return measure ? measure.duration : null;
  });
  if (entry != null) boot.push(entry);
}

const medium = JSON.parse(
  await readFile(join(ROOT, "ui/dist/fixtures/medium.json"), "utf8"),
);
const large = JSON.parse(
  await readFile(join(ROOT, "ui/dist/fixtures/large.json"), "utf8"),
);

for (const [label, payload] of [
  ["sample", null],
  ["120 KB", medium],
  ["700 KB", large],
]) {
  const times = [];
  for (let i = 0; i < (payload ? 3 : 5); i += 1) {
    await openApp(page, server.url);
    if (payload) times.push(await renderPaints(payload));
    else {
      await breathe();
      times.push(
        await page.evaluate(() => {
          const measure = performance.getEntriesByName("folio:boot")[0];
          return measure ? measure.duration : 0;
        }),
      );
    }
  }
  samples.push({ label, times });
}

const results = samples.map(({ label, times }) => ({
  label,
  runs: times.length,
  medianMs: median(times),
  p95Ms: percentile(times, 95),
  minMs: Math.min(...times),
  maxMs: Math.max(...times),
}));

const output = {
  generatedAt: new Date().toISOString(),
  host: `${process.platform}/${process.arch}`,
  browser: await browser.version(),
  viewport: "1280×900",
  bootMedianMs: median(boot),
  results,
};
await writeFile(join(ROOT, "bench/ui.json"), JSON.stringify(output, null, 2));

console.log("UI performance (headless Chrome, 1280×900, reduced motion)");
console.log(`  boot → first render (sample): ${output.bootMedianMs.toFixed(1)} ms`);
for (const row of results) {
  console.log(
    `  ${row.label.padEnd(7)} paint: median ${row.medianMs.toFixed(1)} ms · p95 ${row.p95Ms.toFixed(1)} ms · min ${row.minMs.toFixed(1)} ms`,
  );
}
if (errors.length) console.error("page errors:", errors);
console.log("wrote bench/ui.json");

await browser.close();
await server.close();
