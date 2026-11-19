import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { launch, openApp, screenshot, startServer } from "./helpers.mjs";

const ROOT = process.cwd();

execFileSync(process.execPath, ["scripts/build-ui.mjs"], { stdio: "inherit" });
execFileSync(process.execPath, ["scripts/fixtures.mjs"], { stdio: "inherit" });

const server = await startServer();
const { browser, page, errors } = await launch();
await browser
  .defaultBrowserContext()
  .overridePermissions(server.url, ["clipboard-read", "clipboard-write"]);

let passed = 0;
let failed = 0;
const timings = {};

async function test(name, fn) {
  try {
    await fn();
    passed += 1;
    console.log(`  ✓ ${name}`);
  } catch (error) {
    failed += 1;
    console.error(`  ✗ ${name}`);
    console.error(error);
  }
}

async function reset() {
  await page.goto(server.url, { waitUntil: "domcontentloaded" });
  await page.evaluate(() => localStorage.clear());
  await page.emulateMediaFeatures([
    { name: "prefers-reduced-motion", value: "reduce" },
    { name: "prefers-color-scheme", value: "light" },
  ]);
}

async function openSample() {
  await reset();
  await openApp(page, server.url);
  await page.bringToFront();
}

async function themeOf() {
  return page.evaluate(() => ({
    id: document.documentElement.dataset.theme,
    appearance: document.documentElement.dataset.appearance,
  }));
}

async function injectAxe() {
  await page.addScriptTag({ path: join(ROOT, "node_modules/axe-core/axe.min.js") });
}

async function a11y(label) {
  await injectAxe();
  const violations = await page.evaluate(async () => {
    const result = await axe.run(document, { resultTypes: ["violations"] });
    return result.violations.map((violation) => ({
      id: violation.id,
      impact: violation.impact,
      help: violation.help,
      nodes: violation.nodes.length,
    }));
  });
  const serious = violations.filter(
    (violation) => violation.impact === "serious" || violation.impact === "critical",
  );
  assert.deepEqual(
    serious,
    [],
    `${label} has serious accessibility violations:\n${JSON.stringify(violations, null, 2)}`,
  );
  return violations;
}

console.log("\nFolio end-to-end\n");

await test("welcome presents the app and disables find", async () => {
  await reset();
  await page.goto(`${server.url}/?welcome=1`, { waitUntil: "networkidle0" });
  await page.waitForSelector('.app[data-state="welcome"] .welcome h1');
  const title = await page.$eval(".welcome h1", (node) => node.textContent.trim());
  assert.match(title, /A quiet reader/i);
  assert.equal(
    await page.$eval("#find-btn", (node) => node.disabled),
    true,
  );
  assert.equal(await page.$eval("#doc", (node) => node.innerHTML), "");
  assert.equal(await page.$eval("#theme-name", (node) => node.textContent), "Blend");
  await screenshot(page, "test-welcome.png");
});

await test("a document becomes a designed page", async () => {
  await openSample();
  assert.equal(await page.$eval("#app", (node) => node.dataset.state), "doc");
  assert.match(
    await page.$eval(".doc-title", (node) => node.textContent),
    /The Shape of a Good Document/,
  );
  assert.equal(await page.$$eval(".doc-kicker", (nodes) => nodes.length), 1);
  assert.equal(await page.$$eval(".doc-tags .doc-tag", (nodes) => nodes.length), 3);
  assert.equal(
    await page.$$eval(".doc h2:not(.footnotes-title)", (nodes) => nodes.length),
    6,
  );
  assert.equal(await page.$$eval(".callout", (nodes) => nodes.length), 4);
  assert.equal(await page.$$eval(".code-block", (nodes) => nodes.length), 4);
  assert.equal(await page.$$eval(".table-wrap table", (nodes) => nodes.length), 1);
  assert.equal(await page.$$eval(".footnotes .footnote", (nodes) => nodes.length), 1);
  assert.equal(await page.$$eval(".task-box", (nodes) => nodes.length), 4);
  assert.match(
    await page.$eval("#doc-stats", (node) => node.textContent),
    /words/,
  );
  assert.match(
    await page.$eval("#colophon-theme", (node) => node.textContent),
    /Set in Blend/,
  );
  assert.equal(await page.$$eval("#outline a", (nodes) => nodes.length), 8);
  assert.equal(await page.$eval("#find-btn", (node) => node.disabled), false);
  assert.equal(
    await page.$eval(".page", (node) => getComputedStyle(node).paddingBottom),
    "200px",
  );
  const boot = await page.evaluate(() => {
    const entry = performance.getEntriesByName("folio:boot")[0];
    return entry ? entry.duration : null;
  });
  timings.bootMs = boot;

  const loaded = await page.evaluate(() =>
    performance
      .getEntriesByType("resource")
      .map((entry) => entry.name)
      .filter((name) => /mermaid|flowDiagram|sequenceDiagram/i.test(name)),
  );
  assert.deepEqual(loaded, [], "a plain document loaded Mermaid assets");
});

