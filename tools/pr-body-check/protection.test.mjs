// 必須のチェックの照合（protection.mjs）のテスト（知見 K-0001 の担保。議決 D-0032、Issue #1037。複数の JSON を合わせるのは議決 D-0044、Issue #1106）。
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { CHECK_NAME, checkProtection, COPY, loadWorkflows, REPO_ROOT } from "./protection.mjs";

const script = fileURLToPath(new URL("./protection.mjs", import.meta.url));
const template = readFileSync(fileURLToPath(new URL("./ci/pr-body-engine.yml", import.meta.url)), "utf8");
const sendback = readFileSync(fileURLToPath(new URL("../sendback-check/ci/sendback-engine.yml", import.meta.url)), "utf8");
const knowledgeEnforcement = readFileSync(fileURLToPath(new URL("../knowledge-enforcement-check/ci/knowledge-enforcement.yml", import.meta.url)), "utf8");

/** gh api repos/{owner}/{repo}/branches/main/protection の形 */
const classic = (contexts) => ({ required_status_checks: { strict: true, contexts, checks: contexts.map((context) => ({ context, app_id: 15368 })) } });
/** gh api repos/{owner}/{repo}/rules/branches/main の形 */
const rulesets = (contexts) => [
  { type: "pull_request", parameters: { required_approving_review_count: 1 } },
  { type: "required_status_checks", parameters: { required_status_checks: contexts.map((context) => ({ context, integration_id: 15368 })) } },
];
const rules = (r) => r.errors.map((e) => e.rule);

test("写しの engine-test（チェックの名前 pr-body-engine-test）が必須のチェックにあれば通る（古い形・rulesets、sendback-engine.yml と並べても）", () => {
  for (const protection of [classic([CHECK_NAME, "backend"]), rulesets(["engine-test", "wiring-order", CHECK_NAME])]) {
    assert.deepEqual(checkProtection({ workflow: template, others: { ".github/workflows/sendback-engine.yml": sendback }, protection }).errors, []);
  }
});

test("必須のチェックにないか、id の engine-test だけを必須にしていると required_check_missing で外れる", () => {
  for (const protection of [classic(["engine-test"]), rulesets(["backend"]), classic([]), { message: "Branch not protected", status: "404" }, []]) {
    assert.deepEqual(rules(checkProtection({ workflow: template, protection })), ["required_check_missing"], JSON.stringify(protection));
  }
});

test("写しがない・engine-test のジョブがないと workflow_not_copied で外れる", () => {
  const protection = classic([CHECK_NAME]);
  assert.deepEqual(rules(checkProtection({ workflow: null, protection })), ["workflow_not_copied"]);
  assert.deepEqual(rules(checkProtection({ workflow: template.replace("  engine-test:\n", "  pr-body:\n"), protection })), ["workflow_not_copied"]);
});

test("写しのチェックの名前を id のままの engine-test に戻すと、必須でも check_name_changed と check_name_shared で外れる（knowledge-enforcement.yml も engine-test を報告する）", () => {
  const same = template.replace(`    name: ${CHECK_NAME}\n`, "");
  const r = checkProtection({ workflow: same, others: { ".github/workflows/sendback-engine.yml": sendback,
    ".github/workflows/knowledge-enforcement.yml": knowledgeEnforcement }, protection: classic(["engine-test"]) });
  assert.deepEqual(rules(r), ["check_name_changed", "check_name_shared"]);
  assert.match(r.errors[1].message, /knowledge-enforcement\.yml の engine-test も同じチェックの名前 engine-test/);
});

test("ほかのワークフローのジョブが pr-body-engine-test を名乗ると check_name_shared で外れる", () => {
  const other = `name: x\non:\n  pull_request:\njobs:\n  fake:\n    name: ${CHECK_NAME}\n    runs-on: ubuntu-latest\n    steps:\n      - run: "true"\n`;
  assert.deepEqual(rules(checkProtection({ workflow: template, others: { ".github/workflows/x.yml": other }, protection: classic([CHECK_NAME]) })),
    ["check_name_shared"]);
});

