import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { PR_URL, REASONS, continuationSummary, legacyBody, newPrBody } from "./c0005.mjs";
import { RUBRIC_ID, gate } from "./gate.mjs";

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
