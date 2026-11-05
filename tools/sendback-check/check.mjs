// supervisor が差し戻し（sendback）・続きのジョブを起こす判定を決定論で検査する（知見 K-0004 の担保。議決 D-0004、Issue #902）。LLM を使わない。
//
//   node check.mjs [--record <PR の記録の JSON>] [--json <出力先>]    # --record を省くと標準入力から読む
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

import { readFileSync, writeFileSync } from "node:fs";
import { parseArgs } from "node:util";

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

function main() {
  const { values } = parseArgs({ options: { record: { type: "string" }, json: { type: "string" } } });
  let record;
  try {
    record = JSON.parse(readFileSync(values.record ?? 0, "utf8"));
  } catch (e) {
    console.error(`sendback-check: 記録を読めなかった: ${e.message}`);
    process.exit(2);
  }
  const errors = checkRecord(record);
  if (values.json) writeFileSync(values.json, JSON.stringify({ ok: errors.length === 0, errors }, null, 2) + "\n");
  for (const e of errors) console.log(`${e.rule}: ${e.message}`);
  const c = counts(record.judgments ?? []);
  console.log(`sendback-check: 裁き ${c.judged} 件（欠陥の採用 ${c.adopted_defects}・仮決めの承認 ${c.approved_provisionals}）、外れ ${errors.length} 件`);
  process.exit(errors.length === 0 ? 0 : 1);
}

if (import.meta.url === `file://${process.argv[1]}`) main();
