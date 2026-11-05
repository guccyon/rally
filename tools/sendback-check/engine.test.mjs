// Engine の本物の判定（judgment.record_judged → dispatcher.act_on_judgment、supervisor.inspector_approved）を
// check.test.mjs と同じ 3 つの場合で確かめる（知見 K-0004 の担保。議決 D-0009・D-0010、Issue #935・#944・#945）。
// 参照の実装（check.mjs）ではなく本物の判定の担保で、check.mjs の ci_check の配線より先に入れる（D-0010）。
// Engine のチェックアウトは環境変数 ENGINE_DIR で渡す（なければリポジトリの隣の ../engine）。どちらもなければ飛ばす。
// ただし ENGINE_DIR を渡したときと CI（CI=true）では飛ばさずに落とす（議決 D-0011、Issue #948）。
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { approvalBasis, checkRecord } from "./check.mjs";

const harness = fileURLToPath(new URL("./engine_harness.py", import.meta.url));
const script = fileURLToPath(new URL("./check.mjs", import.meta.url));
const engineDir = process.env.ENGINE_DIR || fileURLToPath(new URL("../../../engine", import.meta.url));
const required = Boolean(process.env.ENGINE_DIR) || process.env.CI === "true";
const skip = required || existsSync(`${engineDir}/orchestrator/dispatcher/supervisor.py`)
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

// 取り出す関数（engine_harness.py の JUDGMENT_FUNCTIONS・ENGINE_FUNCTIONS）。Engine で名前や置き場が変わったら落ちる
const ENGINE_FUNCTIONS = ["record_judged", "act_on_judgment", "file_sendback", "enqueue_body_fix", "is_body_only", "inspector_approved"];

test("(0) Engine のチェックアウトがあり、判定の関数をすべて取り出せる", { skip }, () => {
  let out;
  try {
    out = execFileSync("python3", ["-I", harness, "--check", engineDir], { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
  } catch (e) {
    assert.fail(`engine_harness.py が Engine から関数を取り出せない（ENGINE_DIR=${engineDir}）: ${e.stderr || e.message}`);
  }
  const { functions } = JSON.parse(out);
  for (const name of ENGINE_FUNCTIONS) assert.ok(functions.includes(name), `${name} を取り出せない`);
});

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
// approvalBasis() の形に合わせるのは建設者（ワーカーの権限の外）。合わせたら todo を外して落とす検査にする。
// todo のうちに check.mjs の ci_check を配線すると、すべての承認が basis_not_split で落ちる（D-0010 の順序）
test("(3) Engine の inspector_approved の basis に欠陥の採用数と仮決めの承認数を分けて書く",
  { skip, todo: skip ? false : "Engine の basis が adopt の合計だけ（建設者が Engine を直したら todo を外す）" }, () => {
    const judgments = [...PR_395.judgments, defect("adopt", "BE-003", { location: "apps/backend/src/x.rs" })];
    const out = engine({ ...PR_395, merged: false, judgments });
    assert.equal(out.basis, approvalBasis(judgments));
    assert.deepEqual(checkRecord({ merged: false, judgments, followups: out.followups, basis: out.basis }), []);
  });

// イベントの経路（Issue #945）: Engine が記録したイベント列を、README の SQL と同じ形で check.mjs に渡す。
// ci_check がワークフローで走らせるのと同じ呼び方（--events --pr --summary、GITHUB_ACTIONS=true）で CLI を動かす

/** イベント列を ci_check と同じ呼び方で check.mjs に渡し、終了コード・標準出力・JSON・要約を返す */
function ciCheck(events, prUrl = PR_395.pr_url) {
  const dir = mkdtempSync(join(tmpdir(), "sendback-engine-events-"));
  writeFileSync(join(dir, "events.json"), JSON.stringify(events, null, 2));
  writeFileSync(join(dir, "summary.md"), "");
  const r = spawnSync("node", [script, "--events", join(dir, "events.json"), "--pr", prUrl, "--json", join(dir, "out.json"),
    "--summary", join(dir, "summary.md")], { env: { ...process.env, GITHUB_ACTIONS: "true" }, encoding: "utf8" });
  return { status: r.status, stdout: r.stdout, result: JSON.parse(readFileSync(join(dir, "out.json"), "utf8")),
    summary: readFileSync(join(dir, "summary.md"), "utf8") };
}

const adoptedDefect = defect("adopt", "BE-003", { location: "apps/backend/src/x.rs" });
/** Engine の今の判定で後続が線に合う場合（PR #395 と同じ入力、直っていない採用欠陥のマージ前・後、本文だけの欠陥） */
const ENGINE_CASES = [
  ["PR #395 と同じ入力", PR_395],
  ["採用欠陥・マージ前", { ...PR_395, merged: false, judgments: [...PR_395.judgments, adoptedDefect] }],
  ["採用欠陥・マージ後", { ...PR_395, merged: true, judgments: [...PR_395.judgments, adoptedDefect] }],
  ["本文だけの欠陥", { ...PR_395, merged: false, judgments: [defect("adopt", "PR-002")] }],
];

test("イベントの経路: Engine が記録したイベント列を check.mjs --events にそのまま渡すと、1 つの head に組めて後続の判定は線に合う", { skip }, () => {
  for (const [name, record] of ENGINE_CASES) {
    const out = engine(record);
    const { result, stdout } = ciCheck(out.events);
    assert.deepEqual(result.errors.map((e) => e.rule).filter((r) => !r.startsWith("basis_")), [], name);
    assert.match(stdout, new RegExp(`head 1 個・裁き ${record.judgments.length} 件`), name);
  }
});

test("イベントの経路: PR #395 の流れ（直すもののない差し戻し）を Engine の形のイベント列で落とす", { skip }, () => {
  // PR #395 と同じ入力の Engine のイベント列に、Engine が差し戻しで記録する sendback.filed（採用欠陥のマージ後の場合から取る）を足す
  const sendback = engine(ENGINE_CASES[2][1]).events.find((e) => e.name === "sendback.filed");
  assert.ok(sendback, "Engine が採用欠陥のマージ後に sendback.filed を記録していない");
  const { status, stdout, result, summary } = ciCheck([...engine(PR_395).events, sendback]);
  assert.equal(status, 1);
  assert.ok(result.errors.some((e) => e.rule === "followup_without_defect" && e.pr_url === PR_395.pr_url && e.head_sha === "head1"));
  assert.match(stdout, /^::error title=sendback-check followup_without_defect::https:\/\/github\.com\/guccyon\/rally\/pull\/395@head1 /m);
  assert.match(summary, /外れた: https:\/\/github\.com\/guccyon\/rally\/pull\/395@head1（裁き 5 件。欠陥の採用 0・仮決めの承認 3）/);
});

// 今の Engine の basis は adopt の合計だけなので、承認のある head はすべて basis_not_split で落ちる（ci_check を配線してはいけない状態）。
// 建設者が Engine の basis を直したらこの todo と (3) の todo を外し、通ってから ci_check を配線する（D-0010 の順序）
test("イベントの経路: Engine の正しい判定のイベント列は check.mjs --events で通る（ci_check を配線してよい合図）",
  { skip, todo: skip ? false : "Engine の basis が adopt の合計だけで basis_not_split で落ちる（建設者が Engine を直したら todo を外す）" }, () => {
    for (const [name, record] of ENGINE_CASES) {
      const { status, result } = ciCheck(engine(record).events);
      assert.deepEqual(result.errors, [], name);
      assert.equal(status, 0, name);
    }
  });
