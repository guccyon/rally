// 見出しの正本（template.json）と合わせる側（CLAUDE.md・Engine の rubric の PR-002）の照合（followers.mjs）のテスト（知見 K-0005 の担保。Issue #1121）。
// Engine のチェックアウトは engine.test.mjs と同じく環境変数 ENGINE_DIR で渡す（なければリポジトリの隣の ../engine）。どちらもなければ本物の rubric の照合だけ飛ばす。
// ただし ENGINE_DIR を渡したときと CI（CI=true）では飛ばさずに落とす。

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { parseTemplate } from "./check.mjs";
import {
  FollowersError, SourceError, checkFollowers, dirUrl, extract, loadFollowers, parseFollowers, reader, rubricField,
} from "./followers.mjs";

const FOLLOWERS = loadFollowers();
const CONFIG = JSON.parse(readFileSync(new URL("./followers.json", import.meta.url), "utf8"));
const CLAUDE_MD = readFileSync(new URL("../../CLAUDE.md", import.meta.url), "utf8");
const byId = (id) => FOLLOWERS.find((f) => f.id === id);

/** template.json に合わせた PR-002 を持つ rubric（Engine の core-gates/rubric.yaml の形）。 */
const rubric = (check) => `version: 1
items:
  - id: PR-001
    title: "PR は 1 つの目的"
    check: "テンプレートの見出し（概要）は関係ない"

  - id: PR-002
    title: "PR の説明は背景から結論まで追える構造にする"
    gate: process
    check: ${JSON.stringify(check)}
    severity: should

  - id: PR-003
    check: "必須チェック（CI）が通る"
`;
const ALIGNED = "テンプレートの見出し（概要・背景・問題・対応内容・影響範囲・互換性・動作確認・補足・仮決め・知見）のうち変更に必要なものが埋まり、該当しないものは削除されている";
/** Issue #1121 の時点の Engine の PR-002（影響範囲の名前と、仮決め・知見がない）。 */
const ENGINE_NOW = "テンプレートの見出し（概要・背景・問題・対応内容・影響範囲・動作確認・補足）のうち変更に必要なものが埋まり、該当しないものは削除されている";

const texts = (claude, rubricText) => (f) => (f.root === "repo" ? claude : rubricText);
const rules = (claude, rubricText) => checkFollowers({ read: texts(claude, rubricText) }).map((e) => `${e.follower}:${e.rule}`);

test("正本は template.json の 1 つで、合わせる側は CLAUDE.md と Engine の rubric の PR-002", () => {
  assert.equal(CONFIG.canonical, "template.json");
  assert.deepEqual(FOLLOWERS.map((f) => [f.id, f.root, f.path, f.rubric_id]),
    [["claude-md", "repo", "CLAUDE.md", null], ["rubric-pr-002", "engine", "core-gates/rubric.yaml", "PR-002"]]);
  assert.ok(Object.isFrozen(FOLLOWERS) && FOLLOWERS.every((f) => Object.isFrozen(f)));
});

test("今の CLAUDE.md の「PR の説明」の見出しと必須は template.json と同じ", () => {
  assert.deepEqual(checkFollowers({ followers: [byId("claude-md")], read: reader({ repo: new URL("../../", import.meta.url) }) }), []);
});

test("template.json に合わせた PR-002 と今の CLAUDE.md は通る", () => {
  assert.deepEqual(rules(CLAUDE_MD, rubric(ALIGNED)), []);
});

test("Issue #1121 の時点の Engine の PR-002（影響範囲・仮決め・知見の食い違い）は headings_mismatch で落ちる", () => {
  const errors = checkFollowers({ read: texts(CLAUDE_MD, rubric(ENGINE_NOW)) });
  assert.equal(errors.length, 1);
  assert.equal(errors[0].rule, "headings_mismatch");
  assert.equal(errors[0].follower, "rubric-pr-002");
  assert.equal(errors[0].actual, "概要・背景・問題・対応内容・影響範囲・動作確認・補足");
  assert.equal(errors[0].expected, "概要・背景・問題・対応内容・影響範囲・互換性・動作確認・補足・仮決め・知見");
});

