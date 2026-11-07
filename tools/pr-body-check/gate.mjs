// PR 本文の更新の直前に呼ぶ関所（知見 K-0001 の担保。議決 D-0013 の案、Issue #956）。LLM を使わない。
// 続きのジョブの summary を組み込んだ本文（gh pr edit で書く前の本文）を check.mjs の checkBody で見て、外れたら本文を書かず、
// body_fix に回す入力（Engine の dispatcher.enqueue_body_fix が job.enqueued の input に入れる形）を返す。
//
//   node gate.mjs --pr-url <PR の URL> [--body <組み込んだ本文のファイル>] [--json <出力先>]   # --body を省くと標準入力から読む
//   終了コード: 0 = 本文を書いてよい、1 = 本文を書かずに body_fix に回す、2 = 読めなかった（本文は書かない）、
//             3 = 見出しの定義（template.json）が不正（本文は照らしておらず書かない。body_fix には回さない）
//   標準出力: 1 のときは body_fix の入力の JSON 1 行。0 のときは何も出さない
//
// 呼ぶ場所: Engine の worker/entrypoint.sh が続きのジョブ（pr.head_updated・handoff.saved になる push）で NEW_BODY を組んだあと、
// gh pr edit の前。README の「Engine に入れること」を参照。

import { readFileSync, writeFileSync } from "node:fs";
import { parseArgs } from "node:util";

import { EXIT_TEMPLATE_ERROR, FACTORY_RECORD, TemplateError, checkBody } from "./check.mjs";

/** 本文の構成の指摘の id（Engine の core-gates/rubric.yaml）。検査役が同じ欠陥に付ける id に揃え、採用済みの指摘と同じ線で数えられるようにする。 */
export const RUBRIC_ID = "PR-002";

const HINT = {
  duplicate_h2: `続きのジョブの報告の ## 見出しを ### 以下に下げて「## ${FACTORY_RECORD}」の下に入れ、本文の上の節と同じ見出しを 2 回置かない`,
  outside_factory_record: `「## ${FACTORY_RECORD}」より後ろの見出しを ### 以下に下げる（その節の下はすべて工場の記録）`,
  continuation_outside_record: `続きのジョブの記録を「## ${FACTORY_RECORD}」の下の ### に移す`,
};

/** 外れた箇所を body_fix の指摘（Engine の is_body_only が本文だけの指摘と読む location）にする。 */
export function toDefect(error) {
  return {
    rubric_id: RUBRIC_ID,
    severity: "should",
    location: `PR 本文 ${error.line} 行目`,
    evidence: `pr-body-check ${error.rule}: ${error.message}`,
    remediation_hint: HINT[error.rule],
  };
}

/**
 * 組み込んだ本文を見て、書いてよいか、body_fix に回すかを返す。
 * @returns {{ ok: boolean, errors: object[], body_fix: null | { pr_url: string, defects: object[] } }}
 */
export function gate(body, prUrl) {
  const errors = checkBody(body);
  if (errors.length === 0) return { ok: true, errors, body_fix: null };
  return { ok: false, errors, body_fix: { pr_url: prUrl, defects: errors.map(toDefect) } };
}

function main() {
  const { values } = parseArgs({ options: { body: { type: "string" }, "pr-url": { type: "string" }, json: { type: "string" } } });
  if (!values["pr-url"]) {
    console.error("pr-body-check gate: --pr-url がない");
    process.exit(2);
  }
  let body;
  try {
    body = readFileSync(values.body ?? 0, "utf8");
  } catch (e) {
    console.error(`pr-body-check gate: 本文を読めなかった: ${e.message}`);
    process.exit(2);
  }
  let result;
  try {
    result = gate(body, values["pr-url"]);
  } catch (e) {
    if (!(e instanceof TemplateError)) throw e;
    console.error(`pr-body-check gate: 見出しの定義が不正: ${e.reason}`);
    process.exit(EXIT_TEMPLATE_ERROR);
  }
  if (values.json) writeFileSync(values.json, JSON.stringify(result, null, 2) + "\n");
  for (const e of result.errors) console.error(`${e.line} 行目: ${e.message}`);
  if (!result.ok) console.log(JSON.stringify(result.body_fix));
  process.exit(result.ok ? 0 : 1);
}

if (import.meta.url === `file://${process.argv[1]}`) main();
