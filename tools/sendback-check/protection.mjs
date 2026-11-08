// ci/sendback-engine.yml の写しが branch protection の必須のチェックに入っているかを決定論で照らす（知見 K-0004 の担保。議決 D-0026、Issue #1018）。LLM を使わない。
//
//   gh api repos/{owner}/{repo}/branches/main/protection > protection.json   # 古い形の branch protection
//   gh api repos/{owner}/{repo}/rules/branches/main > protection.json        # rulesets（どちらの形でもよい）
//   node protection.mjs --protection protection.json [--repo <リポジトリの根>] [--json <出力先>] [--summary <GITHUB_STEP_SUMMARY>]
//   終了コード: 0 = 通る、1 = 外れた、2 = 読めなかった
//
// 写しを消す PR（写しがなければ wiring-order も走らない）や ci.test.mjs ごと見張りを書き換える PR は、必須のチェック（sendback-engine-test・sendback-wiring-order）が
// 報告されないことでしか止まらない。別のワークフローの間は needs で待てないので、check.mjs --events を engine.test.mjs を待たずに
// 走らせる形も、engine-test を必須のチェックにする設定でしか止まらない（議決 D-0038）。
// その止め方が効いているか、つまり .github/workflows/sendback-engine.yml の 2 つのジョブが報告するチェックの名前が、
// 既定のブランチの必須のチェックに入っているかを見る。branch protection は PR の GITHUB_TOKEN では読めないので、
// 建設者が gh api で取り出した JSON を渡して走らせ、結果を Issue #948 に記録する（README の「必須のチェックの照合」）。
//
// 見ること:
//   - 写し（.github/workflows/sendback-engine.yml）がないか、engine-test・wiring-order のジョブがない（workflow_not_copied）
//   - そのジョブが報告するチェックの名前（ジョブの name: があればそれ、なければ id）が CHECK_NAMES でない（check_name_changed。ひな形と写しのずれは ci.test.mjs が見る）
//   - そのチェックの名前が必須のチェックにない（required_check_missing）
//   - 同じチェックの名前を .github/workflows/ のほかのジョブも報告する（check_name_shared。どちらの報告でも必須のチェックが満たされ、
//     写しを消しても、別のワークフローで check.mjs --events を engine.test.mjs を待たずに走らせても止まらない。議決 D-0038、Issue #1083）

import { appendFileSync, existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { parseArgs } from "node:util";

import { REPO_ROOT } from "./order.mjs";

/** 写しの道（リポジトリの根から）。ci.test.mjs が見張るのもこの道。 */
export const COPY = ".github/workflows/sendback-engine.yml";
/** 必須のチェックにするジョブ（ci/sendback-engine.yml の id）。 */
export const REQUIRED_JOBS = Object.freeze(["engine-test", "wiring-order"]);
/** そのジョブが報告するチェックの名前（ジョブの name:）。id のままの engine-test はほかのワークフローも報告するので使わない。 */
export const CHECK_NAMES = Object.freeze({ "engine-test": "sendback-engine-test", "wiring-order": "sendback-wiring-order" });

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

/** 写しの中身（ないときは null）、ほかのワークフロー（{ 道: 中身 }）、branch protection の JSON を照らす。 */
export function checkProtection({ workflow, others = {}, protection }) {
  const errors = [];
  const contexts = requiredContexts(protection);
  if (workflow == null) {
    errors.push({ rule: "workflow_not_copied", message: `${COPY} がない（ci/sendback-engine.yml を写してから必須のチェックにする）` });
    return { contexts: [...contexts], errors };
  }
  const jobs = jobCheckNames(workflow);
  const otherJobs = Object.entries(others).flatMap(([path, text]) => [...jobCheckNames(text)].map(([job, name]) => ({ path, job, name })));
  for (const job of REQUIRED_JOBS) {
    if (!jobs.has(job)) {
      errors.push({ rule: "workflow_not_copied", message: `${COPY} に ${job} のジョブがない` });
      continue;
    }
    const name = jobs.get(job);
    if (name !== CHECK_NAMES[job]) {
      errors.push({ rule: "check_name_changed",
        message: `${job} のチェックの名前が ${name}（${CHECK_NAMES[job]} にする。ひな形から写し直す）` });
    }
    if (!contexts.has(name)) {
      errors.push({ rule: "required_check_missing",
        message: `${job} のジョブ（チェックの名前 ${name}）が必須のチェックにない。写しを消す・ジョブの名前を変える PR が止まらない` });
    }
    for (const other of otherJobs.filter((o) => o.name === name)) {
      errors.push({ rule: "check_name_shared",
        message: `${other.path} の ${other.job} も同じチェックの名前 ${name} を報告する。どちらの報告でも必須のチェックが満たされ、${COPY} を消しても止まらない` });
    }
  }
  return { contexts: [...contexts], errors };
}

/** リポジトリの根から、写し（copy）と .github/workflows/ のほかのワークフローを読む。 */
export function loadWorkflows(root, copy = COPY) {
  const dir = join(root, ".github", "workflows");
  const others = {};
  if (existsSync(dir)) {
    for (const f of readdirSync(dir).sort()) {
      const path = `.github/workflows/${f}`;
      if (path !== copy && /\.ya?ml$/.test(f)) others[path] = readFileSync(join(dir, f), "utf8");
    }
  }
  return { workflow: existsSync(join(root, copy)) ? readFileSync(join(root, copy), "utf8") : null, others };
}

export function formatErrors(errors, annotate) {
  return errors.map((e) => annotate ? `::error title=sendback-check ${e.rule}::${e.message}` : `${e.rule}: ${e.message}`);
}

export function renderSummary({ contexts, errors }) {
  const out = ["## sendback-check の必須のチェック", "", `必須のチェック: ${contexts.length === 0 ? "なし" : contexts.join("・")}`, ""];
  if (errors.length === 0) out.push(`通る: ${REQUIRED_JOBS.map((j) => `${CHECK_NAMES[j]}（${j}）`).join("・")}が必須のチェックに入っている`);
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
