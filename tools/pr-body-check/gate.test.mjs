import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { FIRST_SUMMARY, ISSUE, PASSING_REPORTS, PR_URL, REASONS, continuationSummary, legacyBody, newPrBody, recordedBodies } from "./c0005.mjs";
import { RUBRIC_ID, gate } from "./gate.mjs";
import { BROKEN_HEADS, C0008_JOB_3873, PR_URL_678, PR_URL_821, firstPrBody } from "./k0016.mjs";

test("新しい PR の本文は書いてよい", () => {
  assert.deepEqual(gate(newPrBody(), PR_URL), { ok: true, errors: [], body_fix: null });
});

test("続きのジョブの記録が「## 工場の記録」の下の ### にあれば書いてよい", () => {
  const body = `${newPrBody()}\n### 続きのジョブ（job 2801）の記録\n- CHANGELOG.md の衝突を解いた\n`;
  assert.equal(gate(body, PR_URL).ok, true);
});

test("C-0005 の 21 回の追記と同じ入力で落ち、body_fix に回す", () => {
  assert.equal(REASONS.length, 21);
  assert.deepEqual([REASONS.filter((r) => r === "conflict").length, REASONS.filter((r) => r === "ci_failed").length], [20, 1]);
  const result = gate(legacyBody(newPrBody()), PR_URL);
  assert.equal(result.ok, false);
  const dup = result.errors.filter((e) => e.rule === "duplicate_h2");
  // 続きのジョブ 21 回 × 3 つの ##（概要・対応内容・動作確認）が、どれも本文の上の節と重なる
  assert.equal(dup.length, 21 * 3);
  assert.deepEqual([...new Set(dup.map((e) => e.heading))].sort(), ["動作確認", "対応内容", "概要"].sort());
  assert.equal(result.errors.filter((e) => e.rule === "outside_factory_record").length, 21 * 3);
  assert.equal(result.body_fix.pr_url, PR_URL);
  assert.equal(result.body_fix.defects.length, result.errors.length);
});

test("1 回目の追記で、もう落ちる（重なりが 21 回まで積もる前に止める）", () => {
  const body = `${newPrBody()}\n\n${continuationSummary(REASONS[0], 0)}\n`;
  const result = gate(body, PR_URL);
  assert.equal(result.ok, false);
  assert.deepEqual(result.errors.filter((e) => e.rule === "duplicate_h2").map((e) => e.heading), ["概要", "対応内容", "動作確認"]);
});

test("body_fix の指摘は本文の構成の指摘（PR-002）で、箇所は PR 本文の行", () => {
  const { body_fix } = gate(`${newPrBody()}\n## 概要\n`, PR_URL);
  for (const d of body_fix.defects) {
    assert.equal(d.rubric_id, RUBRIC_ID);
    assert.match(d.location, /^PR 本文 \d+ 行目$/);
    assert.match(d.evidence, /^pr-body-check (duplicate_h2|outside_factory_record|continuation_outside_record): /);
    assert.ok(d.remediation_hint);
    assert.deepEqual(Object.keys(d).sort(), ["evidence", "location", "remediation_hint", "rubric_id", "severity"]);
  }
});

test("CLI は書いてよければ 0、外れたら 1 で body_fix の入力を出し、読めなければ 2", () => {
  const dir = mkdtempSync(join(tmpdir(), "pr-body-gate-"));
  const script = fileURLToPath(new URL("./gate.mjs", import.meta.url));
  writeFileSync(join(dir, "ok.md"), newPrBody());
  writeFileSync(join(dir, "bad.md"), legacyBody(newPrBody()));
  assert.equal(execFileSync("node", [script, "--pr-url", PR_URL, "--body", join(dir, "ok.md")], { encoding: "utf8" }), "");
  assert.throws(() => execFileSync("node", [script, "--pr-url", PR_URL, "--body", join(dir, "bad.md")], { stdio: "pipe" }), (e) => {
    assert.equal(e.status, 1);
    const out = JSON.parse(e.stdout.toString());
    assert.equal(out.pr_url, PR_URL);
    assert.ok(out.defects.length > 0);
    return true;
  });
  assert.equal(execFileSync("node", [script, "--pr-url", PR_URL], { input: newPrBody(), encoding: "utf8" }), "");
  assert.throws(() => execFileSync("node", [script, "--pr-url", PR_URL, "--body", join(dir, "none.md")], { stdio: "pipe" }), (e) => e.status === 2);
  assert.throws(() => execFileSync("node", [script, "--body", join(dir, "ok.md")], { stdio: "pipe" }), (e) => e.status === 2);
});

