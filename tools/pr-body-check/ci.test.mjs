// ci/pr-body-engine.yml の形の見張りのテスト（知見 K-0001 の担保。議決 D-0032、Issue #1037）。
// 建設者がこのひな形を .github/workflows/pr-body-engine.yml に写し、engine-test を必須のチェックにすると、関所を呼ばない Engine の版では
// engine.test.mjs の (3) で落ちる。その止め方が効くのは、ひな形が PR ごとに必ず走り、落ちたら止め、ENGINE_REF の行を変えた PR でその版の
// Engine を確かめる形のときだけなので、それを見る。写しがあれば ENGINE_REF の行のほかがひな形と同じかも見る（写すときに変えてよいのはその行だけ）。
// このテストは engine-test の node --test の中で走るので、写しから見張りを外す変更も同じジョブで落ちる。
// 通る側のテスト（gate.test.mjs の関所と CLI、engine.test.mjs の (10)）が skip・todo・only・道や名前の絞り込みで実行の対象から外れていないことも見る
// （議決 D-0045 の ci_check の案、Issue #1109）。recheck.test.mjs（知見 K-0012 の担保）も同じく外れていないことを見る（議決 D-0050 の ci_check の案、Issue #1131）。
// ci/pr-body-protection.yml（branch protection を読めるトークンで protection.mjs を既定のブランチへの push と毎日走らせ、外れたら Issue を開くか更新する。
// 議決 D-0044、Issue #1106）の形と手順の振る舞い、写しとのずれも見る。写しがあれば、写しそのものが schedule と workflow_dispatch で走り、
// PR_BODY_PROTECTION_TOKEN を読めないときに protection_unreadable で飛ばさず落ちることも見る（議決 D-0068 の test の案、Issue #1180）。
// ENGINE_DIR の Engine で engine.test.mjs を実際に走らせ、(10) と (11) のちょうど一方だけが走って通ることも見る（議決 D-0069 の test の案、Issue #1186）。
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

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
// false になる共通の skip（ワークフローは ENGINE_DIR を渡す）だけにする。どちらも todo・only・test.skip・t.skip などを置かない。注釈の行は見ない。
const GATE_TEST = "gate.test.mjs";
const ENGINE_TEST = "engine.test.mjs";
const readTest = (name) => readFileSync(fileURLToPath(new URL(`./${name}`, import.meta.url)), "utf8");
const ENGINE_REQUIRED = 'const required = Boolean(process.env.ENGINE_DIR) || process.env.CI === "true";';
const ENGINE_SKIP = /^const skip = required \|\| existsSync\(entrypointPath\) \? false : `[^`]*`;$/;
const PASSING_LOOP = "for (const [label, report] of PASSING_REPORTS) {";

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
    if (lines.some((l) => /\bskip\b/.test(l) && !ENGINE_SKIP.test(l) && !/^\s*test\(.*, \{ skip \}, (\(t\)|\(\)) => \{$/.test(l))) {
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

// engine.test.mjs の通る側は、ENGINE_DIR の Engine について (10) と (11) のちょうど一方だけが走って通る（議決 D-0069 の test の案、Issue #1186）。
// (10) は関所を配線した Engine で通る側を確かめ、(11) は議決 D-0045 の test の案で (10) を ENGINE_DIR の Engine そのもので走らせる形にしたときの、
// 差分を当てた写しでの確かめの移し先（Engine が関所を呼ばない間だけ走る）。上の見張りはファイルの形しか見ないので、関所を呼ぶか（isLegacyEntrypoint）の
// 判定が誤って (10) が飛ぶか両方が飛ぶ形は、engine.test.mjs を ENGINE_DIR を渡して実際に走らせた結果（TAP）で見る。(11) がまだない形では (10) が走って通ればよい
const engineDir = process.env.ENGINE_DIR || fileURLToPath(new URL("../../../engine", import.meta.url));
const engineRequired = Boolean(process.env.ENGINE_DIR) || process.env.CI === "true";
const engineSkip = engineRequired || existsSync(join(engineDir, "orchestrator/worker/entrypoint.sh")) ? false : `Engine のチェックアウトがない（ENGINE_DIR=${engineDir}）`;
const PASSING_SIDES = ["(10)", "(11)"];

/** node --test の TAP の出力から、一番上の階層のテストの名前・通ったか・飛ばしの指定（SKIP・TODO、なければ null）を取り出す。 */
export function tapResults(tap) {
  return tap.split(/\r?\n/).flatMap((l) => {
    const m = l.match(/^(not ok|ok) \d+ - (.*?)(?: # (SKIP|TODO)\b.*)?$/i);
    return m ? [{ name: m[2].replace(/\\#/g, "#"), ok: m[1] === "ok", directive: m[3]?.toUpperCase() ?? null }] : [];
  });
}

/** (10) と (11) の結果から、ちょうど一方だけが走って通っているかの外れ（空なら通る）。走って落ちた側は、通った側があればよい（関所を呼ばない Engine での (10)）。 */
export function passingSideProblems(results) {
  const problems = [];
  const sides = PASSING_SIDES.map((id) => ({ id, tests: results.filter((r) => r.name.startsWith(`${id} `)) }));
  const ran = (s) => s.tests.filter((r) => !r.directive);
  const passed = (s) => s.tests.length >= PASSING_REPORTS.length && s.tests.every((r) => r.ok && !r.directive);
  if (sides[0].tests.length === 0) problems.push("(10) のテストが 1 件も走っていない（名前を変えたか、ループを外した）");
  if (sides[0].tests.some((r) => r.directive)) problems.push("(10) が飛んだ（関所を呼ぶかの判定で (10) を飛ばしている）");
  for (const s of sides) {
    if (ran(s).length > 0 && ran(s).length < s.tests.length) problems.push(`${s.id} の一部だけが飛んだ（関所を呼ぶかの判定が形ごとにずれている）`);
  }
  if (sides.every((s) => ran(s).length === 0)) problems.push("(10) と (11) の両方が飛んだ");
  const passing = sides.filter(passed);
  if (passing.length === 0) problems.push(`(10) と (11) のどちらも、通る側の ${PASSING_REPORTS.length} 通りの形で走って通っていない`);
  if (passing.length > 1) problems.push("(10) と (11) の両方が走って通った（関所を呼ぶかの判定が効いていない）");
  return problems;
}

/** dir の engine.test.mjs の (10)・(11) だけを、ENGINE_DIR を渡して node --test で走らせ、TAP の結果を返す。 */
function runPassingSides(dir) {
  const env = { ...process.env, ENGINE_DIR: engineDir };
  // 外の node --test の子として報告させない。NODE_OPTIONS での絞り込みも持ち込まない
  delete env.NODE_TEST_CONTEXT;
  delete env.NODE_OPTIONS;
  const r = spawnSync(process.execPath, ["--test", "--test-reporter=tap", "--test-name-pattern=^\\((10|11)\\) ", ENGINE_TEST], { cwd: dir, env, encoding: "utf8" });
  return tapResults(r.stdout ?? "");
}

const pass = (name) => ({ name, ok: true, directive: null });
const fail = (name) => ({ name, ok: false, directive: null });
const skipped = (name) => ({ name, ok: true, directive: "SKIP" });
const side = (id, make) => PASSING_REPORTS.map(([label]) => make(`${id} 関所を配線した Engine は、${label}なら本文を書く`));

test("TAP の結果の読み取りは、飛ばし（# SKIP・# TODO）と名前の中の \\# を分ける", () => {
  const tap = ["# Subtest: (10) a", "ok 1 - (10) 報告の見出しを \\#\\#\\# に下げた", "not ok 2 - (11) b", "ok 3 - (11) c # SKIP 関所を呼ぶ Engine", "ok 4 - (11) d # TODO まだ",
    "    ok 1 - 中の階層は見ない"].join("\n");
  assert.deepEqual(tapResults(tap), [
    { name: "(10) 報告の見出しを ### に下げた", ok: true, directive: null },
    { name: "(11) b", ok: false, directive: null },
    { name: "(11) c", ok: true, directive: "SKIP" },
    { name: "(11) d", ok: true, directive: "TODO" },
  ]);
});

test("(10) と (11) のちょうど一方だけが走って通る形は通り、(10) が飛ぶ・両方が飛ぶ・両方が通る形は外れになる", () => {
  const ok = {
    "(11) がまだなく、(10) が写しで通る（今の形）": side("(10)", pass),
    "関所を呼ばない Engine で、(10) が本物で落ち (11) が写しで通る": [...side("(10)", fail), ...side("(11)", pass)],
    "関所を呼ぶ Engine で、(10) が本物で通り (11) が飛ぶ": [...side("(10)", pass), ...side("(11)", skipped)],
  };
  for (const [name, results] of Object.entries(ok)) assert.deepEqual(passingSideProblems(results), [], name);
  const ng = {
    "(10) が飛び (11) が通る": [...side("(10)", skipped), ...side("(11)", pass)],
    "(10) と (11) の両方が飛ぶ": [...side("(10)", skipped), ...side("(11)", skipped)],
    "(11) がなく (10) が飛ぶ": side("(10)", skipped),
    "(10) と (11) の両方が通る": [...side("(10)", pass), ...side("(11)", pass)],
    "(10) が落ち (11) が飛ぶ": [...side("(10)", fail), ...side("(11)", skipped)],
    "(10) が todo": side("(10)", (n) => ({ name: n, ok: true, directive: "TODO" })),
    "(10) が 1 件もない": side("(11)", pass),
    "(10) が形の数より少ない": side("(10)", pass).slice(1),
    "(11) の一部だけが飛ぶ": [...side("(10)", fail), ...side("(11)", pass).slice(1), skipped("(11) 残りの形")],
  };
  for (const [name, results] of Object.entries(ng)) assert.notDeepEqual(passingSideProblems(results), [], `${name} が通った`);
});

test("ENGINE_DIR の Engine で engine.test.mjs を走らせると、(10) と (11) のちょうど一方だけが走って通る", { skip: engineSkip }, () => {
  const results = runPassingSides(fileURLToPath(new URL(".", import.meta.url)));
  assert.deepEqual(passingSideProblems(results), [], `ENGINE_DIR=${engineDir} の結果: ${JSON.stringify(results.map((r) => [r.name.slice(0, 4), r.ok, r.directive]))}`);
});

test("関所を呼ぶかの判定を誤って (10) を飛ばす・両方を飛ばす・両方を走らせる engine.test.mjs は、ENGINE_DIR の Engine で外れになる", { skip: engineSkip }, () => {
  const engineText = readTest(ENGINE_TEST);
  const start = engineText.indexOf(PASSING_LOOP);
  assert.ok(start >= 0, "engine.test.mjs に (10) のループがない");
  const loop = engineText.slice(start, engineText.indexOf("\n}\n", start) + 3);
  const skipTen = (t) => t.replace(/(誤って止めない）`), \{ skip \}, \(\) =>/, '$1, { skip: "関所を呼ぶかの判定を誤った" }, () =>');
  const asEleven = (t) => t.replace("test(`(10) ", "test(`(11) ");
  const mutants = {
    "(10) を飛ばす": skipTen(engineText),
    "(10) を飛ばし、(11) も飛ばす": `${skipTen(engineText)}\n${asEleven(skipTen(loop))}`,
    "(10) と (11) の両方を走らせる": `${engineText}\n${asEleven(loop)}`,
  };
  const src = fileURLToPath(new URL(".", import.meta.url));
  for (const [name, text] of Object.entries(mutants)) {
    assert.notEqual(text, engineText, `${name}: 置き換えが効いていない`);
    // engine.test.mjs は本文の更新の節を ../../ を根にして動かす（node tools/pr-body-check/gate.mjs）ので、同じ並びの写しで走らせる
    const dir = join(mkdtempSync(join(tmpdir(), "pr-body-passing-sides-")), "tools", "pr-body-check");
    cpSync(src, dir, { recursive: true });
    writeFileSync(join(dir, ENGINE_TEST), text);
    assert.notDeepEqual(passingSideProblems(runPassingSides(dir)), [], `${name} が通った`);
  }
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