await test("contents navigates and tracks the section", async () => {
  await openSample();
  await page.$$eval("#outline a", (links) => {
    const link = links.find((node) => node.textContent.includes("Tables"));
    link.click();
  });
  await new Promise((resolve) => setTimeout(resolve, 450));
  const scrollTop = await page.$eval("#scroll", (node) => node.scrollTop);
  assert.ok(scrollTop > 500, `expected the page to scroll, got ${scrollTop}`);
  const active = await page.$eval("#outline a.is-active", (node) => node.textContent);
  assert.match(active, /Tables/);
  const headingTop = await page.$eval(
    "#tables-when-they-earn-their-place",
    (node) => node.getBoundingClientRect().top,
  );
  assert.ok(headingTop > 40 && headingTop < 140, `heading at ${headingTop}`);
});

await test("themes are data and only the active row is checked", async () => {
  await openSample();
  await page.click("#theme-trigger");
  await page.waitForSelector(".theme-row");
  const rows = await page.$$eval(".theme-row", (nodes) =>
    nodes.map((node) => ({
      id: node.dataset.themeId,
      checked: node.getAttribute("aria-checked"),
      checkOpacity: getComputedStyle(
        node.querySelector(".theme-row-check"),
      ).opacity,
    })),
  );
  assert.equal(rows.length, 6);
  assert.equal(rows.filter((row) => row.checked === "true").length, 1);
  assert.equal(rows.find((row) => row.id === "blend").checked, "true");
  assert.equal(rows.find((row) => row.id === "blend").checkOpacity, "1");
  assert.equal(rows.find((row) => row.id === "editorial").checkOpacity, "0");

  await page.$$eval(".theme-row", (nodes) => {
    nodes.find((node) => node.dataset.themeId === "editorial").click();
  });
  await page.evaluate(
    () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))),
  );
  const theme = await themeOf();
  assert.deepEqual(theme, { id: "editorial", appearance: "light" });
  assert.equal(
    await page.evaluate(
      () => getComputedStyle(document.documentElement).getPropertyValue("--f-canvas").trim(),
    ),
    "#F7F2E7",
  );
  assert.match(await page.$eval("#colophon-theme", (node) => node.textContent), /Editorial/);

  await page.reload({ waitUntil: "networkidle0" });
  await page.waitForSelector('.app[data-state="doc"]');
  assert.equal((await themeOf()).id, "editorial");
});

await test("match system follows the counterpart theme", async () => {
  await openSample();
  await page.evaluate(() => window.__folio.theme("editorial"));
  await page.evaluate(() => document.getElementById("theme-trigger").click());
  await page.click("#theme-auto");
  await page.emulateMediaFeatures([
    { name: "prefers-color-scheme", value: "dark" },
    { name: "prefers-reduced-motion", value: "reduce" },
  ]);
  await new Promise((resolve) => setTimeout(resolve, 250));
  let theme = await themeOf();
  assert.deepEqual(theme, { id: "editorial-dark", appearance: "dark" });

  await page.emulateMediaFeatures([
    { name: "prefers-color-scheme", value: "light" },
    { name: "prefers-reduced-motion", value: "reduce" },
  ]);
  await new Promise((resolve) => setTimeout(resolve, 250));
  theme = await themeOf();
  assert.deepEqual(theme, { id: "editorial", appearance: "light" });
});

