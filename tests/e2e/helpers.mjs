import { createServer } from "node:http";
import { readFile, stat } from "node:fs/promises";
import { extname, join, normalize } from "node:path";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import puppeteer from "puppeteer-core";

const ROOT = process.cwd();
const DIST = join(ROOT, "ui/dist");

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json",
  ".svg": "image/svg+xml",
  ".woff2": "font/woff2",
  ".png": "image/png",
  ".md": "text/markdown; charset=utf-8",
};

export async function startServer() {
  const server = createServer(async (request, response) => {
    try {
      const url = new URL(request.url ?? "/", "http://localhost");
      let pathname = decodeURIComponent(url.pathname);
      if (pathname.endsWith("/")) pathname += "index.html";
      const path = normalize(join(DIST, pathname));
      if (!path.startsWith(DIST)) {
        response.writeHead(403).end("forbidden");
        return;
      }
      const info = await stat(path).catch(() => null);
      if (!info?.isFile()) {
        response.writeHead(404).end("not found");
        return;
      }
      const body = await readFile(path);
      response.writeHead(200, {
        "content-type": TYPES[extname(path)] ?? "application/octet-stream",
        "cache-control": "no-store",
      });
      response.end(body);
    } catch (error) {
      response.writeHead(500).end(String(error));
    }
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();
  return {
    url: `http://127.0.0.1:${port}`,
    close: () => new Promise((resolve) => server.close(resolve)),
  };
}

function chromePath() {
  if (process.env.CHROME_BIN) return process.env.CHROME_BIN;
  const candidates = [
    join(homedir(), ".cache/ms-playwright/chromium-1248/chrome-linux64/chrome"),
    join(homedir(), ".cache/ms-playwright/chromium-1243/chrome-linux64/chrome"),
    "/usr/bin/google-chrome",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
  ];
  const found = candidates.find((path) => existsSync(path));
  if (!found) {
    throw new Error(
      "Chrome was not found. Set CHROME_BIN to a Chrome or Chromium executable for the end-to-end suite.",
    );
  }
  return found;
}

export async function launch({ width = 1280, height = 900, scale = 2 } = {}) {
  const browser = await puppeteer.launch({
    executablePath: chromePath(),
    headless: true,
    args: [
      "--no-sandbox",
      "--font-render-hinting=none",
      "--force-color-profile=srgb",
      "--hide-scrollbars",
      "--disable-lcd-text",
    ],
  });
  const page = await browser.newPage();
  await page.setViewport({ width, height, deviceScaleFactor: scale });
  await page.emulateMediaFeatures([
    { name: "prefers-reduced-motion", value: "reduce" },
    { name: "prefers-color-scheme", value: "light" },
  ]);
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(message.text());
  });
  return { browser, page, errors };
}

export async function openApp(page, url, query = "") {
  await page.goto(`${url}/${query}`, { waitUntil: "networkidle0" });
  await page.waitForSelector('.app[data-state="doc"] .doc-title', {
    timeout: 15000,
  });
  await page.evaluate(() => document.fonts.ready);
  await page.evaluate(
    () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))),
  );
}

export async function screenshot(page, name) {
  const path = join(ROOT, "artifacts/screenshots", name);
  await page.screenshot({ path });
  console.log(`screenshot: ${path}`);
}

export async function scrollToHeading(page, text) {
  await page.evaluate((headingText) => {
    const doc = document.getElementById("doc");
    const heading = Array.from(doc.querySelectorAll("h2, h3")).find(
      (node) => node.textContent.includes(headingText),
    );
    if (!heading) throw new Error(`No heading: ${headingText}`);
    const scroll = document.getElementById("scroll");
    const top =
      heading.getBoundingClientRect().top -
      scroll.getBoundingClientRect().top +
      scroll.scrollTop -
      90;
    scroll.scrollTop = Math.max(0, top);
  }, text);
  await page.evaluate(
    () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))),
  );
}
