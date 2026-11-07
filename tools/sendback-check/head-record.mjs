// engine-head（ci/sendback-engine-head.yml）の先頭の Engine での実行の記録が Issue #948 に残っているかを決定論で照らす（知見 K-0004 の担保。議決 D-0031、Issue #1047）。LLM を使わない。
//
//   gh api repos/{owner}/{repo}/issues/948/comments --paginate --jq '.[]' > comments.jsonl
//   node head-record.mjs --comments comments.jsonl [--json <出力先>] [--summary <GITHUB_STEP_SUMMARY>]
//   終了コード: 0 = 通る（記録が 1 件以上ある）、1 = 外れた、2 = 読めなかった
//
// 議決 D-0031 は、建設者が ci/sendback-engine-head.yml を .github/workflows/ に写して schedule で走らせ、先頭の Engine での実行の記録
// （通った・落ちた）が Issue #948 に 1 件残ってから、Issue #977 の解決策の wired と verified を true にすると決めた。
// その「1 件残ってから」を人の目ではなく機械で照らす。Issue のコメントは PR の GITHUB_TOKEN では選んで読めないので PR ごとには走らせず、
// 建設者が gh api で取り出した JSON を渡して走らせる（README の「Engine の先頭での定期の実行」）。
//
// 記録に数えるのは、github-actions[bot] が書いたコメントで、最初の行が engine-head の印
// （<!-- sendback-engine-head: <40 桁の SHA> <engine_head_passed・engine_head_failed・harness_unknown_query> -->）で、
// 本文に実行の URL（/actions/runs/<番号>）があるものだけ。人が印を写したコメントや、先頭の版が取れなかった（unknown）記録は数えない。

import { appendFileSync, readFileSync, writeFileSync } from "node:fs";
import { parseArgs } from "node:util";

/** 記録の種類（ci/sendback-engine-head.yml の kind）。 */
export const KINDS = Object.freeze(["engine_head_passed", "engine_head_failed", "harness_unknown_query"]);
/** 記録を書くのは engine-head の GITHUB_TOKEN。 */
export const AUTHOR = "github-actions[bot]";

const MARKER = new RegExp(`^<!-- sendback-engine-head: ([0-9a-f]{40}) (${KINDS.join("|")}) -->$`);
const RUN_URL = /https:\/\/\S+\/actions\/runs\/\d+/;

/** gh api の出力（JSON の配列か、--jq '.[]' の 1 行 1 件の JSONL）からコメントを取り出す。 */
export function parseComments(text) {
  const trimmed = text.trim();
  if (trimmed === "") return [];
  if (trimmed.startsWith("[")) {
    const parsed = JSON.parse(trimmed);
    if (!Array.isArray(parsed)) throw new Error("コメントの JSON は配列にする");
    return parsed;
  }
  return trimmed.split(/\r?\n/).filter((l) => l.trim() !== "").map((l, i) => {
    try {
      return JSON.parse(l);
    } catch {
      throw new Error(`${i + 1} 行目が JSON でない`);
    }
  });
}

/** コメントのうち engine-head の記録に数えるもの（先頭の版・種類・実行の URL）。 */
export function headRecords(comments) {
  const records = [];
  for (const c of comments) {
    if (c?.user?.login !== AUTHOR || typeof c.body !== "string") continue;
    const m = c.body.split(/\r?\n/, 1)[0].trim().match(MARKER);
    const run = c.body.match(RUN_URL);
    if (m && run) records.push({ head: m[1], kind: m[2], run: run[0], url: c.html_url ?? null });
  }
  return records;
}

export function checkHeadRecord(comments) {
  const records = headRecords(comments);
  const errors = records.length > 0 ? [] : [{ rule: "head_record_missing",
    message: "Issue #948 に engine-head の先頭の Engine での実行の記録（通った・落ちた）がない。.github/workflows/sendback-engine-head.yml を schedule で走らせて記録が残るまで、Issue #977 の解決策の wired と verified を true にしない" }];
  return { records, errors };
}

export function formatErrors(errors, annotate) {
  return errors.map((e) => annotate ? `::error title=sendback-check ${e.rule}::${e.message}` : `${e.rule}: ${e.message}`);
}

export function renderSummary({ records, errors }) {
  const out = ["## sendback-check の engine-head の記録", ""];
  for (const r of records) out.push(`- \`${r.kind}\`（先頭 \`${r.head}\`）: ${r.run}`);
  if (errors.length === 0) out.push("", `通る: 記録 ${records.length} 件`);
  for (const e of errors) out.push(`- 外れた \`${e.rule}\`: ${e.message}`);
  return out.join("\n") + "\n";
}

function main() {
  const { values } = parseArgs({ options: { comments: { type: "string" }, json: { type: "string" }, summary: { type: "string" } } });
  let result;
  try {
    if (!values.comments) throw new Error("--comments に Issue #948 のコメントの JSON を渡す");
    result = checkHeadRecord(parseComments(readFileSync(values.comments, "utf8")));
  } catch (e) {
    console.error(`sendback-check head-record: 読めなかった: ${e.message}`);
    process.exit(2);
  }
  if (values.json) writeFileSync(values.json, JSON.stringify({ ok: result.errors.length === 0, ...result }, null, 2) + "\n");
  if (values.summary) appendFileSync(values.summary, renderSummary(result));
  for (const line of formatErrors(result.errors, process.env.GITHUB_ACTIONS === "true")) console.log(line);
  for (const r of result.records) console.log(`${r.kind} ${r.head} ${r.run}`);
  console.log(`sendback-check head-record: 記録 ${result.records.length} 件、外れ ${result.errors.length} 件`);
  process.exit(result.errors.length === 0 ? 0 : 1);
}

if (import.meta.url === `file://${process.argv[1]}`) main();
