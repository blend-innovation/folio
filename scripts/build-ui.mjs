import { context, build } from "esbuild";
import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";

const watch = process.argv.includes("--watch");
const outdir = "ui/dist";

async function copyStatic() {
  await mkdir(outdir, { recursive: true });
  await cp("ui/assets", `${outdir}/assets`, { recursive: true });
  const html = await readFile("ui/index.html", "utf8");
  await writeFile(`${outdir}/index.html`, html);
  const preview = await readFile("ui/preview.html", "utf8");
  await writeFile(`${outdir}/preview.html`, preview);
}

await rm(outdir, { recursive: true, force: true });
await copyStatic();

const options = {
  entryPoints: {
    app: "ui/src/main.ts",
    preview: "ui/src/preview.ts",
  },
  outdir,
  entryNames: "[name]",
  chunkNames: "chunks/[name]-[hash]",
  bundle: true,
  splitting: true,
  format: "esm",
  target: ["safari15", "chrome105"],
  minify: !watch,
  sourcemap: watch ? "inline" : false,
  legalComments: "none",
  external: ["/assets/*"],
  logLevel: "info",
};

if (watch) {
  const ctx = await context(options);
  await ctx.watch();
  console.log("UI built; watching for changes");
} else {
  await build(options);
  if (!existsSync(`${outdir}/assets/fonts`)) {
    throw new Error("fonts were not copied");
  }
  console.log("UI built");
}
