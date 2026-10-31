// PR の前（ベース）と後の画面のキャプチャを比べ、変わった画面だけを見比べられる形にまとめる。
//
//   node compare.mjs --app admin-web --base <前のキャプチャ> --head <後のキャプチャ> --out <出力先>
//
// キャプチャのフォルダには、画面ごとに <id>.png と <id>.json（{ id, title, ok }）がある前提（各アプリの撮影が書く）。
// 前のフォルダがない・空のとき（撮影を入れる前のベースなど）は、取れた画面をすべて「新しい画面」にする。
//
// 出力:
//   manifest.json  変わった・新しい・消えた・撮れなかった・変わらない画面の一覧（工場や他の道具が読む）
//   index.html     変わった画面の前・後・差分を並べたページ（成果物を開いて人が見る）
//   summary.md     PR の CI の要約（GITHUB_STEP_SUMMARY）に足す表
//   images/        変わった・新しい・消えた画面の画像（<id>.head.png / <id>.base.png / <id>.diff.png）

import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { parseArgs } from "node:util";

import pixelmatch from "pixelmatch";
import { PNG } from "pngjs";

/** 色の差の閾値（pixelmatch の threshold）。文字の縁の微妙な揺れを拾わない程度。 */
const COLOR_THRESHOLD = 0.1;
/** 違う画素がこの割合を超えたら「変わった」とする（描画の揺れで数画素だけ違うのは変わっていない扱い）。 */
export const CHANGED_RATIO = 0.0005;

/** フォルダのキャプチャを id ごとに読む。 */
export function readShots(dir) {
  const shots = new Map();
  if (!dir || !existsSync(dir)) return shots;
  for (const name of readdirSync(dir)) {
    if (!name.endsWith(".json")) continue;
    const meta = JSON.parse(readFileSync(join(dir, name), "utf8"));
    const png = join(dir, `${meta.id}.png`);
    shots.set(meta.id, { ...meta, ok: Boolean(meta.ok) && existsSync(png), png });
  }
  return shots;
}

/** 大きさが違う画像は、大きいほうに合わせて白で埋めてから比べる。 */
function pad(png, width, height) {
  if (png.width === width && png.height === height) return png;
  const out = new PNG({ width, height });
  out.data.fill(255);
  PNG.bitblt(png, out, 0, 0, png.width, png.height, 0, 0);
  return out;
}

/** 2 枚の違う画素の割合と、差分の画像を返す。 */
export function diffImages(basePath, headPath) {
  const base = PNG.sync.read(readFileSync(basePath));
  const head = PNG.sync.read(readFileSync(headPath));
  const width = Math.max(base.width, head.width);
  const height = Math.max(base.height, head.height);
  const diff = new PNG({ width, height });
  const count = pixelmatch(pad(base, width, height).data, pad(head, width, height).data, diff.data, width, height, {
    threshold: COLOR_THRESHOLD,
  });
  const sizeChanged = base.width !== head.width || base.height !== head.height;
  return { ratio: count / (width * height), sizeChanged, diff };
}

/** 前と後を突き合わせて、画面を 5 つに分ける。 */
export function compareShots(base, head, imagesDir) {
  const result = { changed: [], added: [], removed: [], failed: [], unchanged: [] };
  const ids = [...new Set([...base.keys(), ...head.keys()])].sort();
  for (const id of ids) {
    const b = base.get(id);
    const h = head.get(id);
    const title = h?.title ?? b?.title ?? id;
    if (h && !h.ok) {
      result.failed.push({ id, title });
      continue;
    }
    if (!h) {
      if (b?.ok) {
        copyFileSync(b.png, join(imagesDir, `${id}.base.png`));
        result.removed.push({ id, title, base: `images/${id}.base.png` });
      }
      continue;
    }
    if (!b || !b.ok) {
      copyFileSync(h.png, join(imagesDir, `${id}.head.png`));
      result.added.push({ id, title, head: `images/${id}.head.png` });
      continue;
    }
    const { ratio, sizeChanged, diff } = diffImages(b.png, h.png);
    if (!sizeChanged && ratio <= CHANGED_RATIO) {
      result.unchanged.push({ id, title });
      continue;
    }
    copyFileSync(b.png, join(imagesDir, `${id}.base.png`));
    copyFileSync(h.png, join(imagesDir, `${id}.head.png`));
    writeFileSync(join(imagesDir, `${id}.diff.png`), PNG.sync.write(diff));
    result.changed.push({
      id,
      title,
      diff_ratio: Number(ratio.toFixed(4)),
      base: `images/${id}.base.png`,
      head: `images/${id}.head.png`,
      diff: `images/${id}.diff.png`,
    });
  }
  return result;
}

