// マージの前の関所（merge-gate.mjs）のテスト（知見 K-0012 の担保。議決 D-0033、Issue #1052）。
// 事例 C-0006（PR #821）と C-0005（PR #659）の記録を fixtures にして、マージが止まることを確かめる。
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { parseEvents } from "../sendback-check/check.mjs";
import { newPrBody } from "./c0005.mjs";
import { BODY_CHECKED, EventShapeError, auditMerges, mergeGate } from "./merge-gate.mjs";

const here = (p) => fileURLToPath(new URL(p, import.meta.url));
const load = (name) => parseEvents(readFileSync(here(`./fixtures/${name}`), "utf8"));
const C0006 = "https://github.com/guccyon/rally/pull/821";
const C0005 = "https://github.com/guccyon/rally/pull/659";
const beforeMerge = (events) => events.slice(0, events.findIndex((e) => e.name === "supervisor.merged"));
const checked = (head, ok, pr = C0006) => ({ name: BODY_CHECKED, payload: { pr_url: pr, head_sha: head, ok } });
const rules = (r) => r.errors.map((e) => e.rule);

function run(args) {
  try {
    return { code: 0, out: execFileSync("node", [here("./merge-gate.mjs"), ...args], { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }) };
  } catch (e) {
    return { code: e.status, out: e.stdout, err: e.stderr };
  }
}

test("C-0006: inspector_approved 14:25:02・body_amended 14:25:04・merged 14:25:06 の記録で、マージが止まる", () => {
  const events = load("c0006.events.json");
  assert.deepEqual(events.filter((e) => e.at).map((e) => [e.name, e.at]), [
    ["supervisor.judge_enqueued", "14:23:16"], ["supervisor.inspector_approved", "14:25:02"], ["job.finished", "14:25:04"],
    ["pr.body_amended", "14:25:04"], ["supervisor.merged", "14:25:06"],
  ]);
  // マージの直前の時点で関所を引くと、直した本文の再検査がないので止める
  const result = mergeGate(beforeMerge(events), C0006, { head: "c0006-head" });
  assert.equal(result.ok, false);
  assert.deepEqual(rules(result), ["body_not_rechecked"]);
  // 承認（14:25:02）の時点では body_fix がまだ動いていたので、そこでも止める
  const atApproval = events.slice(0, events.findIndex((e) => e.name === "supervisor.inspector_approved") + 1);
  assert.deepEqual(rules(mergeGate(atApproval, C0006)), ["body_fix_running"]);
  // 過去の記録の見直しでも、このマージを外れとして数える
  const audit = auditMerges(events);
  assert.equal(audit.length, 1);
  assert.deepEqual([audit[0].pr_url, audit[0].head_sha, audit[0].at, rules(audit[0])], [C0006, "c0006-head", "14:25:06", ["body_not_rechecked"]]);
});

test("C-0005: body_fix 3308 の終わり（20:08:50）より前のマージ（20:08:43）で、マージが止まる", () => {
  const events = load("c0005.events.json");
  const result = mergeGate(beforeMerge(events), C0005);
  assert.deepEqual(result.errors.map((e) => [e.rule, e.job_id]), [["body_fix_running", 3308]]);
  assert.deepEqual(rules(auditMerges(events)[0]), ["body_fix_running"]);
});

test("body_fix が終わったあとに同じ head の本文の再検査が通っていれば、マージしてよい", () => {
  const events = beforeMerge(load("c0006.events.json"));
  assert.deepEqual(mergeGate([...events, checked("c0006-head", true)], C0006), { ok: true, errors: [] });
  // 再検査を挟んだ列なら、過去の記録の見直しでも外れない
  const fixed = [...events, checked("c0006-head", true), load("c0006.events.json").at(-1)];
  assert.deepEqual(rules(auditMerges(fixed)[0]), []);
});

test("再検査が落ちた・body_fix より前の再検査しかない・別の head の再検査しかない・ほかの PR の再検査しかないときは止める", () => {
  const events = beforeMerge(load("c0006.events.json"));
  assert.deepEqual(rules(mergeGate([...events, checked("c0006-head", false)], C0006)), ["body_check_failed"]);
  // 通ったあとに落ちたら、最後の結果で止める
  assert.deepEqual(rules(mergeGate([...events, checked("c0006-head", true), checked("c0006-head", false)], C0006)), ["body_check_failed"]);
  const early = [...events];
  early.splice(early.findIndex((e) => e.name === "job.finished"), 0, checked("c0006-head", true));
  assert.deepEqual(rules(mergeGate(early, C0006)), ["body_not_rechecked"]);
  assert.deepEqual(rules(mergeGate([...events, checked("other-head", true)], C0006, { head: "c0006-head" })), ["body_not_rechecked"]);
  assert.deepEqual(rules(mergeGate([...events, checked("c0006-head", true, C0005)], C0006)), ["body_not_rechecked"]);
});

test("body_fix が本文を直せなかったら、再検査が通っていても止める", () => {
  const events = beforeMerge(load("c0006.events.json")).map((e) =>
    e.name === "job.finished" ? { ...e, payload: { state: "failed" } } : e.name === "pr.body_amended" ? { ...e, name: "pr.body_amend_failed" } : e);
  assert.deepEqual(rules(mergeGate([...events, checked("c0006-head", true)], C0006)), ["body_fix_failed"]);
});

