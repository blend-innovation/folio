import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { startServer, launch, screenshot, scrollToHeading } from "./helpers.mjs";

execFileSync(process.execPath, ["scripts/build-ui.mjs"], { stdio: "inherit" });
execFileSync(process.execPath, ["scripts/fixtures.mjs"], { stdio: "inherit" });

const server = await startServer();
const { browser, page, errors } = await launch();

async function load(query = "") {
  await page.goto(`${server.url}/${query}`, { waitUntil: "networkidle0" });
  await page.evaluate(() => document.fonts.ready);
  await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
}

async function theme(id) {
  await page.evaluate((value) => window.__folio.theme(value), id);
  await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
}

// Welcome
await load("?welcome=1");
await screenshot(page, "welcome-blend.png");

// Blend: title, code, table
await load();
await screenshot(page, "doc-blend-top.png");
await page.evaluate(() => window.__folio.defaultPrompt());
await page.evaluate(
  () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))),
);
await screenshot(page, "default-prompt.png");
await page.click("#default-no");
await scrollToHeading(page, "Code is part of the prose");
await screenshot(page, "doc-blend-code.png");
await scrollToHeading(page, "Tables, when they earn their place");
await screenshot(page, "doc-blend-table.png");

// Theme panel
await page.evaluate(() => document.getElementById("scroll").scrollTo({ top: 0 }));
await page.evaluate(() => document.getElementById("theme-trigger").click());
await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
await screenshot(page, "theme-panel.png");
await page.evaluate(() => document.getElementById("theme-close").click());

// Blend dark
await theme("blend-dark");
await screenshot(page, "doc-blend-dark.png");
await scrollToHeading(page, "A little colour, used sparingly");
await screenshot(page, "doc-blend-dark-callouts.png");

// Editorial
await page.evaluate(() => document.getElementById("scroll").scrollTo({ top: 0 }));
await theme("editorial");
await screenshot(page, "doc-editorial.png");
await scrollToHeading(page, "Code is part of the prose");
await screenshot(page, "doc-editorial-code.png");

// Editorial dark
await page.evaluate(() => document.getElementById("scroll").scrollTo({ top: 0 }));
await theme("editorial-dark");
await screenshot(page, "doc-editorial-dark.png");

// Studio
await page.evaluate(() => document.getElementById("scroll").scrollTo({ top: 0 }));
await theme("studio");
await screenshot(page, "doc-studio.png");
await scrollToHeading(page, "Tables, when they earn their place");
await screenshot(page, "doc-studio-table.png");

// Studio dark
await page.evaluate(() => document.getElementById("scroll").scrollTo({ top: 0 }));
await theme("studio-dark");
await screenshot(page, "doc-studio-dark.png");

// Find
await page.evaluate(() => window.__folio.find());
await page.type("#find-input", "measure");
await new Promise((r) => setTimeout(r, 250));
await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
await screenshot(page, "find-studio.png");
await page.evaluate(() => document.getElementById("find-close").click());

// Notes states
await load();
await theme("blend");
const features = JSON.parse(
  await readFile(join(process.cwd(), "ui/dist/fixtures/features.json"), "utf8"),
);
await page.evaluate(
  (payload) =>
    window.__folio.render({
      ...payload,
      path: "/Users/ada/notes/previews.md",
      name: "previews.md",
      sample: false,
    }),
  features,
);
await page.waitForFunction(
  () => !document.getElementById("notes-btn").disabled,
);
await page.waitForFunction(
  () => document.getElementById("notes-loading").hidden,
);
await page.click("#notes-btn");
await screenshot(page, "notes-empty.png");

const select = async () =>
  page.evaluate(() => {
    const doc = document.getElementById("doc");
    const walker = document.createTreeWalker(doc, NodeFilter.SHOW_TEXT);
    let node = walker.nextNode();
    while (node && !node.data.includes("Preview features")) {
      node = walker.nextNode();
    }
    if (!node) return;
    const start = node.data.indexOf("Preview features");
    const range = document.createRange();
    range.setStart(node, start);
    range.setEnd(node, start + "Preview features".length);
    const selection = window.getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
    doc.dispatchEvent(new MouseEvent("mouseup", { bubbles: true }));
  });
await select();
await page.waitForSelector("#selection-toolbar:not([hidden])");
await screenshot(page, "notes-selection.png");
await page.click("#selection-note");
await page.type("#note-text", "This passage sets the tone for the preview.");
await screenshot(page, "notes-composer.png");
await page.click("#note-save");
await page.waitForSelector(".note-mark");
await page.click(".note-mark");
await screenshot(page, "notes-list.png");

await theme("blend-dark");
await screenshot(page, "notes-dark.png");
await page.evaluate(() => document.getElementById("notes-close").click());
await theme("blend");

// Mobile blend
await page.setViewport({ width: 430, height: 900, deviceScaleFactor: 2 });
await page.goto(server.url, { waitUntil: "domcontentloaded" });
await page.evaluate(() => localStorage.clear());
await load();
await screenshot(page, "doc-mobile.png");

await page.evaluate(
  (payload) =>
    window.__folio.render({
      ...payload,
      path: "/Users/ada/notes/previews.md",
      name: "previews.md",
      sample: false,
    }),
  features,
);
await page.waitForFunction(
  () => !document.getElementById("notes-btn").disabled,
);
await page.waitForFunction(
  () => document.getElementById("notes-loading").hidden,
);
await page.evaluate(() => document.getElementById("notes-btn").click());
await screenshot(page, "notes-mobile.png");

console.log("errors:", errors);
await browser.close();
await server.close();
