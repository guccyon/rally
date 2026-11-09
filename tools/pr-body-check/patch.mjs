// 関所の配線の差分（engine/pr-body-gate.patch）が、Engine の ENGINE_REF の版と既定のブランチの先頭に当たるかを決定論で照らす
// （知見 K-0001 の担保。議決 D-0068（Issue #1179 の起票時の番号は D-0066。PR #1176）、Issue #1179）。LLM を使わない。
//
//   node patch.mjs --engine <Engine のチェックアウト> [--workflow <ENGINE_REF を読むワークフロー>] [--json <出力先>] [--summary <GITHUB_STEP_SUMMARY>]
//   終了コード: 0 = どちらの版でも当たるかもう入っている、1 = patch_stale か patch_partially_applied、2 = 照らせなかった
//
// 差分は Engine に入れるまで rally に置いたままなので、Engine の先頭が進むと黙って当たらなくなる（Engine 476573e で FACTORY_RESULT の行が
// 変わり、engine.test.mjs の (4)〜(10) が「差分が当たらない」で落ちていた。Issue #1107）。engine.test.mjs は ENGINE_DIR の今の中身に当てるだけで、
// 当たらないのか、もう入っているのか、一部だけ入ったのかを分けない。ここでは版ごとに Engine のファイルを git から取り出し、
// git apply --check と git apply --reverse --check で照らして 4 つに分ける（tools/knowledge-enforcement-check/test_engine_patch.py と同じ形。議決 D-0024・D-0036）。
//   - applies: 差分が当たる
//   - merged: もう入っている（git apply --reverse --check が通る）
//   - patch_partially_applied: 差分で足したもの（MARKERS）はあるのに --reverse --check が通らない（一部だけ入った・入れたあと変わった）
//   - patch_stale: 当たらず、差分で足したものもない
// 照らす版は、ENGINE_REF（写し .github/workflows/pr-body-engine.yml があればそれ、なければひな形 ci/pr-body-engine.yml の env）と、
// --engine のチェックアウトの HEAD（ci/pr-body-patch.yml は ref なしで取るので既定のブランチの先頭）。ENGINE_REF の版を含むよう、全部の履歴で取る。
// ci/pr-body-patch.yml が main への push・毎日・手動で走らせ、patch_stale か patch_partially_applied なら落ちて Issue を開くか更新する。

