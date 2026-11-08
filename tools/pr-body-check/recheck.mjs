// 本文の直し（body_fix）のあとの再検査を待たずにマージしていないかを、制御プレーンのイベント列で決定論に見る（知見 K-0012 の担保。議決 D-0033、Issue #1053）。LLM を使わない。
//
//   node recheck.mjs --events <制御プレーンのイベントの JSON か JSONL> [--pr <PR の URL>] [--json <出力先>]
//   終了コード: 0 = 通る、1 = 外れた、2 = イベント列を読めなかった
//
// イベント列は古い順に並べて渡す（tools/sendback-check の check.mjs --events と同じ形。読み方も parseEvents を使う）。
// 同じ PR の supervisor.merged ごとに、それより前のイベントを見る:
//   - pr.body_amended（本文の直しが終わった）のあと、本文の検査（gate.evaluated）がないまま supervisor.merged が来た（merged_without_body_recheck。事例 C-0006）
//   - 本文の直し（job.enqueued の kind が body_fix）を積んだのに、その終わり（pr.body_amended か pr.body_amend_failed）より前に
//     supervisor.merged が来た（merged_before_body_fix_finished。事例 C-0005 は、直しの終わりがマージの 7 秒後）
//   - 本文の直しが失敗で終わり（pr.body_amend_failed）、そのあと直しが成功しないまま supervisor.merged が来た（merged_after_body_fix_failed）
// 本文の検査は gate.evaluated で数える。gate は問わない（検査役の rubric も、pr-body-check を gate: "pr-body-check" で記録したものも数える）。
// supervisor が直した本文の pr-body-check が通ったのを確かめて残す supervisor.body_rechecked（engine/pr-body-recheck.patch。議決 D-0050、Issue #1122）も数える。
// 本文の直しの積みと終わりは job_id で結ぶ。終わりに job_id がなければ同じ PR の古いものから、積んだ側に job_id がなければ job_id のないものと結ぶ。

import { readFileSync, writeFileSync } from "node:fs";
import { parseArgs } from "node:util";

import { parseEvents } from "../sendback-check/check.mjs";

/** 本文の検査として数えるイベント。 */
export const BODY_CHECK_EVENTS = Object.freeze(["gate.evaluated", "supervisor.body_rechecked"]);
/** 本文の直しの終わり（成功と失敗）。 */
export const BODY_FIX_FINISHED = Object.freeze(["pr.body_amended", "pr.body_amend_failed"]);

const prOf = (e) => e.payload?.pr_url ?? null;
const isBodyFixEnqueued = (e) => e.name === "job.enqueued" && e.payload?.kind === "body_fix";

/**
 * events: [{ name, payload: { pr_url, ... }, job_id?, occurred_at? }]（古い順）。prUrl を渡せばその PR だけを見る。
 * 返す: { merges: 見た supervisor.merged の件数, errors: [{ rule, pr_url, message }] }
 */
export function checkEvents(events, prUrl) {
  const byPr = new Map();
  const errors = [];
  let merges = 0;
  for (const e of events) {
    const pr = prOf(e);
    if (!pr || (prUrl && pr !== prUrl)) continue;
    if (!byPr.has(pr)) byPr.set(pr, { pendingFixes: [], amendedUnchecked: null, fixFailed: null });
    const st = byPr.get(pr);
    if (isBodyFixEnqueued(e)) {
      st.pendingFixes.push(e.job_id ?? null);
    } else if (BODY_FIX_FINISHED.includes(e.name)) {
      let i = e.job_id != null ? st.pendingFixes.indexOf(e.job_id) : 0;
      if (i < 0) i = st.pendingFixes.indexOf(null);   // 積んだ側に job_id がなかった
      if (i >= 0) st.pendingFixes.splice(i, 1);
      if (e.name === "pr.body_amended") { st.amendedUnchecked = e; st.fixFailed = null; } else st.fixFailed = e;
    } else if (BODY_CHECK_EVENTS.includes(e.name)) {
      st.amendedUnchecked = null;
    } else if (e.name === "supervisor.merged") {
      merges += 1;
      if (st.pendingFixes.length > 0) {
        const jobs = st.pendingFixes.map((j) => (j == null ? "job ?" : `job ${j}`)).join("・");
        errors.push({ rule: "merged_before_body_fix_finished", pr_url: pr,
          message: `本文の直し（${jobs}）が終わる前に supervisor.merged が来た（直していない本文のままマージした）` });
      }
      if (st.fixFailed) {
        const job = st.fixFailed.job_id != null ? `（job ${st.fixFailed.job_id}）` : "";
        errors.push({ rule: "merged_after_body_fix_failed", pr_url: pr,
          message: `本文の直し${job}が失敗で終わり（pr.body_amend_failed）、直しが成功しないまま supervisor.merged が来た（直していない本文のままマージした）` });
      }
      if (st.amendedUnchecked) {
        const job = st.amendedUnchecked.job_id != null ? `（job ${st.amendedUnchecked.job_id}）` : "";
        errors.push({ rule: "merged_without_body_recheck", pr_url: pr,
          message: `pr.body_amended${job}のあと本文の検査（${BODY_CHECK_EVENTS.join("・")}）がないまま supervisor.merged が来た（直した本文を再検査せずにマージした）` });
      }
    }
  }
  return { merges, errors };
}

function main() {
  const { values } = parseArgs({ options: { events: { type: "string" }, pr: { type: "string" }, json: { type: "string" } } });
  let result;
  try {
    if (!values.events) throw new Error("--events がない");
    result = checkEvents(parseEvents(readFileSync(values.events, "utf8")), values.pr);
  } catch (e) {
    console.error(`pr-body-recheck: イベント列を読めなかった: ${e.message}`);
    process.exit(2);
  }
  if (values.json) writeFileSync(values.json, JSON.stringify({ ok: result.errors.length === 0, errors: result.errors }, null, 2) + "\n");
  for (const e of result.errors) console.log(`${e.pr_url} ${e.rule}: ${e.message}`);
  console.log(`pr-body-recheck: マージ ${result.merges} 件、外れ ${result.errors.length} 件`);
  process.exit(result.errors.length === 0 ? 0 : 1);
}

if (import.meta.url === `file://${process.argv[1]}`) main();
