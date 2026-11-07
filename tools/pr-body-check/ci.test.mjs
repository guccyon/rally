// ci/pr-body-engine.yml の形の見張りのテスト（知見 K-0001 の担保。議決 D-0032、Issue #1037）。
// 建設者がこのひな形を .github/workflows/pr-body-engine.yml に写し、engine-test を必須のチェックにすると、関所を呼ばない Engine の版では
// engine.test.mjs の (3) で落ちる。その止め方が効くのは、ひな形が PR ごとに必ず走り、落ちたら止め、ENGINE_REF の行を変えた PR でその版の
// Engine を確かめる形のときだけなので、それを見る。写しがあれば ENGINE_REF の行のほかがひな形と同じかも見る（写すときに変えてよいのはその行だけ）。
// このテストは engine-test の node --test の中で走るので、写しから見張りを外す変更も同じジョブで落ちる。
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { jobCheckNames } from "../sendback-check/protection.mjs";
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
    problems.push(`engine-test の name: を ${CHECK_NAME} にする（sendback-engine.yml の engine-test と同じ名前だと、必須のチェックがどちらでも満たされる）`);
  }
  if (engine.some((l) => /^ {4}needs:/.test(l))) problems.push("engine-test に needs があると、待つジョブが落ちたときに飛ばされて走らない");
  const steps = jobSteps(engine);
  const checkout = steps.find((s) => s.some((l) => /repository: guccyon\/factory-engine/.test(l)));
  if (!checkout?.some((l) => /^\s*ref: \$\{\{ env\.ENGINE_REF \}\}\s*$/.test(l))) {
    problems.push("Engine を取る手順の ref が ${{ env.ENGINE_REF }} でない（ENGINE_REF の行を変えても取る版が変わらない）");
  }
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
