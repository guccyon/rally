// ci/sendback-engine.yml の ENGINE_REF の見張りのテスト（知見 K-0004 の担保。議決 D-0014、Issue #960）。
// ENGINE_REF がブランチ名やタグだと同じ PR でも結果が変わるので、engine-test の最初の手順（どのチェックアウトよりも前）で
// 40 桁のコミット SHA かを確かめ、違えば落とす。その手順を取り出して bash で動かす。
// .github/workflows/sendback-engine.yml に写してあれば、ENGINE_REF の行のほかがこのひな形と同じかも見る（写すときに変えてよいのはその行だけ）。
// ひな形が必須のチェックにできる形（PR ごとに必ず走り、落ちたら止める）で、見張りの手順を持っていることも見る（議決 D-0017、Issue #969）。
// 写しとのずれも見るので、見張りを外す変更はひな形と写しを一緒に直しても wiring-order で落ちる。写しを消す・ジョブの名前を変える変更は、
// 必須のチェック（engine-test・wiring-order が報告する sendback-engine-test・sendback-wiring-order）が報告されずに止まる（branch protection は建設者が入れる）。
// そのチェックの名前をほかのワークフロー（ひな形と .github/workflows/）のジョブが報告しないことも見る。重なると、どちらの報告でも必須のチェックが満たされ、
// 別のワークフローで check.mjs --events を engine.test.mjs を待たずに走らせる形を設定で止められない（議決 D-0038、Issue #1083）。
// ENGINE_REF の行を変える PR で engine-test が必ずその版で走る形か（ENGINE_REF の置き場所・needs・Engine を取る版）も、ひな形と写しで見る（議決 D-0028、Issue #1024）。
// ci/sendback-engine-head.yml（Engine の既定のブランチの先頭で定期に走らせ、落ちたら知らせる。議決 D-0020、Issue #977）の形と記録の手順の振る舞いも見る。
// その記録で、engine_harness.py のずれ（harness_unknown_query）と Engine の判定の変化（engine_head_failed）を分けることも見る（議決 D-0031、Issue #1046）。
// 通ったときも engine_head_passed として記録し、その記録を head-record.mjs が数えることも見る（議決 D-0031、Issue #1047）。
// ci/sendback-protection.yml（branch protection を読めるトークンで protection.mjs を毎日走らせ、外れたら Issue を開くか更新する。
// 議決 D-0039、Issue #1087）の形と手順の振る舞い、写しとのずれも見る。
// 同じひな形の protection-pr のジョブ（PR ごとに照らし、外れたら ::error で落とす。Issue は開かない。議決 D-0059（起票時の番号は D-0056）、Issue #1158）の形と振る舞いも見る。
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { headRecords } from "./head-record.mjs";
import { CHECK_NAMES, checkNameProblems, COPY, jobCheckNames, loadTemplates, loadWorkflows, sharedCheckNames } from "./protection.mjs";

const template = readFileSync(fileURLToPath(new URL("./ci/sendback-engine.yml", import.meta.url)), "utf8");
const copyPath = fileURLToPath(new URL("../../.github/workflows/sendback-engine.yml", import.meta.url));

/** ジョブの本体（`  <job>:` の次の行から次のジョブの前まで）。 */
function jobBody(text, job) {
  const lines = text.split(/\r?\n/);
  const start = lines.indexOf(`  ${job}:`);
  if (start < 0) return null;
  const end = lines.findIndex((l, i) => i > start && /^ {2}\S/.test(l));
  return lines.slice(start + 1, end < 0 ? lines.length : end);
}

/** ジョブの手順（`      - ` で始まる列の要素）を、行の塊で返す。 */
function jobSteps(text, job) {
  const body = jobBody(text, job);
  assert.ok(body !== null, `${job} のジョブがない`);
  const steps = [];
  for (const l of body.slice(body.indexOf("    steps:") + 1)) {
    if (l.startsWith("      - ")) steps.push([l]);
    else if (steps.length > 0) steps[steps.length - 1].push(l);
  }
  return steps;
}

/** 手順の `run: |` の中身（字下げを外したもの）。 */
function runScript(step) {
  const i = step.findIndex((l) => /^\s*run: \|\s*$/.test(l));
  assert.ok(i >= 0, "run: | の手順ではない");
  return step.slice(i + 1).filter((l) => l.trim() !== "").map((l) => l.replace(/^ {10}/, "")).join("\n") + "\n";
}

const guard = jobSteps(template, "engine-test")[0];
const ENGINE_REF = template.match(/^\s*ENGINE_REF:\s*(\S+)\s*$/m)?.[1];

const runGuard = (ref) => spawnSync("bash", ["-e", "-c", runScript(guard)], { env: { ...process.env, ENGINE_REF: ref }, encoding: "utf8" });

test("engine-test の最初の手順は ENGINE_REF の見張りで、どのチェックアウトよりも前にある", () => {
  assert.match(guard[0], /ENGINE_REF が 40 桁のコミット SHA か/);
  assert.ok(guard.some((l) => /shell: bash/.test(l)), "[[ =~ ]] を使うので shell: bash にする");
  assert.ok(!guard.some((l) => /actions\/checkout/.test(l)));
  const steps = jobSteps(template, "engine-test");
  assert.ok(steps.slice(1).some((s) => s.some((l) => /repository: guccyon\/factory-engine/.test(l))), "Engine を取る手順が見張りの後ろにない");
});

test("今の ENGINE_REF は 40 桁のコミット SHA で、見張りを通る", () => {
  assert.match(ENGINE_REF, /^[0-9a-f]{40}$/);
  const r = runGuard(ENGINE_REF);
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.match(r.stdout, new RegExp(`^ENGINE_REF=${ENGINE_REF}$`, "m"));
});

test("ENGINE_REF がブランチ名・タグ・短い SHA・大文字・空なら ::error を出して落ちる", () => {
  for (const ref of ["main", "v1.2.0", ENGINE_REF.slice(0, 7), ENGINE_REF.slice(0, 39), `${ENGINE_REF}0`,
    ENGINE_REF.toUpperCase(), `refs/heads/${ENGINE_REF}`, `${ENGINE_REF}\nmain`, ""]) {
    const r = runGuard(ref);
    assert.equal(r.status, 1, `ENGINE_REF=${JSON.stringify(ref)} が通った`);
    assert.match(r.stdout, /^::error title=sendback-engine engine_ref_not_sha::/m);
  }
});

/** ENGINE_REF の行を除いた中身（ずれの比べ方）。 */
const withoutRef = (text) => text.split(/\r?\n/).filter((l) => !/^\s*ENGINE_REF:/.test(l)).join("\n").trimEnd();

