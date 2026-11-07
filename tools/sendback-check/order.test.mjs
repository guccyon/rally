// check.mjs の ci_check を配線する順序の見張り（order.mjs）のテスト（知見 K-0004 の担保。議決 D-0012、Issue #953。包んだ呼び方は議決 D-0017、Issue #967。飛ばせる・待たない配線は議決 D-0025、Issue #1017）。
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { BASIS_TESTS, callsEventsCheck, checkOrder, DEFAULT_SCRIPTS, loadRepo, parseTapReport, REPO_ROOT, runsEngineTest, todoLines } from "./order.mjs";

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
/** ci/sendback-engine.yml の写しに check.mjs --events を呼ぶジョブを足した 1 つのワークフロー（needs は 1 行に書く形のまま入れる。null なら needs を書かない） */
const sameWorkflow = (needs = "engine-test", base = template) => `${base.replace(/\n*$/, "\n")}
  check:
${needs === null ? "" : `    needs: ${needs}\n`}    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
${EVENTS_WORKFLOW.slice(EVENTS_WORKFLOW.indexOf("      - name: 差し戻し"))}`;
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
  assert.deepEqual(rules(checkOrder({ workflows: [wf("a.yml", sameWorkflow())], engineTest: null })), ["engine_test_todo"]);
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

// ENGINE_REF の Engine で走らせた engine.test.mjs の結果を見る（議決 D-0016、Issue #964）。
// 結果は node --test の TAP（ci/sendback-engine.yml の engine-test が engine-test.tap に出す）。名前の中の # は \# に逃がされる

/** engine.test.mjs の TAP の写し。basis は (3) と (c) の結果（"" は通った、" # TODO …" は todo）。 */
const tap = ({ basis = "", events = basis, notOk = Boolean(basis) } = {}) => `TAP version 13
# Subtest: (1) Engine は PR \\#395 と同じ入力では差し戻しも続きのジョブも起こさない
ok 2 - (1) Engine は PR \\#395 と同じ入力では差し戻しも続きのジョブも起こさない
  ---
  duration_ms: 12.3
  ...
# Subtest: ${BASIS_TESTS["(3)"]}
${notOk ? "not " : ""}ok 6 - ${BASIS_TESTS["(3)"]}${basis}
  ---
  duration_ms: 40.1
  ...
# Subtest: ${BASIS_TESTS["(c)"]}
${notOk ? "not " : ""}ok 9 - ${BASIS_TESTS["(c)"]}${events}
1..9
`;
const LEGACY_TODO = " # TODO Engine の basis が adopt の合計だけ（今の形。建設者が Engine を直したら必須になる）";
const WIRED = [wf("sendback-engine.yml", template), wf("sendback-check.yml", EVENTS_WORKFLOW)];

test("TAP の結果を読む（todo・skip・落ちた・通った、逃がした #、字下げした下の段は数えない）", () => {
  const r = parseTapReport(tap({ basis: LEGACY_TODO }) + "    ok 1 - 下の段\nok 10 - 飛ばした # SKIP Engine のチェックアウトがない\nnot ok 11 - 落ちた\n");
  assert.equal(r[BASIS_TESTS["(3)"]], "todo");
  assert.equal(r[BASIS_TESTS["(c)"]], "todo");
  assert.equal(r["(1) Engine は PR #395 と同じ入力では差し戻しも続きのジョブも起こさない"], "pass");
  assert.equal(r["飛ばした"], "skip");
  assert.equal(r["落ちた"], "fail");
  assert.equal(r["下の段"], undefined);
  assert.equal(parseTapReport(tap())[BASIS_TESTS["(3)"]], "pass");
});

