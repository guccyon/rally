// 見出しの正本（template.json）と合わせる側（CLAUDE.md・Engine の rubric の PR-002・Engine の entrypoint.sh の実装役の報告の型）の照合（followers.mjs）のテスト
// （知見 K-0005 の担保。Issue #1121。実装役の報告の型は議決 D-0071 と Issue #1194）。
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
const ENGINE_NOW_LIST = "概要・背景・問題・対応内容・影響範囲・動作確認・補足";
const ENGINE_NOW = `テンプレートの見出し（${ENGINE_NOW_LIST}）のうち変更に必要なものが埋まり、該当しないものは削除されている`;

/** Engine の orchestrator/worker/entrypoint.sh の実装役への約束のうち、最後の出力の見出しを並べた節（今の Engine の行の形）。 */
const entrypoint = (lines) => `#!/usr/bin/env bash
PROMPT=$(cat <<EOF
最後の出力の「## 概要」以下は、この続きのジョブでしたことだけを書く（PR の説明には節として追記する）。
# 約束
- 作業が終わったら、最後の出力を次の見出しだけで構成する（PR の説明になる。該当しない見出しは省く）:
${lines}
- git commit / push / PR 作成はこちらで行うので実行しない
EOF
)
`;
/** template.json に合わせた実装役の報告の型。 */
const REPORT_ALIGNED = entrypoint(`  「## 概要」（1〜3 文）「## 背景」「## 問題」「## 対応内容」（変更点ごとに箇条書き）「## 影響範囲・互換性」「## 動作確認」（実際に確かめたことだけ）「## 補足」（対象外にしたこと）
  「## 仮決め」（上の形）「## 知見」（次の作業に効く事実を 1 行ずつ、最大 3 行。なければ見出しごと省く）`);
/** Issue #1194 の時点の Engine の実装役の報告の型（背景・問題がない）。 */
const REPORT_NOW_LIST = "概要 / 対応内容 / 影響範囲・互換性 / 動作確認 / 補足 / 仮決め / 知見";
const REPORT_NOW = entrypoint(`  「## 概要」（1〜3 文）「## 対応内容」（変更点ごとに箇条書き）「## 影響範囲・互換性」「## 動作確認」（実際に確かめたことだけ）「## 補足」（対象外にしたこと）
  「## 仮決め」（上の形）「## 知見」（次の作業に効く事実を 1 行ずつ、最大 3 行。なければ見出しごと省く）`);

const texts = (claude, rubricText, report = REPORT_ALIGNED) =>
  (f) => (f.root === "repo" ? claude : f.id === "implementer-report" ? report : rubricText);
const rules = (claude, rubricText, report = REPORT_ALIGNED) =>
  checkFollowers({ read: texts(claude, rubricText, report) }).map((e) => `${e.follower}:${e.rule}`);

test("正本は template.json の 1 つで、合わせる側は CLAUDE.md と Engine の rubric の PR-002 と実装役の報告の型", () => {
  assert.equal(CONFIG.canonical, "template.json");
  assert.deepEqual(FOLLOWERS.map((f) => [f.id, f.root, f.path, f.rubric_id]),
    [["claude-md", "repo", "CLAUDE.md", null], ["rubric-pr-002", "engine", "core-gates/rubric.yaml", "PR-002"],
      ["implementer-report", "engine", "orchestrator/worker/entrypoint.sh", null]]);
  assert.ok(Object.isFrozen(FOLLOWERS) && FOLLOWERS.every((f) => Object.isFrozen(f)));
});

test("今の CLAUDE.md の「PR の説明」の見出しと必須は template.json と同じ", () => {
  assert.deepEqual(checkFollowers({ followers: [byId("claude-md")], read: reader({ repo: new URL("../../", import.meta.url) }) }), []);
});

test("template.json に合わせた PR-002・実装役の報告の型と今の CLAUDE.md は通る", () => {
  assert.deepEqual(rules(CLAUDE_MD, rubric(ALIGNED)), []);
  assert.equal(extract(byId("implementer-report"), REPORT_ALIGNED).headings,
    "概要 / 背景 / 問題 / 対応内容 / 影響範囲・互換性 / 動作確認 / 補足 / 仮決め / 知見");
});

