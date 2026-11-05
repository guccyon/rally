// Engine の本物の判定（judgment.record_judged → dispatcher.act_on_judgment、supervisor.inspector_approved）を
// check.test.mjs と同じ 3 つの場合で確かめる（知見 K-0004 の担保。議決 D-0009・Issue #935）。
// Engine のチェックアウトは環境変数 ENGINE_DIR で渡す（なければリポジトリの隣の ../engine）。どちらもなければ飛ばす。
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { approvalBasis, checkRecord } from "./check.mjs";

const harness = fileURLToPath(new URL("./engine_harness.py", import.meta.url));
const engineDir = process.env.ENGINE_DIR || fileURLToPath(new URL("../../../engine", import.meta.url));
const skip = existsSync(`${engineDir}/orchestrator/dispatcher/supervisor.py`)
  ? false : `Engine のチェックアウトがない（ENGINE_DIR=${engineDir}）`;

/** Engine の判定を 1 回動かし、起きた後続と basis を返す */
const engine = (record) =>
  JSON.parse(execFileSync("python3", ["-I", harness, engineDir], { input: JSON.stringify(record), encoding: "utf8" }));

const provisional = (verdict) => ({ subject: "provisional", verdict, defect: null });
const defect = (verdict, rubric_id, extra = {}) => ({ subject: "defect", verdict, defect: { rubric_id, location: "PR 本文", ...extra } });

/** PR #395 の裁き（C-0002）: 仮決め 3 件を adopt、欠陥 2 件（OV-001・PR-002）を reject。マージ済み。 */
const PR_395 = {
  pr_url: "https://github.com/guccyon/rally/pull/395",
  merged: true,
  judgments: [provisional("adopt"), provisional("adopt"), provisional("adopt"),
    defect("reject", "OV-001", { shadow: true }), defect("reject", "PR-002")],
};

/** 後続の判定の線だけを見る（basis は (3) で見る） */
const followupRules = (record) => checkRecord(record).map((e) => e.rule).filter((r) => !r.startsWith("basis_"));

test("(1) Engine は PR #395 と同じ入力では差し戻しも続きのジョブも起こさない", { skip }, () => {
  const out = engine(PR_395);
  assert.deepEqual(out.followups, []);
  assert.equal(out.approved, true);
  assert.deepEqual(followupRules({ ...PR_395, followups: out.followups }), []);
});

test("(2) Engine は直っていない採用欠陥があれば、マージの前後に関わらず起こす", { skip }, () => {
  const judgments = [...PR_395.judgments, defect("adopt", "BE-003", { location: "apps/backend/src/x.rs" })];
  for (const merged of [false, true]) {
    const out = engine({ ...PR_395, merged, judgments });
    // マージ前は同じ担当の続きのジョブ（ADR 0017）、マージ後は差し戻しの Issue で直す
    assert.deepEqual(out.followups, [{ kind: merged ? "sendback" : "review_fix" }], `merged=${merged}`);
    assert.deepEqual(followupRules({ merged, judgments, followups: out.followups }), [], `merged=${merged}`);
  }
});

test("(2) Engine は overlay の shadow のルールの指摘は adopt でも起こさず、本文だけの欠陥は本文の直しで直す", { skip }, () => {
  assert.deepEqual(engine({ ...PR_395, merged: false, judgments: [defect("adopt", "OV-001", { shadow: true })] }).followups, []);
  assert.deepEqual(engine({ ...PR_395, merged: false, judgments: [defect("adopt", "PR-002")] }).followups, [{ kind: "body_fix" }]);
});

// Engine の inspector_approved は今も adopt の合計（「採用 N」）だけを書くので外れる。Engine の basis の文を
// approvalBasis() の形に合わせるのは建設者（ワーカーの権限の外）。合わせたら todo を外して落とす検査にする
test("(3) Engine の inspector_approved の basis に欠陥の採用数と仮決めの承認数を分けて書く",
  { skip, todo: skip ? false : "Engine の basis が adopt の合計だけ（建設者が Engine を直したら todo を外す）" }, () => {
    const judgments = [...PR_395.judgments, defect("adopt", "BE-003", { location: "apps/backend/src/x.rs" })];
    const out = engine({ ...PR_395, merged: false, judgments });
    assert.equal(out.basis, approvalBasis(judgments));
    assert.deepEqual(checkRecord({ merged: false, judgments, followups: out.followups, basis: out.basis }), []);
  });
