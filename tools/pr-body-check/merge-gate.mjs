// マージの前の関所（知見 K-0012 の担保。議決 D-0033 の案、Issue #1052）。LLM を使わない。
// 制御プレーンのイベント列から、PR 本文の指摘を直す body_fix のジョブが終わり、そのあとの head の本文で
// 見出しの検査（check.mjs。CI では pr-body-check のチェック）が通ったかを見て、supervisor.merged を出してよいかを返す。
//
//   node merge-gate.mjs --events <イベントの JSON か JSONL> --pr <PR の URL> [--head <head の SHA>] [--body <今の本文のファイル>] [--json <出力先>]
//   node merge-gate.mjs --events <イベントの JSON か JSONL> --audit [--pr <PR の URL>] [--json <出力先>] [--summary "$GITHUB_STEP_SUMMARY"]
//   終了コード: 0 = マージしてよい（--audit では外れたマージがない）、1 = マージしない（--audit では外れたマージがある）、
//             2 = 読めなかった（マージしない）、3 = 見出しの定義（template.json）が不正（--body を照らしていない。マージしない）
//
// 呼ぶ場所: Engine の dispatcher/supervisor.py が検査役の承認（inspector_approved）のあと、マージ（PUT .../merge と supervisor.merged）の前。
// README の「マージの前の関所（merge-gate.mjs）」を参照。

import { appendFileSync, readFileSync, writeFileSync } from "node:fs";
import { parseArgs } from "node:util";

import { EXIT_TEMPLATE_ERROR, TemplateError, checkBody } from "./check.mjs";
import { parseEvents } from "../sendback-check/check.mjs";

/** 直した本文の再検査の記録。CI の pr-body-check のチェック（pull_request の edited でも起動）の結果を Engine が残す。 */
export const BODY_CHECKED = "pr.body_checked";

/** 本文の直しのジョブの種類（Engine の dispatcher.enqueue_body_fix）。 */
export const BODY_FIX = "body_fix";

/** イベントの形が崩れている（body_fix に job_id がない・再検査の ok が真偽でない）ときに投げる。境界の main で終了コード 2 にする。 */
export class EventShapeError extends Error {
  constructor(reason) {
    super(reason);
    this.name = "EventShapeError";
    this.reason = reason;
  }
}

const prOf = (e) => e.payload?.pr_url ?? null;
const jobOf = (e) => e.job_id ?? e.payload?.job_id ?? null;

/**
 * 1 つの PR について、イベント列の終わりの時点でマージしてよいかを返す。events は古い順（events の id 順）。
 * head は今マージしようとしている head（省けば、その PR のイベントの最後の head_sha）。
 * body は今の本文（省けば照らさない）。渡したときは、列の終わり（直しのあと）の再検査として数える。
 * @returns {{ ok: boolean, errors: { rule: string, message: string, job_id?: number }[] }}
 */
export function mergeGate(events, prUrl, { head = null, body = null } = {}) {
  const fixes = new Map();   // job_id → { pos, state, finishedAt }
  let lastHead = null;
  let lastFinish = -1;       // 最後に終わった body_fix の位置
  const checks = [];         // { pos, head_sha, ok }
  events.forEach((e, pos) => {
    const job = jobOf(e);
    if (e.name === "job.enqueued" && e.payload?.kind === BODY_FIX && prOf(e) === prUrl) {
      if (job === null) throw new EventShapeError(`${pos + 1} 件目の body_fix の job.enqueued に job_id がない`);
      fixes.set(job, { pos, state: null });
      return;
    }
    if (prOf(e) === prUrl && e.payload?.head_sha) lastHead = e.payload.head_sha;
    if (job !== null && fixes.has(job)) {
      const fix = fixes.get(job);
      let state = null;
      if (e.name === "job.finished") state = e.payload?.state === "succeeded" ? "succeeded" : "failed";
      else if (e.name === "pr.body_amended") state = "succeeded";
      else if (e.name === "pr.body_amend_failed") state = "failed";
      if (state !== null) {
        // 同じジョブの job.finished と pr.body_amended は続けて出る。失敗はどちらで見ても失敗にする
        fix.state = fix.state === "failed" ? "failed" : state;
        lastFinish = pos;
      }
      return;
    }
    if (e.name === BODY_CHECKED && prOf(e) === prUrl) {
      if (typeof e.payload?.ok !== "boolean") throw new EventShapeError(`${pos + 1} 件目の ${BODY_CHECKED} の ok が真偽でない`);
      checks.push({ pos, head_sha: e.payload.head_sha ?? null, ok: e.payload.ok });
    }
  });
  if (fixes.size === 0) return { ok: true, errors: [] };   // 本文の直しがなければ、待つものはない

  const errors = [];
  for (const [job, fix] of fixes) {
    if (fix.state === null) {
      errors.push({ rule: "body_fix_running", job_id: job, message: `body_fix（job ${job}）がまだ終わっていない。終わってから直した本文を再検査する` });
    } else if (fix.state === "failed") {
      errors.push({ rule: "body_fix_failed", job_id: job, message: `body_fix（job ${job}）が本文を直せなかった。直っていない本文のままマージしない（人に回す）` });
    }
  }
  if (errors.length > 0) return { ok: false, errors };

  const target = head ?? lastHead;
  if (body !== null) checks.push({ pos: events.length, head_sha: target, ok: checkBody(body).length === 0 });
  const after = checks.filter((c) => c.pos > lastFinish && (target === null || c.head_sha === target));
  const latest = after.at(-1);
  if (latest === undefined) {
    errors.push({ rule: "body_not_rechecked", message: `最後の body_fix が終わったあと、head ${target ?? "?"} の本文の再検査（${BODY_CHECKED}）がない` });
  } else if (!latest.ok) {
    errors.push({ rule: "body_check_failed", message: `最後の body_fix が終わったあとの head ${target ?? "?"} の本文が、見出しの検査に通っていない` });
  }
  return { ok: errors.length === 0, errors };
}