await test("find wraps matches, steps through them, and cleans up", async () => {
  await openSample();
  await page.keyboard.down("Control");
  await page.keyboard.press("f");
  await page.keyboard.up("Control");
  await page.waitForSelector("#findbar:not([hidden])");
  await page.type("#find-input", "measure");
  await new Promise((resolve) => setTimeout(resolve, 250));
  const marks = await page.$$eval("mark.find-hit", (nodes) => nodes.length);
  assert.ok(marks >= 5, `expected matches, got ${marks}`);
  assert.match(await page.$eval("#find-count", (node) => node.textContent), /^1 \//);
  assert.equal(await page.$$eval("mark.find-hit.is-current", (nodes) => nodes.length), 1);
  await page.keyboard.press("Enter");
  await page.waitForFunction(
    () => document.querySelector("#find-count").textContent.startsWith("2 /"),
  );
  await page.keyboard.press("Escape");
  assert.equal(await page.$$eval("mark.find-hit", (nodes) => nodes.length), 0);
  assert.equal(await page.$eval("#findbar", (node) => node.hidden), true);
});

await test("code blocks can be copied", async () => {
  await openSample();
  await page.bringToFront();
  const copy = await page.$(".code-copy");
  assert.ok(copy);
  await copy.click();
  await page.waitForFunction(
    () => document.querySelector(".code-copy").textContent === "Copied",
  );
  const clipboard = await page.evaluate(() => navigator.clipboard.readText());
  assert.match(clipboard, /pub struct Document|fn main/);
});

await test("the default-app prompt offers Yes, No, and Never", async () => {
  await openSample();
  await page.evaluate(() => window.__folio.defaultPrompt());
  await page.waitForSelector("#default-prompt:not([hidden])");
  assert.equal(
    await page.$$eval("#default-prompt button", (nodes) => nodes.length),
    3,
  );
  assert.equal(await page.evaluate(() => document.activeElement.id), "default-yes");
  await page.keyboard.press("Escape");
  assert.equal(await page.$eval("#default-prompt", (node) => node.hidden), true);
  assert.equal(
    await page.evaluate(() => localStorage.getItem("folio.defaultApp")),
    null,
  );

  await openSample();
  await page.evaluate(() => window.__folio.defaultPrompt());
  await page.waitForSelector("#default-prompt:not([hidden])");
  await page.click("#default-never");
  assert.equal(await page.$eval("#default-prompt", (node) => node.hidden), true);
  assert.equal(
    await page.evaluate(() => localStorage.getItem("folio.defaultApp")),
    "never",
  );
});

await test("text size is adjustable and resets", async () => {
  await openSample();
  const before = await page.$eval(".doc", (node) =>
    Number.parseFloat(getComputedStyle(node).fontSize),
  );
  await page.keyboard.down("Control");
  await page.keyboard.press("=");
  await page.keyboard.up("Control");
  await new Promise((resolve) => setTimeout(resolve, 150));
  const larger = await page.$eval(".doc", (node) =>
    Number.parseFloat(getComputedStyle(node).fontSize),
  );
  assert.ok(larger > before + 0.5, `${before} -> ${larger}`);
  await page.keyboard.down("Control");
  await page.keyboard.press("0");
  await page.keyboard.up("Control");
  await new Promise((resolve) => setTimeout(resolve, 150));
  const reset = await page.$eval(".doc", (node) =>
    Number.parseFloat(getComputedStyle(node).fontSize),
  );
  assert.ok(Math.abs(reset - before) < 0.1, `${before} -> ${reset}`);
});

await test("the rail becomes a drawer on a narrow window", async () => {
  await reset();
  await page.setViewport({ width: 430, height: 900, deviceScaleFactor: 2 });
  await openApp(page, server.url);
  assert.equal(
    await page.$eval(".page", (node) => getComputedStyle(node).paddingBottom),
    "140px",
  );
  assert.equal(await page.evaluate(() => document.getElementById("app").classList.contains("rail-open")), false);
  await page.click("#rail-toggle");
  assert.equal(await page.evaluate(() => document.getElementById("app").classList.contains("rail-open")), true);
  await page.$$eval("#outline a", (links) => links[0].click());
  await new Promise((resolve) => setTimeout(resolve, 350));
  assert.equal(await page.evaluate(() => document.getElementById("app").classList.contains("rail-open")), false);
  await page.setViewport({ width: 1280, height: 900, deviceScaleFactor: 2 });
  await page.emulateMediaFeatures([
    { name: "prefers-reduced-motion", value: "reduce" },
    { name: "prefers-color-scheme", value: "light" },
  ]);
});

await test("hiding the rail leaves the document visible and reversible", async () => {
  await openSample();
  const before = await page.$eval(".stage", (node) => node.getBoundingClientRect().width);
  await page.click("#rail-toggle");

  const hidden = await page.evaluate(() => {
    const stage = document.querySelector(".stage").getBoundingClientRect();
    const toggle = document.getElementById("rail-toggle").getBoundingClientRect();
    return {
      hidden: document.getElementById("app").classList.contains("is-rail-hidden"),
      railDisplay: getComputedStyle(document.getElementById("rail")).display,
      stageWidth: stage.width,
      stageLeft: stage.left,
      toggleWidth: toggle.width,
      docVisible: document.querySelectorAll("#doc .doc-title").length > 0,
    };
  });
  assert.equal(hidden.hidden, true);
  assert.equal(hidden.railDisplay, "none");
  assert.equal(hidden.stageLeft, 0);
  assert.ok(hidden.stageWidth > before, `stage did not expand: ${before} -> ${hidden.stageWidth}`);
  assert.ok(hidden.toggleWidth > 0, "the rail toggle disappeared with the rail");
  assert.equal(hidden.docVisible, true);

  // runAction() guards against accidental double toggles for 90 ms.
  await new Promise((resolve) => setTimeout(resolve, 120));
  await page.click("#rail-toggle");
  const restored = await page.evaluate(() => {
    const stage = document.querySelector(".stage").getBoundingClientRect();
    return {
      hidden: document.getElementById("app").classList.contains("is-rail-hidden"),
      railDisplay: getComputedStyle(document.getElementById("rail")).display,
      stageWidth: stage.width,
    };
  });
  assert.equal(restored.hidden, false);
  assert.notEqual(restored.railDisplay, "none");
  assert.ok(Math.abs(restored.stageWidth - before) < 2, "the rail did not return");
});

await test("HTML and Mermaid snippets can be previewed", async () => {
  await reset();
  await openApp(page, server.url);
  const features = JSON.parse(
    await readFile(join(ROOT, "ui/dist/fixtures/features.json"), "utf8"),
  );
  await page.evaluate((payload) => window.__folio.render(payload), features);
  await page.waitForSelector(".html-block .code-preview-toggle");

  await page.click(".html-block .code-preview-toggle");
  await page.waitForSelector(".html-block .html-preview:not([hidden]) iframe");
  const htmlPreview = await page.evaluate(() => {
    const block = document.querySelector(".html-block");
    const frame = block?.querySelector("iframe");
    const toggle = block?.querySelector(".code-preview-toggle");
    return {
      src: frame?.getAttribute("src") ?? "",
      sandbox: frame?.getAttribute("sandbox"),
      expanded: toggle?.getAttribute("aria-expanded"),
      note: block?.querySelector(".html-preview-note")?.textContent ?? "",
      openButtons: document.querySelectorAll(".code-open-preview:not([hidden])").length,
    };
  });
  assert.match(htmlPreview.src, /^blob:/, "HTML preview did not use a blob URL");
  assert.equal(htmlPreview.sandbox, "", "HTML preview iframe is not sandboxed");
  assert.equal(htmlPreview.expanded, "true");
  assert.match(htmlPreview.note, /Sandboxed preview/);
  assert.equal(htmlPreview.openButtons, 0, "open-window should be native-only");

  const previewFrame = page.frames().find((frame) => frame.url().startsWith("blob:"));
  assert.ok(previewFrame, "HTML preview frame was not loaded");
  const previewContent = await previewFrame.evaluate(() => ({
    title: document.querySelector("h1")?.textContent,
    linkHref: document.querySelector("a")?.getAttribute("href") ?? null,
  }));
  assert.match(previewContent.title ?? "", /small HTML page/);
  assert.equal(previewContent.linkHref, null, "preview links were not made inert");

  await new Promise((resolve) => setTimeout(resolve, 120));
  await page.click(".html-block .code-preview-toggle");
  assert.equal(
    await page.$eval(".html-block .html-preview", (node) => node.hidden),
    true,
  );

  await page.waitForSelector(".mermaid-block .mermaid-diagram svg", {
    timeout: 30000,
  });
  const mermaid = await page.evaluate(() => {
    const block = document.querySelector(".mermaid-block");
    return {
      state: block?.dataset.mermaidState,
      theme: block?.dataset.mermaidTheme,
      sourceHidden: block?.querySelector("pre")?.hidden,
      sourceButtonHidden: block?.querySelector(".code-source-toggle")?.hidden,
      svg: !!block?.querySelector(".mermaid-diagram svg"),
    };
  });
  assert.equal(mermaid.state, "ready");
  assert.equal(mermaid.sourceHidden, true);
  assert.equal(mermaid.sourceButtonHidden, false);
  assert.equal(mermaid.svg, true);

  await page.click(".mermaid-block .code-source-toggle");
  const sourceShown = await page.evaluate(() => {
    const block = document.querySelector(".mermaid-block");
    return {
      sourceHidden: block?.querySelector("pre")?.hidden,
      diagramHidden: block?.querySelector(".mermaid-diagram")?.hidden,
      button: block?.querySelector(".code-source-toggle")?.textContent,
    };
  });
  assert.equal(sourceShown.sourceHidden, false);
  assert.equal(sourceShown.diagramHidden, true);
  assert.equal(sourceShown.button, "Diagram");

  await page.evaluate(() => window.__folio.theme("blend-dark"));
  await page.waitForFunction(
    () =>
      document.querySelector(".mermaid-block")?.dataset.mermaidTheme ===
      "blend-dark",
    { timeout: 30000 },
  );
  assert.equal(
    await page.$eval(".mermaid-block .mermaid-diagram svg", (node) => !!node),
    true,
  );
  await a11y("previews");
});

await test("configuration pins theme, text size, and rail visibility", async () => {
  await reset();
  const base = JSON.parse(
    await readFile(join(ROOT, "ui/dist/fixtures/bootstrap.json"), "utf8"),
  );
  base.config = {
    theme: "editorial",
    matchSystem: false,
    textSize: 1.2,
    showOutline: false,
  };
  await page.setRequestInterception(true);
  const intercept = (request) => {
    if (request.url().endsWith("/fixtures/bootstrap.json")) {
      request.respond({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(base),
      });
    } else {
      request.continue();
    }
  };
  page.on("request", intercept);
  try {
    await openApp(page, server.url);
    const configured = await page.evaluate(() => ({
      theme: document.documentElement.dataset.theme,
      scale: document.documentElement.style.getPropertyValue("--f-body-size"),
      railHidden: document.getElementById("app").classList.contains("is-rail-hidden"),
      stageWidth: document.querySelector(".stage").getBoundingClientRect().width,
    }));
    assert.equal(configured.theme, "editorial");
    assert.match(configured.scale, /1\.2/);
    assert.equal(configured.railHidden, true);
    assert.ok(configured.stageWidth > 1000, `stage width ${configured.stageWidth}`);
  } finally {
    page.off("request", intercept);
    await page.setRequestInterception(false);
  }
});

await test("a 700 KB document renders quickly in the webview", async () => {
  await reset();
  const large = JSON.parse(
    await readFile(join(ROOT, "ui/dist/fixtures/large.json"), "utf8"),
  );
  await openApp(page, server.url);
  const medium = JSON.parse(
    await readFile(join(ROOT, "ui/dist/fixtures/medium.json"), "utf8"),
  );
  await page.evaluate(() => window.__folio.welcome());
  await page.evaluate(
    () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))),
  );
  timings.mediumRenderMs = await page.evaluate(async (payload) => {
    const start = performance.now();
    window.__folio.render(payload);
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
    return performance.now() - start;
  }, medium);
  assert.ok(
    timings.mediumRenderMs < 1200,
    `120 KB document paint took ${timings.mediumRenderMs.toFixed(1)} ms`,
  );

  await page.evaluate(() => window.__folio.welcome());
  await page.evaluate(
    () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))),
  );
  const duration = await page.evaluate(async (payload) => {
    const start = performance.now();
    window.__folio.render(payload);
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
    return performance.now() - start;
  }, large);
  timings.largeRenderMs = duration;
  timings.largeParts = await page.evaluate(() => {
    const at = (name) => performance.getEntriesByName(name).at(-1)?.startTime ?? 0;
    return {
      html: at("folio:html-complete") - at("folio:html-start"),
      decorate: at("folio:decorate-complete") - at("folio:html-complete"),
      enhance: at("folio:enhance-complete") - at("folio:decorate-complete"),
      outline: at("folio:outline-complete") - at("folio:enhance-complete"),
    };
  });
  assert.ok(
    duration < 3500,
    `large document paint took ${duration.toFixed(1)} ms`,
  );
  assert.ok((await page.$$eval(".code-block", (nodes) => nodes.length)) >= 1000);
});

