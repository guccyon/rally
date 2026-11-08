// sendback-protection の定期の実行（ci/sendback-protection.yml の protection のジョブ）が最後に通ってからの時間を決定論で照らす
// （知見 K-0004 の担保。議決 D-0061（Issue #1162 の起票時の番号は D-0058。PR #1161）、Issue #1162）。LLM を使わない。
//
//   gh api "repos/{owner}/{repo}/actions/artifacts?name=sendback-protection-passed&per_page=100" --paginate --jq '.artifacts[]' > artifacts.jsonl
//   node heartbeat.mjs --artifacts artifacts.jsonl [--branch main] [--max-age-hours 48] [--now <ISO 8601>] [--json <出力先>] [--summary <GITHUB_STEP_SUMMARY>]
//   終了コード: 0 = 通る、1 = 外れた、2 = 読めなかった
//
// schedule のワークフローは、GitHub が長く動きのないリポジトリで止めたり、UI で無効にされたり、シークレットが切れて落ち続けたりしても、
// 字面は変わらず誰にも届かない（議決 D-0061 の担保の担当と反対の担当）。protection のジョブは schedule で通るたびに、通った記録として
// 成果物（ARTIFACT）を上げる。この検査はその成果物の一覧を読み、既定のブランチの実行で上げた最後のものの作られた時刻から
// 今までが max-age-hours（既定 48 時間 = 2 日）より長ければ落とす。別のワークフロー（ci/sendback-protection-heartbeat.yml）から走らせ、
// 外れたら Issue を開くか更新する。
//
// 見ること:
//   - 既定のブランチの実行で上げた ARTIFACT の成果物が 1 件もない（heartbeat_missing。保持の期間が過ぎて消えた場合も）
//   - 最後のものの created_at から今までが max-age-hours より長い（heartbeat_stale）
// 時刻は成果物の created_at（GitHub が付ける）を使い、中身は読まない。PR のブランチの実行や名前の違う成果物は数えない。

import { appendFileSync, readFileSync, writeFileSync } from "node:fs";
import { parseArgs } from "node:util";

/** protection のジョブが通ったときに上げる成果物の名前（ci/sendback-protection.yml の upload-artifact の name）。 */
export const ARTIFACT = "sendback-protection-passed";
/** 最後に通ってから許す時間（2 日）。 */
export const MAX_AGE_HOURS = 48;

/**
 * gh api の出力から成果物を取り出す。{ artifacts: [...] }（1 ページ分か、--paginate で続けて出たもの）・JSON の配列・
 * --jq '.artifacts[]' の 1 行 1 件の JSONL のどれも読む。
 */
export function parseArtifacts(text) {
  const trimmed = text.trim();
  if (trimmed === "") return [];
  if (trimmed.startsWith("[")) {
    const parsed = JSON.parse(trimmed);
    if (!Array.isArray(parsed)) throw new Error("成果物の JSON は配列にする");
    return parsed;
  }
  const out = [];
  trimmed.split(/\r?\n/).filter((l) => l.trim() !== "").forEach((l, i) => {
    let v;
    try {
      v = JSON.parse(l);
    } catch {
      throw new Error(`${i + 1} 行目が JSON でない`);
    }
    if (v && typeof v === "object" && Array.isArray(v.artifacts)) out.push(...v.artifacts);
    else out.push(v);
  });
  return out;
}

/** 成果物のうち、通った記録に数えるもの（名前が ARTIFACT で、branch の実行で上げ、created_at が読める）を新しい順に。 */
export function passRecords(artifacts, branch = "main") {
  const records = [];
  for (const a of artifacts) {
    if (a?.name !== ARTIFACT || a.workflow_run?.head_branch !== branch) continue;
    const at = Date.parse(a.created_at);
    if (Number.isNaN(at)) continue;
    records.push({ at: new Date(at).toISOString(), run: a.workflow_run?.id ?? null });
  }
  return records.sort((x, y) => (x.at < y.at ? 1 : x.at > y.at ? -1 : 0));
}

export function checkHeartbeat(artifacts, { branch = "main", now = new Date(), maxAgeHours = MAX_AGE_HOURS } = {}) {
  const records = passRecords(artifacts, branch);
  const last = records[0] ?? null;
  const errors = [];
  if (last === null) {
    errors.push({ rule: "heartbeat_missing",
      message: `${branch} の sendback-protection の定期の実行が通った記録（成果物 ${ARTIFACT}）がない。schedule が止まっているか、落ち続けている` });
    return { last, ageHours: null, maxAgeHours, errors };
  }
  const ageHours = (now.getTime() - Date.parse(last.at)) / 3_600_000;
  if (ageHours > maxAgeHours) {
    errors.push({ rule: "heartbeat_stale",
      message: `sendback-protection の定期の実行が最後に通ったのは ${last.at}（${Math.floor(ageHours)} 時間前）で、${maxAgeHours} 時間より長くたっている。schedule が止まっているか、落ち続けている` });
  }
  return { last, ageHours, maxAgeHours, errors };
}

export function formatErrors(errors, annotate) {
  return errors.map((e) => annotate ? `::error title=sendback-check ${e.rule}::${e.message}` : `${e.rule}: ${e.message}`);
}

export function renderSummary({ last, ageHours, maxAgeHours, errors }) {
  const out = ["## sendback-protection の定期の実行", "", `最後に通った: ${last === null ? "記録なし" : `${last.at}（${Math.floor(ageHours)} 時間前）`}`, ""];
  if (errors.length === 0) out.push(`通る: 最後に通ってから ${maxAgeHours} 時間以内`);
  for (const e of errors) out.push(`- 外れた \`${e.rule}\`: ${e.message}`);
  return out.join("\n") + "\n";
}

function main() {
  const { values } = parseArgs({ options: { artifacts: { type: "string" }, branch: { type: "string" }, "max-age-hours": { type: "string" },
    now: { type: "string" }, json: { type: "string" }, summary: { type: "string" } } });
  let result;
  try {
    if (!values.artifacts) throw new Error("--artifacts に成果物の一覧の JSON を渡す");
    const now = values.now === undefined ? new Date() : new Date(values.now);
    if (Number.isNaN(now.getTime())) throw new Error(`--now が時刻でない: ${values.now}`);
    const maxAgeHours = values["max-age-hours"] === undefined ? MAX_AGE_HOURS : Number(values["max-age-hours"]);
    if (!(maxAgeHours > 0)) throw new Error(`--max-age-hours は正の数にする: ${values["max-age-hours"]}`);
    result = checkHeartbeat(parseArtifacts(readFileSync(values.artifacts, "utf8")), { branch: values.branch ?? "main", now, maxAgeHours });
  } catch (e) {
    console.error(`sendback-check heartbeat: 読めなかった: ${e.message}`);
    process.exit(2);
  }
  if (values.json) writeFileSync(values.json, JSON.stringify({ ok: result.errors.length === 0, ...result }, null, 2) + "\n");
  if (values.summary) appendFileSync(values.summary, renderSummary(result));
  for (const line of formatErrors(result.errors, process.env.GITHUB_ACTIONS === "true")) console.log(line);
  console.log(`sendback-check heartbeat: 最後に通った ${result.last?.at ?? "記録なし"}、外れ ${result.errors.length} 件`);
  process.exit(result.errors.length === 0 ? 0 : 1);
}

if (import.meta.url === `file://${process.argv[1]}`) main();
