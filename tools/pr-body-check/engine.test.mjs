// Engine の本物の本文の組み込み（worker/entrypoint.sh の続きのジョブの節）と本文の直しの積み方（dispatcher.is_body_only・enqueue_body_fix）で、
// 続きのジョブの summary を組み込んだ本文が関所（gate.mjs）に照らされ、外れたら本文を書かずに body_fix に回ることを確かめる
// （知見 K-0001 の担保。議決 D-0013、Issue #956。関所の配線の差分 engine/pr-body-gate.patch は議決 D-0019、Issue #975）。入力は事例 C-0005（PR #659）の 21 回の追記（c0005.mjs）。
// Engine のチェックアウトは環境変数 ENGINE_DIR で渡す（なければリポジトリの隣の ../engine）。どちらもなければ飛ばす。
// ただし ENGINE_DIR を渡したときと CI（CI=true）では飛ばさずに落とす（sendback-check の engine.test.mjs と同じ。議決 D-0011）。
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { FIRST_SUMMARY, ISSUE, PASSING_REPORTS, PR_URL, REASONS, continuationSummary, legacyBody, newPrBody } from "./c0005.mjs";
import { checkBody } from "./check.mjs";
import { gate } from "./gate.mjs";
import { BROKEN_HEADS, PR_URL_678 } from "./k0016.mjs";
import { checkEvents } from "./recheck.mjs";

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
    // 新しい PR の本文を組んでから PR を作るまで（PR_BODY から、新しい PR の分岐を閉じる fi の前まで。知見 K-0016、Issue #1219）
    create: extract(source, (l) => l.startsWith("PR_BODY=$(cat <<EOF"), (l) => l.startsWith("CHANGED=$(git diff "), false)
      ?.replace(/\n\s*fi\s*$/, "\n"),
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

// (3) は必須のテスト（議決 D-0019、Issue #975 で todo を外した）。Engine の entrypoint.sh が関所（tools/pr-body-check/gate.mjs）を
// 呼ばない形なら、外れた本文を書くので落ちる。関所の配線は engine/pr-body-gate.patch で、(4)〜(6) がそれを当てた Engine の写しで確かめる。
// 建設者が差分を Engine に入れて ENGINE_REF を上げると (3) が通る
const PATCH = fileURLToPath(new URL("./engine/pr-body-gate.patch", import.meta.url));
const PATCH_FILES = ["orchestrator/worker/entrypoint.sh", "orchestrator/dispatcher/dispatcher.py", "definitions/events.yaml"];

/** Engine の entrypoint.sh が関所を呼ばない形か */
export const isLegacyEntrypoint = (source) => !source.includes("tools/pr-body-check/gate.mjs");

/**
 * 偽の gh（pr view は持っている本文を返し、pr edit --body は本文を、--title は題を書き換え、pr create は --body を本文にして createdUrl を出す）。
 * 呼び出しは calls に残し、ほかの呼び出しは落とす。
 */
function fakeGh(body, createdUrl = PR_URL) {
  const dir = mkdtempSync(join(tmpdir(), "pr-body-engine-"));
  writeFileSync(join(dir, "body"), body);
  writeFileSync(join(dir, "calls"), "");
  writeFileSync(join(dir, "gh"), `#!/usr/bin/env bash
echo "$1 $2" >> "${dir}/calls"
case "$1 $2" in
  "pr view") cat "${dir}/body"; echo ;;
  "pr edit")
    while [ $# -gt 0 ]; do
      if [ "$1" = "--body" ]; then printf '%s' "$2" > "${dir}/body"; fi
      if [ "$1" = "--title" ]; then printf '%s' "$2" > "${dir}/title"; fi
      shift
    done ;;
  "pr ready") ;;
  "pr create")
    while [ $# -gt 0 ]; do
      if [ "$1" = "--body" ]; then printf '%s' "$2" > "${dir}/body"; fi
      shift
    done
    echo "${createdUrl}" ;;
  *) echo "偽の gh が知らない呼び出し: $*" >&2; exit 99 ;;
esac
`);
  chmodSync(join(dir, "gh"), 0o755);
  return { dir, body: () => readFileSync(join(dir, "body"), "utf8"), calls: () => readFileSync(join(dir, "calls"), "utf8").trim().split("\n").filter(Boolean),
    title: () => (existsSync(join(dir, "title")) ? readFileSync(join(dir, "title"), "utf8") : null) };
}

/**
 * 本文の更新の節（update）を偽の gh で C-0005 の 21 回分動かし、回ごとに書かれた本文と引き継ぎ（HANDOFF_DIR）の body_gate.json を返す。
 * 本文は前の回に書かれたものから続ける（書かなかった回は前の本文のまま）。
 */
function runUpdates(update, newBody) {
  let current = newBody;
  return REASONS.map((reason, i) => {
    const gh = fakeGh(current);
    const handoff = mkdtempSync(join(tmpdir(), "pr-body-handoff-"));
    const jobId = String(2800 + i);
    let log = "";
    try {
      log = bash(update, { ...JOB, PATH: `${gh.dir}:${process.env.PATH}`, JOB_ID: jobId, CONTINUE_REASON: reason,
        CONTINUE_PR: PR_NUMBER, PR_URL, SUMMARY: continuationSummary(reason, i), HANDOFF_DIR: handoff }, repoRoot);
    } catch (e) {
      // 関所で止めたあとに終了コードで知らせる形でもよい。見るのは書いた本文だけ
      log = String(e.stdout || "");
    }
    current = gh.body();
    const gateFile = join(handoff, "jobs", jobId, "body_gate.json");
    return { reason, body: current, log, gate: existsSync(gateFile) ? JSON.parse(readFileSync(gateFile, "utf8")) : null };
  });
}

