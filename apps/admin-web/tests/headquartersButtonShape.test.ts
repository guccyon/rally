// @vitest-environment node
// 本部の画面（src/components/headquarters/）の押せるボタンが Button（specs/design/components.yaml の button）を使い、
// 手書きのピル型のボタンを残していないこと（governance.yaml の lint の button-shape。#1001）。
import { readdirSync, readFileSync } from "node:fs";

import { expect, test } from "vitest";

const dir = new URL("../src/components/headquarters/", import.meta.url);
const sources = readdirSync(dir)
  .filter((name) => name.endsWith(".tsx"))
  .map((name) => ({ name, text: readFileSync(new URL(name, dir), "utf8") }));

test("本部の画面に手書きの button の要素がない", () => {
  const offenders = sources.filter(({ text }) => /<button\b/.test(text)).map(({ name }) => name);
  expect(offenders).toEqual([]);
});

test("本部の画面にピル型の角丸（rounded-pill・rounded-full）がない", () => {
  const offenders = sources
    .filter(({ text }) => /\brounded-(pill|full)\b/.test(text))
    .map(({ name }) => name);
  expect(offenders).toEqual([]);
});
