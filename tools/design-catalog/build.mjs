// デザインシステムの正本（specs/design/）から部品のカタログを組み立てる。
// 色・書体・余白・角丸は tokens.yaml、部品の見本は components.yaml の preview と preview_css、コピーは concept_tone.yaml の copy。
// 正本だけを読み、手で書いた見本を持たない。Web フォントは読まない（書体は tokens.yaml の typography.families の sans と numeric の名前で指定するだけ）。
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { parse } from "yaml";

const here = dirname(fileURLToPath(import.meta.url));
export const SPEC_DIR = resolve(here, "../../specs/design");
export const OUT_DIR = join(SPEC_DIR, "catalog");

/** 正本の YAML を読む。 */
export function readSpec(specDir = SPEC_DIR) {
  const read = (name) => parse(readFileSync(join(specDir, name), "utf8"));
  return {
    tokens: read("tokens.yaml"),
    components: read("components.yaml"),
    concept: read("concept_tone.yaml"),
    governance: read("governance.yaml"),
  };
}

const kebab = (name) => String(name).replaceAll("_", "-");

const escape = (text) =>
  String(text).replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");

/**
 * tokens.yaml を CSS の変数にする。名前は admin-web の @theme と同じ写し方（color-primary・radius-md）。
 * 状態の色は preview_css が使う color-<状態>-fg / -container / -on-container。
 */
export function tokenVariables(tokens) {
  const vars = {};
  for (const [name, value] of Object.entries(tokens.color.roles)) vars[`color-${kebab(name)}`] = value;
  for (const [status, colors] of Object.entries(tokens.color.status)) {
    for (const [part, value] of Object.entries(colors)) vars[`color-${status}-${kebab(part)}`] = value;
  }
  for (const [step, px] of Object.entries(tokens.spacing.scale_px)) vars[`space-${step}`] = `${px}px`;
  for (const [step, px] of Object.entries(tokens.shape.radius_px)) vars[`radius-${step}`] = `${px}px`;
  vars["font-sans"] = tokens.typography.families.sans;
  vars["font-numeric"] = tokens.typography.families.numeric;
  return vars;
}

/** typography.scale の段ごとの class（.type-display・.type-figure-sm など）。preview の html が使う。family のある段はその書体。 */
export function typeClasses(tokens) {
  return Object.entries(tokens.typography.scale)
    .map(
      ([name, s]) =>
        `.type-${kebab(name)}{${s.family ? `font-family:var(--font-${kebab(s.family)});` : ""}font-size:${s.size}px;font-weight:${s.weight};line-height:${s.line_height};letter-spacing:${s.letter_spacing}}`,
    )
    .join("\n");
}

// カタログそのものの枠の見た目。部品の見本（preview_css）と名前がぶつからないよう cat- を付ける
const CATALOG_CSS = `
*{box-sizing:border-box}
body{margin:0;background:var(--color-background);color:var(--color-text);font-family:var(--font-sans);font-feature-settings:"tnum";-webkit-font-smoothing:antialiased}
.cat{max-width:1120px;margin:0 auto;padding:var(--space-xl) var(--space-lg)}
.cat h1,.cat h2,.cat h3{margin:0}
.cat-hero{background:var(--color-inverse-surface);color:var(--color-on-inverse-surface);border-radius:var(--radius-md);padding:var(--space-xl) var(--space-lg);position:relative;overflow:hidden;display:flex;flex-direction:column;gap:var(--space-sm)}
.cat-hero .cat-eb{color:var(--color-on-inverse-surface-muted)}
.cat-hero .cat-line{border-bottom:4px solid var(--color-secondary);align-self:flex-start;padding-bottom:var(--space-xs)}
.cat-hero .cat-band{position:absolute;right:-40px;top:40px;width:240px;height:24px;background:var(--color-secondary);transform:rotate(-12deg)}
.cat-section{margin-top:var(--space-xl);display:flex;flex-direction:column;gap:var(--space-md)}
.cat-muted{color:var(--color-text-muted)}
.cat-table{border-collapse:collapse;width:100%}
.cat-table th,.cat-table td{text-align:left;vertical-align:top;padding:var(--space-sm) var(--space-md);border-bottom:1px solid var(--color-border)}
.cat-table th{white-space:nowrap}
.cat-swatches{display:flex;flex-wrap:wrap;gap:var(--space-md)}
.cat-swatch{width:168px;display:flex;flex-direction:column;gap:var(--space-xs)}
.cat-swatch b{overflow-wrap:anywhere}
.cat-chip{height:56px;border-radius:var(--radius-md);border:1px solid var(--color-border)}
.cat-type{display:flex;align-items:baseline;gap:var(--space-lg);padding:var(--space-sm) 0;border-bottom:1px solid var(--color-border)}
.cat-type .cat-meta{width:200px;flex:none}
.cat-steps{display:flex;flex-wrap:wrap;gap:var(--space-lg);align-items:flex-end}
.cat-step{display:flex;flex-direction:column;gap:var(--space-xs);align-items:flex-start}
.cat-step i{display:block;background:var(--color-primary-container);border:1px solid var(--color-primary)}
.cat-component{border-top:1px solid var(--color-border);padding-top:var(--space-lg);display:flex;flex-direction:column;gap:var(--space-md)}
.cat-previews{display:flex;flex-wrap:wrap;gap:var(--space-lg);align-items:flex-start}
.cat-preview{display:flex;flex-direction:column;gap:var(--space-sm)}
`;