test("engine.test.mjs の (3) と (c) は BASIS_TESTS の名前で、engine-test はその TAP を order.mjs --engine-report に渡す", () => {
  assert.match(engineTestNow, /^test\(BASIS_TESTS\["\(3\)"\], \{ skip \}/m);
  assert.match(engineTestNow, /^test\(BASIS_TESTS\["\(c\)"\], \{ skip \}/m);
  const dest = template.match(/--test-reporter=tap --test-reporter-destination=(\S+)/)?.[1];
  assert.ok(dest, "engine-test が engine.test.mjs の TAP をファイルに出していない");
  // テストが落ちても結果を見るよう、engine.test.mjs と同じ手順で渡す（ci.test.mjs が if: を許さないので、別の手順に分けない）
  assert.match(template, new RegExp(`engine\\.test\\.mjs[^]*\\n\\s*node order\\.mjs --engine-report ${dest.replace(/\./g, "\\.")} [^\\n]*\\|\\| rc=1\\n\\s*exit \\$rc\\n`));
  // 結果を渡す手順そのものは check.mjs --events の配線に数えない
  assert.deepEqual(checkOrder({ workflows: [wf("sendback-engine.yml", template)], engineTest: engineTestNow }), { wired: [], errors: [] });
});

test("check.mjs --events を配線したのに、ENGINE_REF の Engine で (3) か (c) が LEGACY_BASIS の todo になったら engine_basis_not_passed で落ちる", () => {
  const r = checkOrder({ workflows: WIRED, engineTest: ENGINE_TEST_FIXED, engineReport: tap({ basis: LEGACY_TODO }) });
  assert.deepEqual(rules(r), ["engine_basis_not_passed", "engine_basis_not_passed"]);
  assert.match(r.errors[0].message, /ENGINE_REF の Engine で .*engine\.test\.mjs の \(3\)（.*）が todo になった/);
  assert.match(r.errors[1].message, /の \(c\)（.*）が todo になった/);
  // (c) だけが todo でも、飛ばした・落ちた・結果にないときも落とす（黙って通さない）
  assert.deepEqual(rules(checkOrder({ workflows: WIRED, engineTest: ENGINE_TEST_FIXED, engineReport: tap({ events: LEGACY_TODO, notOk: false }) })),
    ["engine_basis_not_passed"]);
  assert.deepEqual(rules(checkOrder({ workflows: WIRED, engineTest: ENGINE_TEST_FIXED, engineReport: tap({ basis: " # SKIP x", notOk: false }) })),
    ["engine_basis_not_passed", "engine_basis_not_passed"]);
  assert.deepEqual(rules(checkOrder({ workflows: WIRED, engineTest: ENGINE_TEST_FIXED, engineReport: tap({ basis: "", notOk: true }) })),
    ["engine_basis_not_passed", "engine_basis_not_passed"]);
  const missing = checkOrder({ workflows: WIRED, engineTest: ENGINE_TEST_FIXED, engineReport: "" });
  assert.deepEqual(rules(missing), ["engine_basis_not_passed", "engine_basis_not_passed"]);
  assert.match(missing.errors[0].message, /結果にない/);
});

test("(3) と (c) が通った結果なら落ちず、check.mjs --events を配線していなければ todo の結果でも通る（今のリポジトリ）", () => {
  assert.deepEqual(rules(checkOrder({ workflows: WIRED, engineTest: ENGINE_TEST_FIXED, engineReport: tap() })), []);
  assert.deepEqual(checkOrder({ workflows: [wf("sendback-engine.yml", template)], engineTest: engineTestNow, engineReport: tap({ basis: LEGACY_TODO }) }),
    { wired: [], errors: [] });
  assert.deepEqual(checkOrder({ ...loadRepo(REPO_ROOT), engineReport: "" }), { wired: [], errors: [] });
});

test("CLI: order.mjs --engine-report は結果の TAP を読み、(3) か (c) が todo なら終了コード 1、ファイルがなければ結果にないとして見る", () => {
  const root = fixture({ "sendback-engine.yml": template, "sendback-check.yml": EVENTS_WORKFLOW }, ENGINE_TEST_FIXED);
  const runOrder = (report) => spawnSync("node", [orderScript, "--repo", root, "--engine-report", report],
    { env: { ...process.env, GITHUB_ACTIONS: "true" }, encoding: "utf8" });
  writeFileSync(join(root, "legacy.tap"), tap({ basis: LEGACY_TODO }));
  writeFileSync(join(root, "fixed.tap"), tap());
  const legacy = runOrder(join(root, "legacy.tap"));
  assert.equal(legacy.status, 1);
  assert.match(legacy.stdout, /^::error title=sendback-check engine_basis_not_passed::/m);
  assert.equal(runOrder(join(root, "fixed.tap")).status, 0);
  const none = runOrder(join(root, "none.tap"));
  assert.equal(none.status, 1);
  assert.match(none.stdout, /結果にない/);
  // 今のリポジトリ（まだ配線していない）では、結果のファイルがなくても通る
  assert.equal(spawnSync("node", [orderScript, "--engine-report", join(root, "none.tap")], { encoding: "utf8" }).status, 0);
});

// engine.test.mjs の手順やジョブを飛ばせる・落ちても通る形にした写しと、check.mjs --events のジョブが engine-test を待たない写し（議決 D-0025、Issue #1017）

const ENGINE_STEP = "      - name: Engine の本物の判定のテスト（K-0004）\n";
const ENGINE_JOB = "  engine-test:\n    runs-on: ubuntu-latest\n";
const stepWith = (key) => template.replace(ENGINE_STEP, `${ENGINE_STEP}        ${key}\n`);
const jobWith = (key) => template.replace(ENGINE_JOB, `${ENGINE_JOB}    ${key}\n`);
/** engine.test.mjs を走らせたことにならない写し */
const UNGATED_TEMPLATES = {
  "手順に continue-on-error: true": stepWith("continue-on-error: true"),
  "手順に continue-on-error: ${{ true }}": stepWith('continue-on-error: "${{ true }}"'),
  "手順に if: false": stepWith("if: false"),
  "手順に if: ${{ false }}": stepWith("if: ${{ false }}"),
  "手順に if: github.event_name == 'push'": stepWith("if: github.event_name == 'push'"),
  "ジョブに continue-on-error: true": jobWith("continue-on-error: true"),
  "ジョブに if: false": jobWith("if: false"),
  "ジョブに if: ${{ false }}": jobWith("if: ${{ false }}"),
};

test("engine.test.mjs の手順かジョブに continue-on-error か if: を付けた写しは、engine.test.mjs を走らせる手順に数えない", () => {
  for (const [name, text] of Object.entries(UNGATED_TEMPLATES)) {
    assert.notEqual(text, template, `${name}: 写しを作れていない`);
    assert.ok(!runsEngineTest(text), name);
  }
  // continue-on-error: false はゲートのまま。ほかの手順・ジョブ（wiring-order）の continue-on-error・if: は見ない
  assert.ok(runsEngineTest(stepWith("continue-on-error: false")));
  assert.ok(runsEngineTest(jobWith("continue-on-error: ${{ false }}")));
  assert.ok(runsEngineTest(template.replace("  wiring-order:\n    runs-on: ubuntu-latest\n", "  wiring-order:\n    if: false\n    runs-on: ubuntu-latest\n")));
  // run の中の文字（シェルの if など）は手順の鍵に数えない
  assert.ok(runsEngineTest(template.replace("          rc=0\n", "          if: true\n          rc=0\n")));
  // jobs: のない手順だけの写しでも、手順の continue-on-error は見る
  assert.ok(!runsEngineTest("steps:\n  - working-directory: tools/sendback-check\n    continue-on-error: true\n    run: npm test\n"));
  assert.ok(!runsEngineTest("steps:\n  - if: false\n    working-directory: tools/sendback-check\n    run: npm test\n"));
});

test("engine.test.mjs を飛ばせる・落ちても通る形で配線して check.mjs --events を配線すると engine_test_not_wired で落ちる（同じワークフローでも）", () => {
  for (const [name, text] of Object.entries(UNGATED_TEMPLATES)) {
    const r = checkOrder({ workflows: [wf("sendback-engine.yml", text), wf("sendback-check.yml", EVENTS_WORKFLOW)], engineTest: ENGINE_TEST_FIXED });
    assert.deepEqual(rules(r), ["engine_test_not_wired"], name);
    assert.match(r.errors[0].message, /continue-on-error/, name);
    const same = checkOrder({ workflows: [wf("sendback-engine.yml", sameWorkflow("engine-test", text))], engineTest: ENGINE_TEST_FIXED });
    assert.deepEqual(rules(same), ["engine_test_not_wired"], name);
  }
});

test("同じワークフローで check.mjs --events のジョブが engine.test.mjs のジョブを needs で待たないと engine_test_not_required で落ちる", () => {
  const order = (text) => checkOrder({ workflows: [wf("sendback-engine.yml", text)], engineTest: ENGINE_TEST_FIXED });
  for (const needs of [null, "wiring-order", "[wiring-order]", "other"]) {
    const r = order(sameWorkflow(needs));
    assert.deepEqual(rules(r), ["engine_test_not_required"], String(needs));
    assert.match(r.errors[0].message, /\.github\/workflows\/sendback-engine\.yml の check のジョブが check\.mjs --events を呼ぶのに、.*engine-test を needs で待たない/);
  }
  // 同じジョブの中で engine.test.mjs の手順より前に呼ぶのも待たない
  const before = template.replace(ENGINE_STEP, `${EVENTS_WORKFLOW.slice(EVENTS_WORKFLOW.indexOf("      - name: 差し戻し"))}${ENGINE_STEP}`);
  assert.deepEqual(rules(order(before)), ["engine_test_not_required"]);
  // needs で待てば通る（1 行・配列・列の形、間に別のジョブを挟む形も）
  for (const needs of ["engine-test", "[wiring-order, engine-test]", '["engine-test"]', "\n      - wiring-order\n      - engine-test"]) {
    assert.deepEqual(rules(order(sameWorkflow(needs))), [], needs);
  }
  const chained = sameWorkflow("gate") + "\n  gate:\n    needs: engine-test\n    runs-on: ubuntu-latest\n    steps:\n      - run: echo ok\n";
  assert.deepEqual(rules(order(chained)), []);
  // 別のワークフローの間は needs で待てないので見ない（engine-test を必須のチェックにして止める）
  assert.deepEqual(rules(checkOrder({ workflows: WIRED, engineTest: ENGINE_TEST_FIXED })), []);
  // todo が残っていれば両方で落ちる
  assert.deepEqual(rules(checkOrder({ workflows: [wf("sendback-engine.yml", sameWorkflow(null))], engineTest: engineTestNow })),
    ["engine_test_not_required", "engine_test_todo"]);
});

test("CLI: order.mjs --repo は engine.test.mjs を待たない配線と、continue-on-error を付けた engine.test.mjs で終了コード 1 を返す", () => {
  const run = (root) => spawnSync("node", [orderScript, "--repo", root], { env: { ...process.env, GITHUB_ACTIONS: "true" }, encoding: "utf8" });
  const unwaited = run(fixture({ "sendback-engine.yml": sameWorkflow(null) }, ENGINE_TEST_FIXED));
  assert.equal(unwaited.status, 1);
  assert.match(unwaited.stdout, /^::error title=sendback-check engine_test_not_required::/m);
  const ungated = run(fixture({ "sendback-engine.yml": UNGATED_TEMPLATES["手順に continue-on-error: true"], "sendback-check.yml": EVENTS_WORKFLOW },
    ENGINE_TEST_FIXED));
  assert.equal(ungated.status, 1);
  assert.match(ungated.stdout, /^::error title=sendback-check engine_test_not_wired::/m);
  assert.equal(run(fixture({ "sendback-engine.yml": sameWorkflow() }, ENGINE_TEST_FIXED)).status, 0);
});
