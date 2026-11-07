// ci/sendback-engine.yml の ENGINE_REF の見張りのテスト（知見 K-0004 の担保。議決 D-0014、Issue #960）。
// ENGINE_REF がブランチ名やタグだと同じ PR でも結果が変わるので、engine-test の最初の手順（どのチェックアウトよりも前）で
// 40 桁のコミット SHA かを確かめ、違えば落とす。その手順を取り出して bash で動かす。
// .github/workflows/sendback-engine.yml に写してあれば、ENGINE_REF の行のほかがこのひな形と同じかも見る（写すときに変えてよいのはその行だけ）。
// ひな形が必須のチェックにできる形（PR ごとに必ず走り、落ちたら止める）で、見張りの手順を持っていることも見る（議決 D-0017、Issue #969）。
// 写しとのずれも見るので、見張りを外す変更はひな形と写しを一緒に直しても wiring-order で落ちる。写しを消す・ジョブの名前を変える変更は、
// 必須のチェック（engine-test・wiring-order）が報告されずに止まる（branch protection は建設者が入れる）。
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
