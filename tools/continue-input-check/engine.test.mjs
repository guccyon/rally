// Engine の本物の続きのジョブの積み方（supervisor.conflict_step・ci_fix_step → continuation.enqueue）で、continue（conflict・ci_failed）の
// implement ジョブがワーカーに渡す入力に、その PR の未解消の採用済みの指摘の id と body_fix が残した本文の決まりが入ることを確かめる
// （知見 K-0009 の担保。議決 D-0013、Issue #955）。求めるもの（line.mjs の requiredInput）は、Engine が記録したイベントから組む。
// Engine のチェックアウトは環境変数 ENGINE_DIR で渡す（なければリポジトリの隣の ../engine）。どちらもなければ飛ばす。
// ただし ENGINE_DIR を渡したときと CI（CI=true）では飛ばさずに落とす（sendback-check の engine.test.mjs と同じ。議決 D-0011）。
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { checkInput, requiredInput } from "./line.mjs";

const harness = fileURLToPath(new URL("./engine_harness.py", import.meta.url));
const engineDir = process.env.ENGINE_DIR || fileURLToPath(new URL("../../../engine", import.meta.url));
const required = Boolean(process.env.ENGINE_DIR) || process.env.CI === "true";
const skip = required || existsSync(`${engineDir}/orchestrator/dispatcher/continuation.py`)
  ? false : `Engine のチェックアウトがない（ENGINE_DIR=${engineDir}）`;

const PR = "https://github.com/guccyon/rally/pull/659";
const RULE = "続きのジョブの報告は ## の見出しを足さず、「## 工場の記録」の下に ### で書く";

/** Engine で judge の裁き → body_fix → 続きのジョブ（reason）を 1 回動かし、ワーカーに渡す入力と、積んだ時点のイベント列を返す */
const engine = (record) =>
  JSON.parse(execFileSync("python3", ["-I", harness, engineDir], { input: JSON.stringify(record), encoding: "utf8" }));

const provisional = (verdict) => ({ subject: "provisional", verdict, defect: null });
const defect = (verdict, rubric_id, extra = {}) => ({ subject: "defect", verdict, defect: { rubric_id, location: "PR 本文", ...extra } });

/** PR #659（事例 C-0005）の形: 仮決めの承認と、本文の指摘 PR-002 の採用。body_fix が本文を直して決まりを残した */
const PR_659 = { pr_url: PR, judgments: [provisional("adopt"), defect("adopt", "PR-002")], body_rules: [RULE] };

// 取り出す関数（engine_harness.py の ENGINE_FUNCTIONS・CONTINUATION_NAMES）。Engine で名前や置き場が変わったら落ちる
const ENGINE_FUNCTIONS = ["record_judged", "act_on_judgment", "enqueue_body_fix", "is_body_only", "conflict_step", "ci_fix_step",
  "continuation.enqueue", "continuation.SIGNALED"];

test("(0) Engine のチェックアウトがあり、続きのジョブを積む関数をすべて取り出せる", { skip }, () => {
  let out;
  try {
    out = execFileSync("python3", ["-I", harness, "--check", engineDir], { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
  } catch (e) {
    assert.fail(`engine_harness.py が Engine から関数を取り出せない（ENGINE_DIR=${engineDir}）: ${e.stderr || e.message}`);
  }
  const { functions } = JSON.parse(out);
  for (const name of ENGINE_FUNCTIONS) assert.ok(functions.includes(name), `${name} を取り出せない`);
});

// 今の Engine は、conflict の続きのジョブには入力を渡さず（SIGNALED にない）、ci_failed には CI の合図（head_sha・failed）だけを渡す。
// (1) は必須のテストだが、Engine の入力がこの今の形（LEGACY）のときだけ todo にする（外れても落ちないが結果に出る）。sendback-check の
// todoWhileLegacy と同じ扱い（議決 D-0011）。Engine の入力が今の形から変わったら、その版からは指摘の id と決まりがなければ落ちる。
// 建設者が Engine の続きのジョブの入力に adopted_defects・body_rules を入れて ENGINE_DIR の版を上げたら、isLegacyInput と todo を消してよい
const LEGACY_SIGNAL_KEYS = ["failed", "head_sha"];
const LEGACY_TODO = "Engine の続きのジョブの入力が今の形（conflict は入力なし、ci_failed は CI の合図だけ。建設者が Engine を直したら必須になる）";

/** Engine の続きのジョブの入力が今の形か */
export function isLegacyInput(out) {
  if (out.reason === "conflict") return !out.piped && out.input == null;
  if (out.reason === "ci_failed") {
    const keys = Object.keys(out.delivered ?? {});
    return out.piped && keys.every((k) => LEGACY_SIGNAL_KEYS.includes(k));
  }
  return false;
}

test("(1) の todo は Engine の今の入力の形にだけ効き、指摘の id か決まりを入れ始めた形は必須にする", () => {
  assert.ok(isLegacyInput({ reason: "conflict", piped: false, input: null, delivered: null }));
  assert.ok(isLegacyInput({ reason: "ci_failed", piped: true, input: { head_sha: "h", failed: [] }, delivered: { head_sha: "h", failed: [] } }));
  assert.ok(!isLegacyInput({ reason: "conflict", piped: true, input: { adopted_defects: [] }, delivered: { adopted_defects: [] } }));
  assert.ok(!isLegacyInput({ reason: "conflict", piped: false, input: { adopted_defects: [] }, delivered: null }));
  const partial = { head_sha: "h", failed: [], body_rules: [RULE] };
  assert.ok(!isLegacyInput({ reason: "ci_failed", piped: true, input: partial, delivered: partial }));
  assert.ok(!isLegacyInput({ reason: "review_fix", piped: true, input: {}, delivered: {} }));
});

for (const reason of ["conflict", "ci_failed"]) {
  test(`(1) Engine は採用済みの指摘がある PR で continue（${reason}）を積むと、入力に指摘の id と本文の決まりを入れる`, { skip }, (t) => {
    const out = engine({ ...PR_659, reason });
    assert.equal(out.enqueued, true, "Engine が続きのジョブを積んでいない");
    const need = requiredInput(out.events, PR);
    if (isLegacyInput(out)) t.todo(LEGACY_TODO);
    assert.deepEqual(checkInput(need, out.delivered), []);
  });

  test(`(2) Engine の記録から、continue（${reason}）の入力に求めるもの（PR-002 と決まり）を組める`, { skip }, () => {
    // (1) が見るものが空でないこと（Engine の記録の形が変わって何も求めなくなると、(1) が黙って通る）
    const out = engine({ ...PR_659, reason });
    assert.deepEqual(requiredInput(out.events, PR), { defect_ids: ["PR-002"], body_rules: [RULE] });
    assert.ok(out.events.some((e) => e.name === "job.enqueued" && e.payload.kind === "body_fix"), "Engine が PR-002 を body_fix に回していない");
  });

  test(`(3) 採用済みの指摘がない PR では、continue（${reason}）の入力に何も求めない`, { skip }, () => {
    const out = engine({ pr_url: PR, reason, judgments: [provisional("adopt"), defect("reject", "PR-002"),
      defect("adopt", "OV-001", { shadow: true })], body_rules: [] });
    assert.equal(out.enqueued, true);
    assert.deepEqual(requiredInput(out.events, PR), { defect_ids: [], body_rules: [] });
    assert.deepEqual(checkInput(requiredInput(out.events, PR), out.delivered), []);
  });
}
