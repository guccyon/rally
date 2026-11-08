// sendback-protection の定期の実行の見張り（heartbeat.mjs）のテスト（知見 K-0004 の担保。議決 D-0061（起票時の番号は D-0058）、Issue #1162）。
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { ARTIFACT, checkHeartbeat, MAX_AGE_HOURS, parseArtifacts, passRecords } from "./heartbeat.mjs";

const script = fileURLToPath(new URL("./heartbeat.mjs", import.meta.url));

const NOW = new Date("2026-11-08T12:00:00Z");
/** gh api repos/{owner}/{repo}/actions/artifacts の 1 件の形 */
const artifact = (createdAt, { name = ARTIFACT, branch = "main", id = 1 } = {}) => ({
  id, name, expired: false, created_at: createdAt, expires_at: "2026-11-15T00:00:00Z",
  workflow_run: { id: 100 + id, repository_id: 1, head_repository_id: 1, head_branch: branch, head_sha: "a".repeat(40) },
});
const hoursAgo = (h) => new Date(NOW.getTime() - h * 3_600_000).toISOString();

test("2 日（48 時間）は ci/sendback-protection-heartbeat.yml の MAX_AGE_HOURS と同じ", () => {
  const wf = readFileSync(fileURLToPath(new URL("./ci/sendback-protection-heartbeat.yml", import.meta.url)), "utf8");
  assert.equal(MAX_AGE_HOURS, 48);
  assert.match(wf, new RegExp(`^ {2}MAX_AGE_HOURS: ${MAX_AGE_HOURS}$`, "m"));
});

test("既定のブランチで最後に通ってから 48 時間以内なら通る（古い記録が混ざっていても最後のものを見る）", () => {
  for (const h of [0, 1, 36, 48]) {
    const r = checkHeartbeat([artifact(hoursAgo(100), { id: 2 }), artifact(hoursAgo(h))], { now: NOW });
    assert.deepEqual(r.errors, [], `${h} 時間前`);
    assert.equal(r.last.at, hoursAgo(h));
  }
});

test("最後に通ってから 48 時間より長ければ heartbeat_stale で外れる", () => {
  for (const h of [48.01, 60, 24 * 30]) {
    const r = checkHeartbeat([artifact(hoursAgo(h)), artifact(hoursAgo(h + 24), { id: 2 })], { now: NOW });
    assert.deepEqual(r.errors.map((e) => e.rule), ["heartbeat_stale"], `${h} 時間前`);
    assert.match(r.errors[0].message, new RegExp(hoursAgo(h).replace(/[.]/g, "\\.")));
  }
});

test("記録に数えない成果物（名前が違う・PR のブランチの実行・時刻が読めない）だけなら heartbeat_missing で外れる", () => {
  const cases = {
    成果物がない: [],
    名前が違う: [artifact(hoursAgo(1), { name: "protection-result" })],
    "PR のブランチの実行": [artifact(hoursAgo(1), { branch: "factory/issue-1-job-1" })],
    時刻が読めない: [artifact("not a time")],
    ブランチがない: [{ ...artifact(hoursAgo(1)), workflow_run: undefined }],
  };
  for (const [name, artifacts] of Object.entries(cases)) {
    const r = checkHeartbeat(artifacts, { now: NOW });
    assert.deepEqual(r.errors.map((e) => e.rule), ["heartbeat_missing"], name);
    assert.equal(r.last, null, name);
  }
});

test("PR のブランチの新しい記録があっても、既定のブランチの古い記録で外れる（PR から通ったことにできない）", () => {
  const r = checkHeartbeat([artifact(hoursAgo(1), { branch: "feature" }), artifact(hoursAgo(72), { id: 2 })], { now: NOW });
  assert.deepEqual(r.errors.map((e) => e.rule), ["heartbeat_stale"]);
  assert.deepEqual(passRecords([artifact(hoursAgo(1), { branch: "feature" })], "feature").length, 1);
});