test("CLAUDE.md の見出しの順・名前・数や必須を変えると落ちる", () => {
  const cases = {
    "順を入れ替える": [CLAUDE_MD.replace("概要 / 背景 / 問題", "背景 / 概要 / 問題"), "claude-md:headings_mismatch"],
    "名前を変える": [CLAUDE_MD.replace("影響範囲・互換性 / ", "影響範囲 / "), "claude-md:headings_mismatch"],
    "見出しを減らす": [CLAUDE_MD.replace(" / 知見 の見出しで", " の見出しで"), "claude-md:headings_mismatch"],
    "背景・問題を消す": [CLAUDE_MD.replace(" / 背景 / 問題", ""), "claude-md:headings_mismatch"],
    "必須を足す": [CLAUDE_MD.replace("必須は 概要 と 対応内容。", "必須は 概要 と 対応内容 と 動作確認。"), "claude-md:required_mismatch"],
  };
  for (const [name, [claude, rule]] of Object.entries(cases)) {
    assert.notEqual(claude, CLAUDE_MD, name);
    assert.deepEqual(rules(claude, rubric(ALIGNED)), [rule], name);
  }
});

test("template.json だけを変えると、合わせる側の両方が落ちる（正本を変えたら同じ PR でそろえる）", () => {
  const data = JSON.parse(readFileSync(new URL("./template.json", import.meta.url), "utf8"));
  const added = parseTemplate({ h2: [...data.h2, { text: "参考", required: false }] });
  assert.deepEqual(checkFollowers({ template: added, read: texts(CLAUDE_MD, rubric(ALIGNED)) }).map((e) => `${e.follower}:${e.rule}`),
    ["claude-md:headings_mismatch", "rubric-pr-002:headings_mismatch"]);
  const required = parseTemplate({ h2: data.h2.map((t) => ({ ...t, required: t.text === "概要" })) });
  assert.deepEqual(checkFollowers({ template: required, read: texts(CLAUDE_MD, rubric(ALIGNED)) }).map((e) => `${e.follower}:${e.rule}`),
    ["claude-md:required_mismatch"]);
});

test("一覧を取り出せない合わせる側は、照らさずに SourceError（黙って通さない）", () => {
  const cases = {
    "CLAUDE.md に見出しの一覧がない": [CLAUDE_MD.replace("PR の説明は", "PR の本文は"), rubric(ALIGNED)],
    "CLAUDE.md に見出しの一覧が 2 か所": [`${CLAUDE_MD}\n- PR の説明は 概要 の見出しで\n`, rubric(ALIGNED)],
    "CLAUDE.md に必須の一覧がない": [CLAUDE_MD.replace("（必須は", "（要るのは"), rubric(ALIGNED)],
    "rubric に PR-002 がない": [CLAUDE_MD, rubric(ALIGNED).replace("id: PR-002", "id: PR-009")],
    "rubric に PR-002 が 2 つ": [CLAUDE_MD, `${rubric(ALIGNED)}\n  - id: PR-002\n    check: "x"\n`],
    "PR-002 に check がない": [CLAUDE_MD, rubric(ALIGNED).replace(/^ {4}check: .*\n(?= {4}severity)/m, "")],
    "PR-002 の check が複数行": [CLAUDE_MD, rubric(ALIGNED).replace(/^ {4}check: .*$(?=\n {4}severity)/m, `    check: >-\n      ${ALIGNED}`)],
    "PR-002 の check に見出しの一覧がない": [CLAUDE_MD, rubric("本文が読みやすい")],
  };
  for (const [name, [claude, rubricText]] of Object.entries(cases)) {
    assert.throws(() => checkFollowers({ read: texts(claude, rubricText) }), SourceError, name);
  }
});

test("rubric の項目の欄は、その項目の中からだけ取り出す（隣の項目の check を拾わない）", () => {
  const text = rubric(ALIGNED);
  assert.equal(rubricField(text, "PR-002", "check"), ALIGNED);
  assert.equal(rubricField(text, "PR-001", "check"), "テンプレートの見出し（概要）は関係ない");
  assert.equal(rubricField("items:\n  - id: 'PR-002'\n    check: 'a''b'\n", "PR-002", "check"), "a'b");
  assert.equal(rubricField("items:\n  - id: PR-002\n    check: a b # 注釈\n", "PR-002", "check"), "a b");
  assert.throws(() => rubricField("items:\n  - id: PR-002\n  - id: PR-003\n    check: x\n", "PR-002", "check"), SourceError);
  assert.equal(extract(byId("rubric-pr-002"), text).headings, "概要・背景・問題・対応内容・影響範囲・互換性・動作確認・補足・仮決め・知見");
});

