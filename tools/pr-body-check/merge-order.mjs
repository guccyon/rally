// マージの判断とマージが、検査と裁きと採用したコードの指摘の直しを待っているかを、制御プレーンのイベント列で決定論に見る
// （知見 K-0026 の担保。議決 D-0151、Issue #1392）。LLM を使わない。recheck.mjs（知見 K-0012。本文の直し）のコードの指摘の側。
//
//   node merge-order.mjs --events <制御プレーンのイベントの JSON か JSONL> [--pr <PR の URL>] [--json <出力先>]
//   終了コード: 0 = 通る、1 = 外れた、2 = イベント列を読めなかった
//
// イベント列は古い順に並べて渡す（recheck.mjs と同じ形。読み方も parseEvents を使う）。同じ PR ごとに見る:
//   - supervisor.merge_decided のうち code_change の表が auto のもの（change_types に code_change があり table が auto）が、
//     同じ head の検査役の承認（supervisor.inspector_approved）と裁き（supervisor.judge_enqueued の skipped か、そのあとの judgment.judged）の
//     両方より後に来ていない（merge_decided_before_review。事例 C-0012 は、merge_decided 14:44:39 が検査より前で、承認はその後）
//   - 裁きで採用した検査の指摘（judgment.judged の category が inspection・verdict が adopt）のうち機械的に直せるコードの指摘が、
//     直すジョブ（continue.enqueued の reason が review_fix か、judgment.fix_enqueued）を積まないまま supervisor.merged が来た
//     （merged_without_code_fix。事例 C-0012 は、採用した 3 件を「続きのジョブか差し戻しで直す」としてマージした）
//   - 直すジョブを積んだのに、その終わり（job.finished の state が succeeded）より前に supervisor.merged が来た（merged_before_code_fix_finished）。
//     直すジョブが失敗で終わったら、その指摘はまだ直すジョブがないものに戻す
// 機械的に直せるコードの指摘は、judgment.judged の payload の defect（Judgment の artifact の defect と同じ形）で見る。
// 箇所（defect.location）が PR 本文だけのもの（Engine の dispatcher の is_body_only と同じ線。本文の直しは recheck.mjs が見る）、
// overlay の shadow のルールの指摘（defect.shadow。Engine は差し戻さない）、judge が機械的に直せないとしたもの（mechanical が false）は数えない。
// 箇所が分からない指摘はコードの指摘として数える（Engine の judgment.judged は今は defect を載せないので、本文の指摘と見分けられないときは止める側に倒す）。
// job.finished は pr_url を持たないので、直すジョブの job_id で PR に結ぶ。

import { readFileSync, writeFileSync } from "node:fs";
import { parseArgs } from "node:util";

import { parseEvents } from "../sendback-check/check.mjs";

/** merge_decided で見る変更種別（Engine の definitions/change_type_rules.yaml の既定）と、検査と裁きを待つべき表の値。 */
export const CODE_CHANGE = "code_change";
export const AUTO = "auto";
/** 採用したコードの指摘を直すジョブを積んだイベント（Engine の continuation.enqueue と、人が直す答えを出した判断の受け渡し）。 */
export const FIX_ENQUEUED = Object.freeze(["continue.enqueued", "judgment.fix_enqueued"]);
/** 直すジョブの続きの理由（continue.enqueued の reason）。 */
export const FIX_REASON = "review_fix";
/** 直すジョブが直し終えたとみなす job.finished の state。 */
export const FIX_SUCCEEDED = "succeeded";

const prOf = (e) => e.payload?.pr_url ?? null;
const headOf = (e) => e.payload?.head_sha ?? null;

