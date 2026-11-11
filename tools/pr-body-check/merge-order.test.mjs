import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { checkEvents, isBodyOnly } from "./merge-order.mjs";

// 事例の記録（.factory-config/knowledge/cases/C-0012.md の「出来事」と inspection・process のなぜなぜ）から組み直したイベント列（知見 K-0026 の担保。議決 D-0151、Issue #1392）。
// イベントそのものは事例の記録にないので、事例に書かれた時刻・順・件数を制御プレーンのイベントの形（Engine の supervisor・dispatcher が書く名前と payload）にした。
// judgment.judged の defect は Engine の Judgment の artifact の defect と同じ形で、イベントに載せたもの（merge-order.mjs の冒頭の注釈）。
const URL_982 = "https://github.com/guccyon/rally/pull/982";
const pr = { pr_url: URL_982 };
const CATALOG = "apps/admin-web/src/catalog/catalog.tsx";
/** 採用した検査の指摘の judgment.judged。location が PR 本文なら本文の指摘、ほかはコードの指摘。 */
const judged = (artifact_id, location, extra = {}) =>
  ({ name: "judgment.judged", job_id: 4950, payload: { ...pr, artifact_id, category: "inspection", verdict: "adopt", confidence: 0.9, basis: "self_report", mode: "shadow",
    defect: { rubric_id: "OV-005", severity: "should", location, evidence: "", remediation_hint: "" }, ...extra } });
const decided = (head_sha, table = "auto", change_types = ["code_change"]) =>
  ({ name: "supervisor.merge_decided", payload: { ...pr, head_sha, change_types, table, factory_grant: "auto", execute: table === "auto", executed: false } });
const approved = (head_sha) => ({ name: "supervisor.inspector_approved", payload: { ...pr, head_sha, basis: "judge が裁いた" } });
const merged = (head_sha) => ({ name: "supervisor.merged", payload: { ...pr, head_sha } });
const reviewFix = (job_id, head_sha = "d1") =>
  ({ name: "continue.enqueued", job_id, payload: { ...pr, head_sha, reason: "review_fix", attempt: 1, continue: {}, by: "judge" } });
const finished = (job_id, state = "succeeded") => ({ name: "job.finished", job_id, payload: { state } });

/**
 * C-0012（PR #982）: マージの判断（code_change=auto）が検査の前の 14:44:39 に出て、judge が検査の指摘 8 件をすべて採用し、
 * inspector_approved の 4 秒後の 14:47:58 にマージした。採用した指摘のうちコードのもの 3 件（catalog.tsx の二重持ちのラベル・期間の上限・
 * 2 つの期間の確かめ）は、直すジョブを挟まずに「続きのジョブか差し戻しで直す」としてマージ後の宿題に残った。
 * 記録にあるのは merge_decided とマージの時刻・承認からマージまでの秒数・採用の件数だけなので、日付・head・検査と裁きの時刻・
 * artifact の id は記録の順を崩さない範囲で置いた。残りの 5 件は本文の指摘とした（本文の直しは recheck.mjs の受け持ちなので組み直さない）。
 */
function c0012() {
  return [
    { ...decided("d1"), occurred_at: "2026-11-07T14:44:39+09:00" },
    { name: "supervisor.inspect_enqueued", payload: { ...pr, head_sha: "d1" }, occurred_at: "2026-11-07T14:44:40+09:00" },
    { name: "gate.evaluated", payload: { ...pr, mode: "shadow", gate: "rubric", checked: ["PR-002", "OV-005"] }, occurred_at: "2026-11-07T14:46:00+09:00" },
    { name: "supervisor.judge_enqueued", payload: { ...pr, head_sha: "d1", mode: "shadow", mode_basis: "default", skipped: false, n_provisional: 5, n_defects: 8 }, occurred_at: "2026-11-07T14:46:05+09:00" },
    ...[1, 2, 3, 4, 5].map((i) => ({ ...judged(9820 + i, "PR 本文"), occurred_at: "2026-11-07T14:47:40+09:00" })),
    { ...judged(9826, `${CATALOG}（スロットの状態のラベル）`), occurred_at: "2026-11-07T14:47:40+09:00" },
    { ...judged(9827, `${CATALOG}（期間の上限）`), occurred_at: "2026-11-07T14:47:40+09:00" },
    { ...judged(9828, `${CATALOG}（期間の確かめ）`), occurred_at: "2026-11-07T14:47:40+09:00" },
    { ...approved("d1"), occurred_at: "2026-11-07T14:47:54+09:00" },
    { ...merged("d1"), occurred_at: "2026-11-07T14:47:58+09:00" },
  ];
}

/** C-0012 の採用までの列（マージの判断と承認とマージを除く）。 */
const judgedPart = () => c0012().filter((e) => !["supervisor.merge_decided", "supervisor.inspector_approved", "supervisor.merged"].includes(e.name));