test("body_fix が 2 回あれば、2 回目が終わったあとの再検査を待つ", () => {
  const events = beforeMerge(load("c0006.events.json"));
  const second = [
    ...events, checked("c0006-head", true),
    { name: "job.enqueued", job_id: 4300, payload: { kind: "body_fix", pr_url: C0006 } },
  ];
  assert.deepEqual(rules(mergeGate(second, C0006)), ["body_fix_running"]);
  const done = [...second, { name: "pr.body_amended", job_id: 4300, payload: { pr_url: C0006 } }];
  assert.deepEqual(rules(mergeGate(done, C0006)), ["body_not_rechecked"]);
  assert.equal(mergeGate([...done, checked("c0006-head", true)], C0006).ok, true);
});

test("body_fix のない PR は待つものがないのでマージしてよい", () => {
  const events = load("c0006.events.json").filter((e) => e.job_id === undefined);
  assert.deepEqual(mergeGate(beforeMerge(events), C0006), { ok: true, errors: [] });
  assert.deepEqual(rules(mergeGate(beforeMerge(load("c0006.events.json")), C0005)), []);
});

test("今の本文（--body）を渡せば、列の終わりの再検査として照らす", () => {
  const events = beforeMerge(load("c0006.events.json"));
  assert.equal(mergeGate(events, C0006, { body: newPrBody() }).ok, true);
  // C-0006 の本文の形（冒頭の「## 概要」と「## 対応内容」が抜けた箇条書き）なら止める
  assert.deepEqual(rules(mergeGate(events, C0006, { body: "- お知らせの下書きの一覧を絞れるようにした\n\n## 動作確認\n- テスト\n" })), ["body_check_failed"]);
});

test("body_fix の job.enqueued に job_id がない・再検査の ok が真偽でないときは読めなかったとして投げる（黙って通さない）", () => {
  assert.throws(() => mergeGate([{ name: "job.enqueued", payload: { kind: "body_fix", pr_url: C0006 } }], C0006),
    (e) => e instanceof EventShapeError && /job_id がない/.test(e.reason));
  assert.throws(() => mergeGate([{ name: BODY_CHECKED, payload: { pr_url: C0006, ok: "true" } }], C0006),
    (e) => e instanceof EventShapeError && /ok が真偽でない/.test(e.reason));
});

test("CLI: C-0006 の記録の見直しは 1 で落ち、マージの前の判定は 1、再検査を足せば 0、読めなければ 2", () => {
  const audit = run(["--events", here("./fixtures/c0006.events.json"), "--audit"]);
  assert.equal(audit.code, 1);
  assert.match(audit.out, /pull\/821@c0006-head body_not_rechecked/);
  assert.match(audit.out, /マージ 1 件、外れ 1 件/);

  const dir = mkdtempSync(join(tmpdir(), "merge-gate-"));
  const events = beforeMerge(load("c0006.events.json"));
  const file = join(dir, "events.jsonl");
  writeFileSync(file, events.map((e) => JSON.stringify(e)).join("\n") + "\n");
  const hold = run(["--events", file, "--pr", C0006, "--json", join(dir, "out.json")]);
  assert.equal(hold.code, 1);
  assert.match(hold.out, /^body_not_rechecked: /);
  assert.equal(JSON.parse(readFileSync(join(dir, "out.json"), "utf8")).ok, false);

  writeFileSync(file, [...events, checked("c0006-head", true)].map((e) => JSON.stringify(e)).join("\n") + "\n");
  assert.deepEqual(run(["--events", file, "--pr", C0006, "--head", "c0006-head"]), { code: 0, out: "" });

  const body = join(dir, "body.md");
  writeFileSync(body, newPrBody());
  writeFileSync(file, events.map((e) => JSON.stringify(e)).join("\n") + "\n");
  assert.equal(run(["--events", file, "--pr", C0006, "--body", body]).code, 0);

  writeFileSync(file, JSON.stringify({ name: "job.enqueued", payload: { kind: "body_fix", pr_url: C0006 } }) + "\n");
  const shape = run(["--events", file, "--pr", C0006]);
  assert.equal(shape.code, 2);   // イベントの形の崩れ（EventShapeError）は読めなかった
  assert.match(shape.err, /merge-gate: 読めなかった: 1 件目の body_fix の job.enqueued に job_id がない/);
  assert.equal(run(["--events", file]).code, 2);   // --pr がない
  writeFileSync(file, "{ not json\n");
  assert.equal(run(["--events", file, "--pr", C0006]).code, 2);
});

test("ci/pr-body-check.yml は edited でも起動し、本文を環境変数で受けて check.mjs に渡す", () => {
  const yml = readFileSync(here("./ci/pr-body-check.yml"), "utf8");
  assert.match(yml, /^ {4}types: \[opened, synchronize, reopened, edited\]$/m);
  assert.match(yml, /^ {4}name: pr-body-check$/m);
  assert.match(yml, /^ {10}BODY: \$\{\{ github\.event\.pull_request\.body \}\}$/m);
  assert.match(yml, /^ {8}run: printf '%s' "\$BODY" \| node tools\/pr-body-check\/check\.mjs$/m);
  assert.doesNotMatch(yml, /run:.*\$\{\{ github\.event\.pull_request\.body/);
  assert.doesNotMatch(yml, /continue-on-error|^\s+if:/m);
});
