// ci/sendback-engine.yml の写しが branch protection の必須のチェックに入っているかを決定論で照らす（知見 K-0004 の担保。議決 D-0026、Issue #1018）。LLM を使わない。
//
//   gh api repos/{owner}/{repo}/branches/main/protection > protection.json   # 古い形の branch protection
//   gh api repos/{owner}/{repo}/rules/branches/main > protection.json        # rulesets（どちらの形でもよい）
//   node protection.mjs --protection protection.json [--protection rules.json] [--repo <リポジトリの根>] [--json <出力先>] [--summary <GITHUB_STEP_SUMMARY>]
//   終了コード: 0 = 通る、1 = 外れた、2 = 読めなかった
//
// 写しを消す PR（写しがなければ wiring-order も走らない）や ci.test.mjs ごと見張りを書き換える PR は、必須のチェック（engine-test・wiring-order）が
// 報告されないことでしか止まらない。
// その止め方が効いているか、つまり .github/workflows/sendback-engine.yml の 2 つのジョブが報告するチェックの名前が、
// 既定のブランチの必須のチェックに入っているかを見る。branch protection は PR の GITHUB_TOKEN では読めないので、
// 建設者が gh api で取り出した JSON を渡して走らせ、結果を Issue #948 に記録する（README の「必須のチェックの照合」）。
// 必須のチェックが後で外されたことに気づくよう、ci/sendback-protection.yml が branch protection を読める権限のトークンで
// 毎日と手動で走らせ、外れたら Issue を開くか更新する（議決 D-0039、Issue #1087）。
//
// 見ること:
//   - 写し（.github/workflows/sendback-engine.yml）がないか、engine-test・wiring-order のジョブがない（workflow_not_copied）
//   - そのジョブが報告するチェックの名前（ジョブの name: があればそれ、なければ id）が必須のチェックにない（required_check_missing）

import { appendFileSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { parseArgs } from "node:util";

import { REPO_ROOT } from "./order.mjs";

/** 写しの道（リポジトリの根から）。ci.test.mjs が見張るのもこの道。 */
export const COPY = ".github/workflows/sendback-engine.yml";
/** 必須のチェックにするジョブ（ci/sendback-engine.yml の id）。 */
export const REQUIRED_JOBS = Object.freeze(["engine-test", "wiring-order"]);

/**
 * branch protection の JSON から必須のチェックの名前を取り出す。
 * 古い形（required_status_checks の contexts と checks[].context）と rulesets（type が required_status_checks の規則の
 * parameters.required_status_checks[].context）のどちらも読む。保護がない（404 の { message } など）なら空。
 */
export function requiredContexts(protection) {
  const names = new Set();
  const add = (c) => { if (typeof c === "string" && c !== "") names.add(c); };
  if (Array.isArray(protection)) {
    for (const rule of protection) {
      if (rule?.type !== "required_status_checks") continue;
      for (const c of rule.parameters?.required_status_checks ?? []) add(c?.context);
    }
  } else if (protection && typeof protection === "object") {
    const rsc = protection.required_status_checks;
    for (const c of rsc?.contexts ?? []) add(c);
    for (const c of rsc?.checks ?? []) add(c?.context);
  } else {
    throw new Error("branch protection の JSON はオブジェクトか配列にする");
  }
  return names;
}

/** ワークフローのジョブの id と、そのジョブが報告するチェックの名前（ジョブの name: があればそれ）。 */
export function jobCheckNames(text) {
  const lines = text.split(/\r?\n/);
  const jobs = new Map();
  const start = lines.findIndex((l) => /^jobs:\s*$/.test(l));
  if (start < 0) return jobs;
  let current = null;
  for (const l of lines.slice(start + 1)) {
    if (/^\S/.test(l)) break;
    const id = l.match(/^ {2}([\w-]+):\s*$/);
    if (id) { current = id[1]; jobs.set(current, current); continue; }
    const name = current && l.match(/^ {4}name:\s*(.+?)\s*$/);
    if (name) jobs.set(current, name[1].replace(/^(["'])(.*)\1$/, "$2"));
  }
  return jobs;
}

/**
 * 写しの中身（ないときは null）と branch protection の JSON を照らす。protections に複数渡すと（古い形と rulesets の両方など）、
 * どれかで必須のチェックになっていれば必須に数える。
 */
export function checkProtection({ workflow, protection, protections = [protection] }) {
  const errors = [];
  const contexts = new Set(protections.flatMap((p) => [...requiredContexts(p)]));
  if (workflow == null) {
    errors.push({ rule: "workflow_not_copied", message: `${COPY} がない（ci/sendback-engine.yml を写してから必須のチェックにする）` });
    return { contexts: [...contexts], errors };
  }
  const jobs = jobCheckNames(workflow);
  for (const job of REQUIRED_JOBS) {
    if (!jobs.has(job)) {
      errors.push({ rule: "workflow_not_copied", message: `${COPY} に ${job} のジョブがない` });
    } else if (!contexts.has(jobs.get(job))) {
      errors.push({ rule: "required_check_missing",
        message: `${job} のジョブ（チェックの名前 ${jobs.get(job)}）が必須のチェックにない。写しを消す・ジョブの名前を変える PR が止まらない` });
    }
  }
  return { contexts: [...contexts], errors };
}

export function formatErrors(errors, annotate) {
  return errors.map((e) => annotate ? `::error title=sendback-check ${e.rule}::${e.message}` : `${e.rule}: ${e.message}`);
}

export function renderSummary({ contexts, errors }) {
  const out = ["## sendback-check の必須のチェック", "", `必須のチェック: ${contexts.length === 0 ? "なし" : contexts.join("・")}`, ""];
  if (errors.length === 0) out.push(`通る: ${REQUIRED_JOBS.join("・")} が必須のチェックに入っている`);
  for (const e of errors) out.push(`- 外れた \`${e.rule}\`: ${e.message}`);
  return out.join("\n") + "\n";
}

function main() {
  const { values } = parseArgs({ options: { protection: { type: "string", multiple: true }, repo: { type: "string" }, json: { type: "string" },
    summary: { type: "string" } } });
  let result;
  try {
    if (!values.protection?.length) throw new Error("--protection に branch protection の JSON を渡す");
    const protections = values.protection.map((f) => JSON.parse(readFileSync(f, "utf8")));
    const copy = join(values.repo ?? REPO_ROOT, COPY);
    result = checkProtection({ workflow: existsSync(copy) ? readFileSync(copy, "utf8") : null, protections });
  } catch (e) {
    console.error(`sendback-check protection: 読めなかった: ${e.message}`);
    process.exit(2);
  }
  if (values.json) writeFileSync(values.json, JSON.stringify({ ok: result.errors.length === 0, ...result }, null, 2) + "\n");
  if (values.summary) appendFileSync(values.summary, renderSummary(result));
  for (const line of formatErrors(result.errors, process.env.GITHUB_ACTIONS === "true")) console.log(line);
  console.log(`sendback-check protection: 必須のチェック ${result.contexts.length} 件、外れ ${result.errors.length} 件`);
  process.exit(result.errors.length === 0 ? 0 : 1);
}

if (import.meta.url === `file://${process.argv[1]}`) main();
