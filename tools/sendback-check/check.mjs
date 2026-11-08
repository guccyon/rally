// supervisor が差し戻し（sendback）・続きのジョブを起こす判定を決定論で検査する（知見 K-0004 の担保。議決 D-0004・D-0009、Issue #902・#934）。LLM を使わない。
//
//   node check.mjs [--record <PR の記録の JSON>] [--json <出力先>] [--summary <GITHUB_STEP_SUMMARY>]    # --record を省くと標準入力から読む
//   node check.mjs --events <制御プレーンのイベントの JSON か JSONL> [--pr <PR の URL>] [--json …] [--summary …]   # CI・supervisor から PR ごとに
//   終了コード: 0 = 通る、1 = 外れた、2 = 読めなかった
//
// 判定の線: 差し戻しか続きのジョブを起こすのは「直っていない採用欠陥（defect が null でなく verdict が adopt）か、
// まだ入っていない仮決めの差し替え（replace）が 1 件以上あるとき」だけ。adopt の件数では起こさない。
// マージ済みかどうかは判定に使わない（マージの後でも直っていない採用欠陥があれば起こす）。
//
// 見ること:
//   - 起こすものがないのに差し戻しか続きのジョブが起きた（followup_without_defect）
//   - 起こすものがあるのに差し戻しも続きのジョブも起きていない（missing_followup）
//   - 検査役の承認（supervisor.inspector_approved）の basis に、欠陥の採用数と仮決めの承認数が分けて書かれている（basis_not_split・basis_mismatch）
//   - --events を GitHub Actions で動かすときは、配線の順序も（engine_test_todo・engine_test_not_wired。order.mjs、議決 D-0012）。
//     どの呼び方（npm scripts・ラッパー・eval・シンボリックリンク・引数の順序）で起動されても、記録を読む前に自分で見張りを走らせ、
//     .github/workflows/ に呼ぶ手順を見つけられなくても動いていること自体を配線と数える（議決 D-0026・D-0038、Issue #1084）

import { appendFileSync, readFileSync, writeFileSync } from "node:fs";
import { parseArgs } from "node:util";

import { checkOrder, formatErrors, isMainModule, loadRepo, REPO_ROOT, renderSummary as renderOrderSummary, runningFrom } from "./order.mjs";

/** 差し戻しか続きのジョブとして数える後続の種類（Engine の sendback.filed と、continue の reason・body_fix）。 */
export const FOLLOWUP_KINDS = ["sendback", "review_fix", "body_fix"];

const BASIS_DEFECTS = /欠陥の採用 (\d+)/;
const BASIS_PROVISIONALS = /仮決めの承認 (\d+)/;

/** 欠陥の採用: verdict が adopt で defect が null でない。overlay の shadow のルールの指摘は差し戻さないので数えない（ADR 0013）。 */
export function isAdoptedDefect(j) {
  return j.verdict === "adopt" && j.defect != null && !j.defect.shadow;
}

/** 仮決めの承認: verdict が adopt で defect が null。直すものはない。 */
export function isApprovedProvisional(j) {
  return j.verdict === "adopt" && j.defect == null;
}

/** 仮決めの差し替えで、今の差分にまだ入っていないもの（already_applied でない）。 */
export function isPendingReplacement(j) {
  return j.verdict === "replace" && j.defect == null && !j.already_applied;
}

/** 欠陥の採用数と仮決めの承認数を分けて数える。 */
export function counts(judgments) {
  return {
    judged: judgments.length,
    adopted_defects: judgments.filter(isAdoptedDefect).length,
    approved_provisionals: judgments.filter(isApprovedProvisional).length,
  };
}

/** 差し戻しか続きのジョブで直すもの（まだ直っていない採用欠陥と、まだ入っていない差し替え）。マージ済みかどうかは見ない。 */
export function toFix(judgments) {
  return judgments.filter((j) => !j.fixed && (isAdoptedDefect(j) || isPendingReplacement(j)));
}

/** 差し戻しか続きのジョブを起こすべきか。 */
export function needsFollowup(judgments) {
  return toFix(judgments).length > 0;
}

/** 検査役の承認の basis の文（欠陥の採用数と仮決めの承認数を分けて書く）。 */
export function approvalBasis(judgments) {
  const c = counts(judgments);
  return `judge が ${c.judged} 件を裁いた（欠陥の採用 ${c.adopted_defects}・仮決めの承認 ${c.approved_provisionals}。採用した欠陥は続きのジョブか差し戻しで直す）`;
}

/** basis の文から 2 つの数を読む。分けて書かれていなければ null。 */
export function parseBasis(basis) {
  const d = basis.match(BASIS_DEFECTS);
  const p = basis.match(BASIS_PROVISIONALS);
  return d && p ? { adopted_defects: Number(d[1]), approved_provisionals: Number(p[1]) } : null;
}