test("CLI は見出しの定義が不正なら 3 で終わり、body_fix の入力を出さない", () => {
  const dir = mkdtempSync(join(tmpdir(), "pr-body-gate-template-"));
  for (const f of ["check.mjs", "gate.mjs"]) copyFileSync(fileURLToPath(new URL(`./${f}`, import.meta.url)), join(dir, f));
  writeFileSync(join(dir, "template.json"), JSON.stringify({ h2: [{ text: "概要", required: "yes" }] }));
  writeFileSync(join(dir, "body.md"), newPrBody());
  assert.throws(() => execFileSync("node", [join(dir, "gate.mjs"), "--pr-url", PR_URL, "--body", join(dir, "body.md")], { stdio: "pipe", encoding: "utf8" }),
    (e) => e.status === 3 && e.stdout === "" && /見出しの定義が不正: h2\[0\]\.required が真偽値でない/.test(e.stderr));
});

// 通る側の写し（議決 D-0032 の test の案、Issue #1038）。C-0005 と同じ 21 回の続きのジョブでも、報告が「## 工場の記録」の下に
// 正しく入っていれば、関所は毎回 0（書いてよい）を返す。正しい更新まで止める誤検知を必須のテストで落とす
for (const [label, report] of PASSING_REPORTS) {
  test(`C-0005 の 21 回の続きのジョブでも、${label}本文は毎回書いてよい（誤検知がない）`, () => {
    const bodies = recordedBodies(newPrBody(), report);
    assert.equal(bodies.length, 21);
    bodies.forEach((body, i) => {
      assert.ok(body.includes(report(REASONS[i], i)), `${i + 1} 回目の報告が本文にない`);
      assert.deepEqual(gate(body, PR_URL), { ok: true, errors: [], body_fix: null }, `${i + 1} 回目（${REASONS[i]}）の正しい更新を関所が止めた`);
    });
  });
}

test("CLI は正しく入った続きのジョブの報告の本文で 0 を返し、body_fix の入力を出さない", () => {
  const dir = mkdtempSync(join(tmpdir(), "pr-body-gate-pass-"));
  const script = fileURLToPath(new URL("./gate.mjs", import.meta.url));
  PASSING_REPORTS.forEach(([, report], k) => {
    const bodies = recordedBodies(newPrBody(), report);
    for (const [n, body] of [[1, bodies[0]], [21, bodies.at(-1)]]) {
      const file = join(dir, `pass-${k}-${n}.md`);
      writeFileSync(file, body);
      // 0 以外なら execFileSync が投げる
      assert.equal(execFileSync("node", [script, "--pr-url", PR_URL, "--body", file], { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }), "");
      assert.equal(execFileSync("node", [script, "--pr-url", PR_URL], { input: body, encoding: "utf8", stdio: ["pipe", "pipe", "pipe"] }), "");
    }
  });
});

