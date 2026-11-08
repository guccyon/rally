// patch.mjs のテスト（知見 K-0001 の担保。議決 D-0068（Issue #1179 の起票時の番号は D-0066）、Issue #1179）。
// 作った Engine の写し（git の版）で、当たる・もう入っている・一部だけ入った・入れたあと変わった・どれでもないの判定と、CLI の終了コードを確かめる（Engine は要らない）。
// 本物の Engine では、engine/pr-body-gate.patch が ENGINE_REF の版に当たるかもう入っていることを確かめる。Engine のチェックアウトは環境変数 ENGINE_DIR で渡す
// （なければリポジトリの隣の ../engine）。チェックアウトがないか ENGINE_REF の版を含まないときは飛ばす。ただし ENGINE_DIR を渡したときと CI（CI=true）では
// 飛ばさずに落とす（engine.test.mjs と同じ）。
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { APPLIES, checkPatch, engineRef, hasCommit, MARKERS, MERGED, PARTIAL, patchFiles, patchState, PATCH, STALE, workflowPath } from "./patch.mjs";

const CLI = fileURLToPath(new URL("./patch.mjs", import.meta.url));
const gitEnv = { ...process.env, GIT_AUTHOR_NAME: "t", GIT_AUTHOR_EMAIL: "t@example.invalid", GIT_COMMITTER_NAME: "t",
  GIT_COMMITTER_EMAIL: "t@example.invalid" };
const git = (cwd, ...args) => execFileSync("git", args, { cwd, env: gitEnv, encoding: "utf8" }).trim();

/** 偽の Engine。entrypoint.sh に関所の呼び出し、dispatcher.py に関数を足す差分と、それを当てた・一部だけ入れた・変えた版を作る。 */
function fakeEngine() {
  const root = mkdtempSync(join(tmpdir(), "pr-body-patch-test-"));
  const d = join(root, "engine");
  mkdirSync(join(d, "worker"), { recursive: true });
  mkdirSync(join(d, "dispatcher"), { recursive: true });
  git(d, "init", "-q");
  const sh = join(d, "worker", "entrypoint.sh");
  const py = join(d, "dispatcher", "dispatcher.py");
  const write = (s, p) => { writeFileSync(sh, s); writeFileSync(py, p); };
  const commit = (msg) => { git(d, "add", "."); git(d, "commit", "-qm", msg); return git(d, "rev-parse", "HEAD"); };
  write("a\nb\nc\n", "x\ny\nz\n");
  const base = commit("base");
  write("a\nb\nnode gate.mjs\nc\n", "x\ny\ndef act_on_gate():\nz\n");
  const patch = join(root, "p.patch");
  writeFileSync(patch, git(d, "diff") + "\n");
  const merged = commit("merged");
  write("a\nB\nnode gate.mjs --x\nc\n", "x\ny\ndef act_on_gate(v):\nz\n");
  const changed = commit("merged-then-changed");
  write("a\nb\nnode gate.mjs\nc\n", "x\ny\nz\n");
  const partial = commit("gate-only");
  write("p\nq\nr\n", "x\nY\nz\n");
  const moved = commit("moved");
  const markers = [["worker/entrypoint.sh", "gate.mjs"], ["dispatcher/dispatcher.py", "def act_on_gate("]];
  return { root, dir: d, patch, markers, base, merged, changed, partial, moved };
}

const fake = fakeEngine();
const state = (ref) => patchState(fake.dir, ref, { patch: fake.patch, markers: fake.markers });

test("当たる版は applies、もう入っている版は merged", () => {
  assert.deepEqual(state(fake.base), { state: APPLIES, detail: "" });
  assert.deepEqual(state(fake.merged), { state: MERGED, detail: "" });
});

test("入れたあと変わった版は、足したものがあっても patch_partially_applied", () => {
  const r = state(fake.changed);
  assert.equal(r.state, PARTIAL);
  assert.match(r.detail, /gate\.mjs/);
  assert.match(r.detail, /act_on_gate/);
});

test("一部だけ入った版は patch_partially_applied（入っているものだけを挙げる）", () => {
  const r = state(fake.partial);
  assert.equal(r.state, PARTIAL);
  assert.match(r.detail, /gate\.mjs/);
  assert.doesNotMatch(r.detail, /act_on_gate/);
});

test("当たらず、足したものもない版は patch_stale（git apply の出力を残す）", () => {
  const r = state(fake.moved);
  assert.equal(r.state, STALE);
  assert.match(r.detail, /worker\/entrypoint\.sh/);
});

test("チェックアウトの今の中身ではなく、名指しした版の中身で照らす", () => {
  git(fake.dir, "checkout", "-q", fake.moved);
  assert.equal(state(fake.base).state, APPLIES);
  git(fake.dir, "checkout", "-q", fake.base);
  assert.equal(state(fake.moved).state, STALE);
  git(fake.dir, "checkout", "-q", fake.moved);
});

test("ENGINE_REF を読む（40 桁のコミット SHA でなければ照らせない）", () => {
  assert.match(engineRef(readFileSync(workflowPath(), "utf8")), /^[0-9a-f]{40}$/);
  assert.throws(() => engineRef("env:\n  ENGINE_REF: main\n"), /ENGINE_REF/);
  assert.equal(engineRef(`env:\n  ENGINE_REF: "${"a".repeat(40)}"\n`), "a".repeat(40));
});