/**
 * PR の 1 つの head の記録を検査し、外れた箇所の一覧を返す（空なら通る）。
 * record: { pr_url, merged, judgments: [{ subject, verdict, defect, fixed?, already_applied? }], followups: [{ kind }], basis? }
 */
export function checkRecord(record) {
  const judgments = record.judgments ?? [];
  const followups = (record.followups ?? []).filter((f) => FOLLOWUP_KINDS.includes(f.kind));
  const fix = toFix(judgments);
  const c = counts(judgments);
  const errors = [];

  if (fix.length === 0 && followups.length > 0) {
    errors.push({ rule: "followup_without_defect",
      message: `直っていない採用欠陥も差し替えもないのに ${followups.map((f) => f.kind).join("・")} が起きた（adopt ${judgments.filter((j) => j.verdict === "adopt").length} 件のうち欠陥の採用 ${c.adopted_defects} 件）` });
  }
  if (fix.length > 0 && followups.length === 0) {
    errors.push({ rule: "missing_followup",
      message: `直っていない採用欠陥か差し替えが ${fix.length} 件あるのに、差し戻しも続きのジョブも起きていない${record.merged ? "（マージ済みでも起こす）" : ""}` });
  }
  if (typeof record.basis === "string") {
    const b = parseBasis(record.basis);
    if (b === null) {
      errors.push({ rule: "basis_not_split",
        message: `検査役の承認の basis に「欠陥の採用 N」と「仮決めの承認 N」が分けて書かれていない: ${record.basis}` });
    } else if (b.adopted_defects !== c.adopted_defects || b.approved_provisionals !== c.approved_provisionals) {
      errors.push({ rule: "basis_mismatch",
        message: `basis の数（欠陥の採用 ${b.adopted_defects}・仮決めの承認 ${b.approved_provisionals}）が裁きの記録（欠陥の採用 ${c.adopted_defects}・仮決めの承認 ${c.approved_provisionals}）と合わない` });
    }
  }
  return errors;
}

/** 後続の種類。sendback.filed は sendback、続きのジョブは continue.enqueued の reason、本文の直しは job.enqueued の kind。 */
function followupOf(e) {
  if (e.name === "sendback.filed") return { kind: "sendback" };
  if (e.name === "continue.enqueued") return { kind: e.payload?.reason };
  if (e.name === "job.enqueued" && e.payload?.kind === "body_fix") return { kind: "body_fix" };
  return null;
}

/**
 * judgment.judged の 1 件を裁きの記録にする。中身（subject・defect・already_applied）は artifacts の Judgment（stage=judged）にあるので、
 * イベントに judgment として添えてもらう。添えていなければ category で分ける（record_judged は欠陥を category=inspection にする）。
 */
function judgmentOf(e) {
  const a = e.judgment;
  if (a && typeof a === "object") {
    const j = { subject: a.subject, verdict: a.verdict ?? e.payload?.verdict, defect: a.defect ?? null };
    if (a.already_applied) j.already_applied = true;
    if (a.fixed) j.fixed = true;
    return j;
  }
  const isDefect = e.payload?.category === "inspection";
  return { subject: isDefect ? "defect" : "provisional", verdict: e.payload?.verdict, defect: isDefect ? {} : null };
}

/** イベントがどの PR のものか（sendback.filed は origin_pr）。 */
function prOf(e) {
  return e.payload?.pr_url ?? e.payload?.origin_pr ?? null;
}

/**
 * 制御プレーンのイベント列（古い順）から、PR と head ごとの記録を組む。
 * head の区切りは supervisor.judge_enqueued（head_sha）。その後から次の judge_enqueued までの judgment.judged・後続を、その head のものとする。
 * supervisor.inspector_approved の basis は head_sha の同じ区切りに付ける。最初の judge_enqueued より前のイベントは見ない（裁きがない）。
 * events: [{ name, payload, judgment? }]。prUrl を渡せばその PR だけを組む。
 */
export function recordsFromEvents(events, prUrl) {
  const byPr = new Map();
  for (const e of events) {
    const pr = prOf(e);
    if (!pr || (prUrl && pr !== prUrl)) continue;
    if (!byPr.has(pr)) byPr.set(pr, { rounds: [], merged: false });
    const st = byPr.get(pr);
    if (e.name === "supervisor.merged" || e.name === "pr.merged") {
      st.merged = true;
      continue;
    }
    if (e.name === "supervisor.judge_enqueued") {
      st.rounds.push({ pr_url: pr, head_sha: e.payload.head_sha ?? null, judgments: [], followups: [] });
      continue;
    }
    const cur = st.rounds.at(-1);
    if (!cur) continue;
    if (e.name === "judgment.judged") cur.judgments.push(judgmentOf(e));
    else if (e.name === "supervisor.inspector_approved") {
      const r = st.rounds.findLast((x) => x.head_sha === e.payload.head_sha) ?? cur;
      r.basis = e.payload.basis;
    } else {
      const f = followupOf(e);
      if (f) cur.followups.push(f);
    }
  }
  return [...byPr.values()].flatMap((st) => st.rounds.map((r) => ({ ...r, merged: st.merged })));
}