const escape = (s) => String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);

export function renderHtml(app, result) {
  const figure = (label, src) =>
    `<figure><figcaption>${label}</figcaption><a href="${src}"><img src="${src}" alt="${label}" loading="lazy"></a></figure>`;
  const section = (heading, items, render) =>
    items.length === 0 ? "" : `<h2>${heading}（${items.length}）</h2>${items.map(render).join("")}`;
  const card = (item, figures, note = "") =>
    `<section id="${escape(item.id)}"><h3>${escape(item.title)} <code>${escape(item.id)}</code>${note}</h3><div class="row">${figures}</div></section>`;
  const list = (items) => `<ul>${items.map((i) => `<li>${escape(i.title)} <code>${escape(i.id)}</code></li>`).join("")}</ul>`;
  return `<!doctype html>
<html lang="ja"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>${escape(app)} の画面のキャプチャ</title>
<style>
body{font-family:system-ui,sans-serif;margin:16px;color:#111;background:#fff}
.row{display:flex;gap:12px;align-items:flex-start;overflow-x:auto}
figure{margin:0;flex:1;min-width:240px}figcaption{font-size:13px;color:#555;margin-bottom:4px}
img{width:100%;border:1px solid #ccc}code{font-size:12px;color:#666}section{margin-bottom:32px}
.note{font-size:13px;color:#a15c00;font-weight:normal}
</style></head><body>
<h1>${escape(app)} の画面のキャプチャ</h1>
<p>変わった ${result.changed.length}・新しい ${result.added.length}・消えた ${result.removed.length}・撮れなかった ${result.failed.length}・変わらない ${result.unchanged.length}</p>
${section("変わった画面", result.changed, (i) =>
  card(i, figure("前", i.base) + figure("後", i.head) + figure("差分（赤が違う画素）", i.diff), ` <span class="note">違う画素 ${(i.diff_ratio * 100).toFixed(2)}%</span>`),
)}
${section("新しい画面", result.added, (i) => card(i, figure("後", i.head)))}
${section("消えた画面", result.removed, (i) => card(i, figure("前", i.base)))}
${result.failed.length ? `<h2>撮れなかった画面（${result.failed.length}）</h2><p>たどり方（押す文言など）が今の画面と合っていない可能性があります。</p>${list(result.failed)}` : ""}
${result.unchanged.length ? `<h2>変わらない画面（${result.unchanged.length}）</h2>${list(result.unchanged)}` : ""}
</body></html>
`;
}

export function renderSummary(app, result) {
  const lines = [`### ${app} の画面のキャプチャ`, ""];
  const total = result.changed.length + result.added.length + result.removed.length;
  lines.push(
    total === 0
      ? `見た目が変わった画面はありません（変わらない ${result.unchanged.length} 画面）。`
      : `見た目が変わった画面が ${total} あります。前・後・差分は成果物の index.html で見られます。`,
  );
  lines.push("");
  const rows = [
    ...result.changed.map((i) => `| 変わった | ${i.title} | \`${i.id}\` | ${(i.diff_ratio * 100).toFixed(2)}% |`),
    ...result.added.map((i) => `| 新しい | ${i.title} | \`${i.id}\` | |`),
    ...result.removed.map((i) => `| 消えた | ${i.title} | \`${i.id}\` | |`),
    ...result.failed.map((i) => `| 撮れなかった | ${i.title} | \`${i.id}\` | |`),
  ];
  if (rows.length) lines.push("| 区分 | 画面 | id | 違う画素 |", "|---|---|---|---|", ...rows, "");
  return lines.join("\n");
}

function main() {
  const { values } = parseArgs({
    options: {
      app: { type: "string" },
      base: { type: "string" },
      head: { type: "string" },
      out: { type: "string" },
    },
  });
  if (!values.app || !values.head || !values.out) {
    console.error("使い方: node compare.mjs --app <名前> --base <前> --head <後> --out <出力先>");
    process.exit(2);
  }
  const imagesDir = join(values.out, "images");
  mkdirSync(imagesDir, { recursive: true });
  const result = compareShots(readShots(values.base), readShots(values.head), imagesDir);
  const manifest = { app: values.app, ...result };
  writeFileSync(join(values.out, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
  writeFileSync(join(values.out, "index.html"), renderHtml(values.app, result));
  writeFileSync(join(values.out, "summary.md"), renderSummary(values.app, result));
  console.log(
    `${values.app}: 変わった ${result.changed.length}・新しい ${result.added.length}・消えた ${result.removed.length}・撮れなかった ${result.failed.length}・変わらない ${result.unchanged.length}`,
  );
}

if (import.meta.url === `file://${process.argv[1]}`) main();
