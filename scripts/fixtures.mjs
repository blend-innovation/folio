import { execFileSync } from "node:child_process";
import { readFile, mkdir, stat, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { homedir } from "node:os";

const root = process.cwd();
const bin = process.env.FOLIO_BIN ?? resolve(root, "target/debug/folio");

if (!existsSync(bin) || process.env.FOLIO_SKIP_BUILD !== "1") {
  execFileSync(process.env.CARGO ?? resolve(homedir(), ".cargo/bin/cargo"), ["build", "-q", "-p", "folio-cli"], { stdio: "inherit" });
}

const run = (args) =>
  execFileSync(bin, args, { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });

const samplePath = resolve(root, "docs/sample.md");
const rendered = JSON.parse(run(["render", samplePath, "--json"]));
const source = await readFile(samplePath);
const info = await stat(samplePath);
const themes = JSON.parse(run(["themes", "json"]));
const version = run(["--version"]).trim().split(" ").pop();

const document = {
  path: ":sample",
  name: "sample.md",
  html: rendered.html,
  meta: rendered.meta,
  size: info.size,
  modifiedMs: 0,
  sample: true,
};

const bootstrap = {
  themes,
  recents: [],
  version,
  initialPath: null,
  initialDocument: document,
  sampleDocument: document,
  themesDir: "~/Library/Application Support/com.folio.reader/themes",
  platform: "web",
};

const outDir = resolve(root, "ui/dist/fixtures");
await mkdir(outDir, { recursive: true });

function benchmarkMarkdown(targetBytes) {
  let markdown = "";
  let section = 1;
  while (markdown.length < targetBytes) {
    markdown += `## Section ${section}\n\nA paragraph with **bold** text, \`inline code\`, and a [link](https://example.com/${section}). The measure should stay calm even when a document grows.\n\n- first point\n- second point\n\n\`\`\`rust\nfn section_${section}() -> usize {\n    ${section}\n}\n\`\`\`\n\n> [!NOTE]\n> A note inside section ${section}.\n\n`;
    section += 1;
  }
  return markdown;
}

let largeMarkdown = "";
let section = 1;
while (largeMarkdown.length < 700_000) {
  largeMarkdown += `## Section ${section}\n\nA paragraph with **bold** text, \`inline code\`, and a [link](https://example.com/${section}). The measure should stay calm even when a document grows.\n\n- first point\n- second point\n\n\`\`\`rust\nfn section_${section}() -> usize {\n    ${section}\n}\n\`\`\`\n\n> [!NOTE]\n> A note inside section ${section}.\n\n`;
  section += 1;
}
const largePath = resolve(root, "ui/dist/fixtures/large.md");
await writeFile(largePath, largeMarkdown);
const largeRendered = JSON.parse(run(["render", largePath, "--json"]));
const large = {
  path: ":large",
  name: "large.md",
  html: largeRendered.html,
  meta: largeRendered.meta,
  size: Buffer.byteLength(largeMarkdown),
  modifiedMs: 0,
  sample: true,
};

const mediumMarkdown = benchmarkMarkdown(120_000);
const mediumPath = resolve(root, "ui/dist/fixtures/medium.md");
await writeFile(mediumPath, mediumMarkdown);
const mediumRendered = JSON.parse(run(["render", mediumPath, "--json"]));
const medium = {
  path: ":medium",
  name: "medium.md",
  html: mediumRendered.html,
  meta: mediumRendered.meta,
  size: Buffer.byteLength(mediumMarkdown),
  modifiedMs: 0,
  sample: true,
};

const featuresMarkdown = `# Preview features

## HTML artifact

\`\`\`html
<!doctype html>
<html>
  <head>
    <style>
      body { margin: 0; padding: 24px; background: #f3efe5; color: #24352c; }
      h1 { font: 600 28px Georgia, serif; }
      p { font: 16px/1.6 sans-serif; }
    </style>
  </head>
  <body>
    <h1>A small HTML page</h1>
    <p>This should render inside a sandboxed preview.</p>
    <a href="https://example.com">Inert preview link</a>
  </body>
</html>
\`\`\`

## Mermaid diagram

\`\`\`mermaid
flowchart LR
  A[Markdown] --> B[Folio]
  B --> C[Diagram]
\`\`\`
`;
const featuresPath = resolve(root, "ui/dist/fixtures/features.md");
await writeFile(featuresPath, featuresMarkdown);
const featuresRendered = JSON.parse(run(["render", featuresPath, "--json"]));
const features = {
  path: ":features",
  name: "features.md",
  html: featuresRendered.html,
  meta: featuresRendered.meta,
  size: Buffer.byteLength(featuresMarkdown),
  modifiedMs: 0,
  sample: true,
};

await writeFile(`${outDir}/bootstrap.json`, JSON.stringify(bootstrap, null, 2));
await writeFile(`${outDir}/sample.md`, source);
await writeFile(`${outDir}/large.json`, JSON.stringify(large));
await writeFile(`${outDir}/medium.json`, JSON.stringify(medium));
await writeFile(`${outDir}/features.json`, JSON.stringify(features));
console.log(
  `fixtures: ${document.meta.stats.words} words, ${themes.themes.length} themes, ${(JSON.stringify(bootstrap).length / 1024).toFixed(0)} KB; medium ${(medium.html.length / 1024).toFixed(0)} KB HTML; large ${(large.html.length / 1024 / 1024).toFixed(1)} MB HTML; features ${features.meta.stats.codeBlocks} code blocks`,
);
