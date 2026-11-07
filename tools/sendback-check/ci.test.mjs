// ci/sendback-engine.yml の ENGINE_REF の見張りのテスト（知見 K-0004 の担保。議決 D-0014、Issue #960）。
// ENGINE_REF がブランチ名やタグだと同じ PR でも結果が変わるので、engine-test の最初の手順（どのチェックアウトよりも前）で
// 40 桁のコミット SHA かを確かめ、違えば落とす。その手順を取り出して bash で動かす。
// .github/workflows/sendback-engine.yml に写してあれば、ENGINE_REF の行のほかがこのひな形と同じかも見る（写すときに変えてよいのはその行だけ）。
// ひな形が必須のチェックにできる形（PR ごとに必ず走り、落ちたら止める）で、見張りの手順を持っていることも見る（議決 D-0017、Issue #969）。
// 写しとのずれも見るので、見張りを外す変更はひな形と写しを一緒に直しても wiring-order で落ちる。写しを消す・ジョブの名前を変える変更は、
// 必須のチェック（engine-test・wiring-order）が報告されずに止まる（branch protection は建設者が入れる）。
// ENGINE_REF の行を変える PR で engine-test が必ずその版で走る形か（ENGINE_REF の置き場所・needs・Engine を取る版）も、ひな形と写しで見る（議決 D-0028、Issue #1024）。
// engine-test が engine.test.mjs の TAP を engine-test.tap に出し、order.mjs --engine-report に渡して外れたら落ちる形かも、手順を偽の node で動かして見る（議決 D-0028、Issue #1027）。
// ci/sendback-engine-head.yml（Engine の既定のブランチの先頭で定期に走らせ、落ちたら知らせる。議決 D-0020、Issue #977）の形と記録の手順の振る舞いも見る。
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

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
  else {
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
    "ジョブを飛ばす": template.replace("  wiring-order:\n    runs-on", "  wiring-order:\n    if: false\n    runs-on"),
    "手順を飛ばす": template.replace("      - name: ENGINE_REF の見張りと写しのずれ（K-0004）\n", "      - name: ENGINE_REF の見張りと写しのずれ（K-0004）\n        if: false\n"),
    "落ちても止めない": template.replace("  engine-test:\n    runs-on", "  engine-test:\n    continue-on-error: true\n    runs-on"),
    "ジョブの名前を変える": template.replace("  wiring-order:\n", "  order:\n"),
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
    "engine-test が needs で待つ": template.replace("  engine-test:\n    runs-on", "  engine-test:\n    needs: wiring-order\n    runs-on"),
    "engine-test の needs を列で書く": template.replace("  engine-test:\n    runs-on", "  engine-test:\n    needs:\n      - wiring-order\n    runs-on"),
    "ジョブの env で ENGINE_REF を上書きする": template.replace("  engine-test:\n    runs-on: ubuntu-latest\n",
      `  engine-test:\n    runs-on: ubuntu-latest\n    env:\n      ENGINE_REF: ${"f".repeat(40)}\n`),
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

// engine-test が engine.test.mjs の結果を order.mjs --engine-report に渡して、外れたら落ちるか（議決 D-0028、Issue #1027）。
// 渡す手順が消える・別の TAP を渡す・`|| true` や exit 0 で落ちても通す形だと、ENGINE_REF の Engine で (3) か (c) が通っていなくても
// engine-test が通り、配線の順序の見張り（D-0016）が効かない。書き方の言い回しで逃げられないよう、engine-test の手順を偽の node で
// GitHub Actions と同じ順に動かし（落ちた手順で止める）、振る舞いで見る。

/** 手順の run の中身（`run: |` の塊か 1 行の run）。run のない手順は null。`${{ … }}` は値に置き換える。 */
function stepRun(step, values) {
  const block = step.findIndex((l) => /^\s*(- )?run: \|\s*$/.test(l));
  const script = block >= 0 ? runScript(step) : step.map((l) => l.match(/^\s*(?:- )?run: (?![|>])(.+)$/)?.[1]).find((s) => s != null);
  return script == null ? null : `${script}\n`.replace(/\$\{\{\s*([^}]*?)\s*\}\}/g, (_, k) => values[k] ?? "");
}

