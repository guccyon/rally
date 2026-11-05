import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { approvalBasis, checkRecord, counts, needsFollowup, parseBasis } from "./check.mjs";

const provisional = (verdict) => ({ subject: "provisional", verdict, defect: null });
const defect = (verdict, rubric_id, extra = {}) => ({ subject: "defect", verdict, defect: { rubric_id, location: "PR 本文", ...extra } });

/** PR #395 の裁き（C-0002）: 仮決め 3 件（ui-1・ui-2・eh-1）を adopt、欠陥 2 件（OV-001・PR-002）を reject。マージ済み。 */
const PR_395 = {
  pr_url: "https://github.com/guccyon/rally/pull/395",
  merged: true,
  judgments: [provisional("adopt"), provisional("adopt"), provisional("adopt"),
    defect("reject", "OV-001", { shadow: true }), defect("reject", "PR-002")],
};

const rules = (record) => checkRecord(record).map((e) => e.rule);

test("(1) PR #395 と同じ入力では差し戻しも続きのジョブも起こさない", () => {
  assert.equal(needsFollowup(PR_395.judgments), false);
  assert.deepEqual(counts(PR_395.judgments), { judged: 5, adopted_defects: 0, approved_provisionals: 3 });
  assert.deepEqual(checkRecord({ ...PR_395, followups: [], basis: approvalBasis(PR_395.judgments) }), []);
});

test("(1) PR #395 と同じ入力で差し戻しか続きのジョブが起きたら外れる", () => {
  for (const kind of ["sendback", "review_fix", "body_fix"]) {
    const errors = checkRecord({ ...PR_395, followups: [{ kind }] });
    assert.deepEqual(errors.map((e) => e.rule), ["followup_without_defect"], kind);
    assert.match(errors[0].message, /adopt 3 件のうち欠陥の採用 0 件/);
  }
});

test("(2) 直っていない採用欠陥が 1 件以上あれば、マージの前後に関わらず起こす", () => {
  const judgments = [...PR_395.judgments, defect("adopt", "BE-003", { location: "apps/backend/src/x.rs" })];
  for (const merged of [false, true]) {
    assert.equal(needsFollowup(judgments), true);
    assert.deepEqual(rules({ merged, judgments, followups: [{ kind: "sendback" }] }), []);
    assert.deepEqual(rules({ merged, judgments, followups: [{ kind: "review_fix" }] }), []);
    const missing = checkRecord({ merged, judgments, followups: [] });
    assert.deepEqual(missing.map((e) => e.rule), ["missing_followup"]);
    assert.match(missing[0].message, merged ? /マージ済みでも起こす/ : /起きていない$/);
  }
});

test("(2) 採用欠陥が直っていれば起こさない", () => {
  const judgments = [provisional("adopt"), defect("adopt", "BE-003", {}), defect("adopt", "PR-002")]
    .map((j) => (j.defect ? { ...j, fixed: true } : j));
  assert.equal(needsFollowup(judgments), false);
  assert.deepEqual(rules({ merged: true, judgments, followups: [{ kind: "sendback" }] }), ["followup_without_defect"]);
});

test("overlay の shadow のルールの指摘は adopt でも起こさない", () => {
  const judgments = [defect("adopt", "OV-001", { shadow: true })];
  assert.equal(needsFollowup(judgments), false);
  assert.deepEqual(counts(judgments).adopted_defects, 0);
});

test("まだ入っていない仮決めの差し替えは差し戻しで直す。入っていれば起こさない", () => {
  const replace = { ...provisional("replace") };
  assert.equal(needsFollowup([replace]), true);
  assert.deepEqual(rules({ judgments: [replace], followups: [{ kind: "sendback" }] }), []);
  assert.equal(needsFollowup([{ ...replace, already_applied: true }]), false);
});

test("(3) inspector_approved の basis に欠陥の採用数と仮決めの承認数を分けて書く", () => {
  const judgments = [...PR_395.judgments, defect("adopt", "BE-003")];
  const basis = approvalBasis(judgments);
  assert.equal(basis, "judge が 6 件を裁いた（欠陥の採用 1・仮決めの承認 3。採用した欠陥は続きのジョブか差し戻しで直す）");
  assert.deepEqual(parseBasis(basis), { adopted_defects: 1, approved_provisionals: 3 });
  assert.deepEqual(rules({ judgments, followups: [{ kind: "review_fix" }], basis }), []);
});

test("(3) adopt の合計だけの basis（PR #395 の記録の形）は外れる", () => {
  const basis = "judge が 5 件を裁いた（採用 3。採用した指摘は続きのジョブか差し戻しで直す）";
  assert.equal(parseBasis(basis), null);
  assert.deepEqual(rules({ ...PR_395, followups: [], basis }), ["basis_not_split"]);
});

test("(3) basis の数が裁きの記録と合わなければ外れる", () => {
  const basis = "judge が 5 件を裁いた（欠陥の採用 3・仮決めの承認 0。採用した欠陥は続きのジョブか差し戻しで直す）";
  assert.deepEqual(rules({ ...PR_395, followups: [], basis }), ["basis_mismatch"]);
});

test("CLI は外れたら 1 で終わり、JSON を書く", () => {
  const dir = mkdtempSync(join(tmpdir(), "sendback-"));
  const script = fileURLToPath(new URL("./check.mjs", import.meta.url));
  writeFileSync(join(dir, "ok.json"), JSON.stringify({ ...PR_395, followups: [] }));
  writeFileSync(join(dir, "bad.json"), JSON.stringify({ ...PR_395, followups: [{ kind: "sendback" }] }));
  writeFileSync(join(dir, "broken.json"), "{");
  execFileSync("node", [script, "--record", join(dir, "ok.json")]);
  assert.throws(() => execFileSync("node", [script, "--record", join(dir, "bad.json"), "--json", join(dir, "out.json")]),
    (e) => e.status === 1);
  const out = JSON.parse(readFileSync(join(dir, "out.json"), "utf8"));
  assert.equal(out.ok, false);
  assert.deepEqual(out.errors.map((e) => e.rule), ["followup_without_defect"]);
  const stdin = execFileSync("node", [script], { input: JSON.stringify({ ...PR_395, followups: [] }), encoding: "utf8" });
  assert.match(stdin, /欠陥の採用 0・仮決めの承認 3）、外れ 0 件/);
  assert.throws(() => execFileSync("node", [script, "--record", join(dir, "broken.json")], { stdio: "pipe" }), (e) => e.status === 2);
});