// 本文の先頭の関所（知見 K-0016 の担保。議決 D-0078 の ci_check の案、Issue #1219）。summary.md から最初の PR 本文を組んだときにも
// 同じ gate を呼ぶ。事例 C-0008（PR #678）と C-0006（PR #821）の報告から組んだ本文は、Closes の行のあとの最初の行が「## 概要」でないので落ちる
for (const [label, issue, summary, others] of BROKEN_HEADS) {
  test(`${label}から組んだ最初の本文は、先頭が「## 概要」でないので落ち、body_fix に回す`, () => {
    const body = firstPrBody(issue, summary);
    const result = gate(body, PR_URL_678);
    assert.equal(result.ok, false);
    assert.deepEqual(result.errors.map((e) => e.rule).sort(), ["head_not_first_heading", ...others].sort());
    const head = result.errors.find((e) => e.rule === "head_not_first_heading");
    // 1 行目が Closes、2 行目が空行、3 行目が報告の頭
    assert.equal(head.line, 3);
    assert.equal(head.heading, "概要");
    assert.ok(head.message.includes(summary.split("\n")[0].slice(0, 20)), "外れた最初の行を示していない");
    const defect = result.body_fix.defects.find((d) => d.evidence.startsWith("pr-body-check head_not_first_heading: "));
    assert.equal(defect.location, "PR 本文 3 行目");
    assert.match(defect.remediation_hint, /「## 概要」から始める/);
    // 途中終了の本文（未完了の印と Refs）でも同じく落ちる
    assert.ok(gate(firstPrBody(issue, summary, { incomplete: true }), PR_URL_678).errors.some((e) => e.rule === "head_not_first_heading"));
  });
}

test("C-0008 の job 3873 の報告は、本文の先頭の検査がなければ通ってしまう形（ほかの rule では落ちない）", () => {
  const errors = gate(firstPrBody(670, C0008_JOB_3873), PR_URL_678).errors;
  assert.deepEqual(errors.map((e) => e.rule), ["head_not_first_heading"]);
});

test("Closes・Refs の行と未完了の印のあとが「## 概要」から始まる最初の本文は書いてよい（誤検知がない）", () => {
  for (const body of [firstPrBody(ISSUE, FIRST_SUMMARY), firstPrBody(ISSUE, FIRST_SUMMARY, { incomplete: true }), FIRST_SUMMARY,
    `\n\n${FIRST_SUMMARY}\n`, `closes #${ISSUE}\n${FIRST_SUMMARY}`, FIRST_SUMMARY.replace("## 概要", "## 概要 ##")]) {
    assert.deepEqual(gate(body, PR_URL), { ok: true, errors: [], body_fix: null }, body.slice(0, 60));
  }
});

test("先頭がほかの見出し・### 概要・コードブロック・2 つ目の Closes のあとの文なら落ちる", () => {
  for (const head of ["## 対応内容\n- a\n\n## 概要\nb\n\n## 対応内容\nc", "### 概要\na\n\n## 概要\nb\n\n## 対応内容\nc",
    "```\nx\n```\n\n## 概要\na\n\n## 対応内容\nb", "Closes #1 と #2\n\n## 概要\na\n\n## 対応内容\nb"]) {
    assert.ok(gate(firstPrBody(ISSUE, head), PR_URL).errors.some((e) => e.rule === "head_not_first_heading"), head);
  }
});

test("CLI は C-0008・C-0006 の形の最初の本文で 1 を返し、本文の先頭の body_fix の入力を出す", () => {
  const dir = mkdtempSync(join(tmpdir(), "pr-body-gate-head-"));
  const script = fileURLToPath(new URL("./gate.mjs", import.meta.url));
  BROKEN_HEADS.forEach(([, issue, summary], k) => {
    const file = join(dir, `head-${k}.md`);
    writeFileSync(file, firstPrBody(issue, summary));
    assert.throws(() => execFileSync("node", [script, "--pr-url", PR_URL_821, "--body", file], { stdio: "pipe" }), (e) => {
      assert.equal(e.status, 1);
      const out = JSON.parse(e.stdout.toString());
      assert.equal(out.pr_url, PR_URL_821);
      assert.ok(out.defects.some((d) => d.evidence.startsWith("pr-body-check head_not_first_heading: ")));
      return true;
    });
  });
});
