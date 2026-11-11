// 知見 K-0025 の担保（議決 D-0151 の test の案、Issue #1391）。実装役の最後の文（session の output）と PR 本文の ## 見出しの並びと数を照らす
// reportHeadingProblems が、事例 C-0012（PR #982）の欠けた本文と、並び・数のずれで落ち、そろった本文では通ることを確かめる。
// Engine の worker/entrypoint.sh の取り出しで本文を組んで照らすのは engine.test.mjs の (16)(17)。
import assert from "node:assert/strict";
import { test } from "node:test";

import { checkBody } from "./check.mjs";
import { firstPrBody } from "./k0016.mjs";
import { C0012_CUT_LINE, C0012_PR_982, C0012_REPORT, ISSUE_979, SPLIT_BLOCKS, SPLIT_REPORT, h2s, reportHeadingProblems } from "./k0025.mjs";

const REPORT_H2 = ["概要", "対応内容", "影響範囲・互換性", "動作確認", "補足", "仮決め", "知見"];

test("C-0012 の実装役の最後の文は ## 概要 から書かれ、80 行を超え、末尾の 80 行は事例の summary.md と同じ行から始まる", () => {
  const lines = C0012_REPORT.split("\n");
  assert.equal(lines[0], "## 概要");
  assert.ok(lines.length > 80, `${lines.length} 行`);
  assert.equal(lines.slice(-80)[0], C0012_CUT_LINE);
  assert.equal(lines.filter((l) => l === C0012_CUT_LINE).length, 1, "切れた行が 1 か所に決まらない");
  assert.deepEqual(h2s(C0012_REPORT), REPORT_H2);
});

test("報告をそのまま載せた本文は、## 見出しの並びと数がそろう（Engine の「## 工場の記録」から下は見ない）", () => {
  const body = firstPrBody(ISSUE_979, C0012_REPORT);
  assert.deepEqual(reportHeadingProblems(C0012_REPORT, body), []);
  assert.deepEqual(checkBody(body), []);
});

test("C-0012 の PR #982 の本文（箇条書きの途中から始まる）は、## 見出しの数が報告とそろわず落ちる", () => {
  const problems = reportHeadingProblems(C0012_REPORT, C0012_PR_982);
  assert.deepEqual(problems.map((p) => p.rule), ["report_heading_count"]);
  assert.deepEqual(problems[0].expected, REPORT_H2);
  assert.deepEqual(problems[0].actual, REPORT_H2.slice(2));
  assert.match(problems[0].message, /5 個で、実装役の最後の文の 7 個/);
});

test("報告の末尾だけを拾った本文（tail -80 と同じ切り方）は落ちる", () => {
  const tail = C0012_REPORT.split("\n").slice(-80).join("\n");
  assert.deepEqual(reportHeadingProblems(C0012_REPORT, firstPrBody(ISSUE_979, tail)).map((p) => p.rule), ["report_heading_count"]);
});

test("複数のテキストの塊のうち後ろの塊だけを拾った本文は落ち、塊をすべてつないだ本文は通る", () => {
  assert.equal(SPLIT_REPORT.split("\n").length <= 80, true, "塊の事例は 80 行に収める（長さの事例と分ける）");
  for (let from = 1; from < SPLIT_BLOCKS.length; from++) {
    const body = firstPrBody(ISSUE_979, SPLIT_BLOCKS.slice(from).join("\n\n"));
    assert.deepEqual(reportHeadingProblems(SPLIT_REPORT, body).map((p) => p.rule), ["report_heading_count"], `${from} 個目の塊から拾った`);
  }
  assert.deepEqual(reportHeadingProblems(SPLIT_REPORT, firstPrBody(ISSUE_979, SPLIT_REPORT)), []);
});

test("## 見出しの数が同じでも、並びが違えば落ちる", () => {
  const swapped = C0012_REPORT.replace("## 影響範囲・互換性", "## 仮置き").replace("## 仮決め", "## 影響範囲・互換性").replace("## 仮置き", "## 仮決め");
  const problems = reportHeadingProblems(C0012_REPORT, firstPrBody(ISSUE_979, swapped));
  assert.deepEqual(problems.map((p) => p.rule), ["report_heading_order"]);
  assert.match(problems[0].message, /3 番目の ## 見出しが「## 仮決め」で、実装役の最後の文の「## 影響範囲・互換性」と違う/);
});

test("本文の ## 見出しが報告より多くても（重なり・足した見出し）落ちる", () => {
  const doubled = firstPrBody(ISSUE_979, `${C0012_REPORT}\n\n## 動作確認\n- もう一度確かめた`);
  assert.deepEqual(reportHeadingProblems(C0012_REPORT, doubled).map((p) => p.rule), ["report_heading_count"]);
});

test("### 以下の見出しとコードブロックの中の ## は数えない", () => {
  const report = "## 概要\nx\n\n## 対応内容\n- y\n\n### 細目\n\n```md\n## コードの中\n```\n";
  assert.deepEqual(h2s(report), ["概要", "対応内容"]);
  assert.deepEqual(reportHeadingProblems(report, firstPrBody(ISSUE_979, "## 概要\nx\n\n## 対応内容\n- y\n")), []);
});