test("古い形と rulesets の両方を渡すと、どちらかで必須なら必須に数える（議決 D-0044、Issue #1106）", () => {
  assert.deepEqual(checkProtection({ workflow: template, protections: [{ message: "Branch not protected", status: "404" }, rulesets([CHECK_NAME])] }).errors, []);
  assert.deepEqual(checkProtection({ workflow: template, protections: [classic([CHECK_NAME]), []] }).errors, []);
  assert.deepEqual(rules(checkProtection({ workflow: template, protections: [classic(["engine-test"]), rulesets(["backend"])] })), ["required_check_missing"]);
});

function repoWith(files) {
  const dir = mkdtempSync(join(tmpdir(), "pr-body-protection-"));
  mkdirSync(join(dir, ".github/workflows"), { recursive: true });
  for (const [path, text] of Object.entries(files)) writeFileSync(join(dir, path), text);
  return dir;
}
const run = (args, env = {}) => spawnSync(process.execPath, [script, ...args], { encoding: "utf8", env: { ...process.env, GITHUB_ACTIONS: "", ...env } });

test("写しのほかの .github/workflows/ の YAML を読んで照らす", () => {
  const dir = repoWith({ [COPY]: template, ".github/workflows/sendback-engine.yml": sendback, ".github/workflows/README.md": "x" });
  const { workflow, others } = loadWorkflows(dir);
  assert.equal(workflow, template);
  assert.deepEqual(Object.keys(others), [".github/workflows/sendback-engine.yml"]);
});

test("CLI: 通れば 0、外れれば 1（GitHub Actions では ::error）、読めなければ 2", () => {
  const dir = repoWith({ [COPY]: template, ".github/workflows/sendback-engine.yml": sendback });
  const ok = join(dir, "ok.json");
  const ng = join(dir, "ng.json");
  writeFileSync(ok, JSON.stringify(rulesets(["engine-test", CHECK_NAME])));
  writeFileSync(ng, JSON.stringify(classic(["engine-test"])));
  const out = join(dir, "out.json");
  const summary = join(dir, "summary.md");
  let r = run(["--protection", ok, "--repo", dir, "--json", out, "--summary", summary]);
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.equal(JSON.parse(readFileSync(out, "utf8")).ok, true);
  assert.match(readFileSync(summary, "utf8"), /通る: pr-body-engine-test/);
  r = run(["--protection", ng, "--repo", dir], { GITHUB_ACTIONS: "true" });
  assert.equal(r.status, 1);
  assert.match(r.stdout, /^::error title=pr-body-check required_check_missing::/m);
  r = run(["--protection", ng, "--protection", ok, "--repo", dir]);
  assert.equal(r.status, 0, "--protection を 2 つ渡すと合わせて照らす");
  assert.equal(run(["--protection", ok, "--repo", repoWith({})]).status, 1);
  writeFileSync(ng, "{");
  for (const args of [["--protection", ng, "--repo", dir], ["--repo", dir], ["--protection", join(dir, "none.json"), "--repo", dir]]) {
    assert.equal(run(args).status, 2, args.join(" "));
  }
});

test("このリポジトリ: 写しがまだなければ、必須のチェックがそろっていても workflow_not_copied で外れる", (t) => {
  if (existsSync(join(REPO_ROOT, COPY))) return t.skip("写しがあるので、建設者が gh api の JSON で走らせる（README の「配線の手順（建設者）」）");
  const protection = join(mkdtempSync(join(tmpdir(), "pr-body-protection-")), "protection.json");
  writeFileSync(protection, JSON.stringify(classic([CHECK_NAME])));
  const r = run(["--protection", protection]);
  assert.equal(r.status, 1, r.stdout + r.stderr);
  assert.match(r.stdout, /^workflow_not_copied: \.github\/workflows\/pr-body-engine\.yml がない/m);
});
