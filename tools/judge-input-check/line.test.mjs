// line.mjs の線のテスト（知見 K-0011 の担保。議決 D-0033、Issue #1054）。Engine は使わない。
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { checkInput, normalize, requiredRubricIds } from "./line.mjs";

const C0006 = JSON.parse(readFileSync(new URL("./fixtures/c0006.json", import.meta.url), "utf8"));
const PR_002 = "テンプレートの見出し（概要・背景・問題・対応内容・影響範囲・動作確認・補足）のうち変更に必要なものが埋まり、該当しないものは削除されている";
const OV_001 = "PR の説明（本文）に「## 背景」「## 問題」の見出しがなく、\"なぜ\" この変更が要るのかが `Closes #N` から読み取るしかないときは欠陥とする。";
const SOURCES = { "PR-002": { source: "core", body: PR_002 }, "OV-001": { source: "overlay", body: OV_001 } };

/** judge.py と同じく、rubric を JSON にして埋め込んだ入力 */
const prompt = (rubric) => `# 記録（データ）\n\`\`\`json\n${JSON.stringify({ rubric, defects: C0006.defects }, null, 1)}\n\`\`\``;

test("指摘が引く rubric_id を出た順に 1 回ずつ数え、rubric_id のない指摘は数えない", () => {
  assert.deepEqual(requiredRubricIds(C0006.defects), ["PR-002", "OV-001"]);
  assert.deepEqual(requiredRubricIds([{ rubric_id: "PR-002" }, {}, { rubric_id: "PR-002" }, null]), ["PR-002"]);
  assert.deepEqual(requiredRubricIds(undefined), []);
});

test("C-0006 の指摘が引く rubric の本文がすべて入力にあれば通る（JSON の逃がしと折り返しの空白は見ない）", () => {
  const p = prompt({ "PR-002": { check: PR_002 }, "OV-001": { check: OV_001.replace("、", "、\n  ") } });
  assert.deepEqual(checkInput(C0006.defects, p, SOURCES), []);
});

test("どれか 1 つでも本文が欠けたら外れる（C-0006 の判定役の入力: OV-001 の本文がない）", () => {
  assert.deepEqual(checkInput(C0006.defects, prompt({ "PR-002": { check: PR_002 } }), SOURCES),
    [{ rule: "missing_rubric_body", rubric_id: "OV-001", source: "overlay" }]);
  assert.deepEqual(checkInput(C0006.defects, prompt({}), SOURCES).map((e) => e.rubric_id), ["PR-002", "OV-001"]);
});

test("id だけあって本文がない入力、本文の一部だけの入力は外れる", () => {
  const p = prompt({ "PR-002": { title: "PR の説明は背景から結論まで追える構造にする" }, "OV-001": { check: OV_001.slice(0, 20) } });
  assert.deepEqual(checkInput(C0006.defects, p, SOURCES).map((e) => e.rule), ["missing_rubric_body", "missing_rubric_body"]);
});

test("正本の本文が見つからない rubric_id は、入力に何があっても通さない", () => {
  const defects = [...C0006.defects, { rubric_id: "OV-999" }];
  assert.deepEqual(checkInput(defects, prompt({ "PR-002": { check: PR_002 }, "OV-001": { check: OV_001 } }),
    { ...SOURCES, "OV-999": { source: null, body: null } }), [{ rule: "unknown_rubric", rubric_id: "OV-999", source: null }]);
  assert.deepEqual(checkInput([{ rubric_id: "PR-002" }], "", {}), [{ rule: "unknown_rubric", rubric_id: "PR-002", source: null }]);
});

test("指摘がなければ何も求めない", () => {
  assert.deepEqual(checkInput([], "", {}), []);
});

test("比べる形は空白と JSON の逃がしを落とす", () => {
  assert.equal(normalize(JSON.stringify('a "b"\n c\\d')), '"a"b"c\\d"');
  assert.equal(normalize('a "b"\n c\\d'), 'a"b"c\\d');
  assert.equal(normalize(null), "");
});