/**
 * 過去の記録で、supervisor.merged がこの関所を通っていたかを見る。merged ごとに、その直前までの列で mergeGate を引く。
 * @returns {{ pr_url: string, head_sha: string | null, at: string | null, errors: object[] }[]}
 */
export function auditMerges(events, prUrl = null) {
  return events.flatMap((e, pos) => {
    if (e.name !== "supervisor.merged" || !prOf(e) || (prUrl !== null && prOf(e) !== prUrl)) return [];
    const head = e.payload.head_sha ?? null;
    const { errors } = mergeGate(events.slice(0, pos), prOf(e), { head });
    return [{ pr_url: prOf(e), head_sha: head, at: e.at ?? null, errors }];
  });
}

export function renderSummary(merges) {
  const out = ["## merge-gate", ""];
  if (merges.length === 0) out.push("見るマージがなかった（supervisor.merged がない）");
  for (const m of merges) {
    out.push(`- ${m.errors.length === 0 ? "通る" : "外れた"}: ${m.pr_url}@${m.head_sha ?? "?"}${m.at ? `（${m.at}）` : ""}`);
    for (const e of m.errors) out.push(`  - \`${e.rule}\`: ${e.message}`);
  }
  return out.join("\n") + "\n";
}

function main() {
  const { values } = parseArgs({ options: { events: { type: "string" }, pr: { type: "string" }, head: { type: "string" },
    body: { type: "string" }, audit: { type: "boolean" }, json: { type: "string" }, summary: { type: "string" } } });
  const annotate = process.env.GITHUB_ACTIONS === "true";
  let events;
  let body = null;
  try {
    if (!values.events) throw new Error("--events に制御プレーンのイベントを渡す");
    if (!values.audit && !values.pr) throw new Error("--pr がない");
    events = parseEvents(readFileSync(values.events, "utf8"));
    if (values.body) body = readFileSync(values.body, "utf8");
  } catch (e) {
    console.error(`merge-gate: 読めなかった: ${e.message}`);
    process.exit(2);
  }
  try {
    if (values.audit) {
      const merges = auditMerges(events, values.pr ?? null);
      const bad = merges.filter((m) => m.errors.length > 0);
      if (values.json) writeFileSync(values.json, JSON.stringify({ ok: bad.length === 0, merges }, null, 2) + "\n");
      if (values.summary) appendFileSync(values.summary, renderSummary(merges));
      for (const m of bad) {
        for (const e of m.errors) {
          const where = `${m.pr_url}@${m.head_sha ?? "?"} `;
          console.log(annotate ? `::error title=merge-gate ${e.rule}::${where}${e.message}` : `${where}${e.rule}: ${e.message}`);
        }
      }
      console.log(`merge-gate: マージ ${merges.length} 件、外れ ${bad.length} 件`);
      process.exit(bad.length === 0 ? 0 : 1);
    }
    const result = mergeGate(events, values.pr, { head: values.head ?? null, body });
    if (values.json) writeFileSync(values.json, JSON.stringify(result, null, 2) + "\n");
    for (const e of result.errors) console.log(`${e.rule}: ${e.message}`);
    process.exit(result.ok ? 0 : 1);
  } catch (e) {
    if (e instanceof TemplateError) {
      console.error(`merge-gate: 見出しの定義が不正: ${e.reason}`);
      process.exit(EXIT_TEMPLATE_ERROR);
    }
    if (!(e instanceof EventShapeError)) throw e;   // 形の崩れでない例外はこの関所の不具合なので、読めなかったに混ぜない
    console.error(`merge-gate: 読めなかった: ${e.reason}`);
    process.exit(2);
  }
}

if (import.meta.url === `file://${process.argv[1]}`) main();
