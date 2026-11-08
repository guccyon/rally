// 必須のチェックの照合（protection.mjs）のテスト（知見 K-0004 の担保。議決 D-0026、Issue #1018。複数の JSON を合わせるのは議決 D-0039、Issue #1087）。
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { CHECK_NAMES, checkNameProblems, checkProtection, COPY, jobCheckNames, loadTemplates, loadWorkflows, requiredContexts, sharedCheckNames,
  TEMPLATE } from "./protection.mjs";
import { REPO_ROOT } from "./order.mjs";

const script = fileURLToPath(new URL("./protection.mjs", import.meta.url));
const template = readFileSync(fileURLToPath(new URL("./ci/sendback-engine.yml", import.meta.url)), "utf8");
const knowledgeEnforcement = readFileSync(fileURLToPath(new URL("../knowledge-enforcement-check/ci/knowledge-enforcement.yml", import.meta.url)), "utf8");
const prBody = readFileSync(fileURLToPath(new URL("../pr-body-check/ci/pr-body-engine.yml", import.meta.url)), "utf8");
const ENGINE = CHECK_NAMES["engine-test"];
const WIRING = CHECK_NAMES["wiring-order"];

/** gh api repos/{owner}/{repo}/branches/main/protection の形 */
const classic = (contexts, checks = contexts.map((context) => ({ context, app_id: 15368 }))) =>
  ({ required_status_checks: { strict: true, contexts, checks }, enforce_admins: { enabled: true } });
/** gh api repos/{owner}/{repo}/rules/branches/main の形 */
const rulesets = (contexts) => [
  { type: "pull_request", parameters: { required_approving_review_count: 1 } },
  { type: "required_status_checks", parameters: { strict_required_status_checks_policy: true,
    required_status_checks: contexts.map((context) => ({ context, integration_id: 15368 })) } },
];
const rules = (r) => r.errors.map((e) => e.rule);

test("ひな形のジョブは id と違うチェックの名前（sendback-engine-test・sendback-wiring-order）を報告する", () => {
  const jobs = jobCheckNames(template);
  assert.equal(jobs.get("engine-test"), "sendback-engine-test");
  assert.equal(jobs.get("wiring-order"), "sendback-wiring-order");
});

test("写しの 2 つのジョブが必須のチェックに入っていれば通る（古い形・checks だけ・rulesets、ほかのひな形の写しと並べても）", () => {
  const others = { ".github/workflows/knowledge-enforcement.yml": knowledgeEnforcement, ".github/workflows/pr-body-engine.yml": prBody };
  for (const protection of [classic([ENGINE, WIRING, "backend"]), classic([], [{ context: ENGINE }, { context: WIRING }]),
    rulesets([WIRING, ENGINE, "engine-test"])]) {
    assert.deepEqual(checkProtection({ workflow: template, others, protection }).errors, []);
  }
});

test("どちらかのジョブが必須のチェックにないか、id の engine-test・wiring-order だけを必須にしていると required_check_missing で外れる", () => {
  for (const protection of [classic([ENGINE]), rulesets([WIRING]), classic(["backend"]), classic(["engine-test", "wiring-order"]),
    { message: "Branch not protected", status: "404" }, { required_status_checks: null }, []]) {
    const r = checkProtection({ workflow: template, protection });
    assert.ok(r.errors.length > 0 && r.errors.every((e) => e.rule === "required_check_missing"), JSON.stringify(protection));
  }
  assert.deepEqual(rules(checkProtection({ workflow: template, protection: classic([]) })), ["required_check_missing", "required_check_missing"]);
});

test("写しのチェックの名前をひな形から変えると、必須に入れても check_name_changed で外れる", () => {
  const named = template.replace(`    name: ${ENGINE}\n`, "    name: Engine の判定\n");
  assert.equal(jobCheckNames(named).get("engine-test"), "Engine の判定");
  assert.deepEqual(rules(checkProtection({ workflow: named, protection: classic(["Engine の判定", WIRING]) })), ["check_name_changed"]);
  assert.deepEqual(rules(checkProtection({ workflow: named, protection: classic([ENGINE, WIRING]) })), ["check_name_changed", "required_check_missing"]);
});

