import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { checkEvents } from "./recheck.mjs";

// 事例の記録（.factory-config/knowledge/cases/C-0003.md・C-0005.md・C-0006.md の「出来事」）から組み直したイベント列。
// イベントそのものは事例の記録にないので、事例に書かれた時刻・job・順を制御プレーンのイベントの形（Engine の dispatcher・supervisor が書く名前と payload）にした。
const URL_659 = "https://github.com/guccyon/rally/pull/659";
const URL_821 = "https://github.com/guccyon/rally/pull/821";
const URL_396 = "https://github.com/guccyon/rally/pull/396";
/** 検査役の rubric（shadow_review.py）が見た rubric の id（gate.evaluated の checked）。本文の構成の PR-002 を含む。 */
const RUBRIC_CHECKED = ["PR-001", "PR-002", "CODE-001"];

/**
 * C-0003（PR #396）: 2 回目の検査（job 570）の 14:21:30 のあと、最後の本文の直し（job 572）を積んで終わり、head_sha が変わらないまま
 * 検査をもう一度通さずに 14:23:30 にマージした。記録にあるのは検査の job と時刻・job 563・564・572・マージの時刻だけなので、
 * 直しの積みと終わり・承認の時刻は記録の順を崩さない範囲で置いた（1 回目の直しの job 563 は 2 回目の検査より前に終わっている）。
 */
function c0003() {
  const pr = { pr_url: URL_396 };
  return [
    { name: "gate.evaluated", job_id: 560, payload: { ...pr, mode: "shadow", gate: "rubric", checked: RUBRIC_CHECKED }, occurred_at: "2026-11-01T14:15:47+09:00" },
    { name: "job.enqueued", job_id: 563, payload: { ...pr, by: "judge", kind: "body_fix" }, occurred_at: "2026-11-01T14:16:30+09:00" },
    { name: "job.enqueued", job_id: 564, payload: { ...pr, by: "judge", kind: "implement" }, occurred_at: "2026-11-01T14:16:30+09:00" },
    { name: "pr.body_amended", job_id: 563, payload: { ...pr, defects: 1 }, occurred_at: "2026-11-01T14:18:00+09:00" },
    { name: "gate.evaluated", job_id: 570, payload: { ...pr, mode: "shadow", gate: "rubric", checked: RUBRIC_CHECKED }, occurred_at: "2026-11-01T14:21:30+09:00" },
    { name: "job.enqueued", job_id: 572, payload: { ...pr, by: "judge", kind: "body_fix" }, occurred_at: "2026-11-01T14:22:00+09:00" },
    { name: "pr.body_amended", job_id: 572, payload: { ...pr, defects: 1 }, occurred_at: "2026-11-01T14:23:00+09:00" },
    { name: "supervisor.inspector_approved", payload: { ...pr, head_sha: "c1" }, occurred_at: "2026-11-01T14:23:20+09:00" },
    { name: "supervisor.merged", payload: { ...pr, head_sha: "c1" }, occurred_at: "2026-11-01T14:23:30+09:00" },
  ];
}

/** C-0005（PR #659）の最後の周回: 本文の直し（job 3308）を積み、終わる前の 20:08:43 にマージし、本文が直されたのは 20:08:50。 */
function c0005() {
  const pr = { pr_url: URL_659 };
  return [
    { name: "supervisor.inspect_enqueued", payload: { ...pr, head_sha: "h21" }, occurred_at: "2026-11-04T20:01:10+09:00" },
    { name: "gate.evaluated", payload: { ...pr, mode: "shadow", gate: "rubric", checked: RUBRIC_CHECKED }, occurred_at: "2026-11-04T20:05:02+09:00" },
    { name: "supervisor.judge_enqueued", payload: { ...pr, head_sha: "h21" }, occurred_at: "2026-11-04T20:05:20+09:00" },
    { name: "judgment.judged", payload: { ...pr, verdict: "adopt", category: "inspection" }, occurred_at: "2026-11-04T20:08:30+09:00" },
    { name: "job.enqueued", job_id: 3308, payload: { ...pr, by: "judge", kind: "body_fix" }, occurred_at: "2026-11-04T20:08:31+09:00" },
    { name: "supervisor.inspector_approved", payload: { ...pr, head_sha: "h21" }, occurred_at: "2026-11-04T20:08:42+09:00" },
    { name: "supervisor.merged", payload: { ...pr, head_sha: "h21" }, occurred_at: "2026-11-04T20:08:43+09:00" },
    { name: "pr.body_amended", job_id: 3308, payload: { ...pr, defects: 1 }, occurred_at: "2026-11-04T20:08:50+09:00" },
  ];
}

