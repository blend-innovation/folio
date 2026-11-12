import { execFileSync } from "node:child_process";
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

// Mobile blend
await page.setViewport({ width: 430, height: 900, deviceScaleFactor: 2 });
await page.goto(server.url, { waitUntil: "domcontentloaded" });
await page.evaluate(() => localStorage.clear());
await load();
await screenshot(page, "doc-mobile.png");

console.log("errors:", errors);
await browser.close();
await server.close();
