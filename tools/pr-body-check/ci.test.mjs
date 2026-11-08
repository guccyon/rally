// ci/pr-body-engine.yml の形の見張りのテスト（知見 K-0001 の担保。議決 D-0032、Issue #1037）。
// 建設者がこのひな形を .github/workflows/pr-body-engine.yml に写し、engine-test を必須のチェックにすると、関所を呼ばない Engine の版では
// engine.test.mjs の (3) で落ちる。その止め方が効くのは、ひな形が PR ごとに必ず走り、落ちたら止め、ENGINE_REF の行を変えた PR でその版の
// Engine を確かめる形のときだけなので、それを見る。写しがあれば ENGINE_REF の行のほかがひな形と同じかも見る（写すときに変えてよいのはその行だけ）。
// このテストは engine-test の node --test の中で走るので、写しから見張りを外す変更も同じジョブで落ちる。
// 通る側のテスト（gate.test.mjs の関所と CLI、engine.test.mjs の (10)）が skip・todo・only・道や名前の絞り込みで実行の対象から外れていないことも見る
// （議決 D-0045 の ci_check の案、Issue #1109）。recheck.test.mjs（知見 K-0012 の担保）も同じく外れていないことを見る（議決 D-0050 の ci_check の案、Issue #1131）。
// 字面の照合だけに頼らず、gate.test.mjs と engine.test.mjs を TAP で走らせた結果で、通る側のテストと (10) が pass と数えられたかも見る（議決 D-0070 の test の案、Issue #1183）。
// ci/pr-body-protection.yml（branch protection を読めるトークンで protection.mjs を既定のブランチへの push と毎日走らせ、外れたら Issue を開くか更新する。
// 議決 D-0044、Issue #1106）の形と手順の振る舞い、写しとのずれも見る。写しがあれば、写しそのものが schedule と workflow_dispatch で走り、
// PR_BODY_PROTECTION_TOKEN を読めないときに protection_unreadable で飛ばさず落ちることも見る（議決 D-0068 の test の案、Issue #1180）。
// ci/pr-body-patch.yml（engine/pr-body-gate.patch が ENGINE_REF の版と Engine の既定のブランチの先頭に当たるかを patch.mjs で既定のブランチへの push と毎日照らし、
// patch_stale か patch_partially_applied なら Issue を開くか更新する。議決 D-0068（起票時の番号は D-0066）、Issue #1179）の形と手順の振る舞い、写しとのずれも見る。
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { parseTapReport } from "../sendback-check/order.mjs";
import { jobCheckNames } from "../sendback-check/protection.mjs";
import { PASSING_REPORTS } from "./c0005.mjs";
import { CHECK_NAME, REQUIRED_JOB } from "./protection.mjs";

const template = readFileSync(fileURLToPath(new URL("./ci/pr-body-engine.yml", import.meta.url)), "utf8");
const copyPath = fileURLToPath(new URL("../../.github/workflows/pr-body-engine.yml", import.meta.url));
const sendbackTemplate = readFileSync(fileURLToPath(new URL("../sendback-check/ci/sendback-engine.yml", import.meta.url)), "utf8");
const sendbackCopyPath = fileURLToPath(new URL("../../.github/workflows/sendback-engine.yml", import.meta.url));

const engineRef = (text) => text.match(/^ {2}ENGINE_REF:\s*(\S+)\s*$/m)?.[1];
/** ENGINE_REF の行を除いた中身（ずれの比べ方）。 */
const withoutRef = (text) => text.split(/\r?\n/).filter((l) => !/^\s*ENGINE_REF:/.test(l)).join("\n").trimEnd();

/** ジョブの本体（`  <job>:` の次の行から次のジョブの前まで）。 */
function jobBody(lines, job) {
  const start = lines.indexOf(`  ${job}:`);
  if (start < 0) return null;
  const end = lines.findIndex((l, i) => i > start && /^ {2}\S/.test(l));
  return lines.slice(start + 1, end < 0 ? lines.length : end);
}

/** ジョブの手順（`      - ` で始まる列の要素）を、行の塊で返す。 */
function jobSteps(body) {
  const steps = [];
  for (const l of body.slice(body.indexOf("    steps:") + 1)) {
    if (l.startsWith("      - ")) steps.push([l]);
    else if (steps.length > 0) steps[steps.length - 1].push(l);
  }
  return steps;
}