/** C-0006（PR #821）: 検査（job 4297）の 14:23:16 のあと、本文の直し（job 4299）が 14:25:04 に終わり、その 2 秒後の 14:25:06 にマージした。 */
function c0006() {
  const pr = { pr_url: URL_821 };
  return [
    { name: "supervisor.inspect_enqueued", payload: { ...pr, head_sha: "s1" }, occurred_at: "2026-11-06T14:15:40+09:00" },
    { name: "gate.evaluated", job_id: 4297, payload: { ...pr, mode: "shadow", gate: "rubric", checked: RUBRIC_CHECKED }, occurred_at: "2026-11-06T14:23:16+09:00" },
    { name: "supervisor.judge_enqueued", payload: { ...pr, head_sha: "s1" }, occurred_at: "2026-11-06T14:23:20+09:00" },
    { name: "judgment.judged", payload: { ...pr, verdict: "adopt", category: "inspection" }, occurred_at: "2026-11-06T14:24:40+09:00" },
    { name: "job.enqueued", job_id: 4299, payload: { ...pr, by: "judge", kind: "body_fix" }, occurred_at: "2026-11-06T14:24:41+09:00" },
    { name: "supervisor.inspector_approved", payload: { ...pr, head_sha: "s1" }, occurred_at: "2026-11-06T14:25:02+09:00" },
    { name: "pr.body_amended", job_id: 4299, payload: { ...pr, defects: 1 }, occurred_at: "2026-11-06T14:25:04+09:00" },
    { name: "supervisor.merged", payload: { ...pr, head_sha: "s1" }, occurred_at: "2026-11-06T14:25:06+09:00" },
  ];
}

const rules = (r) => r.errors.map((e) => e.rule);
const without = (events, name) => events.filter((e) => e.name !== name);
/** name のイベントの直前に足す。 */
const before = (events, name, add) => events.flatMap((e) => (e.name === name ? [add, e] : [e]));
const recheck = (pr_url, gate = "rubric", checked = RUBRIC_CHECKED) =>
  ({ name: "gate.evaluated", payload: { pr_url, mode: "shadow", gate, ...(gate === "rubric" ? { checked } : {}) } });

test("C-0006 の列は、直した本文を再検査せずにマージしたので落ちる", () => {
  const r = checkEvents(c0006());
  assert.equal(r.merges, 1);
  assert.deepEqual(rules(r), ["merged_without_body_recheck"]);
  assert.equal(r.errors[0].pr_url, URL_821);
  assert.match(r.errors[0].message, /job 4299/);
});

test("C-0005 の列は、本文の直しが終わる前にマージしたので落ちる", () => {
  const r = checkEvents(c0005());
  assert.equal(r.merges, 1);
  assert.deepEqual(rules(r), ["merged_before_body_fix_finished"]);
  assert.equal(r.errors[0].pr_url, URL_659);
  assert.match(r.errors[0].message, /job 3308/);
});

test("C-0003 の列は、最後の本文の直し（job 572）のあと検査を通さずにマージしたので落ちる", () => {
  const r = checkEvents(c0003());
  assert.equal(r.merges, 1);
  assert.deepEqual(rules(r), ["merged_without_body_recheck"]);
  assert.equal(r.errors[0].pr_url, URL_396);
  assert.match(r.errors[0].message, /job 572/);
  // 1 回目の直し（job 563）は 2 回目の検査（job 570）で再検査されているので、job 572 の直しがなければ通る
  const upTo570 = c0003().filter((e) => e.job_id !== 572);
  assert.deepEqual(checkEvents(upTo570).errors, []);
  // job 572 の直しのあとに本文を検査し直してからマージすれば通る
  assert.deepEqual(checkEvents(before(c0003(), "supervisor.merged", recheck(URL_396))).errors, []);
});

test("2 つの事例を混ぜた列でも、PR ごとに 1 件ずつ落ちる", () => {
  const r = checkEvents([...c0005(), ...c0006()]);
  assert.deepEqual(r.errors.map((e) => [e.pr_url, e.rule]),
    [[URL_659, "merged_before_body_fix_finished"], [URL_821, "merged_without_body_recheck"]]);
  assert.deepEqual(rules(checkEvents([...c0005(), ...c0006()], URL_821)), ["merged_without_body_recheck"]);
});

test("直した本文を再検査してからマージすれば通る（検査役の rubric も pr-body-check も本文の検査に数える）", () => {
  for (const gate of ["rubric", "pr-body-check"]) {
    assert.deepEqual(checkEvents(before(c0006(), "supervisor.merged", recheck(URL_821, gate))).errors, [], gate);
  }
  // C-0005 も、直しの終わりと再検査を待ってからマージすれば通る
  const merged = c0005().find((e) => e.name === "supervisor.merged");
  const waited = [...without(c0005(), "supervisor.merged"), recheck(URL_659), merged];
  assert.deepEqual(checkEvents(waited).errors, []);
});