await test("a long outline scrolls inside the rail", async () => {
  await reset();
  await openApp(page, server.url);
  const medium = JSON.parse(
    await readFile(join(ROOT, "ui/dist/fixtures/medium.json"), "utf8"),
  );
  await page.evaluate((payload) => window.__folio.render(payload), medium);
  await page.evaluate(
    () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))),
  );

  const before = await page.evaluate(() => {
    const outline = document.getElementById("outline");
    return {
      count: outline.querySelectorAll("a").length,
      scrollHeight: outline.scrollHeight,
      clientHeight: outline.clientHeight,
      scrollTop: outline.scrollTop,
      railHeight: document.getElementById("rail").clientHeight,
      viewportHeight: window.innerHeight,
      footTop: document.querySelector(".rail-foot").getBoundingClientRect().top,
    };
  });
  assert.ok(before.count > 100, `expected many headings, got ${before.count}`);
  assert.ok(
    before.scrollHeight > before.clientHeight + 200,
    `outline is not scrollable: ${before.scrollHeight} / ${before.clientHeight}`,
  );
  assert.ok(
    before.railHeight <= before.viewportHeight,
    `rail grew past the window: ${before.railHeight} > ${before.viewportHeight}`,
  );

  const after = await page.$eval("#outline", (node) => {
    node.scrollTop = 320;
    return {
      scrollTop: node.scrollTop,
      footTop: document.querySelector(".rail-foot").getBoundingClientRect().top,
    };
  });
  assert.ok(after.scrollTop > 0, "outline scrollTop did not change");
  assert.equal(after.footTop, before.footTop, "the theme footer moved with the outline");
});