test("Issue #1194 の時点の Engine の実装役の報告の型（背景・問題がない）は headings_mismatch で落ちる", () => {
  const errors = checkFollowers({ read: texts(CLAUDE_MD, rubric(ALIGNED), REPORT_NOW) });
  assert.equal(errors.length, 1);
  assert.equal(errors[0].rule, "headings_mismatch");
  assert.equal(errors[0].follower, "implementer-report");
  assert.equal(errors[0].actual, REPORT_NOW_LIST);
  assert.equal(errors[0].expected, "概要 / 背景 / 問題 / 対応内容 / 影響範囲・互換性 / 動作確認 / 補足 / 仮決め / 知見");
});

test("実装役の報告の型の見出しの順・名前・数を変えると落ちる（注釈は見ない）", () => {
  const cases = {
    "順を入れ替える": REPORT_ALIGNED.replace("「## 背景」「## 問題」", "「## 問題」「## 背景」"),
    "名前を変える": REPORT_ALIGNED.replace("「## 影響範囲・互換性」", "「## 影響範囲」"),
    "知見を消す": REPORT_ALIGNED.replace("「## 知見」", ""),
    "見出しを足す": REPORT_ALIGNED.replace("「## 知見」", "「## 知見」「## 参考」"),
    "見出しの行を足す": REPORT_ALIGNED.replace("（上の形）", "（上の形）\n  「## 参考」"),
  };
  for (const [name, report] of Object.entries(cases)) {
    assert.notEqual(report, REPORT_ALIGNED, name);
    assert.deepEqual(rules(CLAUDE_MD, rubric(ALIGNED), report), ["implementer-report:headings_mismatch"], name);
  }
  assert.deepEqual(rules(CLAUDE_MD, rubric(ALIGNED), REPORT_ALIGNED.replace("（1〜3 文）", "（1〜2 文）")), [], "注釈だけを変える");
});