test("差分が触るファイルを読み、MARKERS のファイルはどれも差分が触る", () => {
  const files = patchFiles(readFileSync(PATCH, "utf8"));
  assert.deepEqual(files, ["definitions/events.yaml", "orchestrator/dispatcher/dispatcher.py", "orchestrator/worker/entrypoint.sh"]);
  for (const [f, s] of MARKERS) {
    assert.ok(files.includes(f), `${f} を差分が触らない`);
    assert.match(readFileSync(PATCH, "utf8"), new RegExp(`^\\+.*${s.replace(/[.()]/g, "\\$&")}`, "m"), `${s} を差分が足さない`);
  }
});

/** 偽の Engine の HEAD を head にし、ENGINE_REF を ref にしたワークフローで、偽の Engine の差分と MARKERS を照らす。 */
function checkFake(ref, head) {
  git(fake.dir, "checkout", "-q", head);
  const workflow = join(fake.root, "w.yml");
  writeFileSync(workflow, `env:\n  ENGINE_REF: ${ref}\n`);
  return checkPatch({ engine: fake.dir, workflow, patch: fake.patch, markers: fake.markers });
}

test("ENGINE_REF の版と既定のブランチの先頭の両方を照らし、patch_stale と patch_partially_applied だけを外れにする", () => {
  const ok = checkFake(fake.base, fake.merged);
  assert.deepEqual(ok.results.map((r) => [r.target, r.ref, r.state]),
    [["ENGINE_REF", fake.base, APPLIES], ["既定のブランチの先頭", fake.merged, MERGED]]);
  assert.deepEqual(ok.errors, []);
  const stale = checkFake(fake.base, fake.moved);
  assert.deepEqual(stale.errors.map((e) => [e.rule, e.target, e.ref]), [[STALE, "既定のブランチの先頭", fake.moved]]);
  const both = checkFake(fake.partial, fake.moved);
  assert.deepEqual(both.errors.map((e) => [e.rule, e.target]), [[PARTIAL, "ENGINE_REF"], [STALE, "既定のブランチの先頭"]]);
});

test("CLI: ENGINE_REF の版がチェックアウトにない・ENGINE_REF を読めないときは照らせず終了コード 2（patch_check_failed）", () => {
  const workflow = join(fake.root, "missing.yml");
  writeFileSync(workflow, `env:\n  ENGINE_REF: ${"0".repeat(40)}\n`);
  const r = spawnSync("node", [CLI, "--engine", fake.dir, "--workflow", workflow], { encoding: "utf8", env: { ...process.env, GITHUB_ACTIONS: "true" } });
  assert.equal(r.status, 2);
  assert.match(r.stdout, /^::error title=pr-body-patch patch_check_failed::.*engine_ref_missing/m);
  writeFileSync(workflow, "env:\n  ENGINE_REF: main\n");
  assert.equal(spawnSync("node", [CLI, "--engine", fake.dir, "--workflow", workflow], { encoding: "utf8" }).status, 2);
  assert.equal(spawnSync("node", [CLI], { encoding: "utf8" }).status, 2);
});

test("CLI: 本物の差分が当たらない版では patch_stale で終了コード 1、JSON と要約に版ごとの状態を書く", () => {
  git(fake.dir, "checkout", "-q", fake.moved);
  const workflow = join(fake.root, "w2.yml");
  writeFileSync(workflow, `env:\n  ENGINE_REF: ${fake.base}\n`);
  const json = join(fake.root, "r2.json");
  const summary = join(fake.root, "s2.md");
  const r = spawnSync("node", [CLI, "--engine", fake.dir, "--workflow", workflow, "--json", json, "--summary", summary],
    { encoding: "utf8", env: { ...process.env, GITHUB_ACTIONS: "true" } });
  assert.equal(r.status, 1, r.stdout + r.stderr);
  assert.match(r.stdout, /^::error title=pr-body-patch patch_stale::/m);
  const out = JSON.parse(readFileSync(json, "utf8"));
  assert.equal(out.ok, false);
  assert.deepEqual(out.results.map((x) => [x.target, x.ref, x.state]), [["ENGINE_REF", fake.base, STALE], ["既定のブランチの先頭", fake.moved, STALE]]);
  assert.match(readFileSync(summary, "utf8"), /ENGINE_REF（`[0-9a-f]{40}`）: patch_stale/);
});

const engineDir = process.env.ENGINE_DIR || fileURLToPath(new URL("../../../engine", import.meta.url));
const required = Boolean(process.env.ENGINE_DIR) || process.env.CI === "true";
const ref = engineRef(readFileSync(workflowPath(), "utf8"));
const hasEngine = existsSync(join(engineDir, ".git")) && hasCommit(engineDir, ref);
const skip = required || hasEngine ? false : `ENGINE_REF ${ref} の版を含む Engine のチェックアウトがない（ENGINE_DIR=${engineDir}）`;

test("本物の Engine: engine/pr-body-gate.patch は ENGINE_REF の版に当たるか、もう入っている", { skip }, () => {
  assert.ok(hasEngine, `Engine のチェックアウト（ENGINE_DIR=${engineDir}）に ENGINE_REF ${ref} の版がない`);
  const { state: s, detail } = patchState(engineDir, ref);
  assert.ok(s === APPLIES || s === MERGED,
    `${s}: engine/pr-body-gate.patch が ENGINE_REF ${ref} の Engine に当たらない。今の Engine に合わせて作り直す: ${detail}`);
});