test("supervisor が直した本文の pr-body-check を確かめて残した supervisor.body_rechecked も再検査に数える（議決 D-0050、Issue #1122）", () => {
  const rechecked = (pr_url) => ({ name: "supervisor.body_rechecked",
    payload: { pr_url, head_sha: "s1", gate: "pr-body-check", job_id: 4299, action: "edited", created_at: "2026-11-06T05:25:03Z" } });
  // C-0006 の列で、pr.body_amended のあと supervisor の再検査を挟んでからマージすれば通る
  const waited = before(c0006(), "supervisor.merged", rechecked(URL_821));
  assert.deepEqual(checkEvents(waited).errors, []);
  // 直す前に残した再検査とほかの PR の再検査は数えない
  assert.deepEqual(rules(checkEvents(before(c0006(), "pr.body_amended", rechecked(URL_821)))), ["merged_without_body_recheck"]);
  assert.deepEqual(rules(checkEvents(before(c0006(), "supervisor.merged", rechecked(URL_659)))), ["merged_without_body_recheck"]);
});

test("supervisor.body_rechecked は、直した本文で始まった実行（edited か、pr.body_amended 以後に始まった）のものだけ数える（D-0100 の反対票の反例、Issue #1268）", () => {
  const rechecked = (action, created_at) => ({ name: "supervisor.body_rechecked",
    payload: { pr_url: URL_821, head_sha: "s2", gate: "pr-body-check", job_id: 4299, action, created_at } });
  const merged = (ev) => rules(checkEvents(before(c0006(), "supervisor.merged", ev)));
  // C-0006 の直し（14:24:41 に積み、pr.body_amended 14:25:04）の最中に push が来て synchronize で始まり、直す前の本文で通った実行の記録は数えない
  assert.deepEqual(merged(rechecked("synchronize", "2026-11-06T05:24:55Z")), ["merged_without_body_recheck"]);
  // 前の実行（PR を開いたとき）の再実行も、もとのイベントの時刻は直す前なので数えない。時刻も action もない記録も数えない
  assert.deepEqual(merged(rechecked("opened", "2026-11-06T05:10:00Z")), ["merged_without_body_recheck"]);
  assert.deepEqual(merged(rechecked(undefined, undefined)), ["merged_without_body_recheck"]);
  // 直しの書き換えの edited で始まった実行は、pr.body_amended より前に始まっていても数える（待ち続けない）
  assert.deepEqual(merged(rechecked("edited", "2026-11-06T05:25:03Z")), []);
  // pr.body_amended 以後に始まった実行は action を問わず数える（直したあとの push、run-name に action のない写し）
  assert.deepEqual(merged(rechecked("synchronize", "2026-11-06T05:25:04Z")), []);
  assert.deepEqual(merged(rechecked(null, "2026-11-06T05:25:05Z")), []);
});

test("直す前の検査や、ほかの PR の検査は再検査に数えない", () => {
  // C-0006 の gate.evaluated（14:23:16）は直す前なので、それだけでは通らない（上の C-0006 のテスト）。ほかの PR の検査を挟んでも落ちる
  assert.deepEqual(rules(checkEvents(before(c0006(), "supervisor.merged", recheck(URL_659)))), ["merged_without_body_recheck"]);
});

test("pr.body_amended のあとに本文と関係のない gate の gate.evaluated だけが来てからマージしたら、再検査に数えず落ちる（議決 D-0072、Issue #1206）", () => {
  // C-0006 の列で、直しの 14:25:04 とマージの 14:25:06 の間にコードの検査だけを挟む
  const codeChecks = [
    { name: "gate.evaluated", payload: { pr_url: URL_821, mode: "shadow", gate: "overlay", fires: [] } },
    { name: "gate.evaluated", payload: { pr_url: URL_821, gate: "L1", result: "pass", defects: [] } },
    recheck(URL_821, "rubric", ["CODE-001", "ARCH-002"]),   // 本文の rubric（PR-002）を見ていない rubric
    { name: "gate.evaluated", payload: { pr_url: URL_821, mode: "shadow", gate: "rubric" } },   // 見た rubric の記録がない
  ];
  for (const ev of codeChecks) {
    const r = checkEvents(before(c0006(), "supervisor.merged", ev));
    assert.deepEqual(rules(r), ["merged_without_body_recheck"], JSON.stringify(ev.payload));
    assert.match(r.errors[0].message, /job 4299/);
  }
  const all = codeChecks.reduce((events, ev) => before(events, "supervisor.merged", ev), c0006());
  assert.deepEqual(rules(checkEvents(all)), ["merged_without_body_recheck"]);
  // コードの検査のあとに本文の検査（pr-body-check か PR-002 を見た rubric）が来れば通る
  for (const body of [recheck(URL_821, "pr-body-check"), recheck(URL_821, "rubric", ["PR-002"])]) {
    assert.deepEqual(checkEvents(before(all, "supervisor.merged", body)).errors, [], JSON.stringify(body.payload));
  }
});