test("不正な followers.json は FollowersError（照らさない）", () => {
  const follower = CONFIG.followers[0];
  const cases = {
    "正本が template.json でない": { ...CONFIG, canonical: "CLAUDE.md" },
    "合わせる側が空": { ...CONFIG, followers: [] },
    "id が重なる": { ...CONFIG, followers: [follower, follower] },
    "root が知らない": { ...CONFIG, followers: [{ ...follower, root: "web" }] },
    "知らない欄": { ...CONFIG, followers: [{ ...follower, note: "x" }] },
    "pattern が正規表現でない": { ...CONFIG, followers: [{ ...follower, headings: { pattern: "(", separator: " / " } }] },
    "pattern の組がない": { ...CONFIG, followers: [{ ...follower, headings: { pattern: "PR の説明は", separator: " / " } }] },
    "pattern の組が 2 つ": { ...CONFIG, followers: [{ ...follower, headings: { pattern: "(PR) の説明は (.+?) の", separator: " / " } }] },
    "区切りが空": { ...CONFIG, followers: [{ ...follower, headings: { ...follower.headings, separator: "" } }] },
    "rubric_id だけ": { ...CONFIG, followers: [{ ...follower, rubric_id: "PR-002" }] },
  };
  for (const [name, data] of Object.entries(cases)) {
    assert.throws(() => parseFollowers(data), FollowersError, name);
  }
});

const cli = (args, env = {}) => {
  try {
    return { code: 0, out: execFileSync("node", [fileURLToPath(new URL("./followers.mjs", import.meta.url)), ...args],
      { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], env: { ...process.env, ...env } }) };
  } catch (e) {
    return { code: e.status, out: e.stdout, err: e.stderr };
  }
};
const fakeEngine = (check) => {
  const dir = mkdtempSync(join(tmpdir(), "pr-body-followers-"));
  mkdirSync(join(dir, "core-gates"));
  writeFileSync(join(dir, "core-gates", "rubric.yaml"), rubric(check));
  return dir;
};

test("CLI: 合っていれば 0、食い違えば 1（--json に食い違い）、Engine を読めなければ 2", () => {
  assert.equal(cli(["--engine-dir", fakeEngine(ALIGNED)]).code, 0);
  assert.equal(cli([], { ENGINE_DIR: fakeEngine(ALIGNED) }).code, 0);
  const out = join(mkdtempSync(join(tmpdir(), "pr-body-followers-")), "out.json");
  const r = cli(["--engine-dir", fakeEngine(ENGINE_NOW), "--json", out]);
  assert.equal(r.code, 1);
  assert.match(r.out, /rubric-pr-002: .*食い違う/);
  const json = JSON.parse(readFileSync(out, "utf8"));
  assert.equal(json.ok, false);
  assert.deepEqual(json.errors.map((e) => e.rule), ["headings_mismatch"]);
  const missing = cli(["--engine-dir", join(tmpdir(), "pr-body-followers-none")]);
  assert.equal(missing.code, 2);
  assert.match(missing.err, /合わせる側を読めなかった/);
});

// 本物の Engine の rubric との照合。Engine の PR-002 が template.json に合うまで落ちる（建設者が Engine を合わせて ENGINE_REF を上げる）。
const engineDir = process.env.ENGINE_DIR || fileURLToPath(new URL("../../../engine", import.meta.url));
const rubricPath = join(engineDir, "core-gates", "rubric.yaml");
const required = Boolean(process.env.ENGINE_DIR) || process.env.CI === "true";
const skip = required || existsSync(rubricPath) ? false : `Engine のチェックアウトがない（ENGINE_DIR=${engineDir}）`;

test("本物の Engine の rubric の PR-002 と CLAUDE.md が template.json と同じ", { skip }, () => {
  const errors = checkFollowers({ read: reader({ repo: new URL("../../", import.meta.url), engine: dirUrl(engineDir) }) });
  assert.deepEqual(errors.map((e) => e.message), [],
    `見出しの一覧が template.json と食い違う（ENGINE_DIR=${engineDir}。Engine の core-gates/rubric.yaml の PR-002 を合わせて ENGINE_REF を上げる）`);
});
