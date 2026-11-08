// ci/pr-body-engine.yml の写しが branch protection の必須のチェックに入っているかを決定論で照らす（知見 K-0001 の担保。議決 D-0032、Issue #1037）。LLM を使わない。
//
//   gh api repos/{owner}/{repo}/branches/main/protection > protection.json   # 古い形の branch protection
//   gh api repos/{owner}/{repo}/rules/branches/main > protection.json        # rulesets（どちらの形でもよい）
//   node protection.mjs --protection protection.json [--repo <リポジトリの根>] [--json <出力先>] [--summary <GITHUB_STEP_SUMMARY>]
//   終了コード: 0 = 通る、1 = 外れた、2 = 読めなかった
//
// 関所を呼ばない Engine の版で engine.test.mjs の (3) が落ちても、そのチェックが必須でなければマージは止まらない。写しを消す PR も、
// 必須のチェックが報告されないことでしか止まらない。branch protection は PR の GITHUB_TOKEN では読めないので、建設者が gh api で取り出した
// JSON を渡して走らせ、結果を記録する（README の「配線の手順（建設者）」）。branch protection の JSON の読み方は tools/sendback-check/protection.mjs のものを使う。
//
// 見ること:
//   - 写し（.github/workflows/pr-body-engine.yml）がないか、engine-test のジョブがない（workflow_not_copied）
//   - engine-test が報告するチェックの名前が CHECK_NAME でない（check_name_changed。ひな形と写しのずれは ci.test.mjs が見る）
//   - そのチェックの名前が必須のチェックにない（required_check_missing）
//   - 同じチェックの名前を .github/workflows/ のほかのジョブも報告する（check_name_shared。どちらの報告でも必須のチェックが満たされ、写しを消しても止まらない）

import { appendFileSync, readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

import { jobCheckNames, loadWorkflows as loadCopyAndOthers, requiredContexts } from "../sendback-check/protection.mjs";

export const REPO_ROOT = fileURLToPath(new URL("../../", import.meta.url));
/** 写しの道（リポジトリの根から）。ci.test.mjs が見張るのもこの道。 */
export const COPY = ".github/workflows/pr-body-engine.yml";
/** 必須のチェックにするジョブ（ci/pr-body-engine.yml の id）。 */
export const REQUIRED_JOB = "engine-test";
/** そのジョブが報告するチェックの名前（ジョブの name:）。 */
export const CHECK_NAME = "pr-body-engine-test";

/**
 * 写しの中身（ないときは null）、ほかのワークフロー（{ 道: 中身 }）、branch protection の JSON を照らす。
 */
export function checkProtection({ workflow, others = {}, protection }) {
  const errors = [];
  const contexts = requiredContexts(protection);
  if (workflow == null) {
    errors.push({ rule: "workflow_not_copied", message: `${COPY} がない（ci/pr-body-engine.yml を写してから必須のチェックにする）` });
    return { contexts: [...contexts], errors };
  }
  const jobs = jobCheckNames(workflow);
  if (!jobs.has(REQUIRED_JOB)) {
    errors.push({ rule: "workflow_not_copied", message: `${COPY} に ${REQUIRED_JOB} のジョブがない` });
    return { contexts: [...contexts], errors };
  }
  const name = jobs.get(REQUIRED_JOB);
  if (name !== CHECK_NAME) {
    errors.push({ rule: "check_name_changed",
      message: `${REQUIRED_JOB} のチェックの名前が ${name}（${CHECK_NAME} にする。ひな形から写し直す）` });
  }
  if (!contexts.has(name)) {
    errors.push({ rule: "required_check_missing",
      message: `${REQUIRED_JOB} のジョブ（チェックの名前 ${name}）が必須のチェックにない。関所を呼ばない Engine で落ちてもマージが止まらない` });
  }
  for (const [path, text] of Object.entries(others)) {
    for (const [job, other] of jobCheckNames(text)) {
      if (other === name) {
        errors.push({ rule: "check_name_shared",
          message: `${path} の ${job} も同じチェックの名前 ${name} を報告する。どちらの報告でも必須のチェックが満たされ、${COPY} を消しても止まらない` });
      }
    }
  }
  return { contexts: [...contexts], errors };
}

/** リポジトリの根から、写しとほかのワークフローを読む（読み方は tools/sendback-check/protection.mjs のもの）。 */
export const loadWorkflows = (root) => loadCopyAndOthers(root, COPY);

export function formatErrors(errors, annotate) {
  return errors.map((e) => annotate ? `::error title=pr-body-check ${e.rule}::${e.message}` : `${e.rule}: ${e.message}`);
}

export function renderSummary({ contexts, errors }) {
  const out = ["## pr-body-check の必須のチェック", "", `必須のチェック: ${contexts.length === 0 ? "なし" : contexts.join("・")}`, ""];
  if (errors.length === 0) out.push(`通る: ${CHECK_NAME}（${COPY} の ${REQUIRED_JOB}）が必須のチェックに入っている`);
  for (const e of errors) out.push(`- 外れた \`${e.rule}\`: ${e.message}`);
  return out.join("\n") + "\n";
}

function main() {
  const { values } = parseArgs({ options: { protection: { type: "string" }, repo: { type: "string" }, json: { type: "string" },
    summary: { type: "string" } } });
  let result;
  try {
    if (!values.protection) throw new Error("--protection に branch protection の JSON を渡す");
    const protection = JSON.parse(readFileSync(values.protection, "utf8"));
    result = checkProtection({ ...loadWorkflows(values.repo ?? REPO_ROOT), protection });
  } catch (e) {
    console.error(`pr-body-check protection: 読めなかった: ${e.message}`);
    process.exit(2);
  }
  if (values.json) writeFileSync(values.json, JSON.stringify({ ok: result.errors.length === 0, ...result }, null, 2) + "\n");
  if (values.summary) appendFileSync(values.summary, renderSummary(result));
  for (const line of formatErrors(result.errors, process.env.GITHUB_ACTIONS === "true")) console.log(line);
  console.log(`pr-body-check protection: 必須のチェック ${result.contexts.length} 件、外れ ${result.errors.length} 件`);
  process.exit(result.errors.length === 0 ? 0 : 1);
}

if (import.meta.url === `file://${process.argv[1]}`) main();