test("Issue #1121 の時点の Engine の PR-002（影響範囲・仮決め・知見の食い違い）は headings_mismatch で落ちる", () => {
  const errors = checkFollowers({ read: texts(CLAUDE_MD, rubric(ENGINE_NOW)) });
  assert.equal(errors.length, 1);
  assert.equal(errors[0].rule, "headings_mismatch");
  assert.equal(errors[0].follower, "rubric-pr-002");
  assert.equal(errors[0].actual, ENGINE_NOW_LIST);
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

test("template.json だけを変えると、合わせる側のすべてが落ちる（正本を変えたら同じ PR でそろえる）", () => {
  const data = JSON.parse(readFileSync(new URL("./template.json", import.meta.url), "utf8"));
  const added = parseTemplate({ h2: [...data.h2, { text: "参考", required: false }] });
  assert.deepEqual(checkFollowers({ template: added, read: texts(CLAUDE_MD, rubric(ALIGNED)) }).map((e) => `${e.follower}:${e.rule}`),
    ["claude-md:headings_mismatch", "rubric-pr-002:headings_mismatch", "implementer-report:headings_mismatch"]);
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
    "実装役の報告の型の節がない": [CLAUDE_MD, rubric(ALIGNED), REPORT_ALIGNED.replace("最後の出力を次の見出しだけで構成する", "最後に報告する")],
    "実装役の報告の型の節が 2 か所": [CLAUDE_MD, rubric(ALIGNED), `${REPORT_ALIGNED}${REPORT_ALIGNED}`],
    "実装役の報告の型の節の次の行が見出しでない": [CLAUDE_MD, rubric(ALIGNED), entrypoint("  次の見出しで書く")],
  };
  for (const [name, [claude, rubricText, report]] of Object.entries(cases)) {
    assert.throws(() => checkFollowers({ read: texts(claude, rubricText, report) }), SourceError, name);
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
    "item が正規表現でない": { ...CONFIG, followers: [{ ...follower, headings: { ...follower.headings, item: "(" } }] },
    "item の組がない": { ...CONFIG, followers: [{ ...follower, headings: { ...follower.headings, item: "「## " } }] },
    "item が空": { ...CONFIG, followers: [{ ...follower, headings: { ...follower.headings, item: "" } }] },
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
const fakeEngine = (check, report = REPORT_ALIGNED) => {
  const dir = mkdtempSync(join(tmpdir(), "pr-body-followers-"));
  mkdirSync(join(dir, "core-gates"));
  writeFileSync(join(dir, "core-gates", "rubric.yaml"), rubric(check));
  mkdirSync(join(dir, "orchestrator", "worker"), { recursive: true });
  writeFileSync(join(dir, "orchestrator", "worker", "entrypoint.sh"), report);
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

// 本物の Engine との照合（rubric の PR-002 と実装役の報告の型）。ENGINE_DIR を渡したときと CI（CI=true）では飛ばさない。
// Engine の合わせる側の食い違いが、どれも知っている今の形（PR-002 は Issue #1121 の時点の ENGINE_NOW_LIST、実装役の報告の型は Issue #1194 の時点の
// REPORT_NOW_LIST）の headings_mismatch のときだけ todo にする（建設者が Engine を template.json に合わせて ENGINE_REF を上げるまで）。
// ほかの形の食い違いや、Engine を読めない・一覧を取り出せないときは todo にせず落とす。どちらも合わせた Engine では todo が外れて必須になる。
const engineDir = process.env.ENGINE_DIR || fileURLToPath(new URL("../../../engine", import.meta.url));
const rubricPath = join(engineDir, "core-gates", "rubric.yaml");
const required = Boolean(process.env.ENGINE_DIR) || process.env.CI === "true";
const skip = required || existsSync(rubricPath) ? false : `Engine のチェックアウトがない（ENGINE_DIR=${engineDir}）`;
/** Engine の合わせる側の、建設者が合わせるまで todo にしてよい今の形（つないだ見出しの一覧）。 */
const ENGINE_NOW_LISTS = { "rubric-pr-002": ENGINE_NOW_LIST, "implementer-report": REPORT_NOW_LIST };
/** Engine の食い違いがどれも知っている今の形なら todo の理由を、そうでなければ（合っている・読めないときも）false を返す。 */
const todoFor = (dir) => {
  let errors;
  try {
    errors = checkFollowers({ read: reader({ repo: new URL("../../", import.meta.url), engine: dirUrl(dir) }) });
  } catch {
    return false;
  }
  const known = errors.length > 0 && errors.every((e) => e.rule === "headings_mismatch" && ENGINE_NOW_LISTS[e.follower] === e.actual);
  return known
    ? `Engine の ${errors.map((e) => e.follower).join("・")} が Issue #1121・#1194 の時点の形のまま（Engine を template.json に合わせて ENGINE_REF を上げたら外れる）`
    : false;
};
const todo = todoFor(engineDir);

test("本物の Engine の rubric の PR-002 と実装役の報告の型と CLAUDE.md が template.json と同じ", { skip, todo }, () => {
  const errors = checkFollowers({ read: reader({ repo: new URL("../../", import.meta.url), engine: dirUrl(engineDir) }) });
  assert.deepEqual(errors.map((e) => e.message), [],
    `見出しの一覧が template.json と食い違う（ENGINE_DIR=${engineDir}。Engine の core-gates/rubric.yaml の PR-002 と orchestrator/worker/entrypoint.sh の報告の型を合わせて ENGINE_REF を上げる）`);
});

test("本物の Engine の照合の todo は、Engine の合わせる側が知っている今の形のときだけ（ほかの食い違いや読めない Engine は落とす）", () => {
  assert.ok(todoFor(fakeEngine(ENGINE_NOW, REPORT_NOW)));
  assert.ok(todoFor(fakeEngine(ENGINE_NOW)));
  assert.ok(todoFor(fakeEngine(ALIGNED, REPORT_NOW)));
  assert.equal(todoFor(fakeEngine(ALIGNED)), false);
  assert.equal(todoFor(fakeEngine(ENGINE_NOW.replace("・補足", ""))), false);
  assert.equal(todoFor(fakeEngine(ENGINE_NOW, REPORT_NOW.replace("「## 補足」", ""))), false);
  assert.equal(todoFor(fakeEngine(ALIGNED, REPORT_ALIGNED.replace("「## 背景」", ""))), false);
  assert.equal(todoFor(fakeEngine(ENGINE_NOW, entrypoint("  次の見出しで書く"))), false);
  assert.equal(todoFor(join(tmpdir(), "pr-body-followers-none")), false);
});