/** 偽の node。engine.test.mjs なら --test-reporter と --test-reporter-destination の組ごとに出し、order.mjs なら --engine-report の中身を記録する。 */
const FAKE_NODE = `import { appendFileSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
const args = process.argv.slice(2);
const log = (o) => appendFileSync(process.env.FAKE_LOG, JSON.stringify(o) + "\\n");
const value = (name) => {
  const out = [];
  args.forEach((a, i) => { if (a === name) out.push(args[i + 1]); else if (a.startsWith(name + "=")) out.push(a.slice(name.length + 1)); });
  return out;
};
if (args.includes("--test") && args.some((a) => /(^|\\/)engine\\.test\\.mjs$/.test(a))) {
  const dests = value("--test-reporter-destination");
  value("--test-reporter").forEach((r, i) => {
    const text = r === "tap" ? "TAP version 13\\nok 1 - (3)\\n1..1\\n" : "spec の出力\\n";
    const dest = dests[i] ?? "stdout";
    if (dest === "stdout") process.stdout.write(text); else if (dest === "stderr") process.stderr.write(text); else writeFileSync(dest, text);
  });
  log({ engineTest: true });
  process.exit(Number(process.env.FAKE_TEST_RC));
}
if (args.some((a) => /(^|\\/)order\\.mjs$/.test(a))) {
  const report = value("--engine-report")[0];
  const path = report == null ? null : resolve(report);
  log({ order: true, report: path, tap: path != null && existsSync(path) && readFileSync(path, "utf8").startsWith("TAP version 13") });
  process.exit(Number(process.env.FAKE_ORDER_RC));
}
`;

/** engine-test の手順を、engine.test.mjs を走らせる手順から偽の node で順に動かす。落ちた手順で止める（if: は使わない形なので見ない）。 */
function simulateEngineTest(text, { testRc, orderRc }) {
  const root = mkdtempSync(join(tmpdir(), "sendback-engine-report-"));
  const bin = join(root, "bin");
  mkdirSync(bin);
  writeFileSync(join(root, "fake-node.mjs"), FAKE_NODE);
  writeFileSync(join(bin, "node"), `#!/usr/bin/env bash\nexec "${process.execPath}" "${join(root, "fake-node.mjs")}" "$@"\n`);
  chmodSync(join(bin, "node"), 0o755);
  const log = join(root, "calls.jsonl");
  const steps = jobSteps(text, "engine-test");
  const start = steps.findIndex((s) => /node --test (.* )?engine\.test\.mjs(\s|$)/.test(stepRun(s, {}) ?? ""));
  let status = 0;
  for (const step of start < 0 ? [] : steps.slice(start)) {
    const script = stepRun(step, { "github.workspace": root, "env.ENGINE_REF": ENGINE_REF });
    if (script == null) continue;
    const cwd = join(root, step.map((l) => l.match(/^\s*(?:- )?working-directory: (\S+)\s*$/)?.[1]).find((d) => d != null) ?? "");
    mkdirSync(cwd, { recursive: true });
    const shell = step.some((l) => /^\s*(- )?shell: bash\s*$/.test(l)) ? ["--noprofile", "--norc", "-e", "-o", "pipefail"] : ["-e"];
    status = spawnSync("bash", [...shell, "-c", script], {
      cwd,
      env: { ...process.env, PATH: `${bin}:${process.env.PATH}`, FAKE_LOG: log, FAKE_TEST_RC: String(testRc), FAKE_ORDER_RC: String(orderRc),
        ENGINE_REF, GITHUB_STEP_SUMMARY: join(root, "summary"), GITHUB_WORKSPACE: root },
      encoding: "utf8",
    }).status;
    if (status !== 0) break;
  }
  const calls = existsSync(log) ? readFileSync(log, "utf8").trim().split("\n").map((l) => JSON.parse(l)) : [];
  return { ran: start >= 0, status, calls };
}

