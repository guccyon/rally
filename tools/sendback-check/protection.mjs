// ci/sendback-engine.yml の写しが branch protection の必須のチェックに入っているかを決定論で照らす（知見 K-0004 の担保。議決 D-0026、Issue #1018）。LLM を使わない。
//
//   gh api repos/{owner}/{repo}/branches/main/protection > protection.json   # 古い形の branch protection
//   gh api repos/{owner}/{repo}/rules/branches/main > protection.json        # rulesets（どちらの形でもよい）
//   node protection.mjs --protection protection.json [--repo <リポジトリの根>] [--json <出力先>] [--summary <GITHUB_STEP_SUMMARY>]
//   終了コード: 0 = 通る、1 = 外れた、2 = 読めなかった
//
// 写しを消す PR（写しがなければ wiring-order も走らない）や ci.test.mjs ごと見張りを書き換える PR は、必須のチェック（engine-test・wiring-order）が
// 報告されないことでしか止まらない。
// その止め方が効いているか、つまり .github/workflows/sendback-engine.yml の 2 つのジョブが報告するチェックの名前が、
// 既定のブランチの必須のチェックに入っているかを見る。branch protection は PR の GITHUB_TOKEN では読めないので、
// 建設者が gh api で取り出した JSON を渡して走らせ、結果を Issue #948 に記録する（README の「必須のチェックの照合」）。
//
// 見ること:
//   - 写し（.github/workflows/sendback-engine.yml）がないか、engine-test・wiring-order のジョブがない（workflow_not_copied）
//   - そのジョブが報告するチェックの名前（ジョブの name: があればそれ、なければ id）が必須のチェックにない（required_check_missing）
//
// GitHub Actions で走るとき（ci.test.mjs が wiring-order で呼ぶ。議決 D-0026、Issue #1020）は、JSON を渡さずに
// gh api repos/{owner}/{repo}/branches/{既定のブランチ}/protection/required_status_checks を自分で読む（readRequiredChecks）。
// 保護か必須のチェックがない（404 の Branch not protected・Required status checks not enabled）なら必須のチェックが空として
// required_check_missing、ほかの理由で読めなければ protection_unreadable で落とす（読めないのを通る側に数えない）。

import { spawnSync } from "node:child_process";
import { appendFileSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

import { REPO_ROOT } from "./order.mjs";

/** 写しの道（リポジトリの根から）。ci.test.mjs が見張るのもこの道。 */
export const COPY = ".github/workflows/sendback-engine.yml";
/** 必須のチェックにするジョブ（ci/sendback-engine.yml の id）。 */
export const REQUIRED_JOBS = Object.freeze(["engine-test", "wiring-order"]);

/**
 * branch protection の JSON から必須のチェックの名前を取り出す。
 * 古い形（required_status_checks の contexts と checks[].context）、その required_status_checks だけ（…/protection/required_status_checks の
 * 応答。contexts と checks が根にある）、rulesets（type が required_status_checks の規則の parameters.required_status_checks[].context）を読む。
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
    const rsc = "required_status_checks" in protection ? protection.required_status_checks : protection;
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

/** 写しの中身（ないときは null）と branch protection の JSON を照らす。 */
export function checkProtection({ workflow, protection }) {
  const errors = [];
  const contexts = requiredContexts(protection);
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

/** 保護か必須のチェックがないことを表す 404 の message（読めなかったのではなく、必須のチェックが空）。 */
const NOT_PROTECTED = /\b(Branch not protected|Required status checks not enabled)\b/;

/** gh を呼んで標準出力を返す。落ちたら gh の出力（応答の JSON か stderr）を message に持つ Error を投げる。 */
function ghApi(args, { gh = "gh", env = process.env } = {}) {
  const r = spawnSync(gh, ["api", ...args], { env, encoding: "utf8" });
  if (r.status === 0) return r.stdout;
  let message = (r.stderr || r.error?.message || "").trim();
  try { message = JSON.parse(r.stdout).message ?? message; } catch { /* 応答が JSON でない */ }
  const err = new Error(message || `gh api ${args[0]} が終了コード ${r.status} で落ちた`);
  err.apiMessage = message;
  throw err;
}

/**
 * 既定のブランチの必須のチェックを gh api で読む（GitHub Actions で走るとき。議決 D-0026、Issue #1020）。
 * 既定のブランチは GITHUB_EVENT_PATH のイベントの repository.default_branch、なければ gh api repos/{owner}/{repo} で引く。
 * 返すのは requiredContexts() に渡せる JSON と、引いたブランチ。読めなければ Error を投げる（保護がないときは空の JSON を返す）。
 */
export function readRequiredChecks({ env = process.env, gh = "gh" } = {}) {
  const repository = env.GITHUB_REPOSITORY;
  if (!/^[\w.-]+\/[\w.-]+$/.test(repository ?? "")) throw new Error(`GITHUB_REPOSITORY（${repository ?? ""}）が owner/repo の形でない`);
  let branch = null;
  if (env.GITHUB_EVENT_PATH && existsSync(env.GITHUB_EVENT_PATH)) {
    try { branch = JSON.parse(readFileSync(env.GITHUB_EVENT_PATH, "utf8"))?.repository?.default_branch ?? null; } catch { branch = null; }
  }
  if (typeof branch !== "string" || branch === "") branch = ghApi([`repos/${repository}`, "--jq", ".default_branch"], { gh, env }).trim();
  if (branch === "") throw new Error(`${repository} の既定のブランチが引けない`);
  try {
    const out = ghApi([`repos/${repository}/branches/${encodeURIComponent(branch)}/protection/required_status_checks`], { gh, env });
    return { branch, protection: JSON.parse(out) };
  } catch (e) {
    if (NOT_PROTECTED.test(e.apiMessage ?? "")) return { branch, protection: { contexts: [], checks: [] } };
    throw new Error(`${branch} の必須のチェックを読めない（${e.message}）。branch protection を読める権限のトークンを GH_TOKEN に渡す`);
  }
}

/**
 * GitHub Actions での照合（ci.test.mjs が呼ぶ）。gh api で読んだ必須のチェックと、写し（なければひな形）のジョブのチェックの名前を照らす。
 * 写しがないことは ci.test.mjs の別のテストが落とすので、ここではひな形の名前で照らす。読めなければ protection_unreadable。
 */
export function checkProtectionInActions({ env = process.env, gh = "gh", repo = REPO_ROOT } = {}) {
  const copy = join(repo, COPY);
  const workflow = readFileSync(existsSync(copy) ? copy : fileURLToPath(new URL("./ci/sendback-engine.yml", import.meta.url)), "utf8");
  try {
    const { branch, protection } = readRequiredChecks({ env, gh });
    return { branch, ...checkProtection({ workflow, protection }) };
  } catch (e) {
    return { branch: null, contexts: [], errors: [{ rule: "protection_unreadable", message: e.message }] };
  }
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
  const { values } = parseArgs({ options: { protection: { type: "string" }, repo: { type: "string" }, json: { type: "string" },
    summary: { type: "string" } } });
  let result;
  try {
    if (!values.protection) throw new Error("--protection に branch protection の JSON を渡す");
    const protection = JSON.parse(readFileSync(values.protection, "utf8"));
    const copy = join(values.repo ?? REPO_ROOT, COPY);
    result = checkProtection({ workflow: existsSync(copy) ? readFileSync(copy, "utf8") : null, protection });
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