/**
 * イベントの JSON（配列か { events: [...] }。整形してあってもよい）か JSONL を読む。
 * 全体を JSON として読めなければ JSONL として読む。形が合わない（events が配列でない、name のない件がある）ときは投げる（読めなかった）。
 */
export function parseEvents(text) {
  const t = text.trim();
  if (t === "") return [];
  let events;
  let whole;
  try {
    whole = JSON.parse(t);
  } catch {
    whole = undefined;
  }
  if (Array.isArray(whole)) events = whole;
  else if (whole !== undefined && whole !== null && typeof whole === "object" && "events" in whole) {
    if (!Array.isArray(whole.events)) throw new Error("events が配列でない");
    events = whole.events;
  } else if (whole !== undefined) events = [whole];   // 1 行だけの JSONL
  else events = t.split("\n").filter((l) => l.trim() !== "").map((l) => JSON.parse(l));
  events.forEach((e, i) => {
    if (e === null || typeof e !== "object" || typeof e.name !== "string") throw new Error(`${i + 1} 件目に name がない`);
  });
  return events;
}

export function renderSummary(results) {
  const out = ["## sendback-check", ""];
  if (results.length === 0) out.push("見る裁きがなかった（supervisor.judge_enqueued がない）");
  for (const r of results) {
    const c = counts(r.record.judgments ?? []);
    const where = [r.record.pr_url, r.record.head_sha].filter(Boolean).join("@") || "記録";
    out.push(`- ${r.errors.length === 0 ? "通る" : "外れた"}: ${where}（裁き ${c.judged} 件。欠陥の採用 ${c.adopted_defects}・仮決めの承認 ${c.approved_provisionals}）`);
    for (const e of r.errors) out.push(`  - \`${e.rule}\`: ${e.message}`);
  }
  return out.join("\n") + "\n";
}

function main() {
  const { values } = parseArgs({ options: { record: { type: "string" }, events: { type: "string" }, pr: { type: "string" },
    json: { type: "string" }, summary: { type: "string" } } });
  const annotate = process.env.GITHUB_ACTIONS === "true";
  // CI で --events を動かすときは、記録を読む前に配線の順序（engine.test.mjs の todo を外して CI に配線してから）を見る（議決 D-0012、Issue #953）。
  // 動いていること自体を配線と数え、静的な辿りが呼ぶ手順を見つけられなくても見る（議決 D-0026・D-0038、Issue #1084）
  let order = { wired: [], errors: [] };
  if (values.events !== undefined && annotate) {
    try {
      order = checkOrder({ ...loadRepo(process.env.GITHUB_WORKSPACE || REPO_ROOT), running: runningFrom(process.env) });
    } catch (e) {
      console.error(`sendback-check: 配線の順序を読めなかった: ${e.message}`);
      process.exit(2);
    }
  }
  let records;
  try {
    if (values.events !== undefined) {
      records = recordsFromEvents(parseEvents(readFileSync(values.events, "utf8")), values.pr);
    } else {
      records = [JSON.parse(readFileSync(values.record ?? 0, "utf8"))];
    }
  } catch (e) {
    for (const line of formatErrors(order.errors, annotate)) console.log(line);   // 記録を読めなくても、配線の順序の外れは出す
    console.error(`sendback-check: 記録を読めなかった: ${e.message}`);
    process.exit(2);
  }
  const results = records.map((record) => ({ record, errors: checkRecord(record) }));
  const errors = results.flatMap((r) => r.errors.map((e) => ({ ...e, ...(values.events !== undefined ? { pr_url: r.record.pr_url, head_sha: r.record.head_sha } : {}) })));
  const all = [...order.errors, ...errors];
  if (values.json) writeFileSync(values.json, JSON.stringify({ ok: all.length === 0, errors: all }, null, 2) + "\n");
  if (values.summary) appendFileSync(values.summary, renderSummary(results) + (order.errors.length > 0 ? "\n" + renderOrderSummary(order) : ""));
  for (const line of formatErrors(order.errors, annotate)) console.log(line);
  for (const e of errors) {
    const where = e.pr_url ? `${e.pr_url}@${e.head_sha ?? "?"} ` : "";
    console.log(annotate ? `::error title=sendback-check ${e.rule}::${where}${e.message}` : `${where}${e.rule}: ${e.message}`);
  }
  const c = counts(records.flatMap((r) => r.judgments ?? []));
  const heads = values.events !== undefined ? `head ${records.length} 個・` : "";
  console.log(`sendback-check: ${heads}裁き ${c.judged} 件（欠陥の採用 ${c.adopted_defects}・仮決めの承認 ${c.approved_provisionals}）、外れ ${all.length} 件`);
  process.exit(all.length === 0 ? 0 : 1);
}

if (isMainModule(import.meta.url)) main();
