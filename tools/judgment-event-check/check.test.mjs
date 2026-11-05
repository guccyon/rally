import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { checkEvents, loadSchema, parseEvents, validate } from "./check.mjs";

const PR = "https://github.com/guccyon/rally/pull/395";

/** judgment.judged の payload（Engine の orchestrator/dispatcher/judgment.py が書く形）に subject を足したもの。 */
const judged = (over = {}) => ({
  artifact_id: 1, category: "ui", verdict: "adopt", subject: "provisional", confidence: 0.8,
  basis: "self_report", mode: "delegated", pr_url: PR, ...over,
});

/** 差し戻しの incident（Engine の orchestrator/dispatcher/case.py の sendbacks）を、採用の数を分けた形にしたもの。 */
const sendback = (over = {}) => ({
  key: `sendback:${PR}`, kind: "sendback", pr_url: PR, adopted_defects: 0, approved_provisionals: 3, ...over,
});

const judgedErrors = (v) => validate(v, loadSchema("judgment.judged"));
const sendbackErrors = (v) => validate(v, loadSchema("incident.sendback"));

test("PR #395 の裁き（仮決めの採用 3 件・欠陥の却下 2 件）は、subject があれば通る", () => {
  const events = [
    judged({ artifact_id: 1, category: "ui" }),
    judged({ artifact_id: 2, category: "ui" }),
    judged({ artifact_id: 3, category: "error_handling" }),
    judged({ artifact_id: 4, category: "OV-001", verdict: "reject", subject: "defect" }),
    judged({ artifact_id: 5, category: "PR-002", verdict: "reject", subject: "defect" }),
  ];
  assert.deepEqual(checkEvents("judgment.judged", events.map((value, i) => ({ line: i + 1, value }))), []);
});

test("欠陥の採用（subject=defect の adopt）は通る", () => {
  assert.deepEqual(judgedErrors(judged({ subject: "defect" })), []);
});

test("adopt なのに subject がないと外れる（仮決めの承認か欠陥の採用か読めない）", () => {
  const { subject, ...noSubject } = judged();
  const errors = judgedErrors(noSubject);
  assert.equal(errors.length, 1);
  assert.match(errors[0].message, /^subject がない/);
  assert.match(errors[0].message, /provisional = 仮決めの承認、defect = 欠陥の採用/);
});

test("adopt 以外の verdict は subject がなくても通る", () => {
  for (const verdict of ["replace", "reject", "escalate", "human_decided"]) {
    const { subject, ...noSubject } = judged({ verdict });
    assert.deepEqual(judgedErrors(noSubject), [], verdict);
  }
});

test("subject が provisional・defect 以外だと外れる", () => {
  const errors = judgedErrors(judged({ subject: "both" }));
  assert.deepEqual(errors.map((e) => e.path), ["$.subject"]);
});

test("verdict が決まった値のどれでもないと外れる", () => {
  assert.deepEqual(judgedErrors(judged({ verdict: "approve" })).map((e) => e.path), ["$.verdict"]);
});

test("Engine の必須の欄がないと外れる", () => {
  const { pr_url, ...noPr } = judged();
  assert.deepEqual(judgedErrors(noPr).map((e) => e.message), ["pr_url がない"]);
});

test("差し戻しの incident は、欠陥の採用と仮決めの承認を分けて書けば通る", () => {
  assert.deepEqual(sendbackErrors(sendback()), []);
  assert.deepEqual(sendbackErrors(sendback({ adopted_defects: 2, approved_provisionals: 0 })), []);
});

test("差し戻しの incident に合計の adopted があると外れる（PR #395 の adopted: 3 の形）", () => {
  const errors = sendbackErrors(sendback({ adopted: 3 }));
  assert.equal(errors.length, 1);
  assert.match(errors[0].message, /adopted_defects と approved_provisionals に分けて書く/);
});

test("合計の adopted だけで分けた数がないと外れる", () => {
  const errors = sendbackErrors({ key: `sendback:${PR}`, kind: "sendback", pr_url: PR, adopted: 3 });
  assert.deepEqual(errors.map((e) => e.message).sort(), [
    "adopted_defects がない", "approved_provisionals がない",
    "採用の合計（adopted）は置かない。adopted_defects と approved_provisionals に分けて書く",
  ].sort());
});

test("分けた数は 0 以上の整数", () => {
  assert.deepEqual(sendbackErrors(sendback({ adopted_defects: -1 })).map((e) => e.path), ["$.adopted_defects"]);
  assert.deepEqual(sendbackErrors(sendback({ approved_provisionals: 1.5 })).map((e) => e.path), ["$.approved_provisionals"]);
});

test("incident のスキーマは合計の欄を定義していない", () => {
  const schema = loadSchema("incident.sendback");
  assert.ok(!("adopted" in schema.properties));
  assert.deepEqual(schema.not.required, ["adopted"]);
  for (const key of ["adopted_defects", "approved_provisionals"]) assert.ok(schema.required.includes(key), key);
});

test("JSON Lines と JSON の配列を読み、events の行（{ name, payload }）は payload を見る", () => {
  const { subject, ...noSubject } = judged();
  const lines = [JSON.stringify(judged()), "", JSON.stringify({ name: "judgment.judged", payload: noSubject })].join("\n");
  const fromLines = checkEvents("judgment.judged", parseEvents(lines));
  assert.deepEqual(fromLines.map((e) => [e.line, e.path]), [[3, "$"]]);
  const fromArray = checkEvents("judgment.judged", parseEvents(JSON.stringify([judged(), noSubject])));
  assert.deepEqual(fromArray.map((e) => e.line), [2]);
});

test("知らないイベントの名前は投げる", () => {
  assert.throws(() => loadSchema("judgment.graded"), /知らないイベント/);
});

const CLI = fileURLToPath(new URL("./check.mjs", import.meta.url));
const run = (args, input) => {
  try {
    return { code: 0, out: execFileSync("node", [CLI, ...args], { input, encoding: "utf8", stdio: ["pipe", "pipe", "pipe"] }) };
  } catch (e) {
    return { code: e.status, out: e.stdout };
  }
};

test("CLI: 通れば 0、外れれば 1 と --json、読めなければ 2", () => {
  const dir = mkdtempSync(join(tmpdir(), "judgment-event-check-"));
  assert.equal(run(["--event", "incident.sendback"], JSON.stringify(sendback())).code, 0);

  const out = join(dir, "result.json");
  const bad = run(["--event", "incident.sendback", "--json", out], JSON.stringify(sendback({ adopted: 3 })));
  assert.equal(bad.code, 1);
  assert.match(bad.out, /1 行目 \$: 採用の合計/);
  const result = JSON.parse(readFileSync(out, "utf8"));
  assert.equal(result.ok, false);
  assert.equal(result.errors.length, 1);

  const file = join(dir, "events.jsonl");
  writeFileSync(file, "{ not json\n");
  assert.equal(run(["--event", "judgment.judged", "--file", file]).code, 2);
  assert.equal(run(["--event", "judgment.graded"], "{}").code, 2);
});
