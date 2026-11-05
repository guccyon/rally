// 続きのジョブ（continue の conflict・ci_failed）の implement ジョブに渡す入力の線（知見 K-0009 の担保。議決 D-0013、Issue #955）。LLM を使わない。
//
// 線: 採用済みの指摘が残っている PR で continue（conflict・ci_failed）の implement ジョブを積んだら、ワーカーに渡す入力（標準入力の JSON）に
//   - その PR の未解消の採用済みの指摘の id（rubric_id。PR-002 など）がすべて adopted_defects[].rubric_id にある
//   - body_fix が残した本文の決まり（pr.body_amended の rules）がすべて body_rules にある
// ことを求める。どちらもなければ何も求めない（入力がなくても通る）。
//
// 見ること:
//   - 求めるものがあるのに入力がない（input_missing。conflict の続きのジョブに入力を渡さないときも）
//   - 未解消の採用済みの指摘の id が入力にない（missing_defect_id）
//   - body_fix が残した本文の決まりが入力にない（missing_body_rule）

import { isAdoptedDefect, recordsFromEvents } from "../sendback-check/check.mjs";

/** この線が見る continue の reason。review_fix は直す指摘そのものを渡すので見ない */
export const REASONS = ["conflict", "ci_failed"];

/**
 * その PR の未解消の採用済みの指摘。最後に裁いた head（supervisor.judge_enqueued の区切り）の裁きのうち、採用欠陥
 * （defect が null でなく verdict が adopt。overlay の shadow のルールの指摘は除く）で fixed でないもの。前の head の裁きは後の裁きで置き換わる。
 * events: 制御プレーンのイベント列（古い順。judgment.judged には Judgment を judgment として添える。sendback-check の README の SQL の形）
 */
export function unresolvedDefects(events, prUrl) {
  const last = recordsFromEvents(events, prUrl).findLast((r) => r.judgments.length > 0);
  return (last?.judgments ?? []).filter((j) => isAdoptedDefect(j) && !j.fixed).map((j) => j.defect);
}

/** body_fix が残した本文の決まり。その PR の pr.body_amended の rules を古い順に、同じ文は 1 回だけ */
export function bodyRules(events, prUrl) {
  const rules = events
    .filter((e) => e.name === "pr.body_amended" && e.payload?.pr_url === prUrl)
    .flatMap((e) => (Array.isArray(e.payload?.rules) ? e.payload.rules : []))
    .filter((r) => typeof r === "string" && r.trim() !== "");
  return [...new Set(rules)];
}

/** continue の implement ジョブを積んだ時点のイベント列から、入力に求めるもの（指摘の id と本文の決まり）を組む */
export function requiredInput(events, prUrl) {
  const ids = unresolvedDefects(events, prUrl).map((d) => d?.rubric_id).filter((id) => typeof id === "string" && id !== "");
  return { defect_ids: [...new Set(ids)], body_rules: bodyRules(events, prUrl) };
}

/**
 * ワーカーに渡す入力を、求めるものに照らす。外れた箇所の一覧を返す（空なら通る）。
 * input: ワーカーが標準入力で受け取る JSON（渡さないときは null）
 */
export function checkInput(required, input) {
  const { defect_ids: ids, body_rules: rules } = required;
  if (ids.length === 0 && rules.length === 0) return [];
  if (input === null || typeof input !== "object" || Array.isArray(input)) {
    return [{ rule: "input_missing",
      message: `未解消の採用済みの指摘（${ids.join("・") || "なし"}）か本文の決まり（${rules.length} 件）があるのに、続きのジョブに入力を渡していない` }];
  }
  const given = new Set((Array.isArray(input.adopted_defects) ? input.adopted_defects : []).map((d) => d?.rubric_id));
  const givenRules = new Set(Array.isArray(input.body_rules) ? input.body_rules : []);
  return [
    ...ids.filter((id) => !given.has(id)).map((id) => ({ rule: "missing_defect_id",
      message: `未解消の採用済みの指摘 ${id} が入力の adopted_defects にない` })),
    ...rules.filter((r) => !givenRules.has(r)).map((r) => ({ rule: "missing_body_rule",
      message: `body_fix が残した本文の決まりが入力の body_rules にない: ${r}` })),
  ];
}