test("再検査のあとにもう一度本文を直したら、その直しの再検査も要る", () => {
  const amended = c0006().find((e) => e.name === "pr.body_amended");
  const events = before(c0006(), "supervisor.merged", recheck(URL_821));
  assert.deepEqual(rules(checkEvents(before(events, "supervisor.merged", { ...amended, job_id: 4310 }))), ["merged_without_body_recheck"]);
});

test("本文の直しがない PR や、まだマージしていない PR は通る", () => {
  assert.deepEqual(checkEvents(without(without(c0006(), "job.enqueued"), "pr.body_amended")).errors, []);
  assert.deepEqual(checkEvents(without(c0006(), "supervisor.merged")), { merges: 0, errors: [] });
});

test("job_id のない記録でも、積んだ直しと終わりを古い順に結ぶ", () => {
  const strip = (events) => events.map(({ job_id, ...e }) => e);
  assert.deepEqual(rules(checkEvents(strip(c0005()))), ["merged_before_body_fix_finished"]);
  assert.deepEqual(rules(checkEvents(strip(c0006()))), ["merged_without_body_recheck"]);
});

test("本文の直しが失敗で終わったあと、直しが成功しないままマージしたら落ちる", () => {
  const failed = c0005().map((e) => (e.name === "pr.body_amended" ? { ...e, name: "pr.body_amend_failed" } : e));
  const merged = failed.find((e) => e.name === "supervisor.merged");
  const afterFailure = [...without(failed, "supervisor.merged"), merged];
  const r = checkEvents(afterFailure);
  assert.deepEqual(rules(r), ["merged_after_body_fix_failed"]);
  assert.match(r.errors[0].message, /job 3308/);
  // job_id のない記録でも同じ
  assert.deepEqual(rules(checkEvents(afterFailure.map(({ job_id, ...e }) => e))), ["merged_after_body_fix_failed"]);
  // C-0005 の順（失敗の終わりがマージの後）なら、終わる前のマージとして落ちる
  assert.deepEqual(rules(checkEvents(failed)), ["merged_before_body_fix_finished"]);
  // 失敗のあと直し直して成功し、再検査してからマージすれば通る
  const pr = { pr_url: URL_659 };
  const retried = [...without(failed, "supervisor.merged"),
    { name: "job.enqueued", job_id: 3315, payload: { ...pr, kind: "body_fix" } },
    { name: "pr.body_amended", job_id: 3315, payload: pr }, recheck(URL_659), merged];
  assert.deepEqual(checkEvents(retried).errors, []);
});

test("CLI: 終了コード 0・1・2 と --json", () => {
  const cli = fileURLToPath(new URL("./recheck.mjs", import.meta.url));
  const dir = mkdtempSync(join(tmpdir(), "pr-body-recheck-"));
  const run = (...args) => {
    try {
      return { code: 0, out: execFileSync(process.execPath, [cli, ...args], { encoding: "utf8", stdio: "pipe" }) };
    } catch (e) {
      return { code: e.status, out: e.stdout, err: e.stderr };
    }
  };
  const bad = join(dir, "c0006.jsonl");
  writeFileSync(bad, c0006().map((e) => JSON.stringify(e)).join("\n") + "\n");
  const json = join(dir, "out.json");
  const r1 = run("--events", bad, "--json", json);
  assert.equal(r1.code, 1);
  assert.match(r1.out, /merged_without_body_recheck/);
  assert.deepEqual(JSON.parse(readFileSync(json, "utf8")).errors.map((e) => e.rule), ["merged_without_body_recheck"]);

  const good = join(dir, "good.json");
  writeFileSync(good, JSON.stringify({ events: before(c0006(), "supervisor.merged", recheck(URL_821)) }));
  const r0 = run("--events", good);
  assert.equal(r0.code, 0);
  assert.match(r0.out, /マージ 1 件、外れ 0 件/);
  assert.equal(run("--events", bad, "--pr", URL_659).code, 0);

  const broken = join(dir, "broken.json");
  writeFileSync(broken, `[{ "payload": {} }]`);
  assert.equal(run("--events", broken).code, 2);
  assert.equal(run().code, 2);
});
