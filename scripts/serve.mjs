import { createServer } from "node:http";
import { readFile, stat } from "node:fs/promises";
import { extname, join, normalize } from "node:path";

const root = join(process.cwd(), "ui/dist");
const port = Number(process.env.PORT ?? 4173);
const types = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json",
  ".svg": "image/svg+xml",
  ".woff2": "font/woff2",
  ".png": "image/png",
  ".md": "text/markdown; charset=utf-8",
};

const server = createServer(async (request, response) => {
  try {
    const url = new URL(request.url ?? "/", "http://localhost");
    let pathname = decodeURIComponent(url.pathname);
    if (pathname.endsWith("/")) pathname += "index.html";
    const path = normalize(join(root, pathname));
    if (!path.startsWith(root)) {
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
      "content-type": types[extname(path)] ?? "application/octet-stream",
      "cache-control": "no-store",
    });
    response.end(body);
  } catch (error) {
    response.writeHead(500).end(String(error));
  }
});

server.listen(port, "127.0.0.1", () => {
  console.log(`Folio preview: http://127.0.0.1:${port}`);
});
