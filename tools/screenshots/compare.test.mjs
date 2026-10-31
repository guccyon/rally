import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

import { PNG } from "pngjs";

import { compareShots, readShots, renderSummary } from "./compare.mjs";

/** 単色の画像に、(x, y) から w×h の黒い四角を描いて保存する。 */
function shot(dir, id, { title = id, ok = true, width = 100, height = 100, box } = {}) {
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, `${id}.json`), JSON.stringify({ id, title, ok }));
  if (!ok) return;
  const png = new PNG({ width, height });
  png.data.fill(255);
  if (box) {
    for (let y = box.y; y < box.y + box.h; y++)
      for (let x = box.x; x < box.x + box.w; x++) {
        const i = (y * width + x) * 4;
        png.data[i] = png.data[i + 1] = png.data[i + 2] = 0;
      }
  }
  writeFileSync(join(dir, `${id}.png`), PNG.sync.write(png));
}

function setup() {
  const root = mkdtempSync(join(tmpdir(), "shots-"));
  const dirs = { base: join(root, "base"), head: join(root, "head"), images: join(root, "images") };
  mkdirSync(dirs.images, { recursive: true });
  return dirs;
}

test("同じ画面は変わらない・違う画面は変わった・片方だけの画面は新しい／消えた", () => {
  const d = setup();
  shot(d.base, "same");
  shot(d.head, "same");
  shot(d.base, "moved", { box: { x: 0, y: 0, w: 10, h: 10 } });
  shot(d.head, "moved", { box: { x: 50, y: 50, w: 10, h: 10 } });
  shot(d.base, "gone");
  shot(d.head, "new");
  shot(d.head, "broken", { ok: false });

  const r = compareShots(readShots(d.base), readShots(d.head), d.images);
  assert.deepEqual(r.unchanged.map((i) => i.id), ["same"]);
  assert.deepEqual(r.changed.map((i) => i.id), ["moved"]);
  assert.deepEqual(r.added.map((i) => i.id), ["new"]);
  assert.deepEqual(r.removed.map((i) => i.id), ["gone"]);
  assert.deepEqual(r.failed.map((i) => i.id), ["broken"]);
  assert.equal(r.changed[0].diff, "images/moved.diff.png");
});

test("数画素だけの揺れは変わっていない扱い・大きさが違えば変わった", () => {
  const d = setup();
  shot(d.base, "jitter", { width: 200, height: 200 });
  shot(d.head, "jitter", { width: 200, height: 200, box: { x: 0, y: 0, w: 2, h: 2 } });
  shot(d.base, "taller");
  shot(d.head, "taller", { height: 120 });

  const r = compareShots(readShots(d.base), readShots(d.head), d.images);
  assert.deepEqual(r.unchanged.map((i) => i.id), ["jitter"]);
  assert.deepEqual(r.changed.map((i) => i.id), ["taller"]);
});

test("ベースにキャプチャがなければ、取れた画面はすべて新しい画面", () => {
  const d = setup();
  shot(d.head, "a");
  shot(d.head, "b");
  const r = compareShots(readShots(join(d.base, "missing")), readShots(d.head), d.images);
  assert.deepEqual(r.added.map((i) => i.id), ["a", "b"]);
});

test("要約は変わった画面がなければ一言だけ", () => {
  const empty = { changed: [], added: [], removed: [], failed: [], unchanged: [{ id: "a", title: "A" }] };
  assert.match(renderSummary("admin-web", empty), /見た目が変わった画面はありません（変わらない 1 画面）/);
  assert.doesNotMatch(renderSummary("admin-web", empty), /\| 区分/);
});