/** engine-test が engine.test.mjs の TAP を engine-test.tap に出し、order.mjs --engine-report に渡して外れたら落ちるかの外れ（空なら通る）。注釈の行は見ない。 */
function engineReportProblems(text) {
  const problems = [];
  const lines = text.split(/\r?\n/).filter((l) => !/^\s*#/.test(l));
  const body = jobBody(lines.join("\n"), "engine-test");
  if (body === null) return ["engine-test のジョブがない（必須のチェックの名前）"];
  if (body.some((l) => /^ {4}(continue-on-error|if):/.test(l))) problems.push("engine-test のジョブに if: か continue-on-error があると、飛ばされるか落ちても通る");
  const steps = jobSteps(lines.join("\n"), "engine-test");
  const start = steps.findIndex((s) => /node --test (.* )?engine\.test\.mjs(\s|$)/.test(stepRun(s, {}) ?? ""));
  if (start < 0) return [...problems, "engine-test が engine.test.mjs を走らせていない"];
  // engine.test.mjs の手順から order.mjs を呼ぶ手順まで（後ろの手順も含む）に if: と continue-on-error がないこと
  for (const step of steps.slice(start)) {
    if (step.some((l) => /^\s*(- )?(continue-on-error|if):/.test(l))) {
      problems.push(`engine-test の手順「${step[0].trim()}」に if: か continue-on-error がある（order.mjs --engine-report が飛ばされるか、落ちても通る）`);
    }
  }
  const pass = simulateEngineTest(lines.join("\n"), { testRc: 0, orderRc: 0 });
  const orders = pass.calls.filter((c) => c.order);
  if (pass.status !== 0) problems.push("engine.test.mjs と order.mjs が通っても engine-test が通らない");
  if (orders.length === 0) problems.push("engine-test が engine.test.mjs の後で order.mjs --engine-report を呼ばない");
  for (const c of orders) {
    if (c.report == null) problems.push("order.mjs に --engine-report を渡していない");
    else if (!c.report.endsWith("/engine-test.tap")) problems.push(`order.mjs --engine-report に engine-test.tap でないファイル（${c.report.split("/").pop()}）を渡している`);
    else if (!c.tap) problems.push("order.mjs --engine-report に渡す engine-test.tap が、その前に engine.test.mjs が出した TAP でない");
  }
  if (orders.length > 0 && simulateEngineTest(lines.join("\n"), { testRc: 0, orderRc: 1 }).status === 0) {
    problems.push("order.mjs --engine-report が外れても engine-test が通る（|| true や exit 0 で落ちても止めない）");
  }
  if (simulateEngineTest(lines.join("\n"), { testRc: 1, orderRc: 0 }).status === 0) problems.push("engine.test.mjs が落ちても engine-test が通る");
  return problems;
}

test("ひな形の engine-test は engine.test.mjs の TAP を engine-test.tap に出し、order.mjs --engine-report に渡して、外れたら落ちる", () => {
  assert.deepEqual(engineReportProblems(template), []);
  const r = simulateEngineTest(template, { testRc: 1, orderRc: 1 });
  assert.equal(r.status, 1);
  // テストが落ちても結果は渡す（同じ手順で呼ぶので）
  assert.ok(r.calls.some((c) => c.order && c.tap), "engine.test.mjs が落ちたときに order.mjs --engine-report を呼んでいない");
});

test("order.mjs --engine-report を外す・別の TAP を渡す・落ちても通す・飛ばせる形にする直しは外れになり、後ろの手順に if: なしで分けるのは通る", () => {
  const ORDER = "          node order.mjs --engine-report engine-test.tap --summary \"$GITHUB_STEP_SUMMARY\" || rc=1\n";
  const later = (extra) => `${template.replace(ORDER, "").trimEnd()}\n      - name: 配線の順序（engine-test の結果）\n${extra}`
    + "        working-directory: tools/sendback-check\n        run: node order.mjs --engine-report engine-test.tap --summary \"$GITHUB_STEP_SUMMARY\"\n";
  assert.ok(template.includes(ORDER), "ひな形の order.mjs の行が変わった。このテストの置き換えを直す");
  const cases = {
    "order.mjs の呼び出しを外す": template.replace(ORDER, ""),
    "--engine-report を外す": template.replace("node order.mjs --engine-report engine-test.tap", "node order.mjs"),
    "|| true で落ちても通す": template.replace(ORDER, ORDER.replace("|| rc=1", "|| true")),
    "exit 0 で終える": template.replace("          exit $rc\n", "          exit 0\n"),
    "別の TAP を渡す": template.replace("--engine-report engine-test.tap", "--engine-report other.tap"),
    "TAP を別の名前に出す": template.replace("--test-reporter-destination=engine-test.tap", "--test-reporter-destination=engine.tap"),
    "両方を別の名前にする": template.replace("--test-reporter-destination=engine-test.tap", "--test-reporter-destination=engine.tap")
      .replace("--engine-report engine-test.tap", "--engine-report engine.tap"),
    "TAP の reporter を外す": template.replace("--test-reporter=tap --test-reporter-destination=engine-test.tap", "--test-reporter=spec --test-reporter-destination=engine-test.tap"),
    "engine.test.mjs より前に呼ぶ": template.replace(ORDER, "").replace("          rc=0\n", `          rc=0\n${ORDER}`),
    "後ろの手順で if: always() を付けて呼ぶ": later("        if: always()\n"),
    "後ろの手順で continue-on-error を付けて呼ぶ": later("        continue-on-error: true\n"),
    "テストの手順に continue-on-error を付ける": template.replace("      - name: Engine の本物の判定のテスト（K-0004）\n",
      "      - name: Engine の本物の判定のテスト（K-0004）\n        continue-on-error: true\n"),
    "wiring-order で呼ぶ": template.replace(ORDER, "").replace("\n  engine-test:\n",
      "      - run: node tools/sendback-check/order.mjs --engine-report tools/sendback-check/engine-test.tap\n\n  engine-test:\n"),
  };
  for (const [name, text] of Object.entries(cases)) {
    assert.notEqual(text, template, `${name}: 置き換えが効いていない`);
    assert.notDeepEqual(engineReportProblems(text), [], `${name} が通った`);
  }
  assert.deepEqual(engineReportProblems(later("")), [], "後ろの手順に if: なしで分けた形が外れになった");
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
  assert.deepEqual(engineReportProblems(copy), [], "写しの engine-test が engine.test.mjs の結果を order.mjs --engine-report に渡して落とす形でない");
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
const headNotifyStep = headSteps.find((s) => /先頭で落ちたことを知らせて Issue に記録する/.test(s[0]));

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
  assert.ok(headNotifyStep.some((l) => /^\s*if: failure\(\)$/.test(l)), "記録の手順は落ちたときだけ走らせる");
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
  assert.ok(body.startsWith(`<!-- sendback-engine-head: ${HEAD} -->\n`));
  assert.match(body, new RegExp(`\`${HEAD}\``));
  assert.match(body, new RegExp(`\`${ENGINE_REF}\``));
  assert.match(body, /^not ok 2 - \(3\) basis に欠陥の採用数と仮決めの承認数を分けて書く$/m);
  assert.match(body, /https:\/\/github\.com\/guccyon\/rally\/actions\/runs\/123/);
});

test("同じ先頭の記録が Issue #948 にすでにあれば足さない（::error は出す）", () => {
  const box = sandbox({ gh: `[ "$1" = api ] && printf '%s\\n' "<!-- sendback-engine-head: ${HEAD} -->"; exit 0` });
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