import { execFileSync } from "node:child_process";
import { appendFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

export const REPO_ROOT = fileURLToPath(new URL("../../", import.meta.url));
export const PATCH = fileURLToPath(new URL("./engine/pr-body-gate.patch", import.meta.url));
/** ENGINE_REF を読むワークフロー（リポジトリの根から）。写しがあれば写し、なければひな形。 */
export const COPY = ".github/workflows/pr-body-engine.yml";
export const TEMPLATE = "tools/pr-body-check/ci/pr-body-engine.yml";
/** 差分で Engine に足すもの。どれかがあるのに --reverse --check が通らなければ、一部だけ入ったか入れたあと変わった。 */
export const MARKERS = [
  ["orchestrator/worker/entrypoint.sh", "tools/pr-body-check/gate.mjs"],
  ["orchestrator/dispatcher/dispatcher.py", "def act_on_body_gate("],
  ["definitions/events.yaml", "pr.body_gate_blocked:"],
];

export const APPLIES = "applies";
export const MERGED = "merged";
export const STALE = "patch_stale";
export const PARTIAL = "patch_partially_applied";

/**
 * 照らせなかったこと（差分の古さではない）。code は engine_ref_unreadable（ENGINE_REF を読めない。detail.path）か
 * engine_ref_missing（ENGINE_REF の版がチェックアウトにない。detail.engine・detail.ref）。利用者への文言は CLI（main）で組む。
 */
export class PatchCheckError extends Error {
  constructor(code, detail = {}) {
    super(code);
    this.name = "PatchCheckError";
    this.code = code;
    this.detail = detail;
  }
}

/** ワークフローの env の ENGINE_REF（40 桁のコミット SHA）。 */
export function engineRef(text, path) {
  const m = text.match(/^\s*ENGINE_REF:\s*['"]?([0-9a-f]{40})['"]?\s*$/m);
  if (!m) throw new PatchCheckError("engine_ref_unreadable", { path });
  return m[1];
}

/** ENGINE_REF を読むワークフローの道（写しがあれば写し）。 */
export const workflowPath = (root = REPO_ROOT) => (existsSync(join(root, COPY)) ? join(root, COPY) : join(root, TEMPLATE));

/** 差分が触るファイル（a/ の側の道）。 */
export const patchFiles = (patchText) => [...patchText.matchAll(/^diff --git a\/(\S+) b\/\S+$/gm)].map((m) => m[1]);

const git = (cwd, args, opts = {}) => execFileSync("git", args, { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], ...opts });

export function hasCommit(engine, ref) {
  try {
    git(engine, ["cat-file", "-e", `${ref}^{commit}`]);
    return true;
  } catch {
    return false;
  }
}

/**
 * ref の版の Engine に対する差分の状態（{ state, detail }）。差分が触るファイルと MARKERS のファイルだけを git から取り出して照らすので、
 * チェックアウトの今の中身や版には左右されない。
 */
export function patchState(engine, ref, { patch = PATCH, markers = MARKERS } = {}) {
  const sha = git(engine, ["rev-parse", "--verify", `${ref}^{commit}`]).trim();
  const dir = mkdtempSync(join(tmpdir(), "pr-body-patch-"));
  const files = [...new Set([...patchFiles(readFileSync(patch, "utf8")), ...markers.map(([file]) => file)])];
  // 実行の許可も版のとおりにする（違うと git apply が mode の警告を出す）
  const executable = new Set(git(engine, ["ls-tree", sha, "--", ...files]).split("\n")
    .filter((l) => l.startsWith("100755 ")).map((l) => l.split("\t")[1]));
  for (const f of files) {
    let text;
    try {
      text = execFileSync("git", ["show", `${sha}:${f}`], { cwd: engine, stdio: ["ignore", "pipe", "pipe"] });
    } catch {
      continue;   // その版にないファイル（差分が当たらないので patch_stale に数える）
    }
    mkdirSync(dirname(join(dir, f)), { recursive: true });
    writeFileSync(join(dir, f), text, { mode: executable.has(f) ? 0o755 : 0o644 });
  }
  const apply = (...args) => {
    try {
      git(dir, ["apply", "--check", ...args, patch]);
      return { ok: true, stderr: "" };
    } catch (e) {
      return { ok: false, stderr: String(e.stderr ?? e.message).trim() };
    }
  };
  const fwd = apply();
  if (fwd.ok) return { state: APPLIES, detail: "" };
  if (apply("--reverse").ok) return { state: MERGED, detail: "" };
  const found = markers.filter(([f, s]) => existsSync(join(dir, f)) && readFileSync(join(dir, f), "utf8").includes(s));
  if (found.length > 0) {
    return { state: PARTIAL, detail: `${found.map(([f, s]) => `${f} の ${s}`).join("・")} はあるが git apply --reverse --check が通らない` };
  }
  return { state: STALE, detail: fwd.stderr };
}

/**
 * ENGINE_REF の版と、チェックアウトの HEAD（既定のブランチの先頭）の 2 つを照らす。ENGINE_REF の版がチェックアウトにないときは
 * 照らせない（engine_ref_missing。差分の古さではないので落とすが Issue は開かない）。
 */
export function checkPatch({ engine, workflow = workflowPath(), patch = PATCH, markers = MARKERS }) {
  const ref = engineRef(readFileSync(workflow, "utf8"), workflow);
  if (!hasCommit(engine, ref)) {
    throw new PatchCheckError("engine_ref_missing", { engine, ref });
  }
  const head = git(engine, ["rev-parse", "HEAD"]).trim();
  const targets = [["ENGINE_REF", ref], ["既定のブランチの先頭", head]];
  const results = targets.map(([target, sha]) => ({ target, ref: sha, ...patchState(engine, sha, { patch, markers }) }));
  const errors = results.filter((r) => r.state === STALE || r.state === PARTIAL).map((r) => ({
    rule: r.state, target: r.target, ref: r.ref,
    message: r.state === STALE
      ? `${r.target}（${r.ref}）の Engine に engine/pr-body-gate.patch が当たらず、Engine 側にもまだ入っていない。今の Engine に合わせて作り直す`
      : `${r.target}（${r.ref}）の Engine に ${r.detail}（差分の一部だけが入ったか、入れたあと変わった）。engine/pr-body-gate.patch を今の Engine に合わせて作り直す（全部入れて ENGINE_REF を上げたなら差分を消す）`,
    detail: r.detail,
  }));
  return { results, errors };
}

const LABELS = { [APPLIES]: "当たる", [MERGED]: "もう入っている", [STALE]: "patch_stale", [PARTIAL]: "patch_partially_applied" };

export function formatErrors(errors, annotate) {
  return errors.map((e) => annotate ? `::error title=pr-body-patch ${e.rule}::${e.message}` : `${e.rule}: ${e.message}`);
}

export function renderSummary({ results, errors }) {
  const out = ["## pr-body-check の差分（engine/pr-body-gate.patch）", ""];
  for (const r of results) out.push(`- ${r.target}（\`${r.ref}\`）: ${LABELS[r.state]}`);
  out.push("");
  for (const e of errors) {
    out.push(`- 外れた \`${e.rule}\`: ${e.message}`);
    if (e.detail) out.push("", "```", e.detail, "```", "");
  }
  return out.join("\n") + "\n";
}

/** 照らせなかったことの案内（CLI に出す文言）。 */
export function describeFailure(e) {
  if (!(e instanceof PatchCheckError)) return e.message;
  if (e.code === "engine_ref_unreadable") return `engine_ref_unreadable: ${e.detail.path ?? "ワークフロー"} に ENGINE_REF（40 桁のコミット SHA）がない`;
  if (e.code === "engine_ref_missing") {
    return `engine_ref_missing: Engine のチェックアウト（${e.detail.engine}）に ENGINE_REF ${e.detail.ref} の版がない（全部の履歴で取る）`;
  }
  return e.code;
}

function main() {
  const { values } = parseArgs({ options: { engine: { type: "string" }, workflow: { type: "string" }, json: { type: "string" },
    summary: { type: "string" } } });
  const fail = (reason) => {
    const msg = `pr-body-check patch: 照らせなかった: ${reason}`;
    console.log(process.env.GITHUB_ACTIONS === "true" ? `::error title=pr-body-patch patch_check_failed::${msg}` : msg);
    process.exit(2);
  };
  if (!values.engine) fail("--engine に Engine のチェックアウトを渡す");
  let result;
  try {
    result = checkPatch({ engine: values.engine, workflow: values.workflow ?? workflowPath() });
  } catch (e) {
    fail(describeFailure(e));
  }
  if (values.json) writeFileSync(values.json, JSON.stringify({ ok: result.errors.length === 0, ...result }, null, 2) + "\n");
  if (values.summary) appendFileSync(values.summary, renderSummary(result));
  for (const r of result.results) console.log(`${r.target} ${r.ref}: ${LABELS[r.state]}`);
  for (const line of formatErrors(result.errors, process.env.GITHUB_ACTIONS === "true")) console.log(line);
  for (const e of result.errors) if (e.detail) console.log(e.detail);
  process.exit(result.errors.length === 0 ? 0 : 1);
}

if (import.meta.url === `file://${process.argv[1]}`) main();
