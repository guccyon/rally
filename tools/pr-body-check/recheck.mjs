// 本文の直し（body_fix）のあとの再検査を待たずにマージしていないかを、制御プレーンのイベント列で決定論に見る（知見 K-0012 の担保。議決 D-0033、Issue #1053）。LLM を使わない。
//
//   node recheck.mjs --events <制御プレーンのイベントの JSON か JSONL> [--pr <PR の URL>] [--json <出力先>]
//   終了コード: 0 = 通る、1 = 外れた、2 = イベント列を読めなかった
//
// イベント列は古い順に並べて渡す（tools/sendback-check の check.mjs --events と同じ形。読み方も parseEvents を使う）。
// 同じ PR の supervisor.merged ごとに、それより前のイベントを見る:
//   - pr.body_amended（本文の直しが終わった）のあと、本文の検査（本文を見て通った gate の gate.evaluated）がないまま supervisor.merged が来た（merged_without_body_recheck。事例 C-0006）
//   - 本文の直し（job.enqueued の kind が body_fix）を積んだのに、その終わり（pr.body_amended か pr.body_amend_failed）より前に
//     supervisor.merged が来た（merged_before_body_fix_finished。事例 C-0005 は、直しの終わりがマージの 7 秒後）
//   - 本文の直しが失敗で終わり（pr.body_amend_failed）、そのあと直しが成功しないまま supervisor.merged が来た（merged_after_body_fix_failed）
// 本文の検査は gate.evaluated のうち本文を見た gate だけを数える: pr-body-check を gate: "pr-body-check" で記録したものと、検査役の rubric（gate: "rubric"）で
// 見た rubric（checked）に本文の rubric（PR-002 など）があるもの。コードの検査（overlay・L1 など）と、本文の rubric を見ていない rubric は数えない
// （本文と関係のない検査が直しのあとに来ただけでは、直した本文を再検査したことにならない。議決 D-0072、Issue #1206）。
// そのうえで、本文を見て通ったものだけを数える: 結果（result）があれば "pass" で、欠陥（defects）があれば本文の欠陥がない（pr-body-check は欠陥が 0 件、
// rubric は PR-002 などの本文の rubric の欠陥がない）もの。結果も欠陥の一覧もない記録は通ったと言えないので数えない
// （直した本文で本文の検査が落ちたあとにマージしても、再検査に通ったことにはならない。議決 D-0110、Issue #1293）。
// supervisor が直した本文の pr-body-check が通ったのを確かめて残す supervisor.body_rechecked（engine/pr-body-recheck.patch。議決 D-0050、Issue #1122）も数える。
// ただし直した本文で始まった実行のものだけ: edited で始まった（action が edited）か、もとのイベントの時刻（created_at）が pr.body_amended の occurred_at 以後のもの。
// 直しの最中の push や前の実行の再実行で始まり、直す前の本文で通った実行の記録は数えない（議決 D-0100 の反対票の反例。Issue #1268）。
// 本文の直しの積みと終わりは job_id で結ぶ。終わりに job_id がなければ同じ PR の古いものから、積んだ側に job_id がなければ job_id のないものと結ぶ。

import { readFileSync, writeFileSync } from "node:fs";
import { parseArgs } from "node:util";

import { parseEvents } from "../sendback-check/check.mjs";

/** 本文の検査として数えるイベント。 */
export const BODY_CHECK_EVENTS = Object.freeze(["gate.evaluated", "supervisor.body_rechecked"]);
/** gate.evaluated のうち、それだけで本文の検査に数える gate。 */
export const BODY_CHECK_GATES = Object.freeze(["pr-body-check"]);
/** 検査役の rubric の gate で、見た rubric（checked）にあれば本文の検査に数える rubric（Engine の core-gates/rubric.yaml。PR-002 は本文の構成）。 */
export const BODY_RUBRIC_IDS = Object.freeze(["PR-002"]);
/** gate.evaluated の結果（result）のうち、通ったとみなす値（Engine の definitions/events.yaml の gate.evaluated の result）。 */
export const BODY_CHECK_PASS = "pass";
/** 本文の直しの終わり（成功と失敗）。 */
export const BODY_FIX_FINISHED = Object.freeze(["pr.body_amended", "pr.body_amend_failed"]);

const prOf = (e) => e.payload?.pr_url ?? null;
const isBodyFixEnqueued = (e) => e.name === "job.enqueued" && e.payload?.kind === "body_fix";
/**
 * supervisor.body_rechecked が、amended（pr.body_amended）の直した本文で始まった実行のものか（Issue #1268）。
 * edited で始まったか、もとのイベントの時刻（payload.created_at）が amended の occurred_at 以後なら数える。どちらとも言えなければ数えない。
 */
export function rechecksAmended(e, amended) {
  const p = e.payload ?? {};
  if (p.action === "edited") return true;
  const created = Date.parse(p.created_at ?? "");
  const at = Date.parse(amended?.occurred_at ?? "");
  return Number.isFinite(created) && Number.isFinite(at) && created >= at;
}

/**
 * gate.evaluated の本文の検査が通ったか（Issue #1293）。result があれば BODY_CHECK_PASS で、defects があれば本文の欠陥がない
 * （rubric は rubric_id が BODY_RUBRIC_IDS の欠陥がない、ほかの gate は欠陥が 0 件）。どちらもなければ通ったとは言えないので false。
 */
export function passedBodyCheck(e) {
  const p = e.payload ?? {};
  const hasResult = p.result != null;
  const hasDefects = Array.isArray(p.defects);
  if (!hasResult && !hasDefects) return false;
  if (hasResult && p.result !== BODY_CHECK_PASS) return false;
  if (!hasDefects) return true;
  return p.gate === "rubric" ? !p.defects.some((d) => BODY_RUBRIC_IDS.includes(d?.rubric_id)) : p.defects.length === 0;
}

/** 本文の検査に通ったか。gate.evaluated は本文を見た gate（BODY_CHECK_GATES か、checked に BODY_RUBRIC_IDS がある rubric）で、通ったものだけ。 */
export function isBodyCheck(e) {
  if (e.name === "supervisor.body_rechecked") return true;
  if (e.name !== "gate.evaluated") return false;
  const gate = e.payload?.gate;
  const checked = e.payload?.checked;
  const sawBody = BODY_CHECK_GATES.includes(gate) || (gate === "rubric" && Array.isArray(checked) && checked.some((id) => BODY_RUBRIC_IDS.includes(id)));
  return sawBody && passedBodyCheck(e);
}

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
    } else if (isBodyCheck(e)) {
      if (e.name === "supervisor.body_rechecked" && st.amendedUnchecked && !rechecksAmended(e, st.amendedUnchecked)) continue;
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
          message: `pr.body_amended${job}のあと本文の検査（${BODY_CHECK_EVENTS.join("・")}。gate.evaluated は ${[...BODY_CHECK_GATES, `${BODY_RUBRIC_IDS.join("・")} を見た rubric`].join("・")} で通ったもの）がないまま supervisor.merged が来た（直した本文を再検査して通らないままマージした）` });
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
