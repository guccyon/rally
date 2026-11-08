// @vitest-environment node
// governance.yaml の lint の button-shape と status-tag-shape を、画面のソースに当てる（#998）。
import { readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

import { expect, test } from "vitest";

const SRC = fileURLToPath(new URL("../src", import.meta.url));

function tsxFiles(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return tsxFiles(path);
    return entry.name.endsWith(".tsx") ? [path] : [];
  });
}

const files = tsxFiles(SRC).map((path) => ({
  name: relative(SRC, path).replaceAll("\\", "/"),
  source: readFileSync(path, "utf8"),
}));

// headquarters/ のボタンは #1001 で置き換える。置き換えたらこの除外を外す。
const BUTTON_SHAPE_PENDING = "components/headquarters/";

test("手書きの button にピル型の角丸（rounded-pill・rounded-full）を付けない（button-shape）", () => {
  const offenders = files
    .filter(({ name }) => !name.startsWith(BUTTON_SHAPE_PENDING))
    .flatMap(({ name, source }) =>
      // `=>` を属性の終わりと見ないよう、開きタグを読む
      [...source.matchAll(/<button\b(?:=>|[^>])*>/g)]
        .filter(([tag]) => /\brounded-(pill|full)\b/.test(tag))
        .map(() => name),
    );
  expect(offenders).toEqual([]);
});

test("状態の container の面（bg-success-container など）は data_table の急ぎの行だけ（status-tag-shape）", () => {
  const offenders = files
    .filter(({ name }) => name !== "components/ui/DataTable.tsx")
    .filter(({ source }) => /\bbg-(info|success|warning|danger)-container\b/.test(source))
    .map(({ name }) => name);
  expect(offenders).toEqual([]);
});

// media-no-record（0.7.0。camera_preview の must なので最初から止める）。カメラとマイクを録画・静止画・録音にせず、外へ送らない。
// 通話そのもの（RTCPeerConnection）を足すときは、入室前チェックの画面と部品だけに絞ってこの検査を直す。
test("カメラとマイクを録画・静止画・録音にせず、外へ送る口に渡さない（media-no-record）", () => {
  const offenders = files
    .filter(({ source }) => /\b(MediaRecorder|captureStream|toBlob|toDataURL|RTCPeerConnection)\b/.test(source))
    .map(({ name }) => name);
  expect(offenders).toEqual([]);
});
