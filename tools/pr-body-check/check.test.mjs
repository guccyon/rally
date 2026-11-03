import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { checkBody, headings } from "./check.mjs";

/** 新しい PR の本文（Engine の worker/entrypoint.sh が組む形）。 */
const NEW_PR = `Closes #541

## 概要
本部が予約 1 件の詳細を見る。

## 対応内容
- getBooking を足した

## 動作確認
- cargo test

## 工場の記録
- job: 1499
- model: tier-large
`;

const rules = (body) => checkBody(body).map((e) => e.rule);

test("新しい PR の本文は通る", () => {
  assert.deepEqual(checkBody(NEW_PR), []);
});

test("続きのジョブの記録が「## 工場の記録」の下の ### にあれば通る", () => {
  const body = `${NEW_PR}
### 続きのジョブ（job 1504）の記録
<details><summary>既定ブランチの取り込みと衝突の解消</summary>

- CHANGELOG.md の衝突を解いた
</details>
`;
  assert.deepEqual(checkBody(body), []);
});

test("続きのジョブが同じ見出しの組を末尾に足すと外れる（C-0001 の PR #546）", () => {
  const body = `${NEW_PR}
## 概要
既定ブランチを取り込んだ。

## 対応内容
- CHANGELOG.md の衝突を解いた
`;
  const errors = checkBody(body);
  assert.deepEqual(errors.filter((e) => e.rule === "duplicate_h2").map((e) => e.heading), ["概要", "対応内容"]);
  assert.ok(errors.some((e) => e.rule === "outside_factory_record" && e.heading === "概要"));
  assert.match(errors[0].message, /「## 概要」が 2 回以上ある（3 行目と 16 行目）/);
});

test("「## 工場の記録」がなくても同じ ## 見出しの重なりは外れる", () => {
  assert.deepEqual(rules("## 概要\na\n\n## 補足\nb\n\n## 概要\nc\n"), ["duplicate_h2"]);
});

test("「## 工場の記録」より後ろの ## 見出しは、重なっていなくても外れる", () => {
  assert.deepEqual(rules(`${NEW_PR}\n## 続きのジョブの記録\n\n<details><summary>CI 落ちの直し</summary>\n</details>\n`),
    ["outside_factory_record"]);
  assert.deepEqual(rules(`${NEW_PR}\n# 補足\n`), ["outside_factory_record"]);
});

test("続きのジョブの見出しが「## 工場の記録」より前にあると外れる", () => {
  const body = NEW_PR.replace("## 工場の記録", "### 続きのジョブ（job 1504）の記録\n- 衝突を解いた\n\n## 工場の記録");
  assert.deepEqual(rules(body), ["continuation_outside_record"]);
});

test("### の見出しは重なっても外れない", () => {
  assert.deepEqual(checkBody("## 対応内容\n### 画面\n- a\n\n## 仮決め\n### 画面\n- b\n"), []);
});

test("コードブロックの中の見出しは数えない", () => {
  const body = "## 概要\nこう書く:\n\n```markdown\n## 概要\n```\n\n~~~\n## 概要\n~~~\n";
  assert.deepEqual(headings(body).map((h) => h.line), [1]);
  assert.deepEqual(checkBody(body), []);
});

test("見出しの末尾の # と空白は無視して比べる", () => {
  assert.deepEqual(rules("## 概要\n\n##  概要 ##\n"), ["duplicate_h2"]);
});

test("CLI は外れたら 1 で終わり、JSON を書く", () => {
  const dir = mkdtempSync(join(tmpdir(), "pr-body-"));
  const script = fileURLToPath(new URL("./check.mjs", import.meta.url));
  writeFileSync(join(dir, "ok.md"), NEW_PR);
  writeFileSync(join(dir, "bad.md"), `${NEW_PR}\n## 概要\n`);
  execFileSync("node", [script, "--body", join(dir, "ok.md")]);
  assert.throws(() => execFileSync("node", [script, "--body", join(dir, "bad.md"), "--json", join(dir, "out.json")]),
    (e) => e.status === 1);
  const out = JSON.parse(readFileSync(join(dir, "out.json"), "utf8"));
  assert.equal(out.ok, false);
  assert.deepEqual(out.errors.map((e) => e.rule), ["duplicate_h2", "outside_factory_record"]);
  const stdin = execFileSync("node", [script], { input: NEW_PR, encoding: "utf8" });
  assert.match(stdin, /外れ 0 件/);
  assert.throws(() => execFileSync("node", [script, "--body", join(dir, "none.md")], { stdio: "pipe" }), (e) => e.status === 2);
});