test("写しの名前を id のままの engine-test に戻すと、knowledge-enforcement.yml の engine-test と重なって check_name_shared で外れる（議決 D-0038）", () => {
  const same = template.replace(`    name: ${ENGINE}\n`, "");
  const r = checkProtection({ workflow: same, others: { ".github/workflows/knowledge-enforcement.yml": knowledgeEnforcement },
    protection: classic(["engine-test", WIRING]) });
  assert.deepEqual(rules(r), ["check_name_changed", "check_name_shared"]);
  assert.match(r.errors[1].message, /knowledge-enforcement\.yml の engine-test も同じチェックの名前 engine-test/);
});

test("ほかのワークフローのジョブが sendback-engine-test・sendback-wiring-order を名乗ると check_name_shared で外れる（別のワークフローの check.mjs --events が必須を満たす形）", () => {
  for (const name of [ENGINE, WIRING]) {
    const other = `name: x\non:\n  pull_request:\njobs:\n  events:\n    name: ${name}\n    runs-on: ubuntu-latest\n    steps:\n` +
      "      - run: node tools/sendback-check/check.mjs --events events.json\n";
    assert.deepEqual(rules(checkProtection({ workflow: template, others: { ".github/workflows/x.yml": other }, protection: classic([ENGINE, WIRING]) })),
      ["check_name_shared"], name);
  }
});

test("写しがない・ジョブの名前を変えた写しは workflow_not_copied で外れる", () => {
  const protection = classic([ENGINE, WIRING]);
  assert.deepEqual(rules(checkProtection({ workflow: null, protection })), ["workflow_not_copied"]);
  assert.deepEqual(rules(checkProtection({ workflow: template.replace("  wiring-order:\n", "  order:\n"), protection })), ["workflow_not_copied"]);
});

test("古い形と rulesets を一緒に渡すと、どちらかで必須になっているチェックを必須に数える（議決 D-0039、Issue #1087）", () => {
  const protections = [classic([ENGINE]), rulesets([WIRING])];
  assert.deepEqual(checkProtection({ workflow: template, protections }).errors, []);
  // 保護がない古い形（404）と、どちらにもない wiring-order
  assert.deepEqual(rules(checkProtection({ workflow: template, protections: [{ message: "Branch not protected", status: "404" }, rulesets([ENGINE])] })),
    ["required_check_missing"]);
});

test("branch protection の JSON がオブジェクトでも配列でもなければ読めない", () => {
  for (const v of [null, "engine-test", 1]) assert.throws(() => requiredContexts(v));
});

function repoWith(files) {
  const dir = mkdtempSync(join(tmpdir(), "sendback-protection-"));
  mkdirSync(join(dir, ".github/workflows"), { recursive: true });
  for (const [path, text] of Object.entries(files)) writeFileSync(join(dir, path), text);
  return dir;
}
const run = (args, env = {}) => spawnSync(process.execPath, [script, ...args], { encoding: "utf8", env: { ...process.env, GITHUB_ACTIONS: "", ...env } });

test("写しのほかの .github/workflows/ の YAML を読んで照らす", () => {
  const dir = repoWith({ [COPY]: template, ".github/workflows/knowledge-enforcement.yml": knowledgeEnforcement, ".github/workflows/README.md": "x" });
  const { workflow, others } = loadWorkflows(dir);
  assert.equal(workflow, template);
  assert.deepEqual(Object.keys(others), [".github/workflows/knowledge-enforcement.yml"]);
  assert.equal(loadWorkflows(repoWith({})).workflow, null);
});