function conceptSection({ concept }) {
  const copy = concept.copy;
  const rows = copy.map
    .map(
      (m) =>
        `<tr><th class="type-body">${escape(m.phrase)}</th><td class="type-body">${escape(m.design)}</td><td class="type-small cat-muted">${escape(m.limit)}</td></tr>`,
    )
    .join("");
  return `<section class="cat-section" aria-labelledby="concept"><h2 id="concept" class="type-headline">コンセプト</h2>
<table class="cat-table"><thead><tr><th class="type-small">コピーの言葉</th><th class="type-small">デザイン</th><th class="type-small">抑え</th></tr></thead><tbody>${rows}</tbody></table>
<p class="type-body">${escape(copy.calm_primary)}</p></section>`;
}

function colorSection({ tokens }) {
  const swatch = (name, value) =>
    `<div class="cat-swatch"><div class="cat-chip" style="background:${escape(value)}"></div><div class="type-small"><b>${escape(name)}</b></div><div class="type-small cat-muted">${escape(value)}</div></div>`;
  const roles = Object.entries(tokens.color.roles)
    .map(([name, value]) => swatch(name, value))
    .join("");
  const statuses = Object.entries(tokens.color.status)
    .flatMap(([status, colors]) => Object.entries(colors).map(([part, value]) => swatch(`${status}.${part}`, value)))
    .join("");
  return `<section class="cat-section" aria-labelledby="color"><h2 id="color" class="type-headline">色</h2>
<h3 class="type-title">役割</h3><div class="cat-swatches">${roles}</div>
<h3 class="type-title">状態</h3><div class="cat-swatches">${statuses}</div></section>`;
}

function typeSection({ tokens }) {
  const rows = Object.entries(tokens.typography.scale)
    .map(
      ([name, s]) =>
        `<div class="cat-type"><div class="cat-meta type-small"><b>${escape(name)}</b><br><span class="cat-muted">${s.size}px・${s.weight}・${s.line_height}・${escape(s.letter_spacing)}${s.family ? `・${escape(s.family)}` : ""}</span></div><div class="type-${kebab(name)}">${escape(s.sample)}</div></div>`,
    )
    .join("");
  return `<section class="cat-section" aria-labelledby="type"><h2 id="type" class="type-headline">書体</h2>
<p class="type-small cat-muted">sans ${escape(tokens.typography.families.sans)}。numeric ${escape(tokens.typography.families.numeric)}。${escape(tokens.typography.sample_rule)}</p>${rows}</section>`;
}

