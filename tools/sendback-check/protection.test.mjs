// 必須のチェックの照合（protection.mjs）のテスト（知見 K-0004 の担保。議決 D-0026、Issue #1018）。
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { checkProtection, checkProtectionInActions, COPY, jobCheckNames, readRequiredChecks, requiredContexts } from "./protection.mjs";
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

test("…/protection/required_status_checks の応答（contexts と checks が根にある形）も読む", () => {
  const direct = classic(["engine-test", "wiring-order"]).required_status_checks;
  assert.deepEqual(checkProtection({ workflow: template, protection: direct }).errors, []);
  assert.deepEqual(rules(checkProtection({ workflow: template, protection: { ...direct, contexts: [], checks: [{ context: "engine-test" }] } })),
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

// GitHub Actions での照合（ci.test.mjs が wiring-order で呼ぶ。議決 D-0026、Issue #1020）。gh を偽のコマンドに差し替えて確かめる。

/** 呼び出しを calls.log に書き、引数ごとに決めた応答を返す偽の gh。responses は { 引数の先頭の道: [終了コード, 標準出力, 標準エラー] }。 */
function fakeGh(responses) {
  const dir = mkdtempSync(join(tmpdir(), "sendback-protection-gh-"));
  const log = join(dir, "calls.log");
  const cases = Object.entries(responses).map(([path, [rc, out, err]]) =>
    `  ${JSON.stringify(path)}) printf '%s' ${JSON.stringify(out)}; printf '%s' ${JSON.stringify(err)} >&2; exit ${rc} ;;`).join("\n");
  const gh = join(dir, "gh");
  writeFileSync(gh, `#!/usr/bin/env bash\necho "gh $*" >> ${JSON.stringify(log)}\ncase "$2" in\n${cases}\n  *) echo "gh: Not Found (HTTP 404)" >&2; exit 1 ;;\nesac\n`);
  chmodSync(gh, 0o755);
  return { gh, dir, calls: () => (existsSync(log) ? readFileSync(log, "utf8") : "") };
}

const RSC = "repos/guccyon/rally/branches/main/protection/required_status_checks";
const eventWith = (dir, payload, name = "event.json") => { const p = join(dir, name); writeFileSync(p, JSON.stringify(payload)); return p; };
const actionsEnv = (dir, extra = {}) => ({ ...process.env, GITHUB_REPOSITORY: "guccyon/rally",
  GITHUB_EVENT_PATH: eventWith(dir, { repository: { default_branch: "main" } }), ...extra });

test("Actions: 既定のブランチの必須のチェックに engine-test と wiring-order があれば通る", () => {
  const f = fakeGh({ [RSC]: [0, JSON.stringify(classic(["backend", "engine-test", "wiring-order"]).required_status_checks), ""] });
  const r = checkProtectionInActions({ env: actionsEnv(f.dir), gh: f.gh, repo: repoWith(template) });
  assert.deepEqual(r.errors, []);
  assert.equal(r.branch, "main");
  assert.match(f.calls(), new RegExp(`^gh api ${RSC}$`, "m"));
});

test("Actions: どちらかが contexts にも checks にもなければ required_check_missing で落ちる", () => {
  for (const [body, want] of [[{ contexts: ["engine-test"], checks: [{ context: "engine-test" }] }, 1], [{ contexts: [], checks: [{ context: "wiring-order" }] }, 1],
    [{ contexts: ["backend"], checks: [] }, 2]]) {
    const f = fakeGh({ [RSC]: [0, JSON.stringify(body), ""] });
    assert.deepEqual(rules(checkProtectionInActions({ env: actionsEnv(f.dir), gh: f.gh, repo: repoWith(template) })),
      Array(want).fill("required_check_missing"), JSON.stringify(body));
  }
});

test("Actions: 保護か必須のチェックがない（404）なら必須のチェックが空として required_check_missing で落ちる", () => {
  for (const message of ["Branch not protected", "Required status checks not enabled"]) {
    const f = fakeGh({ [RSC]: [1, JSON.stringify({ message, status: "404" }), `gh: ${message} (HTTP 404)`] });
    assert.deepEqual(rules(checkProtectionInActions({ env: actionsEnv(f.dir), gh: f.gh, repo: repoWith(template) })),
      ["required_check_missing", "required_check_missing"], message);
    const g = fakeGh({ [RSC]: [1, "", `gh: ${message} (HTTP 404)`] });
    assert.deepEqual(rules(checkProtectionInActions({ env: actionsEnv(g.dir), gh: g.gh, repo: repoWith(template) })),
      ["required_check_missing", "required_check_missing"], `${message}（stderr だけ）`);
  }
});

test("Actions: 権限がない・リポジトリが引けない・gh がないときは通さずに protection_unreadable で落ちる", () => {
  const forbidden = fakeGh({ [RSC]: [1, JSON.stringify({ message: "Resource not accessible by integration", status: "403" }), "gh: Resource not accessible by integration (HTTP 403)"] });
  let r = checkProtectionInActions({ env: actionsEnv(forbidden.dir), gh: forbidden.gh, repo: repoWith(template) });
  assert.deepEqual(rules(r), ["protection_unreadable"]);
  assert.match(r.errors[0].message, /main の必須のチェックを読めない（Resource not accessible by integration）/);
  const notFound = fakeGh({});
  assert.deepEqual(rules(checkProtectionInActions({ env: actionsEnv(notFound.dir), gh: notFound.gh, repo: repoWith(template) })), ["protection_unreadable"]);
  assert.deepEqual(rules(checkProtectionInActions({ env: actionsEnv(notFound.dir), gh: join(notFound.dir, "none"), repo: repoWith(template) })),
    ["protection_unreadable"]);
  assert.deepEqual(rules(checkProtectionInActions({ env: { ...actionsEnv(notFound.dir), GITHUB_REPOSITORY: "" }, gh: notFound.gh, repo: repoWith(template) })),
    ["protection_unreadable"]);
});

test("Actions: イベントに既定のブランチがなければ gh api repos/{owner}/{repo} で引き、そのブランチの必須のチェックを読む", () => {
  const rsc = "repos/guccyon/rally/branches/trunk/protection/required_status_checks";
  const f = fakeGh({ "repos/guccyon/rally": [0, "trunk", ""], [rsc]: [0, JSON.stringify({ contexts: ["engine-test", "wiring-order"] }), ""] });
  for (const event of [{ GITHUB_EVENT_PATH: "" }, { GITHUB_EVENT_PATH: eventWith(f.dir, { pull_request: {} }, "pr.json") }]) {
    assert.deepEqual(readRequiredChecks({ env: actionsEnv(f.dir, event), gh: f.gh }).branch, "trunk");
  }
  assert.match(f.calls(), /^gh api repos\/guccyon\/rally --jq \.default_branch$/m);
  assert.match(f.calls(), new RegExp(`^gh api ${rsc}$`, "m"));
});

test("Actions: 写しがあれば写しのジョブの name: で照らし、なければひな形の名前で照らす", () => {
  const named = template.replace("  engine-test:\n    runs-on", "  engine-test:\n    name: Engine の判定\n    runs-on");
  const f = fakeGh({ [RSC]: [0, JSON.stringify({ contexts: ["engine-test", "wiring-order"] }), ""] });
  assert.deepEqual(rules(checkProtectionInActions({ env: actionsEnv(f.dir), gh: f.gh, repo: repoWith(named) })), ["required_check_missing"]);
  assert.deepEqual(checkProtectionInActions({ env: actionsEnv(f.dir), gh: f.gh, repo: repoWith(null) }).errors, []);
});