/** 必須のチェックにできる形かの外れ（空なら通る）。注釈の行は見ない。 */
function requiredCheckProblems(text) {
  const problems = [];
  const lines = text.split(/\r?\n/).filter((l) => !/^\s*#/.test(l));
  const on = lines.findIndex((l) => /^on:\s*$/.test(l));
  const onBody = on < 0 ? [] : lines.slice(on + 1, lines.findIndex((l, i) => i > on && /^\S/.test(l)));
  if (onBody.map((l) => l.trim()).filter((l) => l !== "").join(" ") !== "pull_request:") {
    problems.push("on は絞り込みのない pull_request だけにする（paths・branches・types で絞ると必須のチェックが報告されない PR が出る）");
  }
  if (lines.some((l) => /^\s*(- )?continue-on-error:/.test(l))) problems.push("continue-on-error があると落ちても止めない");
  if (lines.some((l) => /^\s*(- )?if:/.test(l))) problems.push("if: があるとジョブや手順が飛ばされて見張らない");
  const wiring = jobBody(lines.join("\n"), "wiring-order");
  const engine = jobBody(lines.join("\n"), "engine-test");
  if (wiring === null) problems.push("wiring-order のジョブがない（必須のチェックの名前）");
  else {
    if (!wiring.some((l) => /^\s*run: node tools\/sendback-check\/order\.mjs(\s|$)/.test(l))) problems.push("wiring-order が order.mjs を走らせていない");
    if (!wiring.some((l) => /^\s*run: node --test (.* )?ci\.test\.mjs(\s|$)/.test(l))) problems.push("wiring-order が ci.test.mjs を走らせていない");
  }
  if (engine === null) problems.push("engine-test のジョブがない（必須のチェックの名前）");
  for (const { job, expected } of checkNameProblems(jobCheckNames(lines.join("\n")))) {
    problems.push(`${job} の name: を ${expected} にする（id のままの名前はほかのワークフローも報告し、必須のチェックがどちらでも満たされる）`);
  }
  if (engine !== null) {
    if (!engine.some((l) => /^\s*ENGINE_DIR: \S/.test(l))) problems.push("engine-test が ENGINE_DIR を渡していない（渡さないと Engine がないときに飛ばす）");
    if (!engine.some((l) => /node --test (.* )?engine\.test\.mjs(\s|$)/.test(l))) problems.push("engine-test が engine.test.mjs を走らせていない");
  }
  return problems;
}

test("ひな形は必須のチェックにできる形で、見張りの手順（order.mjs・ci.test.mjs・ENGINE_DIR を向けた engine.test.mjs）を持つ", () => {
  assert.deepEqual(requiredCheckProblems(template), []);
});

test("見張りを外す・飛ばす・絞るひな形の直しは外れになる", () => {
  const cases = {
    "paths で絞る": template.replace("  pull_request:\n", "  pull_request:\n    paths: [\"tools/sendback-check/**\"]\n"),
    "types で絞る": template.replace("  pull_request:\n", "  pull_request:\n    types: [opened]\n"),
    "push だけにする": template.replace("  pull_request:\n", "  push:\n"),
    "ジョブを飛ばす": template.replace("  wiring-order:\n", "  wiring-order:\n    if: false\n"),
    "手順を飛ばす": template.replace("      - name: ENGINE_REF の見張りと写しのずれ（K-0004）\n", "      - name: ENGINE_REF の見張りと写しのずれ（K-0004）\n        if: false\n"),
    "落ちても止めない": template.replace("  engine-test:\n", "  engine-test:\n    continue-on-error: true\n"),
    "ジョブの名前を変える": template.replace("  wiring-order:\n", "  order:\n"),
    "チェックの名前を外す": template.replace(`    name: ${CHECK_NAMES["engine-test"]}\n`, ""),
    "チェックの名前を id と同じにする": template.replace(`    name: ${CHECK_NAMES["wiring-order"]}\n`, "    name: wiring-order\n"),
    "order.mjs を外す": template.replace(/^ {8}run: node tools\/sendback-check\/order\.mjs.*\n/m, "        run: echo skip\n"),
    "ci.test.mjs を外す": template.replace(/^ {8}run: node --test --test-reporter=spec ci\.test\.mjs\n/m, "        run: echo skip\n"),
    "ENGINE_DIR を外す": template.replace(/^ {10}ENGINE_DIR: .*\n/m, ""),
    "engine.test.mjs を外す": template.replace("--test-reporter-destination=engine-test.tap engine.test.mjs", "--test-reporter-destination=engine-test.tap"),
  };
  for (const [name, text] of Object.entries(cases)) {
    assert.notEqual(text, template, `${name}: 置き換えが効いていない`);
    assert.notDeepEqual(requiredCheckProblems(text), [], `${name} が通った`);
  }
});

// 必須のチェックの名前の重なり（議決 D-0038、Issue #1083）。別のワークフローの間は needs で待てないので、check.mjs --events を
// engine.test.mjs を待たずに走らせる形は engine-test を必須のチェックにする設定で止める。その名前をほかのジョブも報告すると、
// そのジョブの報告でも必須のチェックが満たされて止まらない。ほかのツールのひな形（tools/*/ci/）と .github/workflows/ のジョブを見る。
const repoRoot = fileURLToPath(new URL("../../", import.meta.url));

/** sendback-engine-test・sendback-wiring-order をほかのジョブも報告する所（空なら通る）。 */
const reservedShared = (workflows) => sharedCheckNames(Object.values(CHECK_NAMES), workflows).map(({ path, job, name }) => `${path} の ${job} が ${name} を報告する`);
const otherWorkflows = (root) => ({ ...loadWorkflows(root).others, ...loadTemplates(root) });

test("sendback-engine-test・sendback-wiring-order は、ほかのツールのひな形と .github/workflows/ のどのジョブも報告しない", () => {
  const workflows = otherWorkflows(repoRoot);
  assert.ok(Object.keys(workflows).includes("tools/knowledge-enforcement-check/ci/knowledge-enforcement.yml"), "ほかのひな形を読めていない");
  assert.ok(!Object.keys(workflows).includes(COPY), "写しはほかのワークフローに数えない");
  assert.deepEqual(reservedShared(workflows), []);
});

test("ほかのワークフローのジョブがその名前を名乗ると外れになり、id の engine-test のままのジョブは外れにしない", () => {
  const events = (name) => `name: x\non:\n  pull_request:\njobs:\n  events:\n${name ? `    name: ${name}\n` : ""}    runs-on: ubuntu-latest\n    steps:\n` +
    "      - run: node tools/sendback-check/check.mjs --events events.json\n";
  for (const name of Object.values(CHECK_NAMES)) {
    assert.deepEqual(reservedShared({ ".github/workflows/x.yml": events(name) }), [`.github/workflows/x.yml の events が ${name} を報告する`]);
  }
  assert.deepEqual(reservedShared({ ".github/workflows/x.yml": events("engine-test"), ".github/workflows/y.yml": events(null) }), []);
});

// ENGINE_REF の行を変える PR で engine-test が必ず走るか（議決 D-0028、Issue #1024）。Engine を上げる PR が変えるのは写しの ENGINE_REF の行だけなので、
// その PR で engine-test が飛ばされる・別の版で走ると、上げた版の Engine を確かめないまま通る。paths で絞ると写しを含めても含めなくても
// 書き方しだいで走らない PR が出るので、絞り込みの中身は問わず絞り込み自体を外れにする（requiredCheckProblems）。ここではそのほかに、
// ENGINE_REF がワークフローの env に 1 行だけあり（ずれの比べ方は ENGINE_REF の行を無視するので、2 行目で上書きされると見逃す）、
// engine-test が needs で待って飛ばされず、見張りの手順と Engine を取る手順がその ENGINE_REF を使うことを見る。

/** ENGINE_REF の行を変える PR で engine-test が走り、その版の Engine を確かめるかの外れ（空なら通る）。注釈の行は見ない。 */
function engineRefRunProblems(text) {
  const problems = [];
  const lines = text.split(/\r?\n/).filter((l) => !/^\s*#/.test(l));
  // ENGINE_REF を決める行: YAML の env の項目、GITHUB_ENV への書き込み、シェルの代入（echo の文の中の「ENGINE_REF=」は数えない）
  const refLines = lines.filter((l) => /^\s*(- )?ENGINE_REF:/.test(l) || (/ENGINE_REF/.test(l) && /GITHUB_ENV/.test(l))
    || /(^|[\s;&|(])(export\s+)?ENGINE_REF=/.test(l));
  const env = lines.findIndex((l) => /^env:\s*$/.test(l));
  const envBody = env < 0 ? [] : lines.slice(env + 1, lines.findIndex((l, i) => i > env && /^\S/.test(l)));
  if (refLines.length !== 1 || !envBody.some((l) => /^ {2}ENGINE_REF: \S+\s*$/.test(l))) {
    problems.push("ENGINE_REF はワークフローの env に 1 行だけ置く（ジョブや手順の env・GITHUB_ENV で上書きすると、ENGINE_REF の行を変えても別の版で走る）");
  }
  const engine = jobBody(lines.join("\n"), "engine-test");
  if (engine === null) return [...problems, "engine-test のジョブがない（必須のチェックの名前）"];
  if (engine.some((l) => /^ {4}needs:/.test(l))) problems.push("engine-test に needs があると、待つジョブが落ちたときに飛ばされて走らない");
  const steps = jobSteps(lines.join("\n"), "engine-test");
  if (!/ENGINE_REF が 40 桁のコミット SHA か/.test(steps[0]?.[0] ?? "") || !steps[0].some((l) => /"\$ENGINE_REF"/.test(l))) {
    problems.push("engine-test の最初の手順が ENGINE_REF の見張りでない");
  }
  const checkout = steps.find((s) => s.some((l) => /repository: guccyon\/factory-engine/.test(l)));
  if (!checkout?.some((l) => /^\s*ref: \$\{\{ env\.ENGINE_REF \}\}\s*$/.test(l))) {
    problems.push("Engine を取る手順の ref が ${{ env.ENGINE_REF }} でない（ENGINE_REF の行を変えても取る版が変わらない）");
  }
  return problems;
}

test("ENGINE_REF の行を変える PR でも、ひな形は必須のチェックにできる形のまま engine-test がその版で走る", () => {
  assert.deepEqual(engineRefRunProblems(template), []);
  const bumped = template.replace(ENGINE_REF, "0123456789abcdef0123456789abcdef01234567");
  assert.equal(withoutRef(bumped), withoutRef(template), "ENGINE_REF の行のほかが変わった");
  assert.deepEqual(requiredCheckProblems(bumped), []);
  assert.deepEqual(engineRefRunProblems(bumped), []);
  assert.equal(runGuard(bumped.match(/^\s*ENGINE_REF:\s*(\S+)\s*$/m)[1]).status, 0);
});

test("ENGINE_REF の行を変える PR で engine-test を走らせない・別の版で走らせる直しは、paths の中身に関わらず外れになる", () => {
  const onPr = "  pull_request:\n";
  const cases = {
    "paths で写しを含めて絞る": template.replace(onPr, `${onPr}    paths: [".github/workflows/sendback-engine.yml", "tools/sendback-check/**"]\n`),
    "paths で写しを含めずに絞る": template.replace(onPr, `${onPr}    paths: ["tools/sendback-check/**"]\n`),
    "paths の列で絞る": template.replace(onPr, `${onPr}    paths:\n      - "tools/**"\n`),
    "paths-ignore で .github を外す": template.replace(onPr, `${onPr}    paths-ignore: [".github/**"]\n`),
    "paths-ignore で写しを外す": template.replace(onPr, `${onPr}    paths-ignore:\n      - ".github/workflows/sendback-engine.yml"\n`),
    "branches で絞る": template.replace(onPr, `${onPr}    branches: [release]\n`),
    "engine-test が needs で待つ": template.replace("  engine-test:\n", "  engine-test:\n    needs: wiring-order\n"),
    "engine-test の needs を列で書く": template.replace("  engine-test:\n", "  engine-test:\n    needs:\n      - wiring-order\n"),
    "ジョブの env で ENGINE_REF を上書きする": template.replace("  engine-test:\n    name: sendback-engine-test\n    runs-on: ubuntu-latest\n",
      `  engine-test:\n    name: sendback-engine-test\n    runs-on: ubuntu-latest\n    env:\n      ENGINE_REF: ${"f".repeat(40)}\n`),
    "手順の env で ENGINE_REF を上書きする": template.replace("          ENGINE_DIR: ${{ github.workspace }}/.engine\n",
      `          ENGINE_DIR: \${{ github.workspace }}/.engine\n          ENGINE_REF: ${"f".repeat(40)}\n`),
    "GITHUB_ENV で ENGINE_REF を上書きする": template.replace("          echo \"ENGINE_REF=${ENGINE_REF}\"\n",
      `          echo "ENGINE_REF=\${ENGINE_REF}"\n          echo "ENGINE_REF=${"f".repeat(40)}" >> "$GITHUB_ENV"\n`),
    "ENGINE_REF をワークフローの env から外す": template.replace(/^env:\n {2}ENGINE_REF: .*\n/m, ""),
    "Engine を取る版を書き込む": template.replace("ref: ${{ env.ENGINE_REF }}", `ref: ${ENGINE_REF}`),
    "Engine を取る版を外す": template.replace(/^ {10}ref: \$\{\{ env\.ENGINE_REF \}\}\n/m, ""),
    "見張りの手順を後ろに回す": template.replace(/^ {6}- name: ENGINE_REF が 40 桁のコミット SHA か（K-0004）\n(?: {8}.*\n)+?(?= {6}- )/m, ""),
  };
  for (const [name, text] of Object.entries(cases)) {
    assert.notEqual(text, template, `${name}: 置き換えが効いていない`);
    assert.notDeepEqual([...requiredCheckProblems(text), ...engineRefRunProblems(text)], [], `${name} が通った`);
  }
  // paths・branches は requiredCheckProblems が、ENGINE_REF の置き場所・needs・取る版は engineRefRunProblems が見る
  for (const name of ["ジョブの env で ENGINE_REF を上書きする", "手順の env で ENGINE_REF を上書きする", "GITHUB_ENV で ENGINE_REF を上書きする",
    "engine-test が needs で待つ", "engine-test の needs を列で書く", "Engine を取る版を書き込む"]) {
    assert.notDeepEqual(engineRefRunProblems(cases[name]), [], `${name} を engineRefRunProblems が見逃した`);
  }
  // 写しで ENGINE_REF の行を足して上書きしても、ずれの比べ方（ENGINE_REF の行を無視する）では同じに見える。その分を engineRefRunProblems で止める
  assert.equal(withoutRef(cases["手順の env で ENGINE_REF を上書きする"]), withoutRef(template));
});

test("GitHub Actions では写し（.github/workflows/sendback-engine.yml）がなければ落ちる（別の名前に写すとずれを見張れない）", {
  skip: process.env.GITHUB_ACTIONS === "true" ? false : "GitHub Actions の外",
}, () => {
  assert.ok(existsSync(copyPath), "写しが .github/workflows/sendback-engine.yml にない。ひな形はこの名前に写す");
});

test("写し（.github/workflows/sendback-engine.yml）があれば、ENGINE_REF の行のほかはひな形と同じで、その ENGINE_REF も見張りを通る", {
  skip: existsSync(copyPath) ? false : "まだ .github/workflows/ に写していない",
}, () => {
  const copy = readFileSync(copyPath, "utf8");
  assert.equal(withoutRef(copy), withoutRef(template), "写しが ENGINE_REF の行のほかでひな形とずれている。ひな形を直してから写し直す");
  assert.deepEqual(engineRefRunProblems(copy), [], "写しの ENGINE_REF の行を変えても engine-test がその版で走らない");
  const ref = copy.match(/^\s*ENGINE_REF:\s*(\S+)\s*$/m)?.[1];
  assert.equal(runGuard(ref ?? "").status, 0, `写しの ENGINE_REF（${ref}）が 40 桁のコミット SHA でない`);
});

test("ずれの比べ方は ENGINE_REF の行だけを無視する", () => {
  const bumped = template.replace(ENGINE_REF, "0".repeat(40));
  assert.equal(withoutRef(bumped), withoutRef(template));
  assert.notEqual(withoutRef(template.replace("ENGINE_REF が 40 桁", "ENGINE_REF が")), withoutRef(template));
});

// ci/sendback-engine-head.yml（議決 D-0020、Issue #977）。engine-test は ENGINE_REF の固定の版で走るので、Engine の判定が変わっても
// ENGINE_REF を上げるまで気づかない。同じ engine.test.mjs を Engine の既定のブランチの先頭で定期に走らせ、落ちたら ::error と Issue #948 への記録で知らせる。
const headTemplate = readFileSync(fileURLToPath(new URL("./ci/sendback-engine-head.yml", import.meta.url)), "utf8");
const headCopyPath = fileURLToPath(new URL("../../.github/workflows/sendback-engine-head.yml", import.meta.url));
const headSteps = jobSteps(headTemplate, "engine-head");
const headTestStep = headSteps.find((s) => /Engine の先頭で本物の判定のテスト/.test(s[0]));
const headNotifyStep = headSteps.find((s) => /先頭での結果（通った・落ちた）を知らせて Issue に記録する/.test(s[0]));

/** 一時のディレクトリに tools/sendback-check の形と、呼び出しを log に書く偽のコマンドを置く。 */
function sandbox(fakes) {
  const root = mkdtempSync(join(tmpdir(), "sendback-engine-head-"));
  const dir = join(root, "tools", "sendback-check");
  mkdirSync(join(dir, "ci"), { recursive: true });
  writeFileSync(join(dir, "ci", "sendback-engine.yml"), template);
  const bin = join(root, "bin");
  mkdirSync(bin);
  for (const [name, body] of Object.entries(fakes)) {
    writeFileSync(join(bin, name), `#!/usr/bin/env bash\necho "${name} $*" >> "${join(root, "calls.log")}"\n${body}\n`);
    chmodSync(join(bin, name), 0o755);
  }
  const calls = () => (existsSync(join(root, "calls.log")) ? readFileSync(join(root, "calls.log"), "utf8") : "");
  return { root, dir, bin, calls };
}

const runStep = (step, box, env) => spawnSync("bash", ["-e", "-o", "pipefail", "-c", runScript(step)], {
  cwd: box.dir,
  env: { ...process.env, PATH: `${box.bin}:${process.env.PATH}`, ISSUE: "948", GITHUB_REPOSITORY: "guccyon/rally",
    GITHUB_SERVER_URL: "https://github.com", GITHUB_RUN_ID: "123", ...env },
  encoding: "utf8",
});

test("engine-head は定期（schedule）と手動でだけ走り、PR では走らない（先頭の Engine の変化で PR を止めない）", () => {
  const lines = headTemplate.split(/\r?\n/).filter((l) => !/^\s*#/.test(l));
  const on = lines.findIndex((l) => /^on:\s*$/.test(l));
  const onBody = lines.slice(on + 1, lines.findIndex((l, i) => i > on && /^\S/.test(l))).map((l) => l.trim()).filter((l) => l !== "");
  assert.ok(onBody.includes("schedule:"), "schedule がない");
  assert.ok(onBody.some((l) => /^- cron: "[^"]+"$/.test(l)), "schedule の cron がない");
  assert.ok(!onBody.some((l) => /^(pull_request|pull_request_target|push):/.test(l)), "PR や push で走らせない");
  assert.ok(!lines.some((l) => /^\s*(- )?continue-on-error:/.test(l)), "continue-on-error があると落ちても気づかない");
  assert.match(headTemplate, /^ {2}issues: write$/m, "Issue に記録するので issues: write が要る");
});

test("engine-head は Engine を既定のブランチの先頭で取り（ref を付けない）、ENGINE_DIR を向けて engine-test と同じ engine.test.mjs を走らせる", () => {
  const checkout = headSteps.find((s) => s.some((l) => /repository: guccyon\/factory-engine/.test(l)));
  assert.ok(checkout, "Engine を取る手順がない");
  assert.ok(!checkout.some((l) => /^\s*ref:/.test(l)), "ref を付けると先頭ではなくなる");
  assert.ok(checkout.some((l) => /^\s*path: \.engine$/.test(l)));
  assert.ok(headSteps.indexOf(checkout) < headSteps.indexOf(headTestStep), "テストが Engine を取る前にある");
  assert.ok(headTestStep.some((l) => /^\s*ENGINE_DIR: \$\{\{ github\.workspace \}\}\/\.engine$/.test(l)), "ENGINE_DIR を渡していない");
  assert.ok(headTestStep.some((l) => /^\s*working-directory: tools\/sendback-check$/.test(l)));
  assert.match(runScript(headTestStep), /node --test (.* )?engine\.test\.mjs(\s|$)/);
  assert.ok(!headTestStep.some((l) => /^\s*if:/.test(l)), "テストの手順を飛ばさない");
  assert.ok(headTestStep.some((l) => /^\s*id: head-test$/.test(l)), "記録の手順がテストの結果を読めるよう id を付ける");
  assert.ok(headNotifyStep.some((l) => /^\s*if: always\(\)$/.test(l)), "記録の手順は通ったときも落ちたときも走らせる");
  assert.ok(headNotifyStep.some((l) => /^\s*HEAD_OUTCOME: \$\{\{ steps\.head-test\.outcome \}\}$/.test(l)), "テストの手順の結果を渡していない");
  assert.ok(headSteps.indexOf(headTestStep) < headSteps.indexOf(headNotifyStep));
});

test("先頭のテストの手順は Engine の先頭の版を ENGINE_HEAD に残し、engine.test.mjs の終了コードで終わる", () => {
  for (const [nodeRc, want] of [[0, 0], [1, 1]]) {
    const box = sandbox({ node: `exit ${nodeRc}` });
    const engine = join(box.root, ".engine");
    mkdirSync(engine);
    for (const args of [["init", "-q"], ["-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "-q", "--allow-empty", "-m", "x"]]) {
      assert.equal(spawnSync("git", args, { cwd: engine }).status, 0);
    }
    const sha = spawnSync("git", ["rev-parse", "HEAD"], { cwd: engine, encoding: "utf8" }).stdout.trim();
    const ghEnv = join(box.root, "github_env");
    const r = runStep(headTestStep, box, { ENGINE_DIR: engine, GITHUB_ENV: ghEnv, GITHUB_STEP_SUMMARY: join(box.root, "summary") });
    assert.equal(r.status, want, r.stdout + r.stderr);
    assert.equal(readFileSync(ghEnv, "utf8"), `ENGINE_HEAD=${sha}\n`);
    assert.match(box.calls(), /^node --test .*--test-reporter-destination=engine-head\.tap engine\.test\.mjs$/m);
    assert.match(readFileSync(join(box.root, "summary"), "utf8"), new RegExp(`Engine の先頭 ${sha}`));
  }
});

const HEAD = "a".repeat(40);
const TAP = "TAP version 13\nok 1 - (0)\nnot ok 2 - (3) basis に欠陥の採用数と仮決めの承認数を分けて書く\n";

test("先頭で落ちたら ::error を出し、Issue #948 に先頭の版・ENGINE_REF・落ちたテスト・実行の URL を記録する", () => {
  const box = sandbox({ gh: `[ "$1" = api ] && printf '%s\\n' "ほかの記録"; exit 0` });
  writeFileSync(join(box.dir, "engine-head.tap"), TAP);
  const r = runStep(headNotifyStep, box, { ENGINE_HEAD: HEAD });
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.match(r.stdout, new RegExp(`^::error title=sendback-engine-head engine_head_failed::.*${HEAD}.*ENGINE_REF（${ENGINE_REF}）`, "m"));
  assert.match(box.calls(), /^gh api repos\/guccyon\/rally\/issues\/948\/comments /m);
  assert.match(box.calls(), /^gh issue comment 948 --repo guccyon\/rally --body-file engine-head-comment\.md$/m);
  const body = readFileSync(join(box.dir, "engine-head-comment.md"), "utf8");
  assert.ok(body.startsWith(`<!-- sendback-engine-head: ${HEAD} engine_head_failed -->\n`));
  assert.doesNotMatch(r.stdout, /harness_unknown_query/);
  assert.match(body, new RegExp(`\`${HEAD}\``));
  assert.match(body, new RegExp(`\`${ENGINE_REF}\``));
  assert.match(body, /^not ok 2 - \(3\) basis に欠陥の採用数と仮決めの承認数を分けて書く$/m);
  assert.match(body, /https:\/\/github\.com\/guccyon\/rally\/actions\/runs\/123/);
});

test("同じ先頭の記録が Issue #948 にすでにあれば足さない（::error は出す）", () => {
  const box = sandbox({ gh: `[ "$1" = api ] && printf '%s\\n' "<!-- sendback-engine-head: ${HEAD} engine_head_failed -->"; exit 0` });
  const r = runStep(headNotifyStep, box, { ENGINE_HEAD: HEAD });
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.match(r.stdout, /^::error title=sendback-engine-head engine_head_failed::/m);
  assert.doesNotMatch(box.calls(), /^gh issue comment/m);
});

test("先頭の版が取れなかった（Engine を取れなかった）ときも ::error を出し、毎回記録する", () => {
  const box = sandbox({ gh: "exit 0" });
  const r = runStep(headNotifyStep, box, { ENGINE_HEAD: "" });
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.match(r.stdout, /^::error title=sendback-engine-head engine_head_failed::.*版を取れなかった/m);
  assert.doesNotMatch(box.calls(), /^gh api/m);
  assert.match(box.calls(), /^gh issue comment 948 /m);
});

test("記録の ENGINE_REF は .github/workflows/sendback-engine.yml の写しがあればその版を使う", () => {
  const box = sandbox({ gh: "exit 0" });
  const bumped = "b".repeat(40);
  mkdirSync(join(box.root, ".github", "workflows"), { recursive: true });
  writeFileSync(join(box.root, ".github", "workflows", "sendback-engine.yml"), template.replace(ENGINE_REF, bumped));
  const r = runStep(headNotifyStep, box, { ENGINE_HEAD: HEAD });
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.match(r.stdout, new RegExp(`ENGINE_REF（${bumped}）`));
});

test("Issue への記録に失敗したら記録の手順も落ちる（黙って通さない）。これまでの記録が読めなければ足す側に倒す", () => {
  const failComment = sandbox({ gh: `[ "$1" = api ] && exit 0; exit 1` });
  assert.notEqual(runStep(headNotifyStep, failComment, { ENGINE_HEAD: HEAD }).status, 0);
  const failApi = sandbox({ gh: `[ "$1" = api ] && exit 1; exit 0` });
  assert.equal(runStep(headNotifyStep, failApi, { ENGINE_HEAD: HEAD }).status, 0);
  assert.match(failApi.calls(), /^gh issue comment 948 /m);
});

test("写し（.github/workflows/sendback-engine-head.yml）があれば、ひな形と同じ", {
  skip: existsSync(headCopyPath) ? false : "まだ .github/workflows/ に写していない",
}, () => {
  assert.equal(readFileSync(headCopyPath, "utf8").trimEnd(), headTemplate.trimEnd(), "写しがひな形とずれている。ひな形を直してから写し直す");
});

// ハーネスのずれと Engine の判定の変化を分ける（議決 D-0031、Issue #1046）。Engine の先頭が新しい問い合わせ（judgment.decided など）を引くと、
// engine_harness.py の偽の cursor が知らずに engine.test.mjs が落ちるが、それは判定の変化ではない。ハーネスは harness_unknown_query の印で落ち、
// engine-head はそれを engine_head_failed と分けて ::error と Issue #948 の記録に出す。偽の Engine で、ハーネスから記録の手順までを通して確かめる。
const harnessPath = fileURLToPath(new URL("./engine_harness.py", import.meta.url));
/** 偽の Engine が引く、ハーネスの知らない問い合わせ（Engine の 1d811cd の judgment.decided に似せた形） */
const UNKNOWN_SQL = "select d.id from events d where d.name='judgment.decided' and d.payload->>'reviewed_pr'=%s";

/** engine_harness.py が取り出す関数だけを持つ偽の Engine を git のチェックアウトとして作る。inspector は inspector_approved の中身 */
function fakeEngine({ inspector = '    return True, "承認"', act = "    pass" } = {}) {
  const dir = mkdtempSync(join(tmpdir(), "sendback-fake-engine-"));
  const disp = join(dir, "orchestrator", "dispatcher");
  mkdirSync(disp, { recursive: true });
  writeFileSync(join(disp, "judgment.py"), "def record_judged(cur, corr, job_id, pr_url, inp, verdicts, mode, note):\n    return {}\n");
  writeFileSync(join(disp, "dispatcher.py"), [
    "def _quote(s):\n    return s\n", "def is_body_only(d):\n    return False\n", "def enqueue_body_fix(*a, **k):\n    pass\n",
    "def file_sendback(*a, **k):\n    pass\n", `def act_on_judgment(cur, repo, corr, job_id, pr_url, summary, mode):\n${act}\n`].join("\n"));
  writeFileSync(join(disp, "supervisor.py"), `def inspector_approved(cur, pr_url, sha):\n${inspector}\n`);
  for (const args of [["init", "-q"], ["add", "."], ["-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "-q", "-m", "x"]]) {
    assert.equal(spawnSync("git", args, { cwd: dir }).status, 0);
  }
  return dir;
}

const UNKNOWN_ENGINE = { inspector: `    cur.execute("${UNKNOWN_SQL}", (pr_url,))\n    return True, "承認"` };
// Engine が例外を握りつぶしても（supervisor.py には except Exception の箇所がある）、知らない問い合わせを引いたことで落とす
const SWALLOWING_ENGINE = { inspector: `    try:\n        cur.execute("${UNKNOWN_SQL}", (pr_url,))\n    except Exception:\n        pass\n    return True, "承認"` };
// 判定が変わった Engine（直すもののない PR でも差し戻しを起こす）。ハーネスは知っている問い合わせだけで動く
const CHANGED_ENGINE = { act: '    record_event(cur, "sendback.filed", corr, job_id, {"pr_url": pr_url})' };

const runHarness = (dir) => spawnSync("python3", ["-I", harnessPath, dir], {
  input: JSON.stringify({ pr_url: "https://github.com/guccyon/rally/pull/395", merged: true, judgments: [] }), encoding: "utf8" });

test("engine_harness.py は偽の cursor が知らない問い合わせを受けたら harness_unknown_query の印と終了コード 3 で落ち、判定の結果を出さない", () => {
  for (const [name, engine] of [["そのまま落とす Engine", UNKNOWN_ENGINE], ["例外を握りつぶす Engine", SWALLOWING_ENGINE]]) {
    const r = runHarness(fakeEngine(engine));
    assert.equal(r.status, 3, `${name}: ${r.stdout}${r.stderr}`);
    assert.match(r.stderr, /^harness_unknown_query: .*judgment\.decided/m, name);
    assert.equal(r.stdout, "", name);
  }
  // 判定が変わっただけの Engine では印を出さずに結果を返す（落とすのは engine.test.mjs の判定のテスト）
  const changed = runHarness(fakeEngine(CHANGED_ENGINE));
  assert.equal(changed.status, 0, changed.stderr);
  assert.doesNotMatch(changed.stderr, /harness_unknown_query/);
  assert.deepEqual(JSON.parse(changed.stdout).followups, [{ kind: "sendback" }]);
});

/** sandbox に engine.test.mjs とその道具を写し、engine-head のテストの手順を本物の node で、記録の手順を偽の gh で動かす */
function runEngineHead(engineDir, gh = "exit 0") {
  const box = sandbox({ gh });
  for (const f of ["engine.test.mjs", "engine_harness.py", "check.mjs", "order.mjs"]) {
    writeFileSync(join(box.dir, f), readFileSync(fileURLToPath(new URL(`./${f}`, import.meta.url))));
  }
  const ghEnv = join(box.root, "github_env");
  const env = { ...process.env, ENGINE_DIR: engineDir, GITHUB_ENV: ghEnv, GITHUB_STEP_SUMMARY: join(box.root, "summary") };
  delete env.NODE_TEST_CONTEXT;   // 外の node --test の子として報告させない
  const test = spawnSync("bash", ["-e", "-o", "pipefail", "-c", runScript(headTestStep)], { cwd: box.dir, env, encoding: "utf8" });
  const head = readFileSync(ghEnv, "utf8").match(/^ENGINE_HEAD=(\S+)$/m)[1];
  const notify = runStep(headNotifyStep, box, { ENGINE_HEAD: head });
  const body = existsSync(join(box.dir, "engine-head-comment.md")) ? readFileSync(join(box.dir, "engine-head-comment.md"), "utf8") : "";
  return { test, notify, head, body, calls: box.calls() };
}

test("先頭の Engine がハーネスの知らない問い合わせを引いて落ちたら、engine_head_failed ではなく harness_unknown_query で知らせて Issue #948 に記録する", () => {
  const { test: headRun, notify, head, body, calls } = runEngineHead(fakeEngine(UNKNOWN_ENGINE));
  assert.notEqual(headRun.status, 0, "engine.test.mjs が通った");
  assert.equal(notify.status, 0, notify.stdout + notify.stderr);
  assert.match(notify.stdout, new RegExp(`^::error title=sendback-engine-head harness_unknown_query::.*${head}.*ハーネスのずれ`, "m"));
  assert.doesNotMatch(notify.stdout, /engine_head_failed/);
  assert.match(calls, /^gh issue comment 948 --repo guccyon\/rally --body-file engine-head-comment\.md$/m);
  assert.ok(body.startsWith(`<!-- sendback-engine-head: ${head} harness_unknown_query -->\n`), body);
  assert.match(body, /ハーネスのずれ/);
  assert.match(body, /^harness_unknown_query: .*judgment\.decided/m);
  assert.match(body, /^not ok \d+ - \(1\) /m);
});

test("先頭の Engine の判定が変わって落ちたら、これまでどおり engine_head_failed で知らせ、harness_unknown_query は出さない", () => {
  const { test: headRun, notify, head, body } = runEngineHead(fakeEngine(CHANGED_ENGINE));
  assert.notEqual(headRun.status, 0, "engine.test.mjs が通った");
  assert.equal(notify.status, 0, notify.stdout + notify.stderr);
  assert.match(notify.stdout, new RegExp(`^::error title=sendback-engine-head engine_head_failed::.*${head}`, "m"));
  assert.doesNotMatch(notify.stdout + body, /harness_unknown_query/);
  assert.ok(body.startsWith(`<!-- sendback-engine-head: ${head} engine_head_failed -->\n`), body);
  assert.match(body, /^not ok \d+ - \(1\) /m);
});

test("同じ先頭でも、ハーネスのずれの記録があるだけなら判定の変化の記録は足す（ハーネスを直したあとの実行を埋もれさせない）", () => {
  const box = sandbox({ gh: `[ "$1" = api ] && printf '%s\\n' "<!-- sendback-engine-head: ${HEAD} harness_unknown_query -->"; exit 0` });
  const r = runStep(headNotifyStep, box, { ENGINE_HEAD: HEAD });
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.match(r.stdout, /^::error title=sendback-engine-head engine_head_failed::/m);
  assert.match(box.calls(), /^gh issue comment 948 /m);
});

// 通ったときも記録する（議決 D-0031、Issue #1047）。先頭の Engine での実行の記録（通った・落ちた）が #948 に 1 件残ってから、
// Issue #977 の解決策の wired と verified を true にするため。落ちたときだけ記録していると、通り続ける限り記録が残らない。
test("先頭で通ったら ::error を出さず、Issue #948 に engine_head_passed として先頭の版・ENGINE_REF・実行の URL を記録する", () => {
  const box = sandbox({ gh: `[ "$1" = api ] && printf '%s\\n' "<!-- sendback-engine-head: ${HEAD} engine_head_failed -->"; exit 0` });
  writeFileSync(join(box.dir, "engine-head.tap"), TAP);
  const r = runStep(headNotifyStep, box, { ENGINE_HEAD: HEAD, HEAD_OUTCOME: "success" });
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.doesNotMatch(r.stdout, /::error/);
  assert.match(r.stdout, new RegExp(`${HEAD}.*通った`));
  assert.match(box.calls(), /^gh issue comment 948 --repo guccyon\/rally --body-file engine-head-comment\.md$/m);
  const body = readFileSync(join(box.dir, "engine-head-comment.md"), "utf8");
  assert.ok(body.startsWith(`<!-- sendback-engine-head: ${HEAD} engine_head_passed -->\n`), body);
  assert.match(body, new RegExp(`\`${ENGINE_REF}\``));
  assert.match(body, /https:\/\/github\.com\/guccyon\/rally\/actions\/runs\/123/);
  assert.doesNotMatch(body, /落ちたテスト|not ok/, "通ったときは落ちたテストを書かない");
  // 書いた記録は head-record.mjs が engine-head の記録に数える
  assert.deepEqual(headRecords([{ user: { login: "github-actions[bot]" }, body }]).map((x) => [x.head, x.kind]), [[HEAD, "engine_head_passed"]]);
});

test("同じ先頭の通った記録が Issue #948 にすでにあれば足さない（毎日同じ記録を積まない）", () => {
  const box = sandbox({ gh: `[ "$1" = api ] && printf '%s\\n' "<!-- sendback-engine-head: ${HEAD} engine_head_passed -->"; exit 0` });
  const r = runStep(headNotifyStep, box, { ENGINE_HEAD: HEAD, HEAD_OUTCOME: "success" });
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.doesNotMatch(box.calls(), /^gh issue comment/m);
});

test("テストの手順が通っていない（failure・skipped・cancelled・渡されない）ときは通ったとして記録しない", () => {
  for (const outcome of ["failure", "skipped", "cancelled", undefined]) {
    const box = sandbox({ gh: "exit 0" });
    const r = runStep(headNotifyStep, box, { ENGINE_HEAD: HEAD, ...(outcome === undefined ? {} : { HEAD_OUTCOME: outcome }) });
    assert.equal(r.status, 0, r.stdout + r.stderr);
    assert.match(r.stdout, /^::error title=sendback-engine-head engine_head_failed::/m, String(outcome));
    assert.ok(readFileSync(join(box.dir, "engine-head-comment.md"), "utf8").startsWith(`<!-- sendback-engine-head: ${HEAD} engine_head_failed -->\n`));
  }
});

// 必須のチェックが後で外されたことに気づく（議決 D-0039、Issue #1087）。protection.mjs を一度走らせるだけでは、必須にしたあとで外されても気づけない。
// ci/sendback-protection.yml が branch protection を読めるトークンで毎日と手動で照らし、外れたら Issue を開くか更新する。手順を偽の gh で動かして確かめる。
const protTemplate = readFileSync(fileURLToPath(new URL("./ci/sendback-protection.yml", import.meta.url)), "utf8");
const protCopyPath = fileURLToPath(new URL("../../.github/workflows/sendback-protection.yml", import.meta.url));
const protSteps = jobSteps(protTemplate, "protection");
const protReadStep = protSteps.find((s) => /branch protection を読む/.test(s[0]));
const protCheckStep = protSteps.find((s) => /必須のチェックを照らす/.test(s[0]));
const protIssueStep = protSteps.find((s) => /外れたら Issue を開くか更新する/.test(s[0]));
const protPrSteps = jobSteps(protTemplate, "protection-pr");
const protPrReadStep = protPrSteps.find((s) => /branch protection を読む/.test(s[0]));
const protPrCheckStep = protPrSteps.find((s) => /必須のチェックを照らす/.test(s[0]));
const ISSUE_TITLE = protTemplate.match(/^ {2}ISSUE_TITLE: "(.+)"$/m)?.[1];

/** 古い形の branch protection の JSON（protection.test.mjs と同じ形） */
const classicJson = (contexts) => JSON.stringify({ required_status_checks: { strict: true, contexts, checks: contexts.map((context) => ({ context })) } });
const rulesJson = (contexts) => JSON.stringify(contexts.length === 0 ? []
  : [{ type: "required_status_checks", parameters: { required_status_checks: contexts.map((context) => ({ context })) } }]);

/**
 * 偽の gh。classic・rules は gh api の branch protection と rulesets の応答（{ out, rc }）、open は開いている Issue の list の出力、
 * recorded は Issue の本文とコメント、create は gh issue create の終了コード。
 */
function fakeProtGh({ classic = { out: classicJson(Object.values(CHECK_NAMES)) }, rules = { out: "[]" }, open = "", recorded = "", create = 0 } = {}) {
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
 * sandbox に protection.mjs と（あれば）sendback-engine.yml の写しとほかのワークフロー（others: { ファイル名: 中身 }）を置き、
 * 読む・照らす・Issue の 3 つの手順を順に動かす。pr が true なら protection-pr のジョブの読む・照らす手順を動かす（Issue の手順はない）。
 */
function runProtection({ gh = {}, copy = template, others = {}, token = "t", pr = false } = {}) {
  const box = sandbox({ gh: fakeProtGh(gh) });
  for (const f of ["protection.mjs", "order.mjs"]) writeFileSync(join(box.dir, f), readFileSync(fileURLToPath(new URL(`./${f}`, import.meta.url))));
  mkdirSync(join(box.root, ".github", "workflows"), { recursive: true });
  if (copy != null) writeFileSync(join(box.root, ".github", "workflows", "sendback-engine.yml"), copy);
  for (const [name, text] of Object.entries(others)) writeFileSync(join(box.root, ".github", "workflows", name), text);
  const env = { BRANCH: "main", ISSUE_TITLE, GITHUB_STEP_SUMMARY: join(box.root, "summary"), GITHUB_ACTIONS: "true" };
  const read = runStep(pr ? protPrReadStep : protReadStep, { ...box, dir: box.root }, { ...env, GH_TOKEN: token });
  const check = read.status === 0 ? runStep(pr ? protPrCheckStep : protCheckStep, box, env) : null;
  const outcome = check === null ? "skipped" : check.status === 0 ? "success" : "failure";
  const issue = pr ? null : runStep(protIssueStep, box, { ...env, GH_TOKEN: "github-token", CHECK_OUTCOME: outcome });
  const commentPath = join(box.dir, "protection-comment.md");
  return { read, check, issue, calls: box.calls(), body: existsSync(commentPath) ? readFileSync(commentPath, "utf8") : "" };
}

/** 注釈の行を除いたひな形の行と、on: の中身（字下げを外したもの） */
function protOn() {
  const lines = protTemplate.split(/\r?\n/).filter((l) => !/^\s*#/.test(l));
  const on = lines.findIndex((l) => /^on:\s*$/.test(l));
  const onBody = lines.slice(on + 1, lines.findIndex((l, i) => i > on && /^\S/.test(l))).map((l) => l.trim()).filter((l) => l !== "");
  return { lines, onBody };
}

test("sendback-protection の protection は定期（schedule）と手動でだけ走り、Issue を開く。トークンは branch protection を読む手順だけに渡す", () => {
  const { lines, onBody } = protOn();
  assert.ok(onBody.some((l) => /^- cron: "[^"]+"$/.test(l)), "schedule の cron がない");
  assert.ok(!lines.some((l) => /^\s*(- )?continue-on-error:/.test(l)), "continue-on-error があると外れても気づかない");
  assert.ok(!lines.some((l) => /pull_request_target/.test(l)), "pull_request_target ではシークレットのある実行でほかのブランチの中身を照らすことになる");
  assert.match(protTemplate, /^permissions:\n {2}contents: read\n\n/m, "ワークフローの既定は contents: read だけ");
  const job = jobBody(protTemplate, "protection");
  assert.ok(job.some((l) => /^ {4}if: github\.event_name == 'schedule' \|\| github\.event_name == 'workflow_dispatch'$/.test(l)), "protection は定期と手動でだけ走らせる");
  assert.ok(job.some((l) => /^ {6}issues: write$/.test(l)), "Issue を開くので protection のジョブに issues: write が要る");
  const secretLines = lines.filter((l) => /secrets\./.test(l));
  assert.deepEqual(secretLines.map((l) => l.trim()), Array(2).fill("GH_TOKEN: ${{ secrets.SENDBACK_PROTECTION_TOKEN }}"), "トークンは 2 つのジョブの読む手順だけ");
  assert.ok(protReadStep.some((l) => /secrets\.SENDBACK_PROTECTION_TOKEN/.test(l)), "トークンを渡すのは branch protection を読む手順だけ");
  assert.ok(!protReadStep.some((l) => /^\s*if:/.test(l)) && !protCheckStep.some((l) => /^\s*if:/.test(l)), "読む・照らす手順を飛ばさない");
  assert.ok(protCheckStep.some((l) => /^\s*id: check$/.test(l)));
  assert.match(runScript(protCheckStep), /node protection\.mjs --protection \S+ --protection \S+/, "古い形と rulesets の両方を渡す");
  assert.ok(protIssueStep.some((l) => /^\s*if: always\(\)$/.test(l)), "Issue の手順は照らす手順が落ちたときも走らせる");
  assert.ok(protIssueStep.some((l) => /^\s*CHECK_OUTCOME: \$\{\{ steps\.check\.outcome \}\}$/.test(l)));
  assert.ok(protSteps.indexOf(protReadStep) < protSteps.indexOf(protCheckStep) && protSteps.indexOf(protCheckStep) < protSteps.indexOf(protIssueStep));
  assert.ok(ISSUE_TITLE, "ISSUE_TITLE がない");
});

// PR ごとにも照らす（議決 D-0059（起票時の番号は D-0056）、Issue #1158）。定期だけでは、必須のチェックを外したあとの PR や、ジョブの名前を変える・同じチェックの名前を
// 報告するジョブを足す PR が、次の定期の実行まで通ってしまう。protection-pr がその PR の .github/workflows/ と本物の branch protection を照らして落とす。
test("sendback-protection の protection-pr は絞り込みのない pull_request で走り、読む・照らす手順は protection と同じで、Issue は開かない", () => {
  const { lines, onBody } = protOn();
  assert.deepEqual(onBody.filter((l) => /^[\w-]+:$/.test(l)), ["schedule:", "workflow_dispatch:", "pull_request:"],
    "schedule・workflow_dispatch・pull_request のほかで走らせない（pull_request は paths・branches・types で絞らない）");
  const job = jobBody(protTemplate, "protection-pr");
  assert.ok(job.some((l) => /^ {4}if: github\.event_name == 'pull_request'$/.test(l)), "protection-pr は PR でだけ走らせる");
  assert.ok(job.some((l) => /^ {6}contents: read$/.test(l)));
  assert.ok(!job.some((l) => /issues: write|pull-requests: write/.test(l)), "PR のジョブに書く権限を持たせない");
  assert.ok(!job.some((l) => /\bgh issue\b/.test(l)), "PR のジョブでは Issue を開かない");
  assert.ok(!lines.some((l) => /^\s*(- )?continue-on-error:/.test(l)));
  assert.deepEqual(protPrSteps.map((s) => s.join("\n").trimEnd()), protSteps.slice(0, protSteps.indexOf(protIssueStep)).map((s) => s.join("\n").trimEnd()),
    "protection-pr の手順は protection の読む・照らす手順と同じにする");
  assert.ok(!protPrSteps.some((s) => s.some((l) => /^\s*if:/.test(l))), "読む・照らす手順を飛ばさない");
});

test("PR ごとの照合: 必須のチェックがそろっていれば通る", () => {
  const r = runProtection({ pr: true });
  assert.equal(r.read.status, 0, r.read.stdout + r.read.stderr);
  assert.equal(r.check.status, 0, r.check.stdout + r.check.stderr);
  assert.doesNotMatch(r.check.stdout, /::error/);
  assert.doesNotMatch(r.calls, /^gh issue/m);
});

test("PR ごとの照合: required_check_missing・check_name_changed・check_name_shared・workflow_not_copied は ::error で落ち、Issue は開かない", () => {
  const renamed = template.replace(`name: ${CHECK_NAMES["wiring-order"]}`, "name: wiring-order");
  assert.notEqual(renamed, template);
  const shared = `name: other\non:\n  pull_request:\njobs:\n  build:\n    name: ${CHECK_NAMES["engine-test"]}\n    runs-on: ubuntu-latest\n    steps:\n      - run: "true"\n`;
  for (const [rule, opts] of [
    ["required_check_missing", { gh: { classic: { out: classicJson([CHECK_NAMES["engine-test"]]) } } }],
    ["required_check_missing", { gh: { classic: { out: '{"message":"Branch not protected","status":"404"}', rc: 1 } } }],
    ["check_name_changed", { copy: renamed, gh: { classic: { out: classicJson([...Object.values(CHECK_NAMES), "wiring-order"]) } } }],
    ["check_name_shared", { others: { "other.yml": shared } }],
    ["workflow_not_copied", { copy: null }],
  ]) {
    const r = runProtection({ ...opts, pr: true });
    assert.equal(r.read.status, 0, rule + r.read.stdout + r.read.stderr);
    assert.equal(r.check.status, 1, `${rule}: ${r.check.stdout}${r.check.stderr}`);
    assert.match(r.check.stdout, new RegExp(`^::error title=sendback-check ${rule}::`, "m"), rule);
    assert.equal(r.issue, null);
    assert.doesNotMatch(r.calls, /^gh issue/m, rule);
  }
});

test("PR ごとの照合: トークンがない（フォークからの PR など）・読めないときは protection_unreadable で落ちる", () => {
  for (const opts of [{ token: "" }, { gh: { rules: { out: '{"message":"Not Found"}', rc: 1 } } }]) {
    const r = runProtection({ ...opts, pr: true });
    assert.equal(r.read.status, 1);
    assert.match(r.read.stdout, /^::error title=sendback-protection protection_unreadable::/m);
    assert.equal(r.check, null);
  }
});

test("必須のチェックがそろっていれば通り、Issue は開かない", () => {
  const r = runProtection();
  assert.equal(r.read.status, 0, r.read.stdout + r.read.stderr);
  assert.equal(r.check.status, 0, r.check.stdout + r.check.stderr);
  assert.equal(r.issue.status, 0, r.issue.stdout + r.issue.stderr);
  assert.doesNotMatch(r.calls, /^gh issue (create|comment|close)/m);
});

test("rulesets だけで必須にしていても通る（古い形は保護なしの 404）", () => {
  const r = runProtection({ gh: { classic: { out: '{"message":"Branch not protected","status":"404"}', rc: 1 }, rules: { out: rulesJson(Object.values(CHECK_NAMES)) } } });
  assert.equal(r.read.status, 0, r.read.stdout + r.read.stderr);
  assert.equal(r.check.status, 0, r.check.stdout + r.check.stderr);
  assert.doesNotMatch(r.calls, /^gh issue (create|comment)/m);
});

test("必須のチェックが後で外されたら required_check_missing で落ち、開いている Issue がなければ開く", () => {
  const r = runProtection({ gh: { classic: { out: classicJson([CHECK_NAMES["engine-test"]]) } } });
  assert.equal(r.check.status, 1);
  assert.match(r.check.stdout, /^::error title=sendback-check required_check_missing::wiring-order/m);
  assert.equal(r.issue.status, 0, r.issue.stdout + r.issue.stderr);
  assert.match(r.calls, new RegExp(`^gh issue create --repo guccyon/rally --title ${ISSUE_TITLE.replace(/[()（）・]/g, ".")} --body-file protection-comment\\.md$`, "m"));
  assert.match(r.body, /^<!-- sendback-protection: [0-9a-f]{16} -->\n/);
  assert.match(r.body, /^required_check_missing: wiring-order のジョブ/m);
  assert.match(r.body, /https:\/\/github\.com\/guccyon\/rally\/actions\/runs\/123/);
});

test("保護ごと外された（古い形が 404・rulesets も空）ときも required_check_missing で Issue を開く", () => {
  const r = runProtection({ gh: { classic: { out: '{"message":"Branch not protected","status":"404"}', rc: 1 } } });
  assert.equal(r.check.status, 1);
  assert.match(r.body, /^required_check_missing: engine-test/m);
  assert.match(r.calls, /^gh issue create /m);
});

test("写しが消えたら workflow_not_copied で落ちて Issue を開く", () => {
  const r = runProtection({ copy: null });
  assert.equal(r.check.status, 1);
  assert.match(r.body, /^workflow_not_copied: /m);
  assert.match(r.calls, /^gh issue create /m);
});

test("開いている Issue があれば、それにコメントで足す（同じ外れ方の記録がすでにあれば足さない）", () => {
  const first = runProtection({ gh: { classic: { out: classicJson([CHECK_NAMES["engine-test"]]) }, open: "42", recorded: "前の記録" } });
  assert.doesNotMatch(first.calls, /^gh issue create/m);
  assert.match(first.calls, /^gh issue comment 42 --repo guccyon\/rally --body-file protection-comment\.md$/m);
  const marker = first.body.split("\n")[0];
  const again = runProtection({ gh: { classic: { out: classicJson([CHECK_NAMES["engine-test"]]) }, open: "42", recorded: `本文\n${marker}` } });
  assert.equal(again.issue.status, 0, again.issue.stdout + again.issue.stderr);
  assert.doesNotMatch(again.calls, /^gh issue (create|comment)/m);
  // 外れ方が変われば足す
  const other = runProtection({ gh: { classic: { out: classicJson([]) }, open: "42", recorded: marker } });
  assert.match(other.calls, /^gh issue comment 42 /m);
});

test("通ったら、開いている Issue に通ったことを書いて閉じる", () => {
  const r = runProtection({ gh: { open: "42" } });
  assert.equal(r.issue.status, 0, r.issue.stdout + r.issue.stderr);
  assert.match(r.calls, /^gh issue comment 42 --repo guccyon\/rally --body-file protection-comment\.md$/m);
  assert.match(r.calls, /^gh issue close 42 --repo guccyon\/rally$/m);
  assert.match(r.body, /必須のチェックに戻った/);
});

test("トークンがない・branch protection を読めないときは protection_unreadable で落ち、照らさず Issue も開かない", () => {
  for (const [name, opts] of [["トークンがない", { token: "" }], ["権限がない（403）", { gh: { classic: { out: '{"message":"Resource not accessible by personal access token","status":"403"}', rc: 1 } } }],
    ["rulesets を読めない", { gh: { rules: { out: '{"message":"Not Found"}', rc: 1 } } }]]) {
    const r = runProtection(opts);
    assert.equal(r.read.status, 1, name);
    assert.match(r.read.stdout, /^::error title=sendback-protection protection_unreadable::/m, name);
    assert.equal(r.check, null);
    assert.equal(r.issue.status, 0, name);
    assert.doesNotMatch(r.calls, /^gh issue (create|comment)/m, name);
  }
});

test("Issue を開けなければ Issue の手順も落ちる（黙って通さない）", () => {
  const r = runProtection({ gh: { classic: { out: classicJson([]) }, create: 1 } });
  assert.notEqual(r.issue.status, 0);
});

test("写し（.github/workflows/sendback-protection.yml）があれば、ひな形と同じ", {
  skip: existsSync(protCopyPath) ? false : "まだ .github/workflows/ に写していない",
}, () => {
  assert.equal(readFileSync(protCopyPath, "utf8").trimEnd(), protTemplate.trimEnd(), "写しがひな形とずれている。ひな形を直してから写し直す");
});
