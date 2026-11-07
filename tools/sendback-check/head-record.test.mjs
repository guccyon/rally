// engine-head の記録の照合（head-record.mjs）のテスト（知見 K-0004 の担保。議決 D-0031、Issue #1047）。
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { checkHeadRecord, headRecords, KINDS, parseComments } from "./head-record.mjs";

const script = fileURLToPath(new URL("./head-record.mjs", import.meta.url));
const headTemplate = readFileSync(fileURLToPath(new URL("./ci/sendback-engine-head.yml", import.meta.url)), "utf8");

const HEAD = "a".repeat(40);
const RUN = "https://github.com/guccyon/rally/actions/runs/123";
/** gh api repos/{owner}/{repo}/issues/948/comments の 1 件の形 */
const comment = (body, login = "github-actions[bot]") =>
  ({ id: 1, html_url: "https://github.com/guccyon/rally/issues/948#issuecomment-1", user: { login, type: "Bot" }, body });
const record = (kind, head = HEAD) => `<!-- sendback-engine-head: ${head} ${kind} -->\n先頭で走らせた。\n\n- 実行: ${RUN}\n`;

test("記録の種類は ci/sendback-engine-head.yml が書く kind と同じ", () => {
  for (const kind of KINDS) assert.match(headTemplate, new RegExp(`kind=${kind}$`, "m"), kind);
  assert.deepEqual([...headTemplate.matchAll(/kind=(\w+)$/gm)].map((m) => m[1]).sort(), [...KINDS].sort());
});

test("engine-head が書いた記録（通った・落ちた・ハーネスのずれ）が 1 件でもあれば通る", () => {
  for (const kind of KINDS) {
    const r = checkHeadRecord([comment("ほかの記録"), comment(record(kind))]);
    assert.deepEqual(r.errors, [], kind);
    assert.deepEqual(r.records, [{ head: HEAD, kind, run: RUN, url: "https://github.com/guccyon/rally/issues/948#issuecomment-1" }]);
  }
});

test("記録がない・印を人が写した・先頭の版が取れなかった・実行の URL がない・印が最初の行にないときは head_record_missing で外れる", () => {
  const cases = {
    コメントがない: [],
    ほかの記録だけ: [comment("PR #395 と同じ入力で CI が通った")],
    人が写した印: [comment(record("engine_head_passed"), "tessan")],
    版が取れなかった: [comment(record("engine_head_failed", "unknown"))],
    短い版: [comment(record("engine_head_passed", "a".repeat(7)))],
    知らない種類: [comment(record("engine_head_skipped"))],
    "実行の URL がない": [comment(`<!-- sendback-engine-head: ${HEAD} engine_head_passed -->\n通った。\n`)],
    印が最初の行にない: [comment(`前置き\n${record("engine_head_passed")}`)],
    本文がない: [{ user: { login: "github-actions[bot]" } }],
  };
  for (const [name, comments] of Object.entries(cases)) {
    const r = checkHeadRecord(comments);
    assert.deepEqual(r.errors.map((e) => e.rule), ["head_record_missing"], name);
    assert.deepEqual(r.records, [], name);
  }
});

test("gh api の出力は JSON の配列と、--jq '.[]' の 1 行 1 件のどちらも読む", () => {
  const comments = [comment("ほか"), comment(record("engine_head_passed"))];
  assert.deepEqual(parseComments(JSON.stringify(comments)), comments);
  assert.deepEqual(parseComments(comments.map((c) => JSON.stringify(c)).join("\n") + "\n"), comments);
  assert.deepEqual(parseComments(""), []);
  assert.throws(() => parseComments("{}\nnot json"), /2 行目が JSON でない/);
});

test("記録の印は engine-head の印と同じ形（ひな形の marker の行）", () => {
  assert.match(headTemplate, /marker="<!-- sendback-engine-head: \$\{head_sha:-unknown\} \$\{kind\} -->"/);
  assert.equal(headRecords([comment(record("engine_head_passed"))]).length, 1);
});

const run = (file, env = {}) => spawnSync("node", [script, ...(file ? ["--comments", file] : [])],
  { env: { ...process.env, GITHUB_ACTIONS: "", ...env }, encoding: "utf8" });

test("終了コードは 0 = 通る、1 = 外れた、2 = 読めなかった。GitHub Actions では ::error の形で出す", () => {
  const dir = mkdtempSync(join(tmpdir(), "head-record-"));
  const ok = join(dir, "ok.jsonl");
  writeFileSync(ok, JSON.stringify(comment(record("engine_head_passed"))) + "\n");
  const pass = run(ok);
  assert.equal(pass.status, 0, pass.stdout + pass.stderr);
  assert.match(pass.stdout, new RegExp(`^engine_head_passed ${HEAD} ${RUN}$`, "m"));

  const none = join(dir, "none.json");
  writeFileSync(none, "[]");
  const missing = run(none, { GITHUB_ACTIONS: "true" });
  assert.equal(missing.status, 1);
  assert.match(missing.stdout, /^::error title=sendback-check head_record_missing::/m);

  const broken = join(dir, "broken.json");
  writeFileSync(broken, "[");
  assert.equal(run(broken).status, 2);
  assert.equal(run(null).status, 2);
});

test("--json と --summary に結果を書く", () => {
  const dir = mkdtempSync(join(tmpdir(), "head-record-"));
  const file = join(dir, "c.json");
  writeFileSync(file, JSON.stringify([comment(record("harness_unknown_query"))]));
  const out = join(dir, "out.json");
  const summary = join(dir, "summary.md");
  const r = spawnSync("node", [script, "--comments", file, "--json", out, "--summary", summary], { encoding: "utf8" });
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.equal(JSON.parse(readFileSync(out, "utf8")).ok, true);
  assert.match(readFileSync(summary, "utf8"), /`harness_unknown_query`（先頭 `a{40}`）/);
});
