// check.mjs の ci_check を配線する順序の見張り（order.mjs）のテスト（知見 K-0004 の担保。議決 D-0012、Issue #953。包んだ呼び方は議決 D-0017、Issue #967）。
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { callsEventsCheck, checkOrder, DEFAULT_SCRIPTS, loadRepo, REPO_ROOT, runsEngineTest, todoLines } from "./order.mjs";

const orderScript = fileURLToPath(new URL("./order.mjs", import.meta.url));
const checkScript = fileURLToPath(new URL("./check.mjs", import.meta.url));
const template = readFileSync(fileURLToPath(new URL("./ci/sendback-engine.yml", import.meta.url)), "utf8");
const engineTestNow = readFileSync(fileURLToPath(new URL("./engine.test.mjs", import.meta.url)), "utf8");

/** README の「ワークフローの例」と同じ check.mjs --events の手順 */
const EVENTS_WORKFLOW = `name: sendback-check
on:
  pull_request:
jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: 差し戻し・続きのジョブの判定と承認の basis（K-0004）
        run: node tools/sendback-check/check.mjs --events events.json --pr "\${{ github.event.pull_request.html_url }}" --summary "$GITHUB_STEP_SUMMARY"
`;
/** todo を外した後の engine.test.mjs（Engine の basis を直して LEGACY_BASIS と todoWhileLegacy を消した形） */
const ENGINE_TEST_FIXED = engineTestNow.replace(/^.*todoWhileLegacy\(t,.*$/gm, "").replace(/^const todoWhileLegacy.*$/m, "");

const wf = (path, text) => ({ path: `.github/workflows/${path}`, text });
const rules = (r) => r.errors.map((e) => e.rule).sort();

test("今の engine.test.mjs には basis の場合の todo が残っていて、外した形には残らない", () => {
  assert.ok(todoLines(engineTestNow).length > 0);
  assert.deepEqual(todoLines(ENGINE_TEST_FIXED), []);
  // 注釈の中の todo は数えない
  assert.deepEqual(todoLines("// todo のうちに配線しない\nconst x = 1;"), []);
  assert.deepEqual(todoLines("test('x', { todo: true }, () => {});"), [1]);
  assert.deepEqual(todoLines("\ntest.todo('x');"), [2]);
});

test("check.mjs --events を呼ぶ手順を見つける（道・working-directory・行の継ぎ）", () => {
  assert.ok(callsEventsCheck(EVENTS_WORKFLOW));
  assert.ok(callsEventsCheck("steps:\n  - working-directory: tools/sendback-check\n    run: node check.mjs --events e.json\n"));
  assert.ok(callsEventsCheck("steps:\n  - run: |\n      node tools/sendback-check/check.mjs \\\n        --events e.json\n"));
  // --events のない呼び方（--record）、注釈の中、ほかの道具の check.mjs は数えない
  assert.ok(!callsEventsCheck("steps:\n  - run: node tools/sendback-check/check.mjs --record r.json\n"));
  assert.ok(!callsEventsCheck("steps:\n  # - run: node tools/sendback-check/check.mjs --events e.json\n"));
  assert.ok(!callsEventsCheck("steps:\n  - run: node tools/pr-body-check/check.mjs --events e.json\n"));
});

/** check.mjs --events を npm scripts・シェルのラッパー・引数の順序を変えた呼び方で配線したワークフローの写し（議決 D-0017、Issue #967） */
const run = (body) => `name: pr-checks\non:\n  pull_request:\njobs:\n  check:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n${body}`;
const WRAPPED_WORKFLOWS = {
  "引数の順序を変える（--pr が先）": run('      - run: node tools/sendback-check/check.mjs --pr 12 --events events.json\n'),
  "--events を最後の継ぎ行に置く": run('      - run: |\n          node tools/sendback-check/check.mjs \\\n            --pr 12 \\\n            --summary "$GITHUB_STEP_SUMMARY" \\\n            --events=events.json\n'),
  "npm run check（working-directory）": run('      - working-directory: tools/sendback-check\n        run: npm run check -- --pr 12 --events ../../events.json\n'),
  "npm --prefix … run check": run('      - run: npm --prefix tools/sendback-check run check -- --pr 12 --events events.json\n'),
  "npm run-script check --prefix": run('      - run: npm run-script check --prefix=tools/sendback-check -- --events events.json\n'),
  "--events を中に入れた npm scripts": run('      - run: npm --prefix tools/sendback-check run check:events -- --pr 12\n'),
  "env の変数に道を入れる": run('      - env:\n          SENDBACK_CHECK: tools/sendback-check/check.mjs\n        run: node "$SENDBACK_CHECK" --pr 12 --events events.json\n'),
  "シェルの関数で包む": run('      - run: |\n          sb() { node tools/sendback-check/check.mjs "$@"; }\n          sb --pr 12 --events events.json\n'),
  "引数を配列で渡す": run('      - run: |\n          args=(--pr 12 --events events.json)\n          node tools/sendback-check/check.mjs "${args[@]}"\n'),
  "cd してから呼ぶ": run('      - run: |\n          cd tools/sendback-check\n          node ./check.mjs --pr 12 --events ../../events.json\n'),
  "シェルのラッパー（--events は呼ぶ側）": run('      - run: ./scripts/sendback.sh --pr 12 --events events.json\n'),
  "シェルのラッパー（--events も中）": run('      - run: bash tools/sendback-check/ci/events.sh 12\n'),
};
/** ラッパーが呼ぶ先（リポジトリの中のファイル）と、--events を中に入れた npm scripts */
const WRAPPER_FILES = {
  "scripts/sendback.sh": '#!/usr/bin/env bash\nset -euo pipefail\nnode tools/sendback-check/check.mjs "$@"\n',
  "tools/sendback-check/ci/events.sh": '#!/usr/bin/env bash\n# PR の番号を受けて check.mjs に渡す\nnode "$(dirname "$0")/../check.mjs" --pr "$1" --events events.json\n',
};
const WRAPPER_SCRIPTS = { check: "node check.mjs", "check:events": "node check.mjs --events ../../events.json", test: "node --test" };
const readWrapper = (rel) => WRAPPER_FILES[rel] ?? null;

test("npm scripts・シェルのラッパー・引数の順序を変えた呼び方も check.mjs --events の配線として見つける", () => {
  for (const [name, text] of Object.entries(WRAPPED_WORKFLOWS)) {
    assert.ok(callsEventsCheck(text, { scripts: WRAPPER_SCRIPTS, readFile: readWrapper }), name);
  }
  // 包んでも --events のない呼び方、check.mjs を呼ばないラッパーや scripts、ほかの道具の npm scripts は数えない
  const opts = { scripts: WRAPPER_SCRIPTS, readFile: (rel) => ({ "scripts/lint.sh": "#!/bin/sh\nnpx eslint .\n" })[rel] ?? readWrapper(rel) };
  assert.ok(!callsEventsCheck(run('      - run: npm --prefix tools/sendback-check run check -- --record r.json\n'), opts));
  assert.ok(!callsEventsCheck(run('      - run: npm --prefix tools/sendback-check run test -- --events e.json\n'), opts));
  assert.ok(!callsEventsCheck(run('      - run: npm --prefix tools/pr-body-check run check -- --events e.json\n'), opts));
  assert.ok(!callsEventsCheck(run('      - run: ./scripts/lint.sh --events e.json\n'), opts));
  assert.ok(!callsEventsCheck(run('      - run: ./scripts/sendback.sh --record r.json\n'), opts));
  // ラッパーの中身が読めなければ（ファイルがない）、ワークフローに sendback-check が出てこない呼び方は見つけられない
  assert.ok(!callsEventsCheck(WRAPPED_WORKFLOWS["シェルのラッパー（--events は呼ぶ側）"]));
});

test("包んだ呼び方の配線でも、todo のうちなら engine_test_todo、engine.test.mjs が未配線なら engine_test_not_wired で落ちる", () => {
  for (const [name, text] of Object.entries(WRAPPED_WORKFLOWS)) {
    const opts = { scripts: WRAPPER_SCRIPTS, readFile: readWrapper };
    const r = checkOrder({ workflows: [wf("sendback-check.yml", text)], engineTest: engineTestNow, ...opts });
    assert.deepEqual(r.wired, [".github/workflows/sendback-check.yml"], name);
    assert.deepEqual(rules(r), ["engine_test_not_wired", "engine_test_todo"], name);
    const todo = checkOrder({ workflows: [wf("sendback-engine.yml", template), wf("sendback-check.yml", text)], engineTest: engineTestNow, ...opts });
    assert.deepEqual(rules(todo), ["engine_test_todo"], name);
    const notWired = checkOrder({ workflows: [wf("sendback-check.yml", text)], engineTest: ENGINE_TEST_FIXED, ...opts });
    assert.deepEqual(rules(notWired), ["engine_test_not_wired"], name);
    // 順序を守れば包んだ呼び方でも通る
    const good = checkOrder({ workflows: [wf("sendback-engine.yml", template), wf("sendback-check.yml", text)], engineTest: ENGINE_TEST_FIXED, ...opts });
    assert.deepEqual(rules(good), [], name);
  }
});

test("engine.test.mjs を走らせる手順を見つける（ci/sendback-engine.yml の形と npm test）", () => {
  assert.ok(runsEngineTest(template));
  assert.ok(runsEngineTest("steps:\n  - working-directory: tools/sendback-check\n    run: npm test\n"));
  assert.ok(runsEngineTest("steps:\n  - run: node --test tools/sendback-check/engine.test.mjs\n"));
  // ほかの道具の npm test、engine.test.mjs を含まない名指し、注釈の中は数えない
  assert.ok(!runsEngineTest("steps:\n  - working-directory: tools/pr-body-check\n    run: npm test\n"));
  assert.ok(!runsEngineTest("steps:\n  - working-directory: tools/sendback-check\n    run: node --test check.test.mjs\n"));
  assert.ok(!runsEngineTest("steps:\n  - working-directory: tools/sendback-check\n    run: echo # npm test\n"));
  assert.ok(!runsEngineTest(EVENTS_WORKFLOW));
});

test("check.mjs --events を配線していなければ、todo があっても通る（今のリポジトリ）", () => {
  assert.deepEqual(checkOrder({ workflows: [], engineTest: engineTestNow }), { wired: [], errors: [] });
  const now = checkOrder(loadRepo(REPO_ROOT));
  assert.deepEqual(now, { wired: [], errors: [] });
});

test("todo のうちに check.mjs --events を配線すると engine_test_todo で落ちる（engine.test.mjs を配線していても）", () => {
  const r = checkOrder({ workflows: [wf("sendback-engine.yml", template), wf("sendback-check.yml", EVENTS_WORKFLOW)], engineTest: engineTestNow });
  assert.deepEqual(r.wired, [".github/workflows/sendback-check.yml"]);
  assert.deepEqual(rules(r), ["engine_test_todo"]);
  assert.match(r.errors[0].message, /\.github\/workflows\/sendback-check\.yml が check\.mjs --events を呼ぶのに、.*todo が残っている（\d+/);
});

test("engine.test.mjs を CI に配線せずに check.mjs --events を配線すると engine_test_not_wired で落ちる（todo を外していても）", () => {
  const r = checkOrder({ workflows: [wf("sendback-check.yml", EVENTS_WORKFLOW)], engineTest: ENGINE_TEST_FIXED });
  assert.deepEqual(rules(r), ["engine_test_not_wired"]);
  const both = checkOrder({ workflows: [wf("sendback-check.yml", EVENTS_WORKFLOW)], engineTest: engineTestNow });
  assert.deepEqual(rules(both), ["engine_test_not_wired", "engine_test_todo"]);
  // engine.test.mjs を消して配線したときも落とす（見張りを黙って外さない）
  assert.deepEqual(rules(checkOrder({ workflows: [wf("a.yml", template + EVENTS_WORKFLOW)], engineTest: null })), ["engine_test_todo"]);
});

test("todo を外し engine.test.mjs を CI に配線してから check.mjs --events を配線すれば通る（同じワークフローでも）", () => {
  assert.deepEqual(rules(checkOrder({ workflows: [wf("sendback-engine.yml", template), wf("sendback-check.yml", EVENTS_WORKFLOW)],
    engineTest: ENGINE_TEST_FIXED })), []);
  const steps = template.replace(/\n*$/, "\n") + EVENTS_WORKFLOW.slice(EVENTS_WORKFLOW.indexOf("      - name: 差し戻し"));
  assert.deepEqual(rules(checkOrder({ workflows: [wf("sendback-engine.yml", steps)], engineTest: ENGINE_TEST_FIXED })), []);
});

/** リポジトリの写しを一時ディレクトリに組む */
function fixture(workflows, engineTest, files = {}) {
  const root = mkdtempSync(join(tmpdir(), "sendback-order-"));
  mkdirSync(join(root, ".github/workflows"), { recursive: true });
  mkdirSync(join(root, "tools/sendback-check"), { recursive: true });
  for (const [name, text] of Object.entries(workflows)) writeFileSync(join(root, ".github/workflows", name), text);
  writeFileSync(join(root, "tools/sendback-check/engine.test.mjs"), engineTest);
  for (const [rel, text] of Object.entries(files)) {
    mkdirSync(dirname(join(root, rel)), { recursive: true });
    writeFileSync(join(root, rel), text);
  }
  return root;
}

test("CLI: order.mjs --repo は外れたら終了コード 1 で ::error と要約を出し、通れば 0", () => {
  const bad = fixture({ "sendback-check.yml": EVENTS_WORKFLOW }, engineTestNow);
  const summary = join(bad, "summary.md");
  const r = spawnSync("node", [orderScript, "--repo", bad, "--json", join(bad, "out.json"), "--summary", summary],
    { env: { ...process.env, GITHUB_ACTIONS: "true" }, encoding: "utf8" });
  assert.equal(r.status, 1);
  assert.match(r.stdout, /^::error title=sendback-check engine_test_todo::/m);
  assert.match(r.stdout, /^::error title=sendback-check engine_test_not_wired::/m);
  assert.equal(JSON.parse(readFileSync(join(bad, "out.json"), "utf8")).ok, false);
  assert.match(readFileSync(summary, "utf8"), /外れた `engine_test_todo`/);

  const good = fixture({ "sendback-engine.yml": template, "sendback-check.yml": EVENTS_WORKFLOW }, ENGINE_TEST_FIXED);
  assert.equal(spawnSync("node", [orderScript, "--repo", good], { encoding: "utf8" }).status, 0);
  assert.equal(spawnSync("node", [orderScript], { encoding: "utf8" }).status, 0);   // 今のリポジトリはまだ配線していない
});

test("CLI: order.mjs --repo は npm scripts とラッパーのスクリプトをリポジトリから読み、包んだ呼び方の配線でも落とす", () => {
  const files = { ...WRAPPER_FILES, "tools/sendback-check/package.json": JSON.stringify({ scripts: WRAPPER_SCRIPTS }) };
  for (const name of ["--events を中に入れた npm scripts", "シェルのラッパー（--events は呼ぶ側）", "シェルのラッパー（--events も中）"]) {
    const bad = fixture({ "sendback-check.yml": WRAPPED_WORKFLOWS[name] }, engineTestNow, files);
    const r = spawnSync("node", [orderScript, "--repo", bad], { encoding: "utf8" });
    assert.equal(r.status, 1, name);
    assert.match(r.stdout, /^engine_test_todo: /m, name);
    assert.match(r.stdout, /^engine_test_not_wired: /m, name);
    const good = fixture({ "sendback-engine.yml": template, "sendback-check.yml": WRAPPED_WORKFLOWS[name] }, ENGINE_TEST_FIXED, files);
    assert.equal(spawnSync("node", [orderScript, "--repo", good], { encoding: "utf8" }).status, 0, name);
  }
  // ラッパーがなければ（読めなければ）、--events を中に入れた npm scripts は名前からは --events を呼ぶと分からない
  const noScripts = fixture({ "sendback-check.yml": WRAPPED_WORKFLOWS["--events を中に入れた npm scripts"] }, engineTestNow);
  assert.equal(spawnSync("node", [orderScript, "--repo", noScripts], { encoding: "utf8" }).status, 0);
  // package.json が壊れていれば、import の時点ではなく loadRepo で読めなかったとして終了コード 2
  const broken = fixture({ "sendback-check.yml": WRAPPED_WORKFLOWS["--events を中に入れた npm scripts"] }, engineTestNow,
    { "tools/sendback-check/package.json": "{ scripts" });
  const r = spawnSync("node", [orderScript, "--repo", broken], { encoding: "utf8" });
  assert.equal(r.status, 2);
  assert.match(r.stderr, /^sendback-check order: 読めなかった: /m);
});

test("scripts を渡さないときの既定は定数で、order.mjs を import してもファイルを読まない", () => {
  assert.deepEqual(DEFAULT_SCRIPTS, { check: "node check.mjs" });
  assert.ok(callsEventsCheck("steps:\n  - run: npm --prefix tools/sendback-check run check -- --events e.json\n"));
  assert.ok(!callsEventsCheck("steps:\n  - run: npm --prefix tools/sendback-check run check:events -- --pr 12\n"));
});

test("CLI: check.mjs --events を GitHub Actions で動かすと、通る記録でも配線の順序が外れていれば落ちる", () => {
  const events = [{ name: "supervisor.judge_enqueued", payload: { pr_url: "https://example.test/pull/1", head_sha: "h1" } }];
  const run = (root) => {
    writeFileSync(join(root, "events.json"), JSON.stringify(events));
    return spawnSync("node", [checkScript, "--events", join(root, "events.json"), "--json", join(root, "out.json")],
      { env: { ...process.env, GITHUB_ACTIONS: "true", GITHUB_WORKSPACE: root }, encoding: "utf8" });
  };
  const bad = run(fixture({ "sendback-check.yml": EVENTS_WORKFLOW }, engineTestNow));
  assert.equal(bad.status, 1);
  assert.match(bad.stdout, /^::error title=sendback-check engine_test_todo::/m);
  const good = run(fixture({ "sendback-engine.yml": template, "sendback-check.yml": EVENTS_WORKFLOW }, ENGINE_TEST_FIXED));
  assert.equal(good.status, 0, good.stdout);
});
