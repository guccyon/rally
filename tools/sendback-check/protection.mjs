// ci/sendback-engine.yml の写しが branch protection の必須のチェックに入っているかを決定論で照らす（知見 K-0004 の担保。議決 D-0026、Issue #1018）。LLM を使わない。
//
//   gh api repos/{owner}/{repo}/branches/main/protection > protection.json   # 古い形の branch protection（管理者の権限が要る）
//   gh api repos/{owner}/{repo}/branches/main > branch.json                  # ブランチ（protection の要約。読む権限で読める）
//   gh api repos/{owner}/{repo}/rules/branches/main > rules.json             # rulesets（読む権限で読める）
//   node protection.mjs --protection branch.json --protection rules.json [--repo <リポジトリの根>] [--json <出力先>] [--summary <GITHUB_STEP_SUMMARY>]
//   --protection は何度でも渡せ、必須のチェックはすべての JSON を合わせたものにする（古い形と rulesets の両方で必須にできるため）
//   終了コード: 0 = 通る、1 = 外れた、2 = 読めなかった
//
// 写しを消す PR（写しがなければ wiring-order も走らない）や ci.test.mjs ごと見張りを書き換える PR は、必須のチェック（engine-test・wiring-order）が
// 報告されないことでしか止まらない。
// その止め方が効いているか、つまり .github/workflows/sendback-engine.yml の 2 つのジョブが報告するチェックの名前が、
// 既定のブランチの必須のチェックに入っているかを見る。ci/sendback-engine.yml の required-checks のジョブが PR ごとに、
// PR の GITHUB_TOKEN でも読めるブランチと rulesets の JSON を gh api で取って走らせ、外れていれば落とす（議決 D-0040、Issue #1091）。
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
 * 古い形（required_status_checks の contexts と checks[].context）、ブランチの JSON（protection.required_status_checks に同じ形）と
 * rulesets（type が required_status_checks の規則の parameters.required_status_checks[].context）のどれも読む。
 * 保護がない（404 の { message } など）なら空。
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
    const rsc = protection.required_status_checks ?? protection.protection?.required_status_checks;
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

/** 写しの中身（ないときは null）と branch protection の JSON（protections に列で渡せば、必須のチェックはそれらを合わせたもの）を照らす。 */
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