function shapeSection({ tokens }) {
  const spaces = Object.entries(tokens.spacing.scale_px)
    .map(
      ([step, px]) =>
        `<div class="cat-step"><i style="width:${px}px;height:${px}px"></i><span class="type-small">${escape(step)} ${px}px</span></div>`,
    )
    .join("");
  const radii = Object.entries(tokens.shape.radius_px)
    .map(
      ([step, px]) =>
        `<div class="cat-step"><i style="width:96px;height:48px;border-radius:${px}px"></i><span class="type-small">${escape(step)} ${px}px</span></div>`,
    )
    .join("");
  return `<section class="cat-section" aria-labelledby="shape"><h2 id="shape" class="type-headline">余白と角丸</h2>
<div class="cat-steps">${spaces}</div><div class="cat-steps">${radii}</div></section>`;
}

function componentSection({ components }) {
  const parts = components.components
    .map((c) => {
      const previews = (c.preview ?? [])
        .map(
          (p) =>
            `<figure class="cat-preview" style="margin:0"><figcaption class="type-small cat-muted">${escape(p.label)}</figcaption>${p.html}</figure>`,
        )
        .join("");
      return `<article class="cat-component" aria-labelledby="c-${escape(c.id)}"><h3 id="c-${escape(c.id)}" class="type-title">${escape(c.ja)} <span class="type-small cat-muted">${escape(c.id)}・${escape(c.lifecycle)}</span></h3>
<p class="type-small cat-muted">${escape(c.use)}</p><div class="cat-previews">${previews}</div></article>`;
    })
    .join("\n");
  return `<section class="cat-section" aria-labelledby="components"><h2 id="components" class="type-headline">部品</h2>${parts}</section>`;
}

/** 正本からカタログの HTML を組み立てる。同じ正本からは同じ文字列になる（日時などを入れない）。 */
export function buildHtml(spec) {
  const vars = Object.entries(tokenVariables(spec.tokens))
    .map(([name, value]) => `--${name}:${value};`)
    .join("");
  const version = spec.governance.version;
  const copy = spec.concept.copy;
  return `<!doctype html>
<!-- tools/design-catalog/build.mjs が specs/design/ から作る。手で直さない（正本を直して作り直す） -->
<html lang="ja">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="color-scheme" content="light">
<title>rally デザインシステム ${escape(version)} 部品のカタログ</title>
<style>
:root{${vars}}
${typeClasses(spec.tokens)}
${CATALOG_CSS.trim()}
${spec.components.preview_css.trim()}
</style>
</head>
<body>
<main class="cat">
<header class="cat-hero"><i class="cat-band"></i>
<div class="cat-eb type-small">rally デザインシステム ${escape(version)}・コンセプト ${escape(spec.concept.concept.id)}</div>
<h1 class="type-headline cat-line">${escape(copy.line)}</h1>
<div class="cat-eb type-small">${escape(spec.concept.concept.name)}</div>
</header>
${conceptSection(spec)}
${colorSection(spec)}
${typeSection(spec)}
${shapeSection(spec)}
${componentSection(spec)}
</main>
</body>
</html>
`;
}

/** catalog.html を書き、png が真なら Chromium で撮って catalog.png も書く。 */
export async function build({ specDir = SPEC_DIR, outDir = OUT_DIR, png = false } = {}) {
  const html = buildHtml(readSpec(specDir));
  mkdirSync(outDir, { recursive: true });
  const htmlPath = join(outDir, "catalog.html");
  writeFileSync(htmlPath, html);
  if (!png) return { htmlPath };
  const { chromium } = await import("playwright");
  const browser = await chromium.launch(
    process.env.PLAYWRIGHT_CHROMIUM_PATH ? { executablePath: process.env.PLAYWRIGHT_CHROMIUM_PATH } : {},
  );
  try {
    const page = await browser.newPage({ viewport: { width: 1200, height: 800 }, deviceScaleFactor: 1, locale: "ja-JP" });
    await page.setContent(html, { waitUntil: "load" });
    const pngPath = join(outDir, "catalog.png");
    await page.screenshot({ path: pngPath, fullPage: true });
    return { htmlPath, pngPath };
  } finally {
    await browser.close();
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const written = await build({ png: process.argv.includes("--png") });
  for (const path of Object.values(written)) console.log(path);
}
