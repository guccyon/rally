// Engine の本物の本文の組み込み（worker/entrypoint.sh の続きのジョブの節）と本文の直しの積み方（dispatcher.is_body_only・enqueue_body_fix）で、
// 続きのジョブの summary を組み込んだ本文が関所（gate.mjs）に照らされ、外れたら本文を書かずに body_fix に回ることを確かめる
// （知見 K-0001 の担保。議決 D-0013、Issue #956）。入力は事例 C-0005（PR #659）の 21 回の追記（c0005.mjs）。
// Engine のチェックアウトは環境変数 ENGINE_DIR で渡す（なければリポジトリの隣の ../engine）。どちらもなければ飛ばす。
// ただし ENGINE_DIR を渡したときと CI（CI=true）では飛ばさずに落とす（sendback-check の engine.test.mjs と同じ。議決 D-0011）。
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { chmodSync, existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { FIRST_SUMMARY, ISSUE, PR_URL, REASONS, continuationSummary, legacyBody, newPrBody } from "./c0005.mjs";
import { checkBody } from "./check.mjs";
import { gate } from "./gate.mjs";

const harness = fileURLToPath(new URL("./engine_harness.py", import.meta.url));
const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const engineDir = process.env.ENGINE_DIR || fileURLToPath(new URL("../../../engine", import.meta.url));
const entrypointPath = `${engineDir}/orchestrator/worker/entrypoint.sh`;
const required = Boolean(process.env.ENGINE_DIR) || process.env.CI === "true";
const skip = required || existsSync(entrypointPath) ? false : `Engine のチェックアウトがない（ENGINE_DIR=${engineDir}）`;

const PR_NUMBER = PR_URL.split("/").pop();

/** entrypoint.sh から、start に合う行から end に合う行までを取り出す（includeEnd が false なら end の行は含めない）。 */
export function extract(source, start, end, includeEnd) {
  const lines = source.split("\n");
  const from = lines.findIndex(start);
  const to = from < 0 ? -1 : lines.findIndex((l, i) => i > from && end(l));
  if (from < 0 || to < 0) return null;
  return lines.slice(from, includeEnd ? to + 1 : to).join("\n");
}

/** Engine の entrypoint.sh から、本文を組む 3 つの節を取り出す。名前や形が変わって取り出せなければ null。 */
export function snippets(source) {
  return {
    // 新しい PR の本文（PR_BODY のヒアドキュメント）
    prBody: extract(source, (l) => l.startsWith("PR_BODY=$(cat <<EOF"), (l) => l === ")", true),
    // 続きのジョブの summary を本文に組み込む（SECTION_TITLE から NEW_BODY まで。本文の更新の前）
    compose: extract(source, (l) => /^\s*case "\$\{CONTINUE_REASON\}" in$/.test(l) && source.includes(`${l}\n    conflict) SECTION_TITLE=`),
      (l) => /^\s*if \[ "\$\{CONTINUE_REASON\}" = "incomplete" \]/.test(l), false),
    // 本文を読んでから書くまで（gh pr view から「continued」のログまで）
    update: extract(source, (l) => l.includes('OLD_BODY=$(gh pr view "${CONTINUE_PR}"'), (l) => l.includes('echo "[worker] continued pr='), true),
  };
}

const entrypoint = existsSync(entrypointPath) ? readFileSync(entrypointPath, "utf8") : "";
const parts = snippets(entrypoint);

/** 使う節を取り出せていなければ落とす（取り出せないまま空の本文で通さない）。 */
const need = (...names) => names.forEach((n) => assert.ok(parts[n], `entrypoint.sh から ${n} の節を取り出せない（ENGINE_DIR=${engineDir}）`));

const bash = (script, env, cwd) =>
  execFileSync("bash", ["-c", script], { env: { PATH: process.env.PATH, ...env }, cwd, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });

const JOB = { REPO: "guccyon/rally", ISSUE_NUMBER: String(ISSUE), TITLE: "コーチの様子に返却待ちの件数を足す", MODEL: "tier-large",
  TURNS: "2", COST: "1.95", CLAUDE_RC: "0", MAX_TURNS: "200", INCOMPLETE: "", LINK_KW: "Closes" };

/** Engine の PR_BODY で新しい PR の本文を組む。 */
const engineNewBody = () => need("prBody") || bash(`${parts.prBody}\nprintf '%s' "$PR_BODY"`, { ...JOB, JOB_ID: "2790", SUMMARY: FIRST_SUMMARY });

/** Engine の組み込みで、続きのジョブ（i 回目）の summary を本文に足した NEW_BODY を返す。 */
const engineCompose = (oldBody, reason, i) =>
  need("compose") || bash(`${parts.compose}\nprintf '%s' "$NEW_BODY"`, { ...JOB, JOB_ID: String(2800 + i), CONTINUE_REASON: reason, CONTINUE_PR: PR_NUMBER,
    OLD_BODY: oldBody, SUMMARY: continuationSummary(reason, i) });

test("(0) Engine のチェックアウトがあり、本文を組む節と本文の直しを積む関数を取り出せる", { skip }, () => {
  assert.ok(entrypoint, `Engine のチェックアウトに orchestrator/worker/entrypoint.sh がない（ENGINE_DIR=${engineDir}）`);
  for (const [name, part] of Object.entries(parts)) {
    assert.ok(part, `entrypoint.sh から ${name} の節を取り出せない（ENGINE_DIR=${engineDir}。形が変わったら snippets を合わせる）`);
  }
  let out;
  try {
    out = execFileSync("python3", ["-I", harness, "--check", engineDir], { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
  } catch (e) {
    assert.fail(`engine_harness.py が Engine から関数を取り出せない（ENGINE_DIR=${engineDir}）: ${e.stderr || e.message}`);
  }
  assert.deepEqual(JSON.parse(out).functions, ["is_body_only", "enqueue_body_fix"]);
});

/** Engine の組み込みで C-0005 の 21 回の追記をした本文。 */
const engineC0005 = () => REASONS.reduce((body, reason, i) => engineCompose(body, reason, i), engineNewBody());

test("(1) Engine の組み込みで C-0005 の 21 回の追記をした本文は、関所で落ちる", { skip }, (t) => {
  const body = engineC0005();
  const result = gate(body, PR_URL);
  if (result.ok) {
    // Engine が組み込み方を直した（見出しを下げる・工場の記録の下に入れる）。それでよい。止めることは (3) が確かめる
    t.diagnostic("Engine の組み込みが、C-0005 の入力でも見出しを重ねない形になっている");
    return;
  }
  assert.ok(result.errors.some((e) => e.rule === "duplicate_h2" && e.heading === "概要"), "続きのジョブの「## 概要」が重なっている");
  assert.equal(result.body_fix.defects.length, result.errors.length);
});

test("(2) 関所の body_fix の入力は、Engine で本文だけの指摘と読まれ、body_fix のジョブが積まれる", { skip }, () => {
  const fromEngine = gate(engineC0005(), PR_URL);
  const { body_fix } = fromEngine.ok ? gate(legacyBody(newPrBody()), PR_URL) : fromEngine;
  const out = JSON.parse(execFileSync("python3", ["-I", harness, engineDir], { input: JSON.stringify(body_fix), encoding: "utf8" }));
  assert.ok(out.body_only.length > 0 && out.body_only.every(Boolean), "関所の指摘の箇所が Engine の is_body_only で本文と読まれない");
  assert.deepEqual(out.jobs, [{ kind: "body_fix" }]);
  assert.deepEqual(out.input, body_fix);
});

// 今の Engine の entrypoint.sh は、組み込んだ本文を関所に照らさずに gh pr edit で書く（tools/pr-body-check を呼んでいない）。
// (3) は必須のテストだが、Engine がこの今の形（関所を呼んでいない）のときだけ todo にする（外れても落ちないが結果に出る）。
// sendback-check の todoWhileLegacy（議決 D-0011）・continue-input-check の isLegacyInput と同じ扱い。Engine が関所を呼ぶようになったら、
// その版からは、外れた本文を書くと落ちる。建設者が Engine に配線して ENGINE_DIR の版を上げたら、isLegacyEntrypoint と todo を消してよい
const LEGACY_TODO = "Engine の entrypoint.sh が関所（tools/pr-body-check/gate.mjs）をまだ呼んでいない（建設者が配線したら必須になる）";

/** Engine の entrypoint.sh が関所を呼ばない今の形か */
export const isLegacyEntrypoint = (source) => !source.includes("tools/pr-body-check/gate.mjs");

/** 偽の gh（pr view は持っている本文を返し、pr edit --body は本文を書き換える）。ほかの呼び出しは落とす。 */
function fakeGh(body) {
  const dir = mkdtempSync(join(tmpdir(), "pr-body-engine-"));
  writeFileSync(join(dir, "body"), body);
  writeFileSync(join(dir, "gh"), `#!/usr/bin/env bash
case "$1 $2" in
  "pr view") cat "${dir}/body"; echo ;;
  "pr edit")
    while [ $# -gt 0 ]; do
      if [ "$1" = "--body" ]; then printf '%s' "$2" > "${dir}/body"; fi
      shift
    done ;;
  "pr ready") ;;
  *) echo "偽の gh が知らない呼び出し: $*" >&2; exit 99 ;;
esac
`);
  chmodSync(join(dir, "gh"), 0o755);
  return { dir, body: () => readFileSync(join(dir, "body"), "utf8") };
}

test("(3) Engine の本文の更新は、C-0005 の続きのジョブの本文が関所で外れたら、その本文を書かない",
  { skip, todo: !skip && isLegacyEntrypoint(entrypoint) ? LEGACY_TODO : false }, () => {
    need("update");
    let current = engineNewBody();
    REASONS.forEach((reason, i) => {
      const gh = fakeGh(current);
      const handoff = mkdtempSync(join(tmpdir(), "pr-body-handoff-"));
      try {
        bash(parts.update, { ...JOB, PATH: `${gh.dir}:${process.env.PATH}`, JOB_ID: String(2800 + i), CONTINUE_REASON: reason,
          CONTINUE_PR: PR_NUMBER, PR_URL, SUMMARY: continuationSummary(reason, i), HANDOFF_DIR: handoff }, repoRoot);
      } catch {
        // 関所で止めたあとに終了コードで知らせる形でもよい。見るのは書いた本文だけ
      }
      assert.deepEqual(checkBody(gh.body()), [], `${i + 1} 回目（${reason}）の続きのジョブで、関所に外れる本文を書いた`);
      current = gh.body();
    });
  });
