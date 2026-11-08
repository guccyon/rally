// 必須のチェックの照合（protection.mjs）のテスト（知見 K-0004 の担保。議決 D-0026、Issue #1018。複数の JSON を合わせるのは議決 D-0039、Issue #1087）。
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { checkProtection, COPY, jobCheckNames, requiredContexts } from "./protection.mjs";
import { REPO_ROOT } from "./order.mjs";

const script = fileURLToPath(new URL("./protection.mjs", import.meta.url));
const template = readFileSync(fileURLToPath(new URL("./ci/sendback-engine.yml", import.meta.url)), "utf8");

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

test("ひな形のジョブは id のままのチェックの名前（engine-test・wiring-order）を報告する", () => {
  const jobs = jobCheckNames(template);
  assert.equal(jobs.get("engine-test"), "engine-test");
  assert.equal(jobs.get("wiring-order"), "wiring-order");
});

test("写しの 2 つのジョブが必須のチェックに入っていれば通る（古い形・checks だけ・rulesets）", () => {
  for (const protection of [classic(["engine-test", "wiring-order", "backend"]), classic([], [{ context: "engine-test" }, { context: "wiring-order" }]),
    rulesets(["wiring-order", "engine-test"])]) {
    assert.deepEqual(checkProtection({ workflow: template, protection }).errors, []);
  }
});

test("どちらかのジョブが必須のチェックにないと required_check_missing で外れる", () => {
  for (const protection of [classic(["engine-test"]), rulesets(["wiring-order"]), classic(["backend"]), { message: "Branch not protected", status: "404" },
    { required_status_checks: null }, []]) {
    const r = checkProtection({ workflow: template, protection });
    assert.ok(r.errors.length > 0 && r.errors.every((e) => e.rule === "required_check_missing"), JSON.stringify(protection));
  }
  assert.deepEqual(rules(checkProtection({ workflow: template, protection: classic([]) })), ["required_check_missing", "required_check_missing"]);
});

test("ジョブに name: があると、その名前が必須のチェックになければ外れる（id で必須にすると報告されずにすべての PR が止まる）", () => {
  const named = template.replace("  engine-test:\n    runs-on", "  engine-test:\n    name: Engine の判定\n    runs-on");
  assert.equal(jobCheckNames(named).get("engine-test"), "Engine の判定");
  assert.deepEqual(rules(checkProtection({ workflow: named, protection: classic(["engine-test", "wiring-order"]) })), ["required_check_missing"]);
  assert.deepEqual(checkProtection({ workflow: named, protection: classic(["Engine の判定", "wiring-order"]) }).errors, []);
});

test("写しがない・ジョブの名前を変えた写しは workflow_not_copied で外れる", () => {
  const protection = classic(["engine-test", "wiring-order"]);
  assert.deepEqual(rules(checkProtection({ workflow: null, protection })), ["workflow_not_copied"]);
  assert.deepEqual(rules(checkProtection({ workflow: template.replace("  wiring-order:\n", "  order:\n"), protection })), ["workflow_not_copied"]);
});

test("古い形と rulesets を一緒に渡すと、どちらかで必須になっているチェックを必須に数える（議決 D-0039、Issue #1087）", () => {
  const protections = [classic(["engine-test"]), rulesets(["wiring-order"])];
  assert.deepEqual(checkProtection({ workflow: template, protections }).errors, []);
  // 保護がない古い形（404）と、どちらにもない wiring-order
  assert.deepEqual(rules(checkProtection({ workflow: template, protections: [{ message: "Branch not protected", status: "404" }, rulesets(["engine-test"])] })),
    ["required_check_missing"]);
});

test("branch protection の JSON がオブジェクトでも配列でもなければ読めない", () => {
  for (const v of [null, "engine-test", 1]) assert.throws(() => requiredContexts(v));
});

function repoWith(copy) {
  const dir = mkdtempSync(join(tmpdir(), "sendback-protection-"));
  if (copy != null) {
    mkdirSync(join(dir, ".github/workflows"), { recursive: true });
    writeFileSync(join(dir, COPY), copy);
  }
  return dir;
}
const run = (args, env = {}) => spawnSync(process.execPath, [script, ...args], { encoding: "utf8", env: { ...process.env, GITHUB_ACTIONS: "", ...env } });

test("CLI: 通れば 0、外れれば 1（GitHub Actions では ::error）、読めなければ 2", () => {
  const dir = repoWith(template);
  const ok = join(dir, "ok.json");
  const ng = join(dir, "ng.json");
  writeFileSync(ok, JSON.stringify(rulesets(["engine-test", "wiring-order"])));
  writeFileSync(ng, JSON.stringify(classic(["engine-test"])));
  const out = join(dir, "out.json");
  const summary = join(dir, "summary.md");
  let r = run(["--protection", ok, "--repo", dir, "--json", out, "--summary", summary]);
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.equal(JSON.parse(readFileSync(out, "utf8")).ok, true);
  assert.match(readFileSync(summary, "utf8"), /通る: engine-test・wiring-order が必須のチェックに入っている/);
  r = run(["--protection", ng, "--repo", dir], { GITHUB_ACTIONS: "true" });
  assert.equal(r.status, 1);
  assert.match(r.stdout, /^::error title=sendback-check required_check_missing::wiring-order のジョブ/m);
  r = run(["--protection", ng, "--protection", ok, "--repo", dir]);
  assert.equal(r.status, 0, "--protection を 2 つ渡すと合わせて照らす");
  r = run(["--protection", ok, "--repo", repoWith(null)]);
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
  writeFileSync(protection, JSON.stringify(classic(["engine-test", "wiring-order"])));
  const r = run(["--protection", protection]);
  assert.equal(r.status, 1, r.stdout + r.stderr);
  assert.match(r.stdout, /^workflow_not_copied: \.github\/workflows\/sendback-engine\.yml がない/m);
});
