// line.mjs の線（参照の実装）のテスト。Engine は要らない（知見 K-0009 の担保。議決 D-0013、Issue #955）
import assert from "node:assert/strict";
import { test } from "node:test";

import { bodyRules, checkInput, requiredInput, unresolvedDefects } from "./line.mjs";

const PR = "https://github.com/guccyon/rally/pull/659";

const judgeEnqueued = (head_sha, pr_url = PR) => ({ name: "supervisor.judge_enqueued", payload: { pr_url, head_sha } });
const judged = (verdict, defect, pr_url = PR) => ({
  name: "judgment.judged",
  payload: { pr_url, verdict, category: defect ? "inspection" : "ui" },
  judgment: { subject: defect ? "defect" : "provisional", verdict, defect },
});
const amended = (rules, pr_url = PR) => ({ name: "pr.body_amended", payload: { pr_url, defects: 1, chars: 100, rules } });

const RULE = "続きのジョブの報告は ## の見出しを足さず、「## 工場の記録」の下に ### で書く";

/** PR #659（事例 C-0005）の形: 本文の指摘 PR-002 が採用され、body_fix が本文を直して決まりを残した */
const PR_659 = [
  judgeEnqueued("head1"),
  judged("adopt", null),
  judged("adopt", { rubric_id: "PR-002", location: "PR 本文" }),
  amended([RULE]),
];

test("採用済みの指摘と body_fix の決まりを、入力に求めるものとして組む", () => {
  assert.deepEqual(requiredInput(PR_659, PR), { defect_ids: ["PR-002"], body_rules: [RULE] });
});

test("入力に指摘の id と本文の決まりがあれば通る", () => {
  const input = { head_sha: "head1", adopted_defects: [{ rubric_id: "PR-002", location: "PR 本文" }], body_rules: [RULE] };
  assert.deepEqual(checkInput(requiredInput(PR_659, PR), input), []);
});

test("採用済みの指摘がある PR で、入力に指摘の id がなければ落とす", () => {
  const errors = checkInput(requiredInput(PR_659, PR), { head_sha: "head1", failed: ["backend"], body_rules: [RULE] });
  assert.deepEqual(errors.map((e) => e.rule), ["missing_defect_id"]);
  assert.match(errors[0].message, /PR-002/);
});

test("body_fix が残した本文の決まりが入力になければ落とす", () => {
  const errors = checkInput(requiredInput(PR_659, PR), { adopted_defects: [{ rubric_id: "PR-002" }] });
  assert.deepEqual(errors.map((e) => e.rule), ["missing_body_rule"]);
});

test("求めるものがあるのに入力を渡さない（今の Engine の conflict の続きのジョブ）と落とす", () => {
  for (const input of [null, undefined, "PR-002", ["PR-002"]]) {
    assert.deepEqual(checkInput(requiredInput(PR_659, PR), input).map((e) => e.rule), ["input_missing"], String(input));
  }
});

test("採用済みの指摘も本文の決まりもない PR では、入力がなくても通る", () => {
  const events = [judgeEnqueued("head1"), judged("adopt", null), judged("reject", { rubric_id: "PR-002", location: "PR 本文" })];
  assert.deepEqual(requiredInput(events, PR), { defect_ids: [], body_rules: [] });
  assert.deepEqual(checkInput(requiredInput(events, PR), null), []);
});

test("overlay の shadow のルールの指摘と fixed の指摘は求めない", () => {
  const events = [
    judgeEnqueued("head1"),
    judged("adopt", { rubric_id: "OV-001", location: "PR 本文", shadow: true }),
    { ...judged("adopt", { rubric_id: "BE-003", location: "apps/backend/src/x.rs" }), judgment: { subject: "defect", verdict: "adopt",
      defect: { rubric_id: "BE-003", location: "apps/backend/src/x.rs" }, fixed: true } },
  ];
  assert.deepEqual(unresolvedDefects(events, PR), []);
});

test("前の head の裁きは、後の head の裁きで置き換わる（後で退けた指摘は求めない）", () => {
  const events = [...PR_659, judgeEnqueued("head2"), judged("reject", { rubric_id: "PR-002", location: "PR 本文" }),
    judged("adopt", { rubric_id: "DOC-001", location: "docs/x.md" })];
  assert.deepEqual(requiredInput(events, PR).defect_ids, ["DOC-001"]);
});

test("裁きのない head（judge_enqueued だけ）は、前の head の採用済みの指摘を消さない", () => {
  assert.deepEqual(requiredInput([...PR_659, judgeEnqueued("head2")], PR).defect_ids, ["PR-002"]);
});

test("ほかの PR のイベントは数えない。本文の決まりは同じ文を 1 回だけ数える", () => {
  const other = "https://github.com/guccyon/rally/pull/700";
  const events = [...PR_659, judgeEnqueued("x", other), judged("adopt", { rubric_id: "BE-003", location: "a.rs" }, other),
    amended(["別の PR の決まり"], other), amended([RULE, "", 3])];
  assert.deepEqual(requiredInput(events, PR), { defect_ids: ["PR-002"], body_rules: [RULE] });
  assert.deepEqual(bodyRules(events, other), ["別の PR の決まり"]);
});
