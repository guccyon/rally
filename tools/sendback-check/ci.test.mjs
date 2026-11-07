// ci/sendback-engine.yml の ENGINE_REF の見張りのテスト（知見 K-0004 の担保。議決 D-0014、Issue #960）。
// ENGINE_REF がブランチ名やタグだと同じ PR でも結果が変わるので、engine-test の最初の手順（どのチェックアウトよりも前）で
// 40 桁のコミット SHA かを確かめ、違えば落とす。その手順を取り出して bash で動かす。
// .github/workflows/sendback-engine.yml に写してあれば、ENGINE_REF の行のほかがこのひな形と同じかも見る（写すときに変えてよいのはその行だけ）。
// ひな形が必須のチェックにできる形（PR ごとに必ず走り、落ちたら止める）で、見張りの手順を持っていることも見る（議決 D-0017、Issue #969）。
// 写しとのずれも見るので、見張りを外す変更はひな形と写しを一緒に直しても wiring-order で落ちる。写しを消す・ジョブの名前を変える変更は、
// 必須のチェック（engine-test・wiring-order）が報告されずに止まる（branch protection は建設者が入れる）。
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
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
  const ref = copy.match(/^\s*ENGINE_REF:\s*(\S+)\s*$/m)?.[1];
  assert.equal(runGuard(ref ?? "").status, 0, `写しの ENGINE_REF（${ref}）が 40 桁のコミット SHA でない`);
});

test("ずれの比べ方は ENGINE_REF の行だけを無視する", () => {
  const bumped = template.replace(ENGINE_REF, "0".repeat(40));
  assert.equal(withoutRef(bumped), withoutRef(template));
  assert.notEqual(withoutRef(template.replace("ENGINE_REF が 40 桁", "ENGINE_REF が")), withoutRef(template));
});