await test("welcome, documents, and the theme picker pass axe", async () => {
  await reset();
  await page.goto(`${server.url}/?welcome=1`, { waitUntil: "networkidle0" });
  await page.waitForSelector(".welcome h1");
  const welcomeViolations = await a11y("welcome");

  await openSample();
  const blendViolations = await a11y("blend");
  await page.evaluate(() => window.__folio.defaultPrompt());
  await page.waitForSelector("#default-prompt:not([hidden])");
  const promptViolations = await a11y("default prompt");
  await page.click("#default-no");
  await page.evaluate(() => document.getElementById("theme-trigger").click());
  const panelViolations = await a11y("theme panel");
  await page.evaluate(() => document.getElementById("theme-close").click());

  for (const theme of ["blend-dark", "editorial", "studio"]) {
    await page.evaluate((id) => window.__folio.theme(id), theme);
    await page.evaluate(
      () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))),
    );
    await a11y(theme);
  }

  const total =
    welcomeViolations.length +
    blendViolations.length +
    promptViolations.length +
    panelViolations.length;
  console.log(`    axe violations on core surfaces: ${total}`);
});

console.log("\nResults");
console.log(`  ${passed} passed, ${failed} failed`);
if (timings.bootMs != null) console.log(`  boot to first render: ${timings.bootMs.toFixed(1)} ms`);
if (timings.mediumRenderMs != null)
  console.log(`  120 KB document DOM render: ${timings.mediumRenderMs.toFixed(1)} ms`);
if (timings.largeRenderMs != null) {
  console.log(`  700 KB document DOM render: ${timings.largeRenderMs.toFixed(1)} ms`);
  const parts = timings.largeParts;
  console.log(
    `    html ${parts.html.toFixed(0)} ms · decorate ${parts.decorate.toFixed(0)} ms · code ${parts.enhance.toFixed(0)} ms · outline ${parts.outline.toFixed(0)} ms`,
  );
}

const fatal = errors.filter((error) => !/favicon/i.test(error));
if (fatal.length > 0) {
  console.error("\nPage errors:");
  for (const error of fatal) console.error("  " + error);
  failed += 1;
}

await browser.close();
await server.close();
process.exit(failed > 0 ? 1 : 0);