/** 箇所が PR 本文だけか（Engine の orchestrator/dispatcher/dispatcher.py の is_body_only と同じ線）。 */
export function isBodyOnly(location) {
  return /^pr\s*(#\d+\s*)?(本文|body)/i.test(String(location ?? "").trim());
}

/** 裁きで採用した、機械的に直せるコードの指摘か。 */
export function isMechanicalCodeDefect(e) {
  if (e.name !== "judgment.judged") return false;
  const p = e.payload ?? {};
  if (p.category !== "inspection" || p.verdict !== "adopt" || p.mechanical === false) return false;
  return !p.defect?.shadow && !isBodyOnly(p.defect?.location);
}

/** merge_decided が code_change の表を auto にしたものか。 */
export function isAutoCodeChange(e) {
  const p = e.payload ?? {};
  return e.name === "supervisor.merge_decided" && Array.isArray(p.change_types) && p.change_types.includes(CODE_CHANGE) && p.table === AUTO;
}

/** 直すジョブを積んだイベントなら、その job_id（なければ null）。直すジョブでなければ undefined。 */
function fixJobOf(e) {
  if (e.name === "continue.enqueued" && e.payload?.reason === FIX_REASON) return e.job_id ?? null;
  if (e.name === "judgment.fix_enqueued") return e.payload?.job_id ?? e.job_id ?? null;
  return undefined;
}

/** head（null は分からない）について、その記録が出ているか。分からない側はどの head の記録でも数える。 */
const seenFor = (heads, head) => (head == null ? heads.size > 0 : heads.has(head) || heads.has(null));

/**
 * events: [{ name, payload: { pr_url, ... }, job_id?, occurred_at? }]（古い順）。prUrl を渡せばその PR だけを見る。
 * 返す: { decisions: 見た code_change が auto の merge_decided の件数, merges: 見た supervisor.merged の件数, errors: [{ rule, pr_url, message }] }
 */
export function checkEvents(events, prUrl) {
  const byPr = new Map();
  const fixJobs = new Map();   // 直すジョブの job_id → PR
  const errors = [];
  let decisions = 0;
  let merges = 0;
  const stateOf = (pr) => {
    if (!byPr.has(pr)) byPr.set(pr, { approved: new Set(), judged: new Set(), judgeHead: null, open: [] });
    return byPr.get(pr);
  };
  for (const e of events) {
    if (e.name === "job.finished") {
      const pr = e.job_id != null ? fixJobs.get(e.job_id) : undefined;
      if (pr === undefined) continue;
      const st = stateOf(pr);
      if (e.payload?.state === FIX_SUCCEEDED) st.open = st.open.filter((d) => d.job !== e.job_id);
      else st.open.forEach((d) => { if (d.job === e.job_id) d.job = null; });
      continue;
    }
    const pr = prOf(e);
    if (!pr || (prUrl && pr !== prUrl)) continue;
    const st = stateOf(pr);
    const fixJob = fixJobOf(e);
    if (fixJob !== undefined) {
      const job = fixJob ?? "?";
      if (fixJob != null) fixJobs.set(fixJob, pr);
      st.open.forEach((d) => { if (d.job == null) d.job = job; });
    } else if (e.name === "supervisor.judge_enqueued") {
      st.judgeHead = headOf(e);
      if (e.payload?.skipped) st.judged.add(st.judgeHead);
    } else if (e.name === "judgment.judged") {
      st.judged.add(st.judgeHead);
      if (isMechanicalCodeDefect(e)) st.open.push({ id: e.payload?.artifact_id ?? null, job: null });
    } else if (e.name === "supervisor.inspector_approved") {
      st.approved.add(headOf(e));
    } else if (isAutoCodeChange(e)) {
      decisions += 1;
      const head = headOf(e);
      const missing = [];
      if (!seenFor(st.approved, head)) missing.push("検査役の承認（supervisor.inspector_approved）");
      if (!seenFor(st.judged, head)) missing.push("裁き（supervisor.judge_enqueued の skipped か judgment.judged）");
      if (missing.length > 0) {
        errors.push({ rule: "merge_decided_before_review", pr_url: pr,
          message: `supervisor.merge_decided（${CODE_CHANGE}=${AUTO}${head ? `、head ${head}` : ""}）が${missing.join("と")}より前に来た（検査と裁きを見ずにマージを決めた）` });
      }
    } else if (e.name === "supervisor.merged") {
      merges += 1;
      const ids = (ds) => ds.map((d) => (d.id == null ? "?" : `judgment ${d.id}`)).join("・");
      const unfixed = st.open.filter((d) => d.job == null);
      const fixing = st.open.filter((d) => d.job != null);
      if (unfixed.length > 0) {
        errors.push({ rule: "merged_without_code_fix", pr_url: pr,
          message: `裁きで採用した機械的に直せるコードの指摘 ${unfixed.length} 件（${ids(unfixed)}）を直すジョブ（${FIX_ENQUEUED.join("・")}）がないまま supervisor.merged が来た（直さずにマージして宿題を残した）` });
      }
      if (fixing.length > 0) {
        const jobs = [...new Set(fixing.map((d) => `job ${d.job}`))].join("・");
        errors.push({ rule: "merged_before_code_fix_finished", pr_url: pr,
          message: `採用したコードの指摘 ${fixing.length} 件（${ids(fixing)}）を直すジョブ（${jobs}）が終わる（job.finished の state が ${FIX_SUCCEEDED}）前に supervisor.merged が来た（直していないコードのままマージした）` });
      }
    }
  }
  return { decisions, merges, errors };
}

function main() {
  const { values } = parseArgs({ options: { events: { type: "string" }, pr: { type: "string" }, json: { type: "string" } } });
  let result;
  try {
    if (!values.events) throw new Error("--events がない");
    result = checkEvents(parseEvents(readFileSync(values.events, "utf8")), values.pr);
  } catch (e) {
    console.error(`merge-order: イベント列を読めなかった: ${e.message}`);
    process.exit(2);
  }
  if (values.json) writeFileSync(values.json, JSON.stringify({ ok: result.errors.length === 0, errors: result.errors }, null, 2) + "\n");
  for (const e of result.errors) console.log(`${e.pr_url} ${e.rule}: ${e.message}`);
  console.log(`merge-order: マージの判断 ${result.decisions} 件、マージ ${result.merges} 件、外れ ${result.errors.length} 件`);
  process.exit(result.errors.length === 0 ? 0 : 1);
}

if (import.meta.url === `file://${process.argv[1]}`) main();
