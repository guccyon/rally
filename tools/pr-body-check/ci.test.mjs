// ci/pr-body-engine.yml の形の見張りのテスト（知見 K-0001 の担保。議決 D-0032、Issue #1037）。
// 建設者がこのひな形を .github/workflows/pr-body-engine.yml に写し、engine-test を必須のチェックにすると、関所を呼ばない Engine の版では
// engine.test.mjs の (3) で落ちる。その止め方が効くのは、ひな形が PR ごとに必ず走り、落ちたら止め、ENGINE_REF の行を変えた PR でその版の
// Engine を確かめる形のときだけなので、それを見る。写しがあれば ENGINE_REF の行のほかがひな形と同じかも見る（写すときに変えてよいのはその行だけ）。
// このテストは engine-test の node --test の中で走るので、写しから見張りを外す変更も同じジョブで落ちる。
// ci/pr-body-protection.yml（branch protection を読めるトークンで protection.mjs を既定のブランチへの push と毎日走らせ、外れたら Issue を開くか更新する。
// 議決 D-0044、Issue #1106）の形と手順の振る舞い、写しとのずれも見る。
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
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
    problems.push(`engine-test の name: を ${CHECK_NAME} にする（id のままの engine-test は knowledge-enforcement.yml も報告し、必須のチェックがどちらでも満たされる）`);
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
  const read = runStep(protReadStep, root, { GH_TOKEN: token });
  const check = read.status === 0 ? runStep(protCheckStep, dir, {}) : null;
  const outcome = check === null ? "skipped" : check.status === 0 ? "success" : "failure";
  const issue = runStep(protIssueStep, dir, { GH_TOKEN: "github-token", CHECK_OUTCOME: outcome });
  const calls = existsSync(join(root, "calls.log")) ? readFileSync(join(root, "calls.log"), "utf8") : "";
  const commentPath = join(dir, "protection-comment.md");
  return { read, check, issue, calls, body: existsSync(commentPath) ? readFileSync(commentPath, "utf8") : "" };
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