test("(3) Engine の本文の更新は、C-0005 の続きのジョブの本文が関所で外れたら、その本文を書かない", { skip }, () => {
  need("update");
  assert.ok(!isLegacyEntrypoint(entrypoint),
    `Engine の entrypoint.sh が関所（tools/pr-body-check/gate.mjs）を呼んでいない（ENGINE_DIR=${engineDir}。engine/pr-body-gate.patch を Engine に入れて ENGINE_REF を上げる）`);
  runUpdates(parts.update, engineNewBody()).forEach(({ reason, body }, i) =>
    assert.deepEqual(checkBody(body), [], `${i + 1} 回目（${reason}）の続きのジョブで、関所に外れる本文を書いた`));
});

/** ENGINE_DIR の Engine の写しに engine/pr-body-gate.patch を当てる（もう関所を呼んでいれば当てずに写すだけ）。写しの場所を返す。 */
function patchedEngine() {
  const dir = mkdtempSync(join(tmpdir(), "pr-body-engine-patched-"));
  for (const f of PATCH_FILES) {
    mkdirSync(dirname(join(dir, f)), { recursive: true });
    copyFileSync(join(engineDir, f), join(dir, f));
  }
  if (isLegacyEntrypoint(entrypoint)) {
    try {
      execFileSync("git", ["apply", PATCH], { cwd: dir, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
    } catch (e) {
      assert.fail(`engine/pr-body-gate.patch が Engine（${engineDir}）に当たらない。今の Engine に合わせて作り直す: ${e.stderr || e.message}`);
    }
  }
  // engine_harness.py は dispatcher の関数を取り出すときに judgment.py も読むので、写しに置く
  copyFileSync(join(engineDir, "orchestrator/dispatcher/judgment.py"), join(dir, "orchestrator/dispatcher/judgment.py"));
  return dir;
}

let patched;
const patchedDir = () => (patched ??= patchedEngine());
const patchedParts = () => snippets(readFileSync(join(patchedDir(), "orchestrator/worker/entrypoint.sh"), "utf8"));

test("(4) 関所を配線した Engine（engine/pr-body-gate.patch）は、C-0005 の 21 回の続きのジョブで外れた本文を一度も書かず、外れた回の body_gate を残す", { skip }, () => {
  const p = patchedParts();
  assert.ok(p.update && !isLegacyEntrypoint(p.update), "差分を当てた entrypoint.sh の本文の更新の節が関所を呼んでいない");
  const runs = runUpdates(p.update, engineNewBody());
  runs.forEach(({ reason, body, gate: g, log }, i) => {
    assert.deepEqual(checkBody(body), [], `${i + 1} 回目（${reason}）の続きのジョブで、関所に外れる本文を書いた`);
    assert.equal(g?.rc, 1, `${i + 1} 回目（${reason}）で関所が外れた記録（body_gate.json の rc 1）がない`);
    assert.equal(g.body_fix.pr_url, PR_URL);
    assert.ok(g.body_fix.defects.length > 0 && g.body_fix.defects.every((d) => d.rubric_id === "PR-002" && /^PR 本文 \d+ 行目$/.test(d.location)));
    assert.ok(g.pending_entry.includes(continuationSummary(reason, i).split("\n")[0]), "書かなかった続きのジョブの報告が body_gate に残っていない");
    assert.match(log, /pr body gate blocked rc=1/);
  });
});

test("(5) 関所を配線した Engine は、## 見出しを持たない続きのジョブの報告なら「## 工場の記録」の下の ### に足して書く（誤って止めない）", { skip }, () => {
  const { update } = patchedParts();
  let body = engineNewBody();
  ["既定ブランチを取り込み、衝突を解いた。", "CI 落ちを直した。"].forEach((summary, i) => {
    const gh = fakeGh(body);
    const handoff = mkdtempSync(join(tmpdir(), "pr-body-handoff-"));
    const jobId = String(2900 + i);
    bash(update, { ...JOB, PATH: `${gh.dir}:${process.env.PATH}`, JOB_ID: jobId, CONTINUE_REASON: i ? "ci_failed" : "conflict", CONTINUE_PR: PR_NUMBER, PR_URL,
      SUMMARY: summary, HANDOFF_DIR: handoff }, repoRoot);
    assert.ok(!existsSync(join(handoff, "jobs", jobId, "body_gate.json")), `${i + 1} 回目で関所に止められた`);
    body = gh.body();
    assert.ok(body.includes(summary), `${i + 1} 回目の続きのジョブの報告を書いていない`);
    assert.deepEqual(checkBody(body), []);
  });
  assert.equal(body.match(/^### 続きのジョブの記録$/gm)?.length, 1, "続きのジョブの記録の見出しが 1 つの ### になっていない");
});

const bodyGate = (gateResult) =>
  JSON.parse(execFileSync("python3", ["-I", harness, "--body-gate", patchedDir()], { input: JSON.stringify({ pr_url: PR_URL, body_gate: gateResult }), encoding: "utf8" }));

test("(6) 関所を配線した Engine の dispatcher は、外れた（rc 1）結果で body_fix を積み、書かなかった報告も渡す。2・3 は人に回す", { skip }, () => {
  const [first] = runUpdates(patchedParts().update, engineNewBody()).slice(0, 1);
  const out = bodyGate(first.gate);
  assert.deepEqual(out.jobs, [{ kind: "body_fix" }]);
  assert.deepEqual(out.stoppages, []);
  assert.ok(out.events.includes("pr.body_gate_blocked"));
  assert.equal(out.input.pr_url, PR_URL);
  assert.deepEqual(out.input.defects.slice(0, -1), first.gate.body_fix.defects);
  const pending = out.input.defects.at(-1);
  assert.equal(pending.location, "PR 本文 末尾");
  assert.ok(pending.evidence.includes(first.gate.pending_entry.trim()), "書かなかった続きのジョブの報告の全文が body_fix に渡っていない");
  assert.ok(first.gate.handoff?.endsWith("/jobs/2800/body_gate.json"), "body_gate に引き継ぎの全文の場所がない");
  assert.ok(pending.evidence.includes(first.gate.handoff), "body_fix の指摘に引き継ぎの全文の場所がない");
  for (const rc of [2, 3]) {
    const r = bodyGate({ rc, body_fix: null, pending_entry: first.gate.pending_entry, handoff: first.gate.handoff });
    assert.deepEqual(r.jobs, [], `rc ${rc} で body_fix を積んだ`);
    assert.deepEqual(r.stoppages.map((x) => x.category), ["human"], `rc ${rc} で人に回していない`);
  }
});

test("(7) 関所を配線した Engine は、4000 字を超える続きのジョブの報告も切らずに body_gate と body_fix の指摘に渡す", { skip }, () => {
  const long = ["## 概要", "長い報告の頭。", ...Array.from({ length: 300 }, (_, i) => `- 変えたこと ${i}: ${"あ".repeat(20)}`), "長い報告の終わり。"].join("\n");
  assert.ok(long.length > 4000);
  const gh = fakeGh(engineNewBody());
  const handoff = mkdtempSync(join(tmpdir(), "pr-body-handoff-"));
  bash(patchedParts().update, { ...JOB, PATH: `${gh.dir}:${process.env.PATH}`, JOB_ID: "2950", CONTINUE_REASON: "conflict", CONTINUE_PR: PR_NUMBER, PR_URL,
    SUMMARY: long, HANDOFF_DIR: handoff }, repoRoot);
  assert.equal(gh.body(), engineNewBody(), "外れた本文を書いた");
  const g = JSON.parse(readFileSync(join(handoff, "jobs", "2950", "body_gate.json"), "utf8"));
  assert.equal(g.rc, 1);
  assert.ok(g.pending_entry.includes(long), "引き継ぎの body_gate.json に報告の全文がない");
  assert.equal(g.handoff, join(handoff, "jobs", "2950", "body_gate.json"));
  const pending = bodyGate(g).input.defects.at(-1);
  assert.ok(pending.evidence.includes(long), "4000 字を超える報告が body_fix の指摘で切れた");
  assert.ok(pending.evidence.includes(g.handoff));
});

/** Engine の PR_BODY で、途中終了した（未完了の印と Refs を持つドラフトの）PR の本文を組む。 */
const engineIncompleteBody = () =>
  need("prBody") || bash(`${parts.prBody}\nprintf '%s' "$PR_BODY"`, { ...JOB, JOB_ID: "2790", SUMMARY: FIRST_SUMMARY, INCOMPLETE: "1", CLAUDE_RC: "1", LINK_KW: "Refs" });

/** 差分を当てた本文の更新の節を、未完了の続きが仕上がった回（CONTINUE_REASON=incomplete・INCOMPLETE が空）として 1 回動かす。 */
function runFinished(oldBody, summary) {
  const gh = fakeGh(oldBody);
  const handoff = mkdtempSync(join(tmpdir(), "pr-body-handoff-"));
  const log = bash(patchedParts().update, { ...JOB, PATH: `${gh.dir}:${process.env.PATH}`, JOB_ID: "2960", CONTINUE_REASON: "incomplete", INCOMPLETE: "",
    CONTINUE_PR: PR_NUMBER, PR_URL, SUMMARY: summary, HANDOFF_DIR: handoff }, repoRoot);
  const gateFile = join(handoff, "jobs", "2960", "body_gate.json");
  return { gh, log, gate: existsSync(gateFile) ? JSON.parse(readFileSync(gateFile, "utf8")) : null };
}

test("(8) 関所を配線した Engine は、未完了の続きが仕上がった回に関所で外れても、仕上げ（未完了の印・Closes・題・ドラフト）は済ませる", { skip }, () => {
  const old = engineIncompleteBody();
  assert.match(old, /^\*\*未完了\*\*:/m);
  assert.match(old, new RegExp(`^Refs #${ISSUE}$`, "m"));
  const { gh, gate: g } = runFinished(old, continuationSummary("incomplete", 0));
  assert.equal(g?.rc, 1, "続きのジョブの報告が関所で外れていない（この場合を確かめられない）");
  assert.deepEqual(g.finished, { applied: true, title: `${JOB.TITLE} (#${ISSUE})`, issue_number: ISSUE, remove_incomplete_mark: true, refs_to_closes: true, mark_ready: true });
  const body = gh.body();
  assert.doesNotMatch(body, /^\*\*未完了\*\*:/m, "未完了の印が残っている");
  assert.match(body, new RegExp(`^Closes #${ISSUE}$`, "m"), "Refs を Closes に直していない");
  assert.ok(!body.includes(g.pending_entry.trim()), "外れた報告を書いた");
  assert.deepEqual(checkBody(body), []);
  assert.equal(gh.title(), `${JOB.TITLE} (#${ISSUE})`);
  assert.ok(gh.calls().includes("pr ready"), "ドラフトを外していない");
  const out = bodyGate(g);
  assert.deepEqual(out.jobs, [{ kind: "body_fix" }], "書かなかった報告を body_fix に回していない");
  assert.deepEqual(out.stoppages, [], "仕上げは済んでいるのに人に回した");
});

test("(9) 仕上がった回で、報告を足さない本文も関所で外れたら、仕上げも書かずに finished.applied を false で残し、dispatcher が人に回す", { skip }, () => {
  const old = `${engineIncompleteBody()}\n\n## 概要\n\n前から重なっていた見出し\n`;
  assert.ok(checkBody(old).length > 0);
  const { gh, gate: g } = runFinished(old, "仕上げた。");
  assert.equal(g?.rc, 1);
  assert.equal(g.finished.applied, false);
  assert.equal(gh.body(), old, "外れた本文を書いた");
  assert.ok(!gh.calls().includes("pr ready") && !gh.calls().includes("pr edit"), "関所で外れたのに PR を書き換えた");
  const out = bodyGate(g);
  assert.deepEqual(out.jobs, [{ kind: "body_fix" }]);
  assert.deepEqual(out.stoppages, [{ key: `body_gate_finish:${PR_URL}`, category: "human" }], "書けなかった仕上げを人に回していない");
  const r = bodyGate({ ...g, rc: 3, body_fix: null });
  assert.deepEqual(r.stoppages.map((x) => x.key), [`body_gate_finish:${PR_URL}`, `body_gate:${PR_URL}`]);
});

/** 関所（gate.mjs）を呼んだ回と終了コードを残す node の包み。ほかの呼び出しはそのまま本物の node に渡す。 */
function gateRecorder() {
  const dir = mkdtempSync(join(tmpdir(), "pr-body-gate-recorder-"));
  writeFileSync(join(dir, "gate_calls"), "");
  writeFileSync(join(dir, "node"), `#!/usr/bin/env bash
"${process.execPath}" "$@"
rc=$?
case " $* " in *" tools/pr-body-check/gate.mjs "*) echo "$rc" >> "${dir}/gate_calls" ;; esac
exit $rc
`);
  chmodSync(join(dir, "node"), 0o755);
  return { dir, rcs: () => readFileSync(join(dir, "gate_calls"), "utf8").trim().split("\n").filter(Boolean).map(Number) };
}

/**
 * C-0005 と同じ 21 回の続きのジョブの報告（report）を、本文の更新の節（update）で前の回の本文に続けて動かし、
 * 毎回関所を 1 回呼んで 0 が返り、body_gate.json を残さずに本文を書き、書いた本文が関所に通ることを確かめる。
 */
function assertWritesEveryTime(update, report) {
  let body = engineNewBody();
  REASONS.forEach((reason, i) => {
    const summary = report(reason, i);
    const gh = fakeGh(body);
    const rec = gateRecorder();
    const handoff = mkdtempSync(join(tmpdir(), "pr-body-handoff-"));
    const jobId = String(3000 + i);
    const log = bash(update, { ...JOB, PATH: `${rec.dir}:${gh.dir}:${process.env.PATH}`, JOB_ID: jobId, CONTINUE_REASON: reason, CONTINUE_PR: PR_NUMBER, PR_URL,
      SUMMARY: summary, HANDOFF_DIR: handoff }, repoRoot);
    const at = `${i + 1} 回目（${reason}）`;
    assert.deepEqual(rec.rcs(), [0], `${at}で関所を 1 回呼んで 0 を返していない`);
    assert.ok(!existsSync(join(handoff, "jobs", jobId, "body_gate.json")), `${at}で関所に止められた`);
    assert.doesNotMatch(log, /pr body gate blocked/, `${at}で関所に止められた`);
    assert.match(log, /\[worker\] continued pr=/);
    assert.ok(gh.calls().includes("pr edit"), `${at}で本文を書いていない`);
    const written = gh.body();
    assert.ok(written.startsWith(body.trimEnd()), `${at}で前の本文を残していない`);
    assert.ok(written.includes(summary), `${at}の続きのジョブの報告を書いていない`);
    assert.deepEqual(checkBody(written), [], `${at}で書いた本文が関所に外れる`);
    body = written;
  });
  assert.equal(body.match(/^### 続きのジョブの記録$/gm)?.length, 1, "続きのジョブの記録の見出しが 1 つの ### になっていない");
  assert.equal(body.match(/^## 概要$/gm)?.length, 1, "「## 概要」が重なった");
}

// (10) は通る側の本物（議決 D-0032 の test の案、Issue #1038。議決 D-0045 の test の案、Issue #1110 で写しから本物に替えた）。
// C-0005 と同じ 21 回の続きのジョブでも、報告が「## 工場の記録」の下に正しく入る形（報告の見出しが ### 以下、または ## 見出しがない）なら、
// 関所は毎回 0 を返し、Engine はその本文を書く。差分を当てた写しではなく、ENGINE_DIR（CI では ENGINE_REF の版）の entrypoint.sh そのものの
// 本文の更新の節を動かすので、Engine に入った配線が正しい更新まで止める誤検知を落とす。必須のテストで、関所を呼ばない Engine では (3) と同じく落ちる
for (const [label, report] of PASSING_REPORTS) {
  test(`(10) ENGINE_REF の Engine は、C-0005 の 21 回の続きのジョブで${label}なら、毎回関所が 0 を返して本文を書く（誤って止めない）`, { skip }, () => {
    need("update");
    assert.ok(!isLegacyEntrypoint(parts.update),
      `Engine の本文の更新の節が関所（tools/pr-body-check/gate.mjs）を呼んでいない（ENGINE_DIR=${engineDir}。engine/pr-body-gate.patch を Engine に入れて ENGINE_REF を上げる）`);
    assertWritesEveryTime(parts.update, report);
  });
}

// (11) は差分の通る側の写し。Engine がまだ関所を呼ばない間だけ、engine/pr-body-gate.patch を当てた写しで (10) と同じことを確かめ、
// 差分そのものが正しい更新まで止めないことを見る。Engine が関所を呼ぶようになったら (10) が本物で確かめるので飛ばす
const engineWired = Boolean(parts.update) && !isLegacyEntrypoint(parts.update);
for (const [label, report] of PASSING_REPORTS) {
  test(`(11) 関所の配線の差分を当てた Engine の写しは、C-0005 の 21 回の続きのジョブで${label}なら、毎回関所が 0 を返して本文を書く（誤って止めない）`,
    { skip: skip || (engineWired ? "Engine がもう関所を呼ぶので、(10) が ENGINE_REF の Engine そのもので確かめる" : false) }, () => {
      assertWritesEveryTime(patchedParts().update, report);
    });
}

// (12) 本文の直しのあとの再検査を待つ supervisor（知見 K-0012 の担保。議決 D-0050 の ci_check の案、Issue #1122）。差分は engine/pr-body-recheck.patch。
// ENGINE_DIR の Engine の写しに当て（もう body_recheck_pending があれば当てずに写す）、supervisor.py の body_fix_pending・body_recheck_pending を
// engine_harness.py --body-recheck で merge_step と同じ順に動かす。入力は事例 C-0006（PR #821）の記録の順（本文の直し job 4299 を 14:24:41 に積み、
// 検査役の承認 14:25:02、pr.body_amended 14:25:04、マージ 14:25:06）。GitHub の pr-body-check のワークフローの実行は、もとのイベントの時刻（created_at）・
// action（run-name の「pr-body-check (action)」）・結果だけを真似る。直した本文で始まった実行として数えるのは、直しが走り始めた（なければ積んだ）より後に
// edited で始まった実行か、pr.body_amended より後に始まった実行だけ（議決 D-0100（起票時の番号は D-0071）の反対票の反例を塞ぐ。Issue #1268）。
// Engine に差分が入ったら（supervisor.py に body_recheck_pending がある）、写さずに ENGINE_DIR（CI では ENGINE_REF の版）のチェックアウトそのものに対して走らせる。
// PR_BODY_RECHECK_ENGINE_ONLY=1 では差分を当てた写しに頼らず、Engine に入っていなければ落とす（check.py が wired: true を照らすときに渡す。議決 D-0105、Issue #1281）
const RECHECK_PATCH = fileURLToPath(new URL("./engine/pr-body-recheck.patch", import.meta.url));
const RECHECK_FILES = ["orchestrator/dispatcher/supervisor.py", "definitions/events.yaml"];
const URL_821 = "https://github.com/guccyon/rally/pull/821";

const RECHECK_ENGINE_ONLY = process.env.PR_BODY_RECHECK_ENGINE_ONLY === "1";

/**
 * (12) で動かす Engine の場所を返す。Engine に差分が入っていれば ENGINE_DIR そのもの、入っていなければ ENGINE_DIR の Engine の写しに
 * engine/pr-body-recheck.patch を当てたもの（RECHECK_ENGINE_ONLY では落とす）。
 */
function recheckEngine() {
  const supervisor = join(engineDir, RECHECK_FILES[0]);
  if (existsSync(supervisor) && readFileSync(supervisor, "utf8").includes("def body_recheck_pending(")) return engineDir;
  assert.ok(!RECHECK_ENGINE_ONLY, `Engine（${engineDir}）の ${RECHECK_FILES[0]} に body_recheck_pending がない。`
    + "PR_BODY_RECHECK_ENGINE_ONLY=1 では engine/pr-body-recheck.patch を当てた写しで確かめない（差分を Engine に入れて ENGINE_REF を上げる）");
  const dir = mkdtempSync(join(tmpdir(), "pr-body-recheck-engine-"));
  for (const f of RECHECK_FILES) {
    mkdirSync(dirname(join(dir, f)), { recursive: true });
    copyFileSync(join(engineDir, f), join(dir, f));
  }
  if (!readFileSync(join(dir, RECHECK_FILES[0]), "utf8").includes("def body_recheck_pending(")) {
    try {
      execFileSync("git", ["apply", RECHECK_PATCH], { cwd: dir, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
    } catch (e) {
      assert.fail(`engine/pr-body-recheck.patch が Engine（${engineDir}）に当たらない。今の Engine に合わせて作り直す: ${e.stderr || e.message}`);
    }
  }
  return dir;
}

let recheckDir;
/** C-0006 の本文の直しの終わり（job 4299 の pr.body_amended、14:25:04）。 */
const AMENDED = { name: "pr.body_amended", job_id: 4299, payload: { pr_url: URL_821, defects: 1 }, occurred_at: "2026-11-06T14:25:04+09:00" };
/** C-0006 の本文の直し（job 4299）を、state で終わらせた入力に over を重ねて supervisor の判断を返す。 */
function supervisorRecheck(over = {}, state = "succeeded") {
  recheckDir ??= recheckEngine();
  const inp = { pr_url: URL_821, head_sha: "s1", events: state === "succeeded" ? [AMENDED] : [], workflow_runs: [],
    jobs: [{ id: 4299, kind: "body_fix", state, enqueued_at: "2026-11-06T14:24:41+09:00", started_at: null }], ...over };
  try {
    return JSON.parse(execFileSync("python3", ["-I", harness, "--body-recheck", recheckDir], { input: JSON.stringify(inp), encoding: "utf8",
      stdio: ["pipe", "pipe", "pipe"] }));
  } catch (e) {
    assert.fail(`engine_harness.py --body-recheck が動かない: ${e.stderr || e.message}`);
  }
}

/** GitHub の pr-body-check のワークフローの実行の 1 件（workflow_runs）。created_at はもとのイベントの時刻で、再実行では run_started_at だけが進む。 */
const bodyCheckRun = (created_at, action, status = "completed", conclusion = "success", run_started_at = created_at) =>
  ({ name: "pr-body-check", display_title: `pr-body-check (${action})`, event: "pull_request", created_at, run_started_at, status,
    conclusion: status === "completed" ? conclusion : null, html_url: `https://github.com/guccyon/rally/actions/runs/${created_at}` });
/**
 * 直す前の本文で走った pr-body-check（PR を開いたとき）と、直しのジョブの中の本文の書き換え（pull_request edited）で走った pr-body-check。
 * 書き換えはジョブの中なので、edited の実行は直しの終わりの記録（pr.body_amended 14:25:04 = 05:25:04Z）より前に始まる。
 */
const BEFORE_FIX = bodyCheckRun("2026-11-06T05:10:00Z", "opened");
const AFTER_FIX = (status, conclusion) => bodyCheckRun("2026-11-06T05:25:03Z", "edited", status, conclusion);

/** (12) の各テストが最後まで来たときに TAP の診断に残す印の頭。ci.test.mjs がこの印の数を見張りの一覧の数と照らす。 */
const RECHECK_ASSERTED = "(12) の確かめの数";
/**
 * (12) のテストの中身 body を、確かめ（body に渡す a の assert）の数を t.plan で expected に固定して走らせ、最後まで来たら走った数を
 * 「# (12) の確かめの数 N: テストの名前」の診断で TAP に残す（知見 K-0012 の担保。議決 D-0106 の test の案、Issue #1283）。
 * 条件付きの早い return で中身を飛ばすと、走った数が plan に足りず落ち、診断の数も ci.test.mjs の ENGINE_RECHECK_ASSERTS とずれる。
 * t.assert は strict でない（equal が ==）ので、確かめは strict の assert で行い、1 回ごとに t.assert.ok(true) で plan に数える。
 */
function recheckPlanned(t, expected, body) {
  let ran = 0;
  const a = new Proxy(assert, {
    get: (target, key) => (typeof target[key] !== "function" ? target[key] : (...args) => {
      const r = target[key](...args);
      ran += 1;
      t.assert.ok(true);
      return r;
    }),
  });
  t.plan(expected);
  body(a);
  t.diagnostic(`${RECHECK_ASSERTED} ${ran}: ${t.name}`);
}

test("(12) 再検査を待つ supervisor（engine/pr-body-recheck.patch）は、merge_step でマージより前に、本文の直しの終わりと直した本文の pr-body-check を待つ", { skip }, (t) => recheckPlanned(t, 3, (a) => {
  const r = supervisorRecheck({}, "running");
  a.equal(r.order.merge_step, true, "supervisor.py に merge_step がない");
  a.equal(r.order.before_merge, true, `merge_step で body_fix_pending と body_recheck_pending を待つ（if …: continue）のがマージより前にない: ${JSON.stringify(r.order)}`);
  a.match(readFileSync(join(recheckDir, "definitions/events.yaml"), "utf8"), /^ {2}supervisor\.body_rechecked:$/m, "events.yaml に supervisor.body_rechecked がない");
}));

test("(12) C-0006 の 14:25:02（本文の直し job 4299 が走っている）では、直しの終わりを待つ", { skip }, (t) => recheckPlanned(t, 2, (a) => {
  const r = supervisorRecheck({ workflow_runs: [BEFORE_FIX] }, "running");
  a.equal(r.fix_pending, true);
  a.deepEqual(r.gh, [], "直している間に GitHub を引いた");
}));

test("(12) 直しのジョブが終わっても、直しのあとに pr-body-check が走って通るまではマージしない（C-0006 の pr.body_amended の前後の窓）", { skip }, (t) => recheckPlanned(t, 18, (a) => {
  const cases = [
    [{ workflow_runs: [BEFORE_FIX] }, /待ち（直した本文で始まっていない/],
    [{ workflow_runs: [] }, /待ち（直した本文で始まっていない/],
    [{ workflow_runs: [BEFORE_FIX, AFTER_FIX("in_progress")] }, /待ち（in_progress）/],
    [{ workflow_runs: [AFTER_FIX("completed", "failure"), BEFORE_FIX] }, /通らない（failure）/],
    [{ workflow_runs: [AFTER_FIX("completed", "cancelled")] }, /通らない（cancelled）/],
    [{ workflow_error: 500, workflow_runs: [AFTER_FIX()] }, /読めない（500）/],
  ];
  for (const [over, reason] of cases) {
    const r = supervisorRecheck(over);
    a.equal(r.fix_pending, false);
    a.match(r.reason ?? "", reason, JSON.stringify(over));
    a.deepEqual(r.events, [], `通っていないのに再検査を記録した: ${JSON.stringify(over)}`);
  }
}));

test("(12) 直した本文の pr-body-check が通ったら待たず、supervisor.body_rechecked を残し、その列は recheck.mjs にも通る", { skip }, (t) => recheckPlanned(t, 8, (a) => {
  const r = supervisorRecheck({ workflow_runs: [BEFORE_FIX, AFTER_FIX()] });
  a.equal(r.reason, null);
  a.equal(r.events.length, 1);
  const [ev] = r.events;
  a.equal(ev.name, "supervisor.body_rechecked");
  a.deepEqual({ ...ev.payload, run: undefined },
    { pr_url: URL_821, head_sha: "s1", gate: "pr-body-check", job_id: 4299, run: undefined, action: "edited", created_at: AFTER_FIX().created_at,
      body_amended_at: "2026-11-06T14:25:04+09:00" });
  // 記録したあとは GitHub を引かずに待たない（同じ直しで 1 回）
  const again = supervisorRecheck({ events: [AMENDED, ev], workflow_runs: [] });
  a.equal(again.reason, null);
  a.deepEqual(again.gh, []);
  // C-0006 の記録の順に、supervisor が残す再検査を挟んだ列は recheck.mjs に通る（挟まない列が落ちることは recheck.test.mjs が確かめる）
  const pr = { pr_url: URL_821 };
  const events = [
    { name: "job.enqueued", job_id: 4299, payload: { ...pr, by: "judge", kind: "body_fix" } },
    { name: "pr.body_amended", job_id: 4299, payload: { ...pr, defects: 1 }, occurred_at: AMENDED.occurred_at },
    { name: ev.name, payload: ev.payload },
    { name: "supervisor.inspector_approved", payload: { ...pr, head_sha: "s1" } },
    { name: "supervisor.merged", payload: { ...pr, head_sha: "s1" } },
  ];
  a.deepEqual(checkEvents(events).errors, []);
  a.deepEqual(checkEvents(events.filter((e) => e.name !== ev.name)).errors.map((e) => e.rule), ["merged_without_body_recheck"]);
}));

test("(12) 直しの途中で push が来て直す前の本文で通った実行は、直しのあとに始まっても再検査に数えない（D-0100 の反対票の反例）", { skip }, (t) => recheckPlanned(t, 11, (a) => {
  // 直しのジョブ（14:24:41 に積んだ）の最中、本文の書き換えより前に push が来て synchronize で始まった実行は、push のときの本文（直す前）を照らす。
  // 直しの開始より後に始まったので、境目が「直しのジョブの開始」だけだと、書き換えの edited の実行が始まる前にこれを再検査に数えてマージしていた
  const push = bodyCheckRun("2026-11-06T05:24:55Z", "synchronize");
  // 前の実行（PR を開いたとき）の再実行も、もとのイベントの本文（直す前）を照らす。始まり（run_started_at）が直しの開始より後でも、もとのイベント（created_at）は前
  const rerun = bodyCheckRun(BEFORE_FIX.created_at, "opened", "completed", "success", "2026-11-06T05:24:58Z");
  // 直しの開始より前の本文の編集（edited）も数えない
  const editedBefore = bodyCheckRun("2026-11-06T05:24:30Z", "edited");
  for (const [label, runs, events] of [
    ["push の実行だけ（pr.body_amended のあと）", [BEFORE_FIX, push], [AMENDED]],
    ["push の実行だけ（pr.body_amended をまだ記録していない）", [BEFORE_FIX, push], []],
    ["前の実行の再実行", [BEFORE_FIX, rerun], [AMENDED]],
    ["直しの前の edited", [editedBefore], [AMENDED]],
  ]) {
    const r = supervisorRecheck({ head_sha: "s2", events, workflow_runs: runs });
    a.match(r.reason ?? "", /待ち（直した本文で始まっていない/, `${label} を直した本文の再検査に数えた`);
    a.deepEqual(r.events, [], `${label} で再検査を記録した`);
  }
  // 書き換えの edited の実行が通れば数える（push のあとの head で走る）
  const fixed = supervisorRecheck({ head_sha: "s2", workflow_runs: [BEFORE_FIX, push, AFTER_FIX()] });
  a.equal(fixed.reason, null);
  a.deepEqual(fixed.events.map((e) => [e.payload.action, e.payload.created_at]), [["edited", AFTER_FIX().created_at]]);
  // 古い本文で通った push の実行より、書き換えの edited の実行が落ちていれば通さない
  a.match(supervisorRecheck({ head_sha: "s2", workflow_runs: [push, AFTER_FIX("completed", "failure")] }).reason ?? "", /通らない（failure）/);
}));

test("(12) 直しのジョブの中の書き換えの edited で始まった実行は、直しの終わり（pr.body_amended）より前に始まっても数え、待ち続けない", { skip }, (t) => recheckPlanned(t, 9, (a) => {
  // 境目を直しの終わり（ジョブの finished_at や pr.body_amended）だけにすると、書き換えの edited の実行（05:25:03Z）は境目（05:25:04Z）より前なので数えず、
  // 本文はもう変わらないので次の実行も来ず、待ち続ける。edited で始まった実行は直しの開始より後なら数えるので、そうならない
  a.ok(Date.parse(AFTER_FIX().created_at) < Date.parse(AMENDED.occurred_at), "edited の実行が直しの終わりより前に始まる入力になっていない");
  for (const [label, events] of [["pr.body_amended のあと", [AMENDED]], ["pr.body_amended をまだ記録していない（ジョブは succeeded）", []]]) {
    const r = supervisorRecheck({ events, workflow_runs: [BEFORE_FIX, AFTER_FIX()] });
    a.equal(r.reason, null, `${label}: edited で始まった実行で通らない（待ち続ける）: ${r.reason}`);
    a.deepEqual(r.events.map((e) => e.name), ["supervisor.body_rechecked"], label);
  }
  // pr.body_amended より後に始まった実行は、action を問わず数える（直したあとの push・run-name に action のない写しの実行）
  for (const run of [bodyCheckRun("2026-11-06T05:25:10Z", "synchronize"), { ...bodyCheckRun("2026-11-06T05:25:10Z", "x"), display_title: "Fix body" }]) {
    const r = supervisorRecheck({ workflow_runs: [BEFORE_FIX, run] });
    a.equal(r.reason, null, `${run.display_title}: ${r.reason}`);
    a.equal(r.events[0].payload.created_at, run.created_at);
  }
}));

test("(12) 本文の直しが失敗で終わったら、直していない本文のままマージせず人に回す（停止 body_recheck:）", { skip }, (t) => recheckPlanned(t, 6, (a) => {
  for (const state of ["failed", "cancelled"]) {
    const r = supervisorRecheck({ workflow_runs: [AFTER_FIX()] }, state);
    a.match(r.reason ?? "", new RegExp(`${state} で終わった`));
    a.deepEqual(r.stoppages, [{ key: `body_recheck:${URL_821}`, category: "human" }]);
    a.deepEqual(r.events, []);
  }
}));

test("(12) 本文の直しがない PR と、pr-body-check のワークフローを持たない製品では待たない（誤って止めない）", { skip }, (t) => recheckPlanned(t, 4, (a) => {
  const none = supervisorRecheck({ jobs: [] });
  a.deepEqual([none.fix_pending, none.reason, none.gh], [false, null, []]);
  const other = supervisorRecheck({ workflow: false });
  a.equal(other.reason, null);
  a.deepEqual(other.events, []);
  a.equal(other.gh.length, 1, "ワークフローがないのに pr-body-check の結果を引いた");
}));

// (13)〜(15) 新しい PR の最初の本文の関所（知見 K-0016 の担保。議決 D-0078 の ci_check の案、Issue #1219）。
// summary.md（SUMMARY）から組んだ最初の本文も、gh pr create の前に同じ関所（gate.mjs）に照らす。入力は事例 C-0008（PR #678）と C-0006（PR #821）の
// 報告の形（k0016.mjs）。差分は engine/pr-body-gate.patch に足した（新しい PR の分岐と、dispatcher の run_job が続きのジョブでなくても body_gate を拾うこと）

/** 新しい PR の作成の節（create）を偽の gh で 1 回動かし、作った本文・関所を呼んだ回の終了コード・引き継ぎの body_gate.json を返す。 */
function runCreate(create, summary, over = {}) {
  const gh = fakeGh("", PR_URL_678);
  const rec = gateRecorder();
  const handoff = mkdtempSync(join(tmpdir(), "pr-body-handoff-"));
  const jobId = "3324";
  const log = bash(create, { ...JOB, PATH: `${rec.dir}:${gh.dir}:${process.env.PATH}`, JOB_ID: jobId, BRANCH: "factory/issue-670-job-3324", DEFAULT_BRANCH: "main",
    PR_TITLE: `${JOB.TITLE} (#${ISSUE})`, SUMMARY: summary, HANDOFF_DIR: handoff, ...over }, repoRoot);
  const gateFile = join(handoff, "jobs", jobId, "body_gate.json");
  return { gh, log, rcs: rec.rcs(), gate: existsSync(gateFile) ? JSON.parse(readFileSync(gateFile, "utf8")) : null };
}

test("(13) ENGINE_REF の Engine は、新しい PR を作る前に、報告から組んだ最初の本文を関所に照らす", { skip }, () => {
  need("create");
  assert.ok(!isLegacyEntrypoint(parts.create),
    `Engine の新しい PR の作成の節が関所（tools/pr-body-check/gate.mjs）を呼んでいない（ENGINE_DIR=${engineDir}。engine/pr-body-gate.patch を Engine に入れて ENGINE_REF を上げる）`);
  const [, , summary] = BROKEN_HEADS[0];
  const { rcs, gate: g } = runCreate(parts.create, summary);
  assert.deepEqual(rcs, [1], "C-0008 の報告から組んだ本文で関所が 1 を返していない");
  assert.equal(g?.rc, 1, "関所で外れた記録（body_gate.json の rc 1）がない");
});

for (const [label, , summary] of BROKEN_HEADS) {
  test(`(14) 関所を配線した Engine は、${label}から組んだ最初の本文を作る前に関所に照らし、作った PR の body_fix を積ませる`, { skip }, () => {
    const p = patchedParts();
    assert.ok(p.create && !isLegacyEntrypoint(p.create), "差分を当てた entrypoint.sh の新しい PR の作成の節が関所を呼んでいない");
    const { gh, log, rcs, gate: g } = runCreate(p.create, summary);
    assert.deepEqual(rcs, [1], "関所を 1 回呼んで 1 を返していない");
    assert.deepEqual(gh.calls(), ["pr create"], "PR を作っていない（作業を失う）か、ほかに書き換えた");
    assert.ok(gh.body().includes(summary), "組んだ本文のまま PR を作っていない");
    assert.equal(g?.rc, 1);
    assert.equal(g.new_pr, true);
    assert.equal(g.pending_entry, "");
    assert.equal(g.body_fix.pr_url, PR_URL_678, "body_fix の入力の pr_url を作った PR に直していない");
    const head = g.body_fix.defects.find((d) => d.evidence.startsWith("pr-body-check head_not_first_heading: "));
    assert.ok(head, "本文の先頭の外れが body_fix の指摘にない");
    assert.match(head.location, /^PR 本文 \d+ 行目$/);
    assert.match(log, new RegExp(`pr body gate blocked rc=1 pr=${PR_URL_678}`));
    const out = bodyGate(g);
    assert.deepEqual(out.jobs, [{ kind: "body_fix" }]);
    assert.deepEqual(out.stoppages, []);
    // 書かなかった報告はないので、関所の指摘のほかに足さない（remediation_hint のない rule は Python を通ると null になるので、箇所と根拠で比べる）
    const brief = (ds) => ds.map((d) => [d.location, d.evidence]);
    assert.deepEqual(brief(out.input.defects), brief(g.body_fix.defects), "書かなかった報告がないのに指摘を足した");
  });
}

test("(14) 関所を配線した Engine の dispatcher は、続きのジョブでない（新しい PR の）結果の body_gate も拾う", { skip }, () => {
  const src = readFileSync(join(patchedDir(), "orchestrator/dispatcher/dispatcher.py"), "utf8");
  assert.match(src, /^ {12}if result and result\.get\("pr_url"\) and isinstance\(result\.get\("body_gate"\), dict\):$/m,
    "run_job が続きのジョブ（cont）のときだけ body_gate を act_on_body_gate に回している");
});

test("(15) 関所を配線した Engine は、「## 概要」から始まる最初の本文（未完了の印と Refs の形も）なら関所が 0 を返し、body_gate を残さずに作る（誤って止めない）", { skip }, () => {
  for (const over of [{}, { INCOMPLETE: "1", CLAUDE_RC: "1", LINK_KW: "Refs" }]) {
    const { gh, log, rcs, gate: g } = runCreate(patchedParts().create, FIRST_SUMMARY, over);
    assert.deepEqual(rcs, [0], JSON.stringify(over));
    assert.equal(g, null, "通る本文で body_gate.json を残した");
    assert.doesNotMatch(log, /pr body gate blocked/);
    assert.deepEqual(checkBody(gh.body()), []);
    assert.ok(gh.body().includes(FIRST_SUMMARY));
  }
});