test("許す時間は変えられる", () => {
  assert.deepEqual(checkHeartbeat([artifact(hoursAgo(30))], { now: NOW, maxAgeHours: 24 }).errors.map((e) => e.rule), ["heartbeat_stale"]);
});

test("gh api の出力は 1 ページの JSON・--paginate で続けて出たページ・JSON の配列・--jq '.artifacts[]' の JSONL のどれも読む", () => {
  const a = [artifact(hoursAgo(1)), artifact(hoursAgo(2), { id: 2 })];
  assert.deepEqual(parseArtifacts(JSON.stringify({ total_count: 2, artifacts: a })), a);
  assert.deepEqual(parseArtifacts(`${JSON.stringify({ total_count: 2, artifacts: [a[0]] })}\n${JSON.stringify({ total_count: 2, artifacts: [a[1]] })}\n`), a);
  assert.deepEqual(parseArtifacts(JSON.stringify(a)), a);
  assert.deepEqual(parseArtifacts(a.map((x) => JSON.stringify(x)).join("\n") + "\n"), a);
  assert.deepEqual(parseArtifacts(""), []);
  assert.throws(() => parseArtifacts("{}\nnot json"), /2 行目が JSON でない/);
});

const run = (args, env = {}) => spawnSync("node", [script, ...args], { env: { ...process.env, GITHUB_ACTIONS: "", ...env }, encoding: "utf8" });

test("終了コードは 0 = 通る、1 = 外れた、2 = 読めなかった。GitHub Actions では ::error の形で出す", () => {
  const dir = mkdtempSync(join(tmpdir(), "sendback-heartbeat-"));
  const file = join(dir, "a.jsonl");
  writeFileSync(file, JSON.stringify(artifact(hoursAgo(10))) + "\n");
  const pass = run(["--artifacts", file, "--now", NOW.toISOString()]);
  assert.equal(pass.status, 0, pass.stdout + pass.stderr);

  const stale = run(["--artifacts", file, "--now", NOW.toISOString(), "--max-age-hours", "5"], { GITHUB_ACTIONS: "true" });
  assert.equal(stale.status, 1);
  assert.match(stale.stdout, /^::error title=sendback-check heartbeat_stale::/m);

  const none = join(dir, "none.jsonl");
  writeFileSync(none, "");
  const missing = run(["--artifacts", none, "--now", NOW.toISOString()], { GITHUB_ACTIONS: "true" });
  assert.equal(missing.status, 1);
  assert.match(missing.stdout, /^::error title=sendback-check heartbeat_missing::/m);

  // 今の時刻を渡さなければ実行した時刻で見る（2026-11-08 の記録は何年もたてば古い）
  assert.equal(run(["--artifacts", file]).status, Date.now() - NOW.getTime() > 38 * 3_600_000 ? 1 : 0);

  const broken = join(dir, "broken.json");
  writeFileSync(broken, "[");
  assert.equal(run(["--artifacts", broken]).status, 2);
  assert.equal(run([]).status, 2);
  assert.equal(run(["--artifacts", file, "--now", "昨日"]).status, 2);
  assert.equal(run(["--artifacts", file, "--max-age-hours", "0"]).status, 2);
  assert.equal(run(["--artifacts", file, "--max-age-hours", "二日"]).status, 2);
});

test("--json と --summary に結果を書く", () => {
  const dir = mkdtempSync(join(tmpdir(), "sendback-heartbeat-"));
  const file = join(dir, "a.json");
  writeFileSync(file, JSON.stringify({ total_count: 1, artifacts: [artifact(hoursAgo(3))] }));
  const out = join(dir, "out.json");
  const summary = join(dir, "summary.md");
  const r = run(["--artifacts", file, "--now", NOW.toISOString(), "--json", out, "--summary", summary]);
  assert.equal(r.status, 0, r.stdout + r.stderr);
  const json = JSON.parse(readFileSync(out, "utf8"));
  assert.equal(json.ok, true);
  assert.equal(json.last.at, hoursAgo(3));
  assert.match(readFileSync(summary, "utf8"), /最後に通った: 2026-11-08T09:00:00\.000Z（3 時間前）/);
});