/**
 * 正しい並び: 検査と裁きのあと、採用したコードの指摘を続きのジョブ（job 5001）で直し、直した head（d2）を検査して裁き、
 * 承認してからマージを決めてマージする。
 */
function corrected() {
  return [
    ...judgedPart(),
    reviewFix(5001),
    finished(5001),
    { name: "supervisor.inspect_enqueued", payload: { ...pr, head_sha: "d2" } },
    { name: "gate.evaluated", payload: { ...pr, mode: "shadow", gate: "rubric", checked: ["PR-002", "OV-005"] } },
    { name: "supervisor.judge_enqueued", payload: { ...pr, head_sha: "d2", mode: "shadow", mode_basis: "default", skipped: true, n_provisional: 0, n_defects: 0 } },
    approved("d2"),
    decided("d2"),
    merged("d2"),
  ];
}

const rules = (r) => r.errors.map((e) => e.rule);
/** name のイベントの直前に足す。 */
const before = (events, name, add) => events.flatMap((e) => (e.name === name ? [...add, e] : [e]));

test("C-0012 の列は、検査と裁きの前にマージを決め、採用したコードの指摘を直さずにマージしたので落ちる", () => {
  const r = checkEvents(c0012());
  assert.equal(r.decisions, 1);
  assert.equal(r.merges, 1);
  assert.deepEqual(rules(r), ["merge_decided_before_review", "merged_without_code_fix"]);
  assert.ok(r.errors.every((e) => e.pr_url === URL_982));
  assert.match(r.errors[0].message, /code_change=auto、head d1/);
  assert.match(r.errors[0].message, /検査役の承認.*と裁き/);
  // 本文の指摘 5 件は数えず、コードの指摘 3 件だけ
  assert.match(r.errors[1].message, /3 件（judgment 9826・judgment 9827・judgment 9828）/);
});

test("正しい並び（検査と裁きと直しのあとにマージを決めてマージする）は通る", () => {
  const r = checkEvents(corrected());
  assert.equal(r.decisions, 1);
  assert.equal(r.merges, 1);
  assert.deepEqual(r.errors, []);
  // 人が直す答えを出した判断の受け渡し（judgment.fix_enqueued）で直しても通る
  const human = corrected().map((e) => (e.name === "continue.enqueued" ? { name: "judgment.fix_enqueued", payload: { ...pr, head_sha: "d1", job_id: 5001, decided: [9826] } } : e));
  assert.deepEqual(checkEvents(human).errors, []);
});

test("マージの判断が承認と裁きの後でも、採用したコードの指摘を直すジョブがないままマージすれば落ちる", () => {
  const r = checkEvents([...judgedPart(), approved("d1"), decided("d1"), merged("d1")]);
  assert.deepEqual(rules(r), ["merged_without_code_fix"]);
  // 差し戻しを起票しただけでは直したことにならない
  const filed = { name: "sendback.filed", payload: { origin_pr: URL_982, fix_issue: 990, fix_corr: "github:guccyon/rally#990", mode: "shadow", defects: 3, replaced: 0 } };
  assert.deepEqual(rules(checkEvents([...judgedPart(), filed, approved("d1"), decided("d1"), merged("d1")])), ["merged_without_code_fix"]);
});

test("マージの判断が承認か裁きのどちらかより前なら落ちる（head が変われば、その head の承認と裁きを待つ）", () => {
  const noJudge = [...judgedPart().filter((e) => e.name !== "judgment.judged" && e.name !== "supervisor.judge_enqueued"), approved("d1"), decided("d1"), merged("d1")];
  const r = checkEvents(noJudge);
  assert.deepEqual(rules(r), ["merge_decided_before_review"]);
  assert.match(r.errors[0].message, /より前に来た/);
  assert.doesNotMatch(r.errors[0].message, /検査役の承認/);
  // 承認の前（裁きの後）
  const r2 = checkEvents(corrected().filter((e) => !(e.name === "supervisor.inspector_approved")));
  assert.deepEqual(rules(r2), ["merge_decided_before_review"]);
  assert.match(r2.errors[0].message, /検査役の承認/);
  // 前の head（d1）の承認と裁きだけで、直したあとの head（d2）のマージを決めた
  const stale = corrected().filter((e) => !(["supervisor.judge_enqueued", "supervisor.inspector_approved"].includes(e.name) && e.payload.head_sha === "d2"));
  assert.deepEqual(rules(checkEvents(before(stale, "continue.enqueued", [approved("d1")]))), ["merge_decided_before_review"]);
});