/** 必須のチェックにでき、ENGINE_REF の版の Engine で engine.test.mjs を飛ばさずに走らせる形かの外れ（空なら通る）。注釈の行は見ない。 */
function workflowProblems(text) {
  const problems = [];
  const lines = text.split(/\r?\n/).filter((l) => !/^\s*#/.test(l));
  const on = lines.findIndex((l) => /^on:\s*$/.test(l));
  const onBody = on < 0 ? [] : lines.slice(on + 1, lines.findIndex((l, i) => i > on && /^\S/.test(l)));
  if (onBody.map((l) => l.trim()).filter((l) => l !== "").join(" ") !== "pull_request:") {
    problems.push("on は絞り込みのない pull_request だけにする（paths・branches・types で絞ると必須のチェックが報告されない PR が出る）");
  }
  if (lines.some((l) => /^\s*(- )?continue-on-error:/.test(l))) problems.push("continue-on-error があると落ちても止めない");
  if (lines.some((l) => /^\s*(- )?if:/.test(l))) problems.push("if: があるとジョブや手順が飛ばされて見張らない");
  // ENGINE_REF を決める行: YAML の env の項目、GITHUB_ENV への書き込み、シェルの代入（echo の文の中の「ENGINE_REF=」は数えない）
  const refLines = lines.filter((l) => /^\s*(- )?ENGINE_REF:/.test(l) || (/ENGINE_REF/.test(l) && /GITHUB_ENV/.test(l))
    || /(^|[\s;&|(])(export\s+)?ENGINE_REF=/.test(l));
  if (refLines.length !== 1 || !/^[0-9a-f]{40}$/.test(engineRef(lines.join("\n")) ?? "")) {
    problems.push("ENGINE_REF はワークフローの env に 40 桁のコミット SHA で 1 行だけ置く（上書きや動く名前だと、ENGINE_REF の行を変えても別の版で走る）");
  }
  const engine = jobBody(lines, REQUIRED_JOB);
  if (engine === null) return [...problems, "engine-test のジョブがない（必須のチェックにするジョブ）"];
  // チェックの名前は protection.mjs と同じ読み方（jobCheckNames）で見る
  if (jobCheckNames(lines.join("\n")).get(REQUIRED_JOB) !== CHECK_NAME) {
    problems.push(`engine-test の name: を ${CHECK_NAME} にする（id のままの engine-test は knowledge-enforcement.yml も報告し、必須のチェックがどちらでも満たされる）`);
  }
  if (engine.some((l) => /^ {4}needs:/.test(l))) problems.push("engine-test に needs があると、待つジョブが落ちたときに飛ばされて走らない");
  const steps = jobSteps(engine);
  const checkout = steps.find((s) => s.some((l) => /repository: guccyon\/factory-engine/.test(l)));
  if (!checkout?.some((l) => /^\s*ref: \$\{\{ env\.ENGINE_REF \}\}\s*$/.test(l))) {
    problems.push("Engine を取る手順の ref が ${{ env.ENGINE_REF }} でない（ENGINE_REF の行を変えても取る版が変わらない）");
  }
  // NODE_OPTIONS に --test-name-pattern・--test-skip-pattern・--test-only などを入れると、道を渡さなくても走るテストを絞れる
  if (lines.some((l) => /NODE_OPTIONS/.test(l))) problems.push("NODE_OPTIONS があると、node --test の走らせるテストを絞れる");
  const run = steps.find((s) => s.some((l) => /node --test/.test(l)));
  if (!run) problems.push("engine-test が node --test を走らせていない");
  else {
    if (!run.some((l) => /^\s*working-directory: tools\/pr-body-check\s*$/.test(l))) problems.push("node --test を tools/pr-body-check で走らせていない");
    if (!run.some((l) => /^\s*ENGINE_DIR: \S/.test(l))) problems.push("ENGINE_DIR を渡していない（渡さないと Engine がないときに engine.test.mjs が飛ばされる）");
    // 引数に道を書くと、そのファイルだけを走らせて engine.test.mjs・ci.test.mjs を外せる
    if (!run.some((l) => /^\s*node --test(\s+--test-reporter=\S+)*\s+>/.test(l))) problems.push("node --test は道を渡さずにディレクトリのテストを全部走らせる");
    if (!run.some((l) => /^\s*exit \$rc\s*$/.test(l))) problems.push("テストの終了コードで手順を落としていない（exit $rc）");
  }
  return problems;
}

test("ひな形は必須のチェックにでき、ENGINE_REF の版の Engine で engine.test.mjs を飛ばさずに走らせる形", () => {
  assert.deepEqual(workflowProblems(template), []);
});

test("ENGINE_REF の行だけを変えたひな形も通る（Engine を上げる PR）", () => {
  const bumped = template.replace(engineRef(template), "0123456789abcdef0123456789abcdef01234567");
  assert.notEqual(bumped, template);
  assert.equal(withoutRef(bumped), withoutRef(template));
  assert.deepEqual(workflowProblems(bumped), []);
});

test("見張りを外す・飛ばす・絞る・別の版で走らせる直しは外れになる", () => {
  const onPr = "  pull_request:\n";
  const testStep = "          node --test --test-reporter=spec > pr-body-test.txt 2>&1 || rc=$?\n";
  const cases = {
    "paths で絞る": template.replace(onPr, `${onPr}    paths: ["tools/pr-body-check/**"]\n`),
    "paths-ignore で .github を外す": template.replace(onPr, `${onPr}    paths-ignore: [".github/**"]\n`),
    "types で絞る": template.replace(onPr, `${onPr}    types: [opened]\n`),
    "branches で絞る": template.replace(onPr, `${onPr}    branches: [release]\n`),
    "push だけにする": template.replace(onPr, "  push:\n"),
    "ジョブを飛ばす": template.replace("  engine-test:\n", "  engine-test:\n    if: false\n"),
    "手順を飛ばす": template.replace("      - name: PR 本文の関所のテスト（K-0001）\n", "      - name: PR 本文の関所のテスト（K-0001）\n        if: false\n"),
    "落ちても止めない": template.replace("  engine-test:\n", "  engine-test:\n    continue-on-error: true\n"),
    "ジョブの id を変える": template.replace("  engine-test:\n", "  pr-body:\n"),
    "チェックの名前を sendback と同じにする": template.replace(`    name: ${CHECK_NAME}\n`, "    name: engine-test\n"),
    "チェックの名前を外す": template.replace(`    name: ${CHECK_NAME}\n`, ""),
    "needs で待つ": template.replace("  engine-test:\n", "  engine-test:\n    needs: lint\n"),
    "ENGINE_REF をブランチ名にする": template.replace(engineRef(template), "main"),
    "ENGINE_REF を短い SHA にする": template.replace(engineRef(template), engineRef(template).slice(0, 7)),
    "手順の env で ENGINE_REF を上書きする": template.replace("          ENGINE_DIR: ${{ github.workspace }}/.engine\n",
      `          ENGINE_DIR: \${{ github.workspace }}/.engine\n          ENGINE_REF: ${"f".repeat(40)}\n`),
    "GITHUB_ENV で ENGINE_REF を上書きする": template.replace(testStep, `          echo "ENGINE_REF=${"f".repeat(40)}" >> "$GITHUB_ENV"\n${testStep}`),
    "Engine を取る版を書き込む": template.replace("ref: ${{ env.ENGINE_REF }}", `ref: ${engineRef(template)}`),
    "ENGINE_DIR を外す": template.replace(/^ {10}ENGINE_DIR: .*\n/m, ""),
    "別のディレクトリで走らせる": template.replace("working-directory: tools/pr-body-check", "working-directory: tools/sendback-check"),
    "engine.test.mjs を外す": template.replace("node --test --test-reporter=spec >", "node --test --test-reporter=spec check.test.mjs gate.test.mjs >"),
    "gate.test.mjs を外す": template.replace("node --test --test-reporter=spec >", "node --test --test-reporter=spec check.test.mjs engine.test.mjs ci.test.mjs >"),
    "テストの名前で (10) を外す": template.replace("node --test --test-reporter=spec >", 'node --test --test-reporter=spec --test-skip-pattern="\\(10\\)" >'),
    "テストの名前で絞る": template.replace("node --test --test-reporter=spec >", 'node --test --test-reporter=spec --test-name-pattern="^\\(3\\)" >'),
    "only のテストだけを走らせる": template.replace("node --test --test-reporter=spec >", "node --test --test-only --test-reporter=spec >"),
    "NODE_OPTIONS で絞る": template.replace("          ENGINE_DIR: ${{ github.workspace }}/.engine\n",
      "          ENGINE_DIR: ${{ github.workspace }}/.engine\n          NODE_OPTIONS: --test-skip-pattern=CLI\n"),
    "終了コードを捨てる": template.replace(/^ {10}exit \$rc\n/m, ""),
  };
  for (const [name, text] of Object.entries(cases)) {
    assert.notEqual(text, template, `${name}: 置き換えが効いていない`);
    assert.notDeepEqual(workflowProblems(text), [], `${name} が通った`);
  }
});

test("ひな形の ENGINE_REF は ci/sendback-engine.yml と同じ版（同じ Engine の本文の組み込みと判定を確かめる）", () => {
  assert.equal(engineRef(template), engineRef(sendbackTemplate),
    "ENGINE_REF が tools/sendback-check/ci/sendback-engine.yml とずれている。上げるときは両方のテストが通る版にそろえる");
});

test("GitHub Actions では写し（.github/workflows/pr-body-engine.yml）がなければ落ちる（別の名前に写すとずれを見張れない）", {
  skip: process.env.GITHUB_ACTIONS === "true" ? false : "GitHub Actions の外",
}, () => {
  assert.ok(existsSync(copyPath), "写しが .github/workflows/pr-body-engine.yml にない。ひな形はこの名前に写す");
});

test("写し（.github/workflows/pr-body-engine.yml）があれば、ENGINE_REF の行のほかはひな形と同じで、その版も見張りを通る", {
  skip: existsSync(copyPath) ? false : "まだ .github/workflows/ に写していない",
}, () => {
  const copy = readFileSync(copyPath, "utf8");
  assert.equal(withoutRef(copy), withoutRef(template), "写しが ENGINE_REF の行のほかでひな形とずれている。ひな形を直してから写し直す");
  assert.deepEqual(workflowProblems(copy), []);
  if (existsSync(sendbackCopyPath)) {
    assert.equal(engineRef(copy), engineRef(readFileSync(sendbackCopyPath, "utf8")),
      "写しの ENGINE_REF が .github/workflows/sendback-engine.yml とずれている");
  }
});

test("ずれの比べ方は ENGINE_REF の行だけを無視する", () => {
  assert.equal(withoutRef(template.replace(engineRef(template), "0".repeat(40))), withoutRef(template));
  assert.notEqual(withoutRef(template.replace(`name: ${CHECK_NAME}`, "name: engine-test")), withoutRef(template));
});

// 通る側のテスト（gate.test.mjs の関所と CLI、engine.test.mjs の (10)）が、ワークフローの node --test の実行の対象から外れていないことの見張り
// （知見 K-0001 の担保。議決 D-0045 の ci_check の案、Issue #1109）。ワークフローの側の道・名前・NODE_OPTIONS による絞り込みは workflowProblems で見るので、
// ここではテストのファイルの側で外していないかを見る。gate.test.mjs には飛ばしを置かない。engine.test.mjs の飛ばしは、ENGINE_DIR を渡すと
// false になる共通の skip（ワークフローは ENGINE_DIR を渡す）と、(10) が本物で確かめるようになったら飛ぶ (11) の差分の写しの飛ばしだけにする
// （Issue #1110）。どちらも todo・only・test.skip・t.skip などを置かない。注釈の行は見ない。
const GATE_TEST = "gate.test.mjs";
const ENGINE_TEST = "engine.test.mjs";
const readTest = (name) => readFileSync(fileURLToPath(new URL(`./${name}`, import.meta.url)), "utf8");
const ENGINE_REQUIRED = 'const required = Boolean(process.env.ENGINE_DIR) || process.env.CI === "true";';
const ENGINE_SKIP = /^const skip = required \|\| existsSync\(entrypointPath\) \? false : `[^`]*`;$/;
const PASSING_LOOP = "for (const [label, report] of PASSING_REPORTS) {";
// (11) の飛ばし。Engine が関所を呼ぶようになったときだけ飛ぶ（(10) は飛ばない）
const PATCHED_SKIP = '    { skip: skip || (engineWired ? "Engine がもう関所を呼ぶので、(10) が ENGINE_REF の Engine そのもので確かめる" : false) }, () => {';
const PATCHED_TEST = "  test(`(11) ";

/** 注釈の行を除いたコードの行。 */
const codeLines = (text) => text.split(/\r?\n/).filter((l) => !/^\s*(\/\/|\/\*|\*)/.test(l));

/** テストのファイルの側で、通る側のテストを実行の対象から外していないかの外れ（空なら通る）。 */
function exclusionProblems(name, text) {
  const problems = [];
  const lines = codeLines(text);
  if (lines.some((l) => /\b(test|it|describe|suite|t)\s*\.\s*(skip|todo|only)\b/.test(l))) problems.push(`${name}: test.skip・t.skip・todo・only でテストを外している`);
  if (lines.some((l) => /\b(todo|only)\b/.test(l))) problems.push(`${name}: todo か only の指定がある（todo は落ちても止めず、only は --test-only でほかを外す）`);
  const loop = lines.indexOf(PASSING_LOOP);
  if (loop < 0 || !/^\s+test\(/.test(lines[loop + 1] ?? "")) problems.push(`${name}: 通る側の 3 通りの形（PASSING_REPORTS）を回すテストがない`);
  if (name === GATE_TEST) {
    if (lines.some((l) => /\bskip\b/.test(l))) problems.push(`${name}: skip の指定がある（関所と CLI のテストは飛ばさない）`);
    if (lines.filter((l) => /^test\("CLI /.test(l)).length < 3) problems.push(`${name}: CLI のテスト（test("CLI …")）が 3 件ない`);
    if (!lines.some((l) => l.startsWith('test("CLI は正しく入った続きのジョブの報告の本文で 0 を返し'))) problems.push(`${name}: 通る側の CLI のテストがない`);
  }
  if (name === ENGINE_TEST) {
    if (!lines.includes(ENGINE_REQUIRED)) problems.push(`${name}: ENGINE_DIR を渡したときに飛ばさない required の行がない（${ENGINE_REQUIRED}）`);
    if (lines.filter((l) => /\b(const|let|var|function)\s+skip\b|\bskip\s*=[^=]/.test(l)).length !== 1 || !lines.some((l) => ENGINE_SKIP.test(l))) {
      problems.push(`${name}: skip は required のときに false になる 1 つの宣言だけにする（上書きや別の skip で (10) を飛ばせる）`);
    }
    // skip を使ってよいのは宣言と、テストの { skip } だけ（{ skip: true } などで個別に飛ばさない）
    // (11) の飛ばしは、(11) のテストの直後の行でだけ認める
    if (lines.some((l, i) => /\bskip\b/.test(l) && !ENGINE_SKIP.test(l) && !/^\s*test\(.*, \{ skip \}, (\(t\)|\(\)) => \{$/.test(l)
      && !(l === PATCHED_SKIP && (lines[i - 1] ?? "").startsWith(PATCHED_TEST)))) {
      problems.push(`${name}: 共通の skip のほかの飛ばしがある`);
    }
    if (!/^\s+test\(`\(10\) .*, \{ skip \}, \(\) => \{$/.test(lines[loop + 1] ?? "")) problems.push(`${name}: (10) が通る側の 3 通りの形を回すテストになっていない`);
  }
  return problems;
}

test("gate.test.mjs と engine.test.mjs の (10) は、node --test の実行の対象から外れていない（skip・todo・only・絞り込みがない）", () => {
  assert.deepEqual(exclusionProblems(GATE_TEST, readTest(GATE_TEST)), []);
  assert.deepEqual(exclusionProblems(ENGINE_TEST, readTest(ENGINE_TEST)), []);
  // 通る側の形を空にすると、ループのテストが 1 件も作られない
  assert.ok(PASSING_REPORTS.length >= 3, "通る側の形（c0005.mjs の PASSING_REPORTS）が 3 通りない");
});

test("gate.test.mjs と engine.test.mjs の (10) を外す・飛ばす直しは外れになる", () => {
  const gateText = readTest(GATE_TEST);
  const engineText = readTest(ENGINE_TEST);
  const gateLoopTest = "  test(`C-0005 の 21 回の続きのジョブでも、";
  const engineLoopTest = "  test(`(10) ";
  const cases = {
    [GATE_TEST]: {
      "通る側のテストを skip にする": gateText.replace(/(本文は毎回書いてよい（誤検知がない）`), \(\) =>/, "$1, { skip: true }, () =>"),
      "通る側のテストを todo にする": gateText.replace(/(本文は毎回書いてよい（誤検知がない）`), \(\) =>/, "$1, { todo: true }, () =>"),
      "通る側のテストを test.skip にする": gateText.replace(gateLoopTest, "  test.skip(`C-0005 の 21 回の続きのジョブでも、"),
      "CLI のテストを test.todo にする": gateText.replace('test("CLI は書いてよければ 0', 'test.todo("CLI は書いてよければ 0'),
      "CLI のテストを条件で飛ばす": gateText.replace('test("CLI は正しく入った続きのジョブの報告の本文で 0 を返し、body_fix の入力を出さない", () =>',
        'test("CLI は正しく入った続きのジョブの報告の本文で 0 を返し、body_fix の入力を出さない", { skip: !process.env.CI }, () =>'),
      "テストの中で t.skip を呼ぶ": gateText.replace('test("新しい PR の本文は書いてよい", () => {', 'test("新しい PR の本文は書いてよい", (t) => {\n  t.skip();'),
      "only を付ける": gateText.replace('test("新しい PR の本文は書いてよい", () =>', 'test("新しい PR の本文は書いてよい", { only: true }, () =>'),
      "通る側のループを外す": gateText.replace(PASSING_LOOP, "for (const [label, report] of PASSING_REPORTS.slice(0, 0)) {\n  if (false)"),
      "通る側の CLI のテストを消す": gateText.replace('test("CLI は正しく入った続きのジョブの報告の本文で 0 を返し', 'void (() => ("CLI は正しく入った続きのジョブの報告の本文で 0 を返し'),
    },
    [ENGINE_TEST]: {
      "(10) を skip にする": engineText.replace(/(誤って止めない）`), \{ skip \}, \(\) =>/, "$1, { skip: true }, () =>"),
      "(10) を todo にする": engineText.replace(/(誤って止めない）`), \{ skip \}, \(\) =>/, "$1, { skip, todo: true }, () =>"),
      "(10) を test.skip にする": engineText.replace(engineLoopTest, "  test.skip(`(10) "),
      "(10) の中で t.skip を呼ぶ": engineText.replace(/(誤って止めない）`, \{ skip \}, )\(\) => \{/, "$1(t) => {\n    return t.skip();"),
      "(10) だけ別の skip で飛ばす": engineText.replace(PASSING_LOOP, `${PASSING_LOOP}\n  const skip = "まだ";`),
      "ENGINE_DIR を渡しても飛ばす": engineText.replace(ENGINE_REQUIRED, 'const required = process.env.CI === "true";'),
      "skip を常に飛ばす形にする": engineText.replace(/^const skip = required \|\| /m, "const skip = "),
      "(10) を回すループを外す": engineText.replace(PASSING_LOOP, "for (const [label, report] of []) {"),
      "(10) に (11) の飛ばしを付ける": engineText.replace(/(\(10\) [^\n]*誤って止めない）`), \{ skip \}, \(\) => \{/, `$1,\n${PATCHED_SKIP}`),
    },
  };
  for (const [file, byName] of Object.entries(cases)) {
    const original = file === GATE_TEST ? gateText : engineText;
    for (const [name, text] of Object.entries(byName)) {
      assert.notEqual(text, original, `${file} ${name}: 置き換えが効いていない`);
      assert.notDeepEqual(exclusionProblems(file, text), [], `${file} ${name} が通った`);
    }
  }
});

// 通る側のテストが実際に走って pass と数えられたかの見張り（知見 K-0001 の担保。議決 D-0070（Issue #1183 の起票時の番号は D-0067。PR #1181）の
// test の案、Issue #1183）。exclusionProblems はテストのファイルの字面を見るだけなので、列挙していない飛ばし方（別名で呼ぶ t.skip、describe.skip、
// ENGINE_DIR が空の文字列で渡るなど）はすり抜けうる。gate.test.mjs と engine.test.mjs を node --test --test-reporter=tap で走らせ、その結果で、
// 通る側のテストが名前のとおりにあって pass（SKIP・TODO の指示のない ok）と数えられ、ファイル全体の skip・todo の数が 0 かを見る。
// engine.test.mjs は (10) のほかが落ちても（関所を呼ばない Engine の (3) など）ここでは見ない（同じ node --test で落ちる）。
const RUN_TOOL_DIR = fileURLToPath(new URL("./", import.meta.url));
const runEngineDir = process.env.ENGINE_DIR || fileURLToPath(new URL("../../../engine", import.meta.url));
// engine.test.mjs と同じ飛ばし方（ENGINE_DIR を渡したときと CI=true では飛ばさない）
const runEngineSkip = Boolean(process.env.ENGINE_DIR) || process.env.CI === "true" || existsSync(`${runEngineDir}/orchestrator/worker/entrypoint.sh`)
  ? false : `Engine のチェックアウトがない（ENGINE_DIR=${runEngineDir}）`;

/** gate.test.mjs の通る側のテストの名前（書いてよい本文の関所、通る側の 3 通りの形、通る側の CLI）。 */
const GATE_PASSING_NAMES = [
  "新しい PR の本文は書いてよい",
  "続きのジョブの記録が「## 工場の記録」の下の ### にあれば書いてよい",
  ...PASSING_REPORTS.map(([label]) => `C-0005 の 21 回の続きのジョブでも、${label}本文は毎回書いてよい（誤検知がない）`),
  "CLI は正しく入った続きのジョブの報告の本文で 0 を返し、body_fix の入力を出さない",
];
/** engine.test.mjs の (10) の名前（通る側の 3 通りの形）。 */
const ENGINE_PASSING_NAMES = PASSING_REPORTS.map(([label]) =>
  `(10) ENGINE_REF の Engine は、C-0005 の 21 回の続きのジョブで${label}なら、毎回関所が 0 を返して本文を書く（誤って止めない）`);
/** engine.test.mjs の (11) の名前。Engine が関所を呼ぶようになったら飛ぶので、その skip は集計から除く。 */
const ENGINE_PATCHED_NAMES = PASSING_REPORTS.map(([label]) =>
  `(11) 関所の配線の差分を当てた Engine の写しは、C-0005 の 21 回の続きのジョブで${label}なら、毎回関所が 0 を返して本文を書く（誤って止めない）`);

/** テストのファイルを node --test の TAP で走らせた出力。外の node --test の NODE_TEST_CONTEXT を渡すと TAP で出さないので外す。終了コードは見ない。 */
function runTap(file, cwd = RUN_TOOL_DIR) {
  const { NODE_TEST_CONTEXT, ...env } = process.env;
  const r = spawnSync(process.execPath, ["--test", "--test-reporter=tap", file], { cwd, env, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
  return `${r.stdout ?? ""}${r.error ? `\n# spawn error: ${r.error.message}` : ""}`;
}

/** TAP の終わりの集計（# skipped N・# todo N。古い node は # skip N）。なければ null。 */
const tapCount = (tap, ...keys) => {
  const m = tap.match(new RegExp(`^# (?:${keys.join("|")}) (\\d+)$`, "m"));
  return m ? Number(m[1]) : null;
};

/** 走らせた結果で、通る側のテストが pass と数えられず、skip・todo が 0 でない（maySkip の名前のテストの skip は除く）ときの外れ（空なら通る）。 */
function runReportProblems(file, tap, names, maySkip = []) {
  const problems = [];
  const results = parseTapReport(tap);
  for (const name of names) {
    const got = results[name];
    if (got !== "pass") problems.push(`${file}: 「${name}」が pass と数えられていない（${got ?? "実行されていない"}）`);
  }
  const allowedSkips = maySkip.filter((name) => results[name] === "skip").length;
  for (const [label, keys] of [["skip", ["skipped", "skip"]], ["todo", ["todo"]]]) {
    const counted = tapCount(tap, ...keys);
    const n = counted === null || label !== "skip" ? counted : counted - allowedSkips;
    if (n === null) problems.push(`${file}: TAP に ${label} の数がない（node --test が最後まで走っていない）`);
    else if (n !== 0) problems.push(`${file}: ${label} の数が ${n}（0 でない）`);
  }
  return problems;
}

test("gate.test.mjs を TAP で走らせると、通る側のテストが pass と数えられ、skip・todo が 0", () => {
  const tap = runTap(GATE_TEST);
  assert.deepEqual(runReportProblems(GATE_TEST, tap, GATE_PASSING_NAMES), [], tap);
});

test("engine.test.mjs を TAP で走らせると、(10) が通る側の 3 通りの形で pass と数えられ、skip・todo が 0", { skip: runEngineSkip }, () => {
  assert.ok(PASSING_REPORTS.length >= 3, "通る側の形（c0005.mjs の PASSING_REPORTS）が 3 通りない");
  const tap = runTap(ENGINE_TEST);
  assert.deepEqual(runReportProblems(ENGINE_TEST, tap, ENGINE_PASSING_NAMES, ENGINE_PATCHED_NAMES), [], tap);
});

test("走らせた結果の見張りは、(10) や通る側のテストの skip・todo・失敗・欠け、ほかのテストの skip・todo、途中で止まった TAP を外れにする", () => {
  const esc = (s) => s.replace(/[#\\]/g, "\\$&");
  const tap = (lines, { skipped = 0, todo = 0 } = {}) =>
    ["TAP version 13", ...lines.map((l, i) => l.replace("ok -", `ok ${i + 1} -`)), `1..${lines.length}`, `# tests ${lines.length}`, `# skipped ${skipped}`, `# todo ${todo}`].join("\n");
  const okAll = ENGINE_PASSING_NAMES.map((n) => `ok - ${esc(n)}`);
  const [first, ...rest] = ENGINE_PASSING_NAMES;
  assert.deepEqual(runReportProblems(ENGINE_TEST, tap([...okAll, "not ok - (3) 関所を呼ばない Engine で落ちる"]), ENGINE_PASSING_NAMES), []);
  // Engine が関所を呼ぶようになって (11) が飛ぶのは外れにしない（Issue #1110）
  const patchedSkipped = ENGINE_PATCHED_NAMES.map((n) => `ok - ${esc(n)} # SKIP Engine がもう関所を呼ぶ`);
  assert.deepEqual(runReportProblems(ENGINE_TEST, tap([...okAll, ...patchedSkipped], { skipped: patchedSkipped.length }),
    ENGINE_PASSING_NAMES, ENGINE_PATCHED_NAMES), []);
  assert.notDeepEqual(runReportProblems(ENGINE_TEST, tap([...okAll, ...patchedSkipped], { skipped: patchedSkipped.length + 1 }),
    ENGINE_PASSING_NAMES, ENGINE_PATCHED_NAMES), [], "(11) のほかの skip が通った");
  assert.notDeepEqual(runReportProblems(ENGINE_TEST, tap([`ok - ${esc(first)} # SKIP まだ`, ...rest.map((n) => `ok - ${esc(n)}`)], { skipped: 1 }),
    ENGINE_PASSING_NAMES, ENGINE_PATCHED_NAMES), [], "(10) の skip が通った");
  const cases = {
    "(10) を SKIP にした": tap([`ok - ${esc(first)} # SKIP まだ`, ...rest.map((n) => `ok - ${esc(n)}`)], { skipped: 1 }),
    "(10) を TODO にした": tap([`not ok - ${esc(first)} # TODO まだ`, ...rest.map((n) => `ok - ${esc(n)}`)], { todo: 1 }),
    "(10) が落ちた": tap([`not ok - ${esc(first)}`, ...rest.map((n) => `ok - ${esc(n)}`)]),
    "(10) が 1 つ走っていない": tap(rest.map((n) => `ok - ${esc(n)}`)),
    "(10) を describe の下に入れた（上の段にない）": tap([`ok - 包み`, ...ENGINE_PASSING_NAMES.map((n) => `    ok - ${esc(n)}`)]),
    "ほかのテストを飛ばした（集計の skip）": tap(okAll, { skipped: 1 }),
    "ほかのテストを todo にした（集計の todo）": tap(okAll, { todo: 1 }),
    "集計がない（途中で止まった）": okAll.map((l, i) => l.replace("ok -", `ok ${i + 1} -`)).join("\n"),
  };
  for (const [name, text] of Object.entries(cases)) assert.notDeepEqual(runReportProblems(ENGINE_TEST, text, ENGINE_PASSING_NAMES), [], `${name} が通った`);
});

test("字面の見張りをすり抜ける飛ばし方（別名で呼ぶ t.skip）も、走らせた結果で外れになる", () => {
  const dir = mkdtempSync(join(tmpdir(), "pr-body-run-"));
  for (const f of ["c0005.mjs", "check.mjs", "gate.mjs", "template.json"]) copyFileSync(join(RUN_TOOL_DIR, f), join(dir, f));
  const loopTest = /(本文は毎回書いてよい（誤検知がない）`), \(\) => \{/;
  const changed = readTest(GATE_TEST).replace(loopTest, '$1, (t) => {\n    if (label) return t["sk" + "ip"]("まだ");');
  assert.notEqual(changed, readTest(GATE_TEST), "置き換えが効いていない");
  assert.deepEqual(exclusionProblems(GATE_TEST, changed), [], "字面の見張りが捕まえた（すり抜けの例になっていない）");
  writeFileSync(join(dir, GATE_TEST), changed);
  const tap = runTap(GATE_TEST, dir);
  const problems = runReportProblems(GATE_TEST, tap, GATE_PASSING_NAMES);
  assert.equal(problems.filter((p) => /pass と数えられていない（skip）/.test(p)).length, PASSING_REPORTS.length, tap);
  assert.ok(problems.includes(`${GATE_TEST}: skip の数が ${PASSING_REPORTS.length}（0 でない）`), tap);
});

// recheck.test.mjs（知見 K-0012 の担保）も、ひな形の engine-test の node --test で走る（議決 D-0050 の ci_check の案、Issue #1131）。
// ワークフローの側の絞り込みは workflowProblems で見るので、ここではテストのファイルの側で外していないか（skip・todo・only がない、
// 事例 C-0003・C-0005・C-0006 の列が落ちるテストが残っている）を見る。注釈の行は見ない。
const RECHECK_TEST = "recheck.test.mjs";
const RECHECK_CASES = ["C-0003", "C-0005", "C-0006"];

/** recheck.test.mjs を実行の対象から外していないかの外れ（空なら通る）。 */
function recheckExclusionProblems(text) {
  const problems = [];
  const lines = codeLines(text);
  if (lines.some((l) => /\b(skip|todo|only)\b/.test(l))) problems.push(`${RECHECK_TEST}: skip・todo・only の指定がある`);
  for (const id of RECHECK_CASES) {
    if (!lines.some((l) => l.startsWith(`test("${id} の列は、`))) problems.push(`${RECHECK_TEST}: ${id} の列が落ちるテスト（test("${id} の列は、…")）がない`);
  }
  return problems;
}

test("recheck.test.mjs は、node --test の実行の対象から外れていない（skip・todo・only がなく、事例の列のテストがある）", () => {
  assert.deepEqual(recheckExclusionProblems(readTest(RECHECK_TEST)), []);
});

test("recheck.test.mjs を外す・飛ばす直しは外れになる", () => {
  const text = readTest(RECHECK_TEST);
  const cases = {
    "C-0003 のテストを skip にする": text.replace('test("C-0003 の列は、最後の本文の直し（job 572）のあと検査を通さずにマージしたので落ちる", () =>',
      'test("C-0003 の列は、最後の本文の直し（job 572）のあと検査を通さずにマージしたので落ちる", { skip: true }, () =>'),
    "C-0006 のテストを test.todo にする": text.replace('test("C-0006 の列は、', 'test.todo("C-0006 の列は、'),
    "only を付ける": text.replace('test("C-0005 の列は、本文の直しが終わる前にマージしたので落ちる", () =>', 'test("C-0005 の列は、本文の直しが終わる前にマージしたので落ちる", { only: true }, () =>'),
    "テストの中で t.skip を呼ぶ": text.replace('test("CLI: 終了コード 0・1・2 と --json", () => {', 'test("CLI: 終了コード 0・1・2 と --json", (t) => {\n  t.skip();'),
    "C-0003 のテストを消す": text.replace('test("C-0003 の列は、', 'void (() => ("C-0003 の列は、'),
  };
  for (const [name, changed] of Object.entries(cases)) {
    assert.notEqual(changed, text, `${name}: 置き換えが効いていない`);
    assert.notDeepEqual(recheckExclusionProblems(changed), [], `${name} が通った`);
  }
});

// 必須のチェックが後で外されたことに気づく（議決 D-0044、Issue #1106）。このテストは守りたい engine-test の中で走るので、写しが消えたり
// 必須から外れたりしたときは自分も走らない。ci/pr-body-protection.yml が branch protection を読めるトークンで、既定のブランチへの push と
// 毎日と手動で照らし、外れたら落ちて Issue を開くか更新する。手順を偽の gh で動かして確かめる。
const protTemplate = readFileSync(fileURLToPath(new URL("./ci/pr-body-protection.yml", import.meta.url)), "utf8");
const protCopyPath = fileURLToPath(new URL("../../.github/workflows/pr-body-protection.yml", import.meta.url));
const protSteps = jobSteps(jobBody(protTemplate.split(/\r?\n/), "protection") ?? []);
const protReadStep = protSteps.find((s) => /branch protection を読む/.test(s[0]));
const protCheckStep = protSteps.find((s) => /必須のチェックを照らす/.test(s[0]));
const protIssueStep = protSteps.find((s) => /外れたら Issue を開くか更新する/.test(s[0]));
const ISSUE_TITLE = protTemplate.match(/^ {2}ISSUE_TITLE: "(.+)"$/m)?.[1];

/** 手順の `run: |` の中身（字下げを外したもの）。 */
function runScript(step) {
  const i = step.findIndex((l) => /^\s*run: \|\s*$/.test(l));
  assert.ok(i >= 0, "run: | の手順ではない");
  return step.slice(i + 1).filter((l) => l.trim() !== "").map((l) => l.replace(/^ {10}/, "")).join("\n") + "\n";
}

/** 古い形の branch protection の JSON（protection.test.mjs と同じ形） */
const classicJson = (contexts) => JSON.stringify({ required_status_checks: { strict: true, contexts, checks: contexts.map((context) => ({ context })) } });
const rulesJson = (contexts) => JSON.stringify(contexts.length === 0 ? []
  : [{ type: "required_status_checks", parameters: { required_status_checks: contexts.map((context) => ({ context })) } }]);
const NOT_PROTECTED = '{"message":"Branch not protected","status":"404"}';

/**
 * 偽の gh。classic・rules は gh api の branch protection と rulesets の応答（{ out, rc }）、open は開いている Issue の list の出力、
 * recorded は Issue の本文とコメント、create は gh issue create の終了コード。
 */
function fakeProtGh({ classic = { out: classicJson([CHECK_NAME]) }, rules = { out: "[]" }, open = "", recorded = "", create = 0 } = {}) {
  const q = (v) => `'${String(v).replace(/'/g, "'\\''")}'`;
  return [
    `case "$1 $2" in`,
    `  "api repos/guccyon/rally/branches/main/protection") printf '%s' ${q(classic.out)}; exit ${classic.rc ?? 0} ;;`,
    `  "api repos/guccyon/rally/rules/branches/main?per_page=100") printf '%s' ${q(rules.out)}; exit ${rules.rc ?? 0} ;;`,
    `  "issue list") [ -n ${q(open)} ] && printf '%s\\n' ${q(open)}; exit 0 ;;`,
    `  "issue create") exit ${create} ;;`,
    `  api\\ *) printf '%s\\n' ${q(recorded)}; exit 0 ;;`,
    `esac`,
    `exit 0`,
  ].join("\n");
}

/**
 * 仮のリポジトリに protection.mjs（と読み方を借りる tools/sendback-check の 2 つ）と、写し・ほかのワークフローを置き、
 * 読む・照らす・Issue の 3 つの手順を順に動かす。copy が null なら写しを置かない。
 */
function runProtection({ gh = {}, copy = template, others = {}, token = "t" } = {}) {
  const { root, dir, runStep } = protSandbox({ gh, copy, others });
  const read = runStep(protReadStep, root, { GH_TOKEN: token });
  const check = read.status === 0 ? runStep(protCheckStep, dir, {}) : null;
  const outcome = check === null ? "skipped" : check.status === 0 ? "success" : "failure";
  const issue = runStep(protIssueStep, dir, { GH_TOKEN: "github-token", CHECK_OUTCOME: outcome });
  const calls = existsSync(join(root, "calls.log")) ? readFileSync(join(root, "calls.log"), "utf8") : "";
  const commentPath = join(dir, "protection-comment.md");
  return { read, check, issue, calls, body: existsSync(commentPath) ? readFileSync(commentPath, "utf8") : "" };
}

/** 偽の gh と protection.mjs を置いた仮のリポジトリと、そこで手順の run を動かす runStep(step, cwd, env) を返す。 */
function protSandbox({ gh = {}, copy = template, others = {} } = {}) {
  const root = mkdtempSync(join(tmpdir(), "pr-body-protection-"));
  const dir = join(root, "tools", "pr-body-check");
  const sendback = join(root, "tools", "sendback-check");
  mkdirSync(dir, { recursive: true });
  mkdirSync(sendback, { recursive: true });
  mkdirSync(join(root, ".github", "workflows"), { recursive: true });
  writeFileSync(join(dir, "protection.mjs"), readFileSync(fileURLToPath(new URL("./protection.mjs", import.meta.url))));
  for (const f of ["protection.mjs", "order.mjs"]) {
    writeFileSync(join(sendback, f), readFileSync(fileURLToPath(new URL(`../sendback-check/${f}`, import.meta.url))));
  }
  if (copy != null) writeFileSync(join(root, ".github", "workflows", "pr-body-engine.yml"), copy);
  for (const [name, text] of Object.entries(others)) writeFileSync(join(root, ".github", "workflows", name), text);
  const bin = join(root, "bin");
  mkdirSync(bin);
  writeFileSync(join(bin, "gh"), `#!/usr/bin/env bash\necho "gh $*" >> "${join(root, "calls.log")}"\n${fakeProtGh(gh)}\n`);
  chmodSync(join(bin, "gh"), 0o755);
  const env = { ...process.env, PATH: `${bin}:${process.env.PATH}`, GITHUB_REPOSITORY: "guccyon/rally", GITHUB_SERVER_URL: "https://github.com",
    GITHUB_RUN_ID: "123", BRANCH: "main", ISSUE_TITLE, GITHUB_STEP_SUMMARY: join(root, "summary"), GITHUB_ACTIONS: "true" };
  const runStep = (step, cwd, extra) => spawnSync("bash", ["-e", "-o", "pipefail", "-c", runScript(step)], { cwd, env: { ...env, ...extra }, encoding: "utf8" });
  return { root, dir, runStep };
}

test("pr-body-protection は既定のブランチへの push・定期（schedule）・手動でだけ走り、PR では走らない。トークンは branch protection を読む手順だけに渡す", () => {
  const lines = protTemplate.split(/\r?\n/).filter((l) => !/^\s*#/.test(l));
  const on = lines.findIndex((l) => /^on:\s*$/.test(l));
  const onBody = lines.slice(on + 1, lines.findIndex((l, i) => i > on && /^\S/.test(l))).filter((l) => l.trim() !== "");
  assert.deepEqual(onBody.filter((l) => /^ {2}[\w-]+:/.test(l)).map((l) => l.trim()), ["push:", "schedule:", "workflow_dispatch:"],
    "push・schedule・workflow_dispatch のほかで走らせない");
  assert.ok(onBody.some((l) => /^ {4}branches: \[main\]$/.test(l)), "push は既定のブランチ（main）だけにする");
  assert.ok(!onBody.some((l) => /^ {4}(paths|paths-ignore|tags|branches-ignore):/.test(l)), "push を道やタグで絞らない");
  assert.ok(onBody.some((l) => /^ {4}- cron: "[^"]+"$/.test(l)), "schedule の cron がない");
  assert.ok(!lines.some((l) => /^\s*(- )?continue-on-error:/.test(l)), "continue-on-error があると外れても気づかない");
  assert.match(protTemplate, /^ {2}contents: read$/m);
  assert.match(protTemplate, /^ {2}issues: write$/m, "Issue を開くので issues: write が要る");
  const secretLines = lines.filter((l) => /secrets\./.test(l));
  assert.deepEqual(secretLines.map((l) => l.trim()), ["GH_TOKEN: ${{ secrets.PR_BODY_PROTECTION_TOKEN }}"]);
  assert.ok(protReadStep?.some((l) => /secrets\.PR_BODY_PROTECTION_TOKEN/.test(l)), "トークンを渡すのは branch protection を読む手順だけ");
  assert.ok(!jobBody(lines, "protection").some((l) => /^ {4}if:/.test(l)), "ジョブを飛ばさない");
  assert.ok(!protReadStep.some((l) => /^\s*if:/.test(l)) && !protCheckStep.some((l) => /^\s*if:/.test(l)), "読む・照らす手順を飛ばさない");
  assert.ok(protCheckStep.some((l) => /^\s*id: check$/.test(l)));
  assert.ok(protCheckStep.some((l) => /^\s*working-directory: tools\/pr-body-check$/.test(l)));
  assert.match(runScript(protCheckStep), /node protection\.mjs --protection \S+ --protection \S+/, "古い形と rulesets の両方を渡す");
  assert.ok(protIssueStep.some((l) => /^\s*if: always\(\)$/.test(l)), "Issue の手順は照らす手順が落ちたときも走らせる");
  assert.ok(protIssueStep.some((l) => /^\s*CHECK_OUTCOME: \$\{\{ steps\.check\.outcome \}\}$/.test(l)));
  assert.ok(protSteps.indexOf(protReadStep) < protSteps.indexOf(protCheckStep) && protSteps.indexOf(protCheckStep) < protSteps.indexOf(protIssueStep));
  assert.ok(ISSUE_TITLE, "ISSUE_TITLE がない");
  // このジョブのチェックの名前は pr-body-engine-test と重ならない（重なると check_name_shared で自分が外れる）
  assert.notEqual(jobCheckNames(protTemplate).get("protection"), CHECK_NAME);
});

test("pr-body-protection: 必須のチェックがそろっていれば通り、Issue は開かない（写しも並べて置いたままで通る）", () => {
  const r = runProtection({ others: { "pr-body-protection.yml": protTemplate, "sendback-engine.yml": sendbackTemplate } });
  assert.equal(r.read.status, 0, r.read.stdout + r.read.stderr);
  assert.equal(r.check.status, 0, r.check.stdout + r.check.stderr);
  assert.equal(r.issue.status, 0, r.issue.stdout + r.issue.stderr);
  assert.doesNotMatch(r.calls, /^gh issue (create|comment|close)/m);
});

test("pr-body-protection: rulesets だけで必須にしていても通る（古い形は保護なしの 404）", () => {
  const r = runProtection({ gh: { classic: { out: NOT_PROTECTED, rc: 1 }, rules: { out: rulesJson([CHECK_NAME]) } } });
  assert.equal(r.read.status, 0, r.read.stdout + r.read.stderr);
  assert.equal(r.check.status, 0, r.check.stdout + r.check.stderr);
  assert.doesNotMatch(r.calls, /^gh issue (create|comment)/m);
});

test("pr-body-protection: 4 つの外れ方のどれでも落ち、開いている Issue がなければ開く", () => {
  const cases = {
    workflow_not_copied: { copy: null },
    check_name_changed: { copy: template.replace(`    name: ${CHECK_NAME}\n`, "    name: pr-body-engine\n"), gh: { classic: { out: classicJson(["pr-body-engine"]) } } },
    required_check_missing: { gh: { classic: { out: classicJson(["engine-test"]) } } },
    check_name_shared: { others: { "x.yml": `name: x\non:\n  pull_request:\njobs:\n  fake:\n    name: ${CHECK_NAME}\n    runs-on: ubuntu-latest\n    steps:\n      - run: "true"\n` } },
  };
  for (const [rule, opts] of Object.entries(cases)) {
    const r = runProtection(opts);
    assert.equal(r.check.status, 1, rule);
    assert.match(r.check.stdout, new RegExp(`^::error title=pr-body-check ${rule}::`, "m"), rule);
    assert.equal(r.issue.status, 0, r.issue.stdout + r.issue.stderr);
    assert.match(r.calls, new RegExp(`^gh issue create --repo guccyon/rally --title ${ISSUE_TITLE.replace(/[()（）:]/g, ".")} --body-file protection-comment\\.md$`, "m"), rule);
    assert.match(r.body, /^<!-- pr-body-protection: [0-9a-f]{16} -->\n/, rule);
    assert.match(r.body, new RegExp(`^${rule}: `, "m"), rule);
    assert.match(r.body, /https:\/\/github\.com\/guccyon\/rally\/actions\/runs\/123/);
  }
});

test("pr-body-protection: 保護ごと外された（古い形が 404・rulesets も空）ときも required_check_missing で Issue を開く", () => {
  const r = runProtection({ gh: { classic: { out: NOT_PROTECTED, rc: 1 } } });
  assert.equal(r.check.status, 1);
  assert.match(r.body, /^required_check_missing: engine-test/m);
  assert.match(r.calls, /^gh issue create /m);
});

test("pr-body-protection: 開いている Issue があれば、それにコメントで足す（同じ外れ方の記録がすでにあれば足さない）", () => {
  const missing = { classic: { out: classicJson(["engine-test"]) } };
  const first = runProtection({ gh: { ...missing, open: "42", recorded: "前の記録" } });
  assert.doesNotMatch(first.calls, /^gh issue create/m);
  assert.match(first.calls, /^gh issue comment 42 --repo guccyon\/rally --body-file protection-comment\.md$/m);
  const marker = first.body.split("\n")[0];
  const again = runProtection({ gh: { ...missing, open: "42", recorded: `本文\n${marker}` } });
  assert.equal(again.issue.status, 0, again.issue.stdout + again.issue.stderr);
  assert.doesNotMatch(again.calls, /^gh issue (create|comment)/m);
  // 外れ方が変われば足す
  const other = runProtection({ copy: null, gh: { open: "42", recorded: marker } });
  assert.match(other.calls, /^gh issue comment 42 /m);
});

test("pr-body-protection: 通ったら、開いている Issue に通ったことを書いて閉じる", () => {
  const r = runProtection({ gh: { open: "42" } });
  assert.equal(r.issue.status, 0, r.issue.stdout + r.issue.stderr);
  assert.match(r.calls, /^gh issue comment 42 --repo guccyon\/rally --body-file protection-comment\.md$/m);
  assert.match(r.calls, /^gh issue close 42 --repo guccyon\/rally$/m);
  assert.match(r.body, /必須のチェックに戻った/);
});

test("pr-body-protection: トークンがない・branch protection を読めないときは protection_unreadable で落ち、照らさず Issue も開かない", () => {
  for (const [name, opts] of [["トークンがない", { token: "" }],
    ["権限がない（403）", { gh: { classic: { out: '{"message":"Resource not accessible by personal access token","status":"403"}', rc: 1 } } }],
    ["rulesets を読めない", { gh: { rules: { out: '{"message":"Not Found"}', rc: 1 } } }]]) {
    const r = runProtection(opts);
    assert.equal(r.read.status, 1, name);
    assert.match(r.read.stdout, /^::error title=pr-body-protection protection_unreadable::/m, name);
    assert.equal(r.check, null);
    assert.equal(r.issue.status, 0, name);
    assert.doesNotMatch(r.calls, /^gh issue (create|comment)/m, name);
  }
});

test("pr-body-protection: Issue を開けなければ Issue の手順も落ちる（黙って通さない）", () => {
  const r = runProtection({ gh: { classic: { out: classicJson([]) }, create: 1 } });
  assert.notEqual(r.issue.status, 0);
});

test("写し（.github/workflows/pr-body-protection.yml）があれば、ひな形と同じ", {
  skip: existsSync(protCopyPath) ? false : "まだ .github/workflows/ に写していない",
}, () => {
  assert.equal(readFileSync(protCopyPath, "utf8").trimEnd(), protTemplate.trimEnd(), "写しがひな形とずれている。ひな形を直してから写し直す");
});

// 定期の照合そのものが黙って止まる外れ方を塞ぐ（議決 D-0068 の test の案。Issue #1180 の起票時の番号は D-0066。PR #1176）。
// 写しがひな形と同じかは上のテストで見るが、ひな形と写しを一緒に変えれば通る。写しから schedule や workflow_dispatch を外したり、
// トークンを読めないときに飛ばして緑にしたりすると、必須のチェックが外されても誰も気づかないので、写しそのものの形と読む手順の振る舞いを見る。
/** 定期・手動で走り、トークンを読めないときに protection_unreadable で落ちる形かの外れ（空なら通る）。注釈の行は見ない。 */
function protectionRunProblems(text) {
  const problems = [];
  const lines = text.split(/\r?\n/).filter((l) => !/^\s*#/.test(l));
  const on = lines.findIndex((l) => /^on:\s*$/.test(l));
  const onBody = on < 0 ? [] : lines.slice(on + 1, lines.findIndex((l, i) => i > on && /^\S/.test(l)));
  const onKeys = onBody.filter((l) => /^ {2}[\w-]+:/.test(l)).map((l) => l.trim().replace(/:.*$/, ""));
  const schedule = onBody.findIndex((l) => /^ {2}schedule:\s*$/.test(l));
  const scheduleBody = schedule < 0 ? [] : onBody.slice(schedule + 1, onBody.findIndex((l, i) => i > schedule && /^ {2}\S/.test(l)) >>> 0);
  if (!scheduleBody.some((l) => /^ {4}- cron: (["'])\S+(\s+\S+){4}\1\s*$/.test(l))) {
    problems.push("on の schedule に cron がない（定期に照らさないと、PR のない日に必須のチェックが外されても気づけない）");
  }
  if (!onKeys.includes("workflow_dispatch")) problems.push("on に workflow_dispatch がない（写したあとに手で走らせて確かめられない）");
  if (lines.some((l) => /^\s*(- )?continue-on-error:/.test(l))) problems.push("continue-on-error があると、読めなくても落ちない");
  const job = jobBody(lines, "protection");
  if (job === null) return [...problems, "protection のジョブがない"];
  if (job.some((l) => /^ {4}if:/.test(l))) problems.push("protection のジョブに if: があると、定期の照合が飛ばされうる");
  const read = jobSteps(job).find((s) => s.some((l) => /secrets\.PR_BODY_PROTECTION_TOKEN/.test(l)));
  if (!read) return [...problems, "PR_BODY_PROTECTION_TOKEN を読む手順がない"];
  if (read.some((l) => /^\s*(- )?if:/.test(l))) problems.push("トークンを読む手順に if: があると、読めないときに飛ばされうる");
  const tokenVar = read.map((l) => l.match(/^\s*(\w+): \$\{\{ secrets\.PR_BODY_PROTECTION_TOKEN \}\}\s*$/)?.[1]).find(Boolean);
  if (!tokenVar) return [...problems, "PR_BODY_PROTECTION_TOKEN は読む手順の env にそのまま渡す"];
  if (!read.some((l) => /^\s*run: \|\s*$/.test(l))) return [...problems, "トークンを読む手順が run: | でない"];
  // 本物の gh はトークンがないと認証の誤りで終了コード 4 を返す
  const cases = [
    ["トークンがない", "", { classic: { out: "", rc: 4 }, rules: { out: "", rc: 4 } }],
    ["権限がない（403）", "t", { classic: { out: '{"message":"Resource not accessible by personal access token","status":"403"}', rc: 1 } }],
    ["rulesets を読めない", "t", { rules: { out: '{"message":"Not Found"}', rc: 1 } }],
  ];
  for (const [name, token, gh] of cases) {
    const { root, runStep } = protSandbox({ gh });
    const r = runStep(read, root, { [tokenVar]: token });
    if (r.status === 0 || !/^::error title=pr-body-protection protection_unreadable::/m.test(r.stdout)) {
      problems.push(`${name}ときに protection_unreadable で落ちない（終了コード ${r.status}）`);
    }
  }
  return problems;
}

test("pr-body-protection のひな形は定期・手動で走り、トークンを読めないときは protection_unreadable で飛ばさず落ちる", () => {
  assert.deepEqual(protectionRunProblems(protTemplate), []);
});

test("pr-body-protection の定期・手動の on を外す・読めないときに飛ばす直しは外れになる", () => {
  const readStep = "      - name: branch protection を読む（K-0001）\n";
  const mutants = {
    "schedule を外す": (t) => t.replace(/^ {2}schedule:\n {4}- cron: "[^"]+"\n/m, ""),
    "cron を空にする": (t) => t.replace(/^( {4}- cron: )"[^"]+"$/m, '$1""'),
    "workflow_dispatch を外す": (t) => t.replace(/^ {2}workflow_dispatch:\n/m, ""),
    "on を 1 行の列にする": (t) => t.replace(/^on:\n(?: {2}.*\n)+/m, "on: [push]\n"),
    "ジョブに if: を足す": (t) => t.replace(/^( {4}runs-on: ubuntu-latest\n)/m, "$1    if: github.event_name != 'schedule'\n"),
    "読む手順に if: を足す": (t) => t.replace(readStep, `${readStep}        if: env.HAS_TOKEN == 'true'\n`),
    "読む手順に continue-on-error を足す": (t) => t.replace(readStep, `${readStep}        continue-on-error: true\n`),
    "トークンがないときは警告にして飛ばす": (t) => t.replace(/(protection_unreadable::シークレット PR_BODY_PROTECTION_TOKEN がない[^\n]*\n\s*)exit 1/, "$1exit 0")
      .replace("::error title=pr-body-protection protection_unreadable::シークレット", "::warning title=pr-body-protection::シークレット"),
    "権限がないときに飛ばす": (t) => t.replace(/(の branch protection を読めなかった[^\n]*\n\s*)exit 1/, "$1exit 0"),
    "rulesets を読めないときに飛ばす": (t) => t.replace(/(の rulesets を読めなかった[^\n]*\n\s*)exit 1/, "$1exit 0"),
    "ジョブの id を変える": (t) => t.replace(/^ {2}protection:\n/m, "  protect:\n"),
  };
  for (const [name, mutate] of Object.entries(mutants)) {
    const changed = mutate(protTemplate);
    assert.notEqual(changed, protTemplate, `${name}: 置き換えが効いていない`);
    assert.notDeepEqual(protectionRunProblems(changed), [], `${name} が通った`);
  }
});

test("写し（.github/workflows/pr-body-protection.yml）があれば、定期・手動で走り、トークンを読めないときは protection_unreadable で飛ばさず落ちる", {
  skip: existsSync(protCopyPath) ? false : "まだ .github/workflows/ に写していない",
}, () => {
  assert.deepEqual(protectionRunProblems(readFileSync(protCopyPath, "utf8")), []);
});

// ci/pr-body-patch.yml（engine/pr-body-gate.patch が ENGINE_REF の版と Engine の既定のブランチの先頭に当たるかを patch.mjs で、既定のブランチへの push・
// 毎日・手動で照らし、patch_stale か patch_partially_applied なら Issue を開くか更新する。議決 D-0068（起票時の番号は D-0066）、Issue #1179）の形と手順の振る舞い
const patchTemplate = readFileSync(fileURLToPath(new URL("./ci/pr-body-patch.yml", import.meta.url)), "utf8");
const patchCopyPath = fileURLToPath(new URL("../../.github/workflows/pr-body-patch.yml", import.meta.url));
const patchSteps = jobSteps(jobBody(patchTemplate.split(/\r?\n/), "patch") ?? []);
const patchEngineStep = patchSteps.find((s) => s.some((l) => /repository: guccyon\/factory-engine/.test(l)));
const patchCheckStep = patchSteps.find((s) => /当たるかを照らす/.test(s[0]));
const patchIssueStep = patchSteps.find((s) => /Issue を開くか更新する/.test(s[0]));
const PATCH_ISSUE_TITLE = patchTemplate.match(/^ {2}ISSUE_TITLE: "(.+)"$/m)?.[1];

test("pr-body-patch は既定のブランチへの push・定期（schedule）・手動でだけ走り、PR では走らない。Engine は既定のブランチの先頭を全部の履歴で取る", () => {
  const lines = patchTemplate.split(/\r?\n/).filter((l) => !/^\s*#/.test(l));
  const on = lines.findIndex((l) => /^on:\s*$/.test(l));
  const onBody = lines.slice(on + 1, lines.findIndex((l, i) => i > on && /^\S/.test(l))).filter((l) => l.trim() !== "");
  assert.deepEqual(onBody.filter((l) => /^ {2}[\w-]+:/.test(l)).map((l) => l.trim()), ["push:", "schedule:", "workflow_dispatch:"],
    "push・schedule・workflow_dispatch のほかで走らせない");
  assert.ok(onBody.some((l) => /^ {4}branches: \[main\]$/.test(l)), "push は既定のブランチ（main）だけにする");
  assert.ok(!onBody.some((l) => /^ {4}(paths|paths-ignore|tags|branches-ignore):/.test(l)), "push を道やタグで絞らない");
  assert.ok(onBody.some((l) => /^ {4}- cron: "[^"]+"$/.test(l)), "schedule の cron がない");
  assert.ok(!lines.some((l) => /^\s*(- )?continue-on-error:/.test(l)), "continue-on-error があると外れても気づかない");
  assert.match(patchTemplate, /^ {2}contents: read$/m);
  assert.match(patchTemplate, /^ {2}issues: write$/m, "Issue を開くので issues: write が要る");
  assert.ok(!lines.some((l) => /secrets\./.test(l)), "シークレットは使わない");
  assert.ok(!jobBody(lines, "patch").some((l) => /^ {4}if:/.test(l)), "ジョブを飛ばさない");
  assert.ok(patchEngineStep, "Engine を取る手順がない");
  assert.ok(!patchEngineStep.some((l) => /^\s*ref:/.test(l)), "Engine は ref なしで既定のブランチの先頭を取る");
  assert.ok(patchEngineStep.some((l) => /^\s*path: \.engine$/.test(l)));
  assert.ok(patchEngineStep.some((l) => /^\s*fetch-depth: 0$/.test(l)), "ENGINE_REF の版も照らすので全部の履歴で取る");
  assert.ok(!patchCheckStep.some((l) => /^\s*if:/.test(l)), "照らす手順を飛ばさない");
  assert.ok(patchCheckStep.some((l) => /^\s*id: check$/.test(l)));
  assert.ok(patchCheckStep.some((l) => /^\s*working-directory: tools\/pr-body-check$/.test(l)));
  assert.match(runScript(patchCheckStep), /node patch\.mjs --engine "\$\{GITHUB_WORKSPACE\}\/\.engine" --json patch-result\.json/);
  assert.doesNotMatch(runScript(patchCheckStep), /\|\||; *true|exit 0/, "照らす手順の外れを握りつぶさない");
  assert.ok(patchIssueStep.some((l) => /^\s*if: always\(\)$/.test(l)), "Issue の手順は照らす手順が落ちたときも走らせる");
  assert.ok(patchIssueStep.some((l) => /^\s*CHECK_OUTCOME: \$\{\{ steps\.check\.outcome \}\}$/.test(l)));
  assert.ok(patchSteps.indexOf(patchEngineStep) < patchSteps.indexOf(patchCheckStep) && patchSteps.indexOf(patchCheckStep) < patchSteps.indexOf(patchIssueStep));
  assert.ok(PATCH_ISSUE_TITLE, "ISSUE_TITLE がない");
  assert.notEqual(PATCH_ISSUE_TITLE, ISSUE_TITLE, "pr-body-protection と同じ題だと、片方が通ったときにもう片方の Issue を閉じる");
});

/** 偽の gh（Issue の list・create・comment・close と、Issue の本文とコメントの api）。 */
function fakePatchGh({ open = "", recorded = "", create = 0 } = {}) {
  const q = (v) => `'${String(v).replace(/'/g, "'\\''")}'`;
  return [
    `case "$1 $2" in`,
    `  "issue list") [ -n ${q(open)} ] && printf '%s\\n' ${q(open)}; exit 0 ;;`,
    `  "issue create") exit ${create} ;;`,
    `  api\\ *) printf '%s\\n' ${q(recorded)}; exit 0 ;;`,
    `esac`,
    `exit 0`,
  ].join("\n");
}

const gitEnv = { ...process.env, GIT_AUTHOR_NAME: "t", GIT_AUTHOR_EMAIL: "t@example.invalid", GIT_COMMITTER_NAME: "t", GIT_COMMITTER_EMAIL: "t@example.invalid" };
const gitIn = (cwd, ...args) => spawnSync("git", args, { cwd, env: gitEnv, encoding: "utf8" }).stdout.trim();

/**
 * 仮のリポジトリに patch.mjs と偽の差分（engine/pr-body-gate.patch。entrypoint.sh に関所の呼び出しを足す）を置き、.engine に偽の Engine を作って、
 * 照らす・Issue の 2 つの手順を順に動かす。refState・headState は ENGINE_REF の版と先頭の版の中身（applies・merged・patch_stale・patch_partially_applied）。
 * engine が false なら Engine を置かない（照らせない）。
 */
function runPatch({ refState = "applies", headState = "applies", engine = true, gh = {} } = {}) {
  const root = mkdtempSync(join(tmpdir(), "pr-body-patch-ci-"));
  const dir = join(root, "tools", "pr-body-check");
  mkdirSync(join(dir, "engine"), { recursive: true });
  mkdirSync(join(dir, "ci"), { recursive: true });
  writeFileSync(join(dir, "patch.mjs"), readFileSync(fileURLToPath(new URL("./patch.mjs", import.meta.url))));
  const e = join(root, ".engine");
  const sh = join(e, "orchestrator", "worker", "entrypoint.sh");
  const bodies = { applies: "a\nb\nc\n", merged: "a\nb\nnode tools/pr-body-check/gate.mjs\nc\n",
    patch_partially_applied: "a\nB\nnode tools/pr-body-check/gate.mjs --x\nc\n", patch_stale: "p\nq\nr\n" };
  mkdirSync(join(e, "orchestrator", "worker"), { recursive: true });
  gitIn(e, "init", "-q");
  writeFileSync(sh, bodies.applies);
  gitIn(e, "add", ".");
  gitIn(e, "commit", "-qm", "base");
  writeFileSync(sh, bodies.merged);
  writeFileSync(join(dir, "engine", "pr-body-gate.patch"), gitIn(e, "diff") + "\n");
  const commit = (state) => { writeFileSync(sh, bodies[state]); gitIn(e, "commit", "-qam", state, "--allow-empty"); return gitIn(e, "rev-parse", "HEAD"); };
  const ref = commit(refState);
  const head = commit(headState);
  writeFileSync(join(dir, "ci", "pr-body-engine.yml"), `env:\n  ENGINE_REF: ${ref}\n`);
  if (!engine) rmSync(e, { recursive: true, force: true });
  const bin = join(root, "bin");
  mkdirSync(bin);
  const env = { ...process.env, PATH: `${bin}:${process.env.PATH}`, GITHUB_REPOSITORY: "guccyon/rally", GITHUB_SERVER_URL: "https://github.com",
    GITHUB_RUN_ID: "123", GITHUB_WORKSPACE: root, ISSUE_TITLE: PATCH_ISSUE_TITLE, GITHUB_STEP_SUMMARY: join(root, "summary"), GITHUB_ACTIONS: "true" };
  const runStep = (step, extra) => spawnSync("bash", ["-e", "-o", "pipefail", "-c", runScript(step)], { cwd: dir, env: { ...env, ...extra }, encoding: "utf8" });
  const check = runStep(patchCheckStep, {});
  /** 偽の gh を opts にして Issue の手順を動かす（同じ照らした結果で何度でも動かせる）。 */
  const runIssue = (opts) => {
    writeFileSync(join(bin, "gh"), `#!/usr/bin/env bash\necho "gh $*" >> "${join(root, "calls.log")}"\n${fakePatchGh(opts)}\n`);
    chmodSync(join(bin, "gh"), 0o755);
    writeFileSync(join(root, "calls.log"), "");
    const issue = runStep(patchIssueStep, { GH_TOKEN: "github-token", CHECK_OUTCOME: check.status === 0 ? "success" : "failure" });
    const commentPath = join(dir, "patch-comment.md");
    return { check, issue, calls: readFileSync(join(root, "calls.log"), "utf8"), ref, head, runIssue,
      body: existsSync(commentPath) ? readFileSync(commentPath, "utf8") : "" };
  };
  return runIssue(gh);
}

test("pr-body-patch: ENGINE_REF の版と先頭のどちらでも当たるかもう入っていれば通り、Issue は開かない", () => {
  for (const [refState, headState] of [["applies", "applies"], ["applies", "merged"], ["merged", "merged"]]) {
    const r = runPatch({ refState, headState });
    assert.equal(r.check.status, 0, r.check.stdout + r.check.stderr);
    assert.match(r.check.stdout, new RegExp(`^ENGINE_REF ${r.ref}: ${refState === "applies" ? "当たる" : "もう入っている"}$`, "m"));
    assert.equal(r.issue.status, 0, r.issue.stdout + r.issue.stderr);
    assert.doesNotMatch(r.calls, /^gh issue (create|comment|close)/m);
  }
});

test("pr-body-patch: patch_stale と patch_partially_applied は、ENGINE_REF の版でも先頭でも落ち、開いている Issue がなければ開く", () => {
  for (const [refState, headState, rule, target] of [["applies", "patch_stale", "patch_stale", "既定のブランチの先頭"],
    ["applies", "patch_partially_applied", "patch_partially_applied", "既定のブランチの先頭"],
    ["patch_stale", "merged", "patch_stale", "ENGINE_REF"], ["patch_partially_applied", "applies", "patch_partially_applied", "ENGINE_REF"]]) {
    const r = runPatch({ refState, headState });
    const label = `${refState}/${headState}`;
    assert.equal(r.check.status, 1, label);
    assert.match(r.check.stdout, new RegExp(`^::error title=pr-body-patch ${rule}::${target}`, "m"), label);
    assert.equal(r.issue.status, 0, r.issue.stdout + r.issue.stderr);
    assert.match(r.calls, /^gh issue create --repo guccyon\/rally --title .+ --body-file patch-comment\.md$/m, label);
    assert.match(r.body, /^<!-- pr-body-patch: [0-9a-f]{16} -->\n/, label);
    assert.match(r.body, new RegExp(`^${rule}: ${target}`, "m"), label);
    assert.match(r.body, new RegExp(`^- ENGINE_REF（\`${r.ref}\`）: ${refState}$`, "m"), label);
    assert.match(r.body, new RegExp(`^- 既定のブランチの先頭（\`${r.head}\`）: ${headState}$`, "m"), label);
    assert.match(r.body, /https:\/\/github\.com\/guccyon\/rally\/actions\/runs\/123/);
  }
});

test("pr-body-patch: 開いている Issue があれば、それにコメントで足す（同じ版の同じ外れ方の記録がすでにあれば足さない）", () => {
  const first = runPatch({ headState: "patch_stale", gh: { open: "42", recorded: "前の記録" } });
  assert.doesNotMatch(first.calls, /^gh issue create/m);
  assert.match(first.calls, /^gh issue comment 42 --repo guccyon\/rally --body-file patch-comment\.md$/m);
  const marker = first.body.split("\n")[0];
  const again = first.runIssue({ open: "42", recorded: `本文\n${marker}` });
  assert.equal(again.issue.status, 0, again.issue.stdout + again.issue.stderr);
  assert.doesNotMatch(again.calls, /^gh issue (create|comment)/m);
  // 外れ方が変われば足す
  const other = runPatch({ headState: "patch_partially_applied", gh: { open: "42", recorded: marker } });
  assert.notEqual(other.body.split("\n")[0], marker);
  assert.match(other.calls, /^gh issue comment 42 /m);
});

test("pr-body-patch: 通ったら、開いている Issue に通ったことを書いて閉じる", () => {
  const r = runPatch({ gh: { open: "42" } });
  assert.equal(r.issue.status, 0, r.issue.stdout + r.issue.stderr);
  assert.match(r.calls, /^gh issue comment 42 --repo guccyon\/rally --body-file patch-comment\.md$/m);
  assert.match(r.calls, /^gh issue close 42 --repo guccyon\/rally$/m);
  assert.match(r.body, /当たるか、もう入っている/);
});

test("pr-body-patch: Engine がない・ENGINE_REF の版がないときは patch_check_failed で落ち、Issue は開かない", () => {
  const r = runPatch({ engine: false });
  assert.equal(r.check.status, 2);
  assert.match(r.check.stdout, /^::error title=pr-body-patch patch_check_failed::/m);
  assert.equal(r.issue.status, 0, r.issue.stdout + r.issue.stderr);
  assert.doesNotMatch(r.calls, /^gh issue (create|comment|close)/m);
});

test("pr-body-patch: Issue を開けなければ Issue の手順も落ちる（黙って通さない）", () => {
  const r = runPatch({ headState: "patch_stale", gh: { create: 1 } });
  assert.notEqual(r.issue.status, 0);
});

test("写し（.github/workflows/pr-body-patch.yml）があれば、ひな形と同じ", {
  skip: existsSync(patchCopyPath) ? false : "まだ .github/workflows/ に写していない",
}, () => {
  assert.equal(readFileSync(patchCopyPath, "utf8").trimEnd(), patchTemplate.trimEnd(), "写しがひな形とずれている。ひな形を直してから写し直す");
});
