/**
 * HTML previews run inside a sandboxed, networkless iframe. The source is
 * parsed with DOMParser first, so nothing executes while we prepare it, and
 * the wrapper CSP keeps the preview inert even if the author included scripts.
 */
const PREVIEW_CSP = [
  "default-src 'none'",
  "img-src data: blob:",
  "style-src 'unsafe-inline'",
  "font-src data:",
  "media-src data: blob:",
  "form-action 'none'",
  "base-uri 'none'",
].join("; ");

export function previewDocument(source: string): string {
  const parsed = new DOMParser().parseFromString(source, "text/html");

  // These can navigate or relax the document before our CSP is considered.
  for (const node of parsed.querySelectorAll(
    'meta[http-equiv="refresh" i], meta[http-equiv="content-security-policy" i], base',
  )) {
    node.remove();
  }

  // Keep previews inert: a link cannot navigate the frame to the network.
  for (const link of parsed.querySelectorAll("a, area")) {
    link.removeAttribute("href");
    link.removeAttribute("xlink:href");
    link.removeAttributeNS("http://www.w3.org/1999/xlink", "href");
    link.removeAttribute("ping");
  }

  const head = parsed.head;
  const csp = parsed.createElement("meta");
  csp.httpEquiv = "Content-Security-Policy";
  csp.content = PREVIEW_CSP;
  const style = parsed.createElement("style");
  style.textContent = "html, body { min-height: 100%; margin: 0; background: #fff; } html { color-scheme: light; }";
  head.prepend(style);
  head.prepend(csp);

  if (!head.querySelector('meta[name="viewport" i]')) {
    const viewport = parsed.createElement("meta");
    viewport.name = "viewport";
    viewport.content = "width=device-width, initial-scale=1";
    head.append(viewport);
  }
  if (!head.querySelector("meta[charset]")) {
    const charset = parsed.createElement("meta");
    charset.setAttribute("charset", "utf-8");
    head.prepend(charset);
  }

  return `<!doctype html>\n${parsed.documentElement.outerHTML}`;
}

export function mountPreview(
  frame: HTMLIFrameElement,
  source: string,
): () => void {
  const url = URL.createObjectURL(
    new Blob([previewDocument(source)], { type: "text/html" }),
  );
  let revoked = false;
  const revoke = (): void => {
    if (revoked) return;
    revoked = true;
    URL.revokeObjectURL(url);
    frame.removeEventListener("load", revoke);
  };
  const clear = (): void => {
    revoke();
    frame.removeAttribute("src");
  };
  frame.addEventListener("load", revoke);
  frame.src = url;
  return clear;
}