test("checkNameProblems は CHECK_NAMES と違う名前のジョブだけを返し、sharedCheckNames は渡した名前を報告するほかのジョブを返す", () => {
  assert.deepEqual(checkNameProblems(jobCheckNames(template)), []);
  assert.deepEqual(checkNameProblems(jobCheckNames(template.replace(`    name: ${ENGINE}\n`, ""))),
    [{ job: "engine-test", name: "engine-test", expected: ENGINE }]);
  assert.deepEqual(checkNameProblems(jobCheckNames(template.replace("  wiring-order:\n", "  order:\n"))), []);
  const others = { ".github/workflows/knowledge-enforcement.yml": knowledgeEnforcement, ".github/workflows/pr-body-engine.yml": prBody };
  assert.deepEqual(sharedCheckNames(["engine-test"], others), [{ path: ".github/workflows/knowledge-enforcement.yml", job: "engine-test", name: "engine-test" }]);
  assert.deepEqual(sharedCheckNames(Object.values(CHECK_NAMES), others), []);
});

test("loadTemplates はツールのひな形（tools/<ツール>/ci/ の YAML）を読み、sendback-engine.yml のひな形は除く", () => {
  const dir = repoWith({});
  for (const [path, text] of [[TEMPLATE, template], ["tools/a/ci/a.yml", "x"], ["tools/a/ci/README.md", "x"], ["tools/b/b.yml", "x"]]) {
    mkdirSync(join(dir, path, ".."), { recursive: true });
    writeFileSync(join(dir, path), text);
  }
  assert.deepEqual(Object.keys(loadTemplates(dir)), ["tools/a/ci/a.yml"]);
  assert.deepEqual(loadTemplates(join(dir, "none")), {});
});

test("CLI: 通れば 0、外れれば 1（GitHub Actions では ::error）、読めなければ 2", () => {
  const dir = repoWith({ [COPY]: template, ".github/workflows/knowledge-enforcement.yml": knowledgeEnforcement });
  const ok = join(dir, "ok.json");
  const ng = join(dir, "ng.json");
  writeFileSync(ok, JSON.stringify(rulesets([ENGINE, WIRING])));
  writeFileSync(ng, JSON.stringify(classic([ENGINE])));
  const out = join(dir, "out.json");
  const summary = join(dir, "summary.md");
  let r = run(["--protection", ok, "--repo", dir, "--json", out, "--summary", summary]);
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.equal(JSON.parse(readFileSync(out, "utf8")).ok, true);
  assert.match(readFileSync(summary, "utf8"), /通る: sendback-engine-test（engine-test）・sendback-wiring-order（wiring-order）が必須のチェックに入っている/);
  r = run(["--protection", ng, "--repo", dir], { GITHUB_ACTIONS: "true" });
  assert.equal(r.status, 1);
  assert.match(r.stdout, /^::error title=sendback-check required_check_missing::wiring-order のジョブ/m);
  r = run(["--protection", ng, "--protection", ok, "--repo", dir]);
  assert.equal(r.status, 0, "--protection を 2 つ渡すと合わせて照らす");
  r = run(["--protection", ok, "--repo", repoWith({})]);
  assert.equal(r.status, 1);
  assert.match(r.stdout, /^workflow_not_copied: /m);
  writeFileSync(ng, "{");
  for (const args of [["--protection", ng, "--repo", dir], ["--repo", dir], ["--protection", join(dir, "none.json"), "--repo", dir]]) {
    assert.equal(run(args).status, 2, args.join(" "));
  }
});

test("このリポジトリ: 写しがまだなければ、必須のチェックがそろっていても workflow_not_copied で外れる", (t) => {
  if (existsSync(join(REPO_ROOT, COPY))) return t.skip("写しがあるので、建設者が gh api の JSON で走らせる（README の「必須のチェックの照合」）");
  const protection = join(mkdtempSync(join(tmpdir(), "sendback-protection-")), "protection.json");
  writeFileSync(protection, JSON.stringify(classic([ENGINE, WIRING])));
  const r = run(["--protection", protection]);
  assert.equal(r.status, 1, r.stdout + r.stderr);
  assert.match(r.stdout, /^workflow_not_copied: \.github\/workflows\/sendback-engine\.yml がない/m);
});