test("直すジョブが終わる前・失敗で終わったままマージすれば落ち、直し終えてからマージすれば通る", () => {
  const tail = [approved("d1"), decided("d1"), merged("d1")];
  const r = checkEvents([...judgedPart(), reviewFix(5001), ...tail]);
  assert.deepEqual(rules(r), ["merged_before_code_fix_finished"]);
  assert.match(r.errors[0].message, /job 5001/);
  // 直しが失敗で終わったら、直すジョブのないものに戻る
  assert.deepEqual(rules(checkEvents([...judgedPart(), reviewFix(5001), finished(5001, "failed"), ...tail])), ["merged_without_code_fix"]);
  // 積み直した直しが成功で終われば通る
  assert.deepEqual(checkEvents([...judgedPart(), reviewFix(5001), finished(5001, "failed"), reviewFix(5002), finished(5002), ...tail]).errors, []);
  // 直しが終わったあとに採用した新しいコードの指摘は、別に直す
  const later = [...judgedPart(), reviewFix(5001), finished(5001), judged(9830, CATALOG), ...tail];
  assert.match(checkEvents(later).errors[0].message, /1 件（judgment 9830）/);
});

test("数えない指摘と見ないマージの判断", () => {
  const tail = [approved("d1"), decided("d1"), merged("d1")];
  const base = judgedPart().filter((e) => e.name !== "judgment.judged");
  // 本文だけの指摘（Engine の is_body_only と同じ線）・overlay の shadow のルール・機械的に直せないとしたもの・採用しなかったもの・仮決め
  const skipped = [
    judged(1, "PR #982 本文"), judged(2, "pr body"), judged(3, CATALOG, { mechanical: false }),
    { ...judged(4, CATALOG), payload: { ...judged(4, CATALOG).payload, defect: { location: CATALOG, shadow: true } } },
    judged(5, CATALOG, { verdict: "reject" }), judged(6, CATALOG, { category: "scope" }),
  ];
  assert.deepEqual(checkEvents([...base, ...skipped, ...tail]).errors, []);
  // 箇所が分からない指摘はコードの指摘として数える（止める側に倒す）
  const unknown = { ...judged(7, ""), payload: { ...judged(7, "").payload, defect: undefined } };
  assert.deepEqual(rules(checkEvents([...base, unknown, ...tail])), ["merged_without_code_fix"]);
  assert.equal(isBodyOnly("PR 本文の ## 概要"), true);
  assert.equal(isBodyOnly(`${CATALOG}（PR 本文に書いた定数）`), false);
  // 表が human_approve のもの・code_change を含まないもの（docs_only）は、検査の前に出ても見ない
  for (const d of [decided("d1", "human_approve"), decided("d1", "auto", ["docs_only"])]) {
    const r = checkEvents([d, ...judgedPart().slice(1)]);
    assert.equal(r.decisions, 0);
    assert.deepEqual(rules(r), []);
  }
});

test("PR ごとに見る（ほかの PR の直しのジョブの終わりで閉じない）", () => {
  const URL_OTHER = "https://github.com/guccyon/rally/pull/983";
  const other = [{ name: "continue.enqueued", job_id: 6001, payload: { pr_url: URL_OTHER, reason: "review_fix" } }, finished(6001)];
  const r = checkEvents([...judgedPart(), reviewFix(5001), ...other, approved("d1"), decided("d1"), merged("d1")]);
  assert.deepEqual(rules(r), ["merged_before_code_fix_finished"]);
  const mixed = [...c0012(), { name: "supervisor.merged", payload: { pr_url: URL_OTHER } }];
  assert.deepEqual(checkEvents(mixed).errors.map((e) => e.pr_url), [URL_982, URL_982]);
  assert.deepEqual(rules(checkEvents(mixed, URL_OTHER)), []);
});

test("CLI: 終了コード 0・1・2 と --json", () => {
  const cli = fileURLToPath(new URL("./merge-order.mjs", import.meta.url));
  const dir = mkdtempSync(join(tmpdir(), "pr-body-merge-order-"));
  const run = (...args) => {
    try {
      return { code: 0, out: execFileSync(process.execPath, [cli, ...args], { encoding: "utf8", stdio: "pipe" }) };
    } catch (e) {
      return { code: e.status, out: e.stdout, err: e.stderr };
    }
  };
  const bad = join(dir, "c0012.jsonl");
  writeFileSync(bad, c0012().map((e) => JSON.stringify(e)).join("\n") + "\n");
  const json = join(dir, "out.json");
  const r1 = run("--events", bad, "--json", json);
  assert.equal(r1.code, 1);
  assert.match(r1.out, /merge_decided_before_review/);
  assert.deepEqual(JSON.parse(readFileSync(json, "utf8")).errors.map((e) => e.rule), ["merge_decided_before_review", "merged_without_code_fix"]);

  const good = join(dir, "good.json");
  writeFileSync(good, JSON.stringify({ events: corrected() }));
  const r0 = run("--events", good);
  assert.equal(r0.code, 0);
  assert.match(r0.out, /マージの判断 1 件、マージ 1 件、外れ 0 件/);
  assert.equal(run("--events", bad, "--pr", "https://github.com/guccyon/rally/pull/1").code, 0);

  const broken = join(dir, "broken.json");
  writeFileSync(broken, `[{ "payload": {} }]`);
  assert.equal(run("--events", broken).code, 2);
  assert.equal(run().code, 2);
});
