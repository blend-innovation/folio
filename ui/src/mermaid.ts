import mermaid from "mermaid";

let counter = 0;

function cssVar(name: string, fallback: string): string {
  const value = getComputedStyle(document.documentElement)
    .getPropertyValue(name)
    .trim();
  return value || fallback;
}

/**
 * Mermaid is loaded as a separate chunk by the caller, only when a document
 * actually contains a diagram. Keeping it out of app.js preserves boot time
 * for the ordinary prose-and-code case.
 */
let queue: Promise<unknown> = Promise.resolve();

export function renderMermaid(source: string): Promise<string> {
  const run = queue.then(() => renderMermaidNow(source));
  queue = run.catch(() => undefined);
  return run;
}

async function renderMermaidNow(source: string): Promise<string> {
  // Mermaid frontmatter and init directives can override initialize()
  // config, including securityLevel. Keep the security posture and reading
  // theme app-owned by dropping that metadata; the diagram body is unchanged.
  const withoutFrontmatter = source.replace(
    /^\s*---\r?\n[\s\S]*?\r?\n---(?:\r?\n|$)/,
    "",
  );
  const diagram = withoutFrontmatter.replace(
    /%%\{\s*(?:init|initialize)\b[\s\S]*?\}%%/gi,
    "",
  );

  const canvas = cssVar("--f-canvas", "#faf9f6");
  const surface = cssVar("--f-surface", "#ffffff");
  const ink = cssVar("--f-ink", "#1c1c1e");
  const muted = cssVar("--f-muted", "#636366");
  const accent = cssVar("--f-accent", "#5c9c7c");
  const accentStrong = cssVar("--f-accent-strong", "#3f7a5d");
  const rule = cssVar("--f-rule", "#e8e5de");
  const bodyFont = cssVar("--f-font-body", "Manrope, sans-serif");
  const monoFont = cssVar("--f-font-mono", "ui-monospace, monospace");
  const fontSize = cssVar("--f-body-size", "17px");

  mermaid.initialize({
    startOnLoad: false,
    securityLevel: "strict",
    theme: "base",
    fontFamily: bodyFont,
    themeVariables: {
      background: canvas,
      primaryColor: cssVar("--f-accent-soft", "#f1f8f4"),
      primaryTextColor: ink,
      primaryBorderColor: cssVar("--f-accent-line", "#e3f0e8"),
      secondaryColor: cssVar("--f-accent-soft", "#f1f8f4"),
      secondaryTextColor: ink,
      secondaryBorderColor: accent,
      tertiaryColor: cssVar("--f-surface-muted", "#f4f3ee"),
      tertiaryTextColor: ink,
      tertiaryBorderColor: rule,
      lineColor: accentStrong,
      textColor: ink,
      mainBkg: cssVar("--f-accent-soft", "#f1f8f4"),
      nodeBorder: cssVar("--f-accent-line", "#e3f0e8"),
      clusterBkg: cssVar("--f-surface-muted", "#f4f3ee"),
      clusterBorder: rule,
      edgeLabelBackground: canvas,
      actorBkg: surface,
      actorBorder: rule,
      actorTextColor: ink,
      actorLineColor: rule,
      signalColor: muted,
      signalTextColor: ink,
      labelBoxBkgColor: surface,
      labelBoxBorderColor: rule,
      labelTextColor: ink,
      loopTextColor: ink,
      noteBkgColor: cssVar("--f-warm-soft", "#f8edea"),
      noteBorderColor: cssVar("--f-warm", "#c97463"),
      noteTextColor: ink,
      activationBkgColor: cssVar("--f-accent-soft", "#f1f8f4"),
      activationBorderColor: accentStrong,
      sequenceNumberColor: canvas,
      pie1: accent,
      pie2: cssVar("--f-warm", "#c97463"),
      pie3: muted,
      pie4: cssVar("--f-syntax-property", "#5b6f7e"),
      pieTitleTextColor: ink,
      pieSectionTextColor: ink,
      pieLegendTextColor: ink,
      pieStrokeColor: canvas,
      pieOuterStrokeColor: rule,
      fontFamily: bodyFont,
      fontSize,
    },
    flowchart: {
      htmlLabels: false,
      useMaxWidth: true,
      curve: "basis",
    },
    sequence: {
      useMaxWidth: true,
    },
    gantt: {
      useMaxWidth: true,
    },
    themeCSS: `svg { font-family: ${bodyFont}; } .edgeLabel, .label { font-family: ${bodyFont}; } code, pre { font-family: ${monoFont}; }`,
  });

  const id = `folio-mermaid-${++counter}`;
  const { svg } = await mermaid.render(id, diagram);
  return svg;
}
