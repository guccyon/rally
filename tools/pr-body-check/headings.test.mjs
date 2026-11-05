import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import {
  checkAll, checkSource, diffHeadings, engineDir, fromClaudeMd, fromRubric, fromWorkerPrompt, loadDefinition,
} from "./headings.mjs";

const REPO = fileURLToPath(new URL("../..", import.meta.url));
const def = loadDefinition();

/** Engine の entrypoint.sh の、実装役への指示の形。 */
const WORKER = `- 文書・コメント・PR の説明・コミットメッセージは日本語で書く
- 作業が終わったら、最後の出力を次の見出しだけで構成する（PR の説明になる。該当しない見出しは省く）:
  「## 概要」（1〜3 文）「## 対応内容」（変更点ごとに箇条書き）「## 影響範囲・互換性」「## 動作確認」（実際に確かめたことだけ）「## 補足」（対象外にしたこと）
  「## 仮決め」（上の形）「## 知見」（次の作業に効く事実を 1 行ずつ、最大 3 行。なければ見出しごと省く）
- git commit / push / PR 作成はこちらで行うので実行しない
EOF
`;

/** Engine の rubric.yaml の形（PR-002 の前後に別の項目がある）。 */
const RUBRIC = `rules:
  - id: PR-001
    check: "見出し（ほか）は見ない"

  - id: PR-002
    title: "PR の説明は背景から結論まで追える構造にする"
    check: "テンプレートの見出し（概要・背景・問題・対応内容・影響範囲・動作確認・補足）のうち変更に必要なものが埋まり、該当しないものは削除されている"
    severity: should

  - id: PR-003
    check: "必須チェック（CI）が通る"
`;

const ENGINE_ENTRY = "orchestrator/worker/entrypoint.sh";
const ENGINE_RUBRIC = "core-gates/rubric.yaml";

test("正の定義は重なりのない見出しの並び", () => {
  assert.ok(def.headings.length > 0);
  assert.equal(new Set(def.headings).size, def.headings.length);
  for (const [name, drift] of Object.entries(def.knownDrift)) assert.ok(Array.isArray(drift), name);
});

test("CLAUDE.md の書き方の行から見出しを取る", () => {
  assert.deepEqual(fromClaudeMd("## 書き方\n\n- PR の説明は 概要 / 影響範囲・互換性 / 知見 の見出しで。該当しない見出しは省く\n"),
    ["概要", "影響範囲・互換性", "知見"]);
  assert.equal(fromClaudeMd("- PR の説明は自由に書く\n"), null);
});

test("実装役への指示から「## 」の見出しを順に取り、次の約束の行で止まる", () => {
  assert.deepEqual(fromWorkerPrompt(WORKER), ["概要", "対応内容", "影響範囲・互換性", "動作確認", "補足", "仮決め", "知見"]);
  assert.equal(fromWorkerPrompt("- 最後に報告する\n"), null);
});

test("rubric は PR-002 の check だけを読み、見出しの中の「・」はつなぐ", () => {
  assert.deepEqual(fromRubric(RUBRIC, def.headings), ["概要", "背景", "問題", "対応内容", "影響範囲", "動作確認", "補足"]);
  const fixed = RUBRIC.replace("概要・背景・問題・対応内容・影響範囲", "概要・対応内容・影響範囲・互換性");
  assert.deepEqual(fromRubric(fixed, def.headings), ["概要", "対応内容", "影響範囲・互換性", "動作確認", "補足"]);
  assert.equal(fromRubric(RUBRIC.replace("id: PR-002", "id: PR-009"), def.headings), null);
});

test("並びの違いは足りない・正にない・順の違いで書く", () => {
  assert.equal(diffHeadings(["a", "b"], ["a", "b"]), null);
  assert.equal(diffHeadings(["a", "b", "c"], ["a", "x"]), "足りない: b・c、正にない: x");
  assert.equal(diffHeadings(["a", "b"], ["b", "a"]), "順が違う: b / a");
});

test("ずれの記録がない場所は、正とずれたら落ちる", () => {
  assert.equal(checkSource("worker", ["a", "b"], ["a", "b"], undefined), null);
  assert.match(checkSource("worker", ["a"], ["a", "b"], undefined), /^worker: 正（headings.json）とずれている。足りない: b$/);
  assert.match(checkSource("worker", null, ["a"], undefined), /見つからない/);
});

test("ずれの記録がある場所は、記録した値のときだけ通り、直ったら記録を消させる", () => {
  assert.equal(checkSource("rubric", ["a", "x"], ["a", "b"], ["a", "x"]), null);
  assert.match(checkSource("rubric", ["a", "y"], ["a", "b"], ["a", "x"]), /known_drift に記録した値から変わった/);
  assert.match(checkSource("rubric", ["a", "b"], ["a", "b"], ["a", "x"]), /known_drift.rubric を消す/);
});

test("このリポジトリの CLAUDE.md の見出しは正と一致する", () => {
  const actual = fromClaudeMd(readFileSync(join(REPO, "CLAUDE.md"), "utf8"));
  assert.equal(checkSource("claude_md", actual, def.headings, def.knownDrift.claude_md), null);
});

test("Engine の実装役の報告の型と rubric の PR-002 は正と一致する（ずれの記録を除く）", (t) => {
  const engine = engineDir();
  if (engine === null) {
    t.skip("Engine が見つからない（FACTORY_ENGINE_DIR で factory-engine の置き場を渡す）");
    return;
  }
  const { errors, checked } = checkAll({ repo: REPO, engine });
  assert.deepEqual(checked, ["claude_md", "worker", "rubric"]);
  assert.deepEqual(errors, []);
});

test("CLI は Engine の定義がずれたら 1 で終わる", () => {
  const engine = mkdtempSync(join(tmpdir(), "engine-"));
  mkdirSync(join(engine, "orchestrator/worker"), { recursive: true });
  mkdirSync(join(engine, "core-gates"), { recursive: true });
  writeFileSync(join(engine, ENGINE_ENTRY), WORKER);
  writeFileSync(join(engine, ENGINE_RUBRIC), RUBRIC);
  const script = fileURLToPath(new URL("./headings.mjs", import.meta.url));
  const ok = execFileSync("node", [script, "--repo", REPO, "--engine", engine], { encoding: "utf8" });
  assert.match(ok, /claude_md・worker・rubric を見て、ずれ 0 件/);

  writeFileSync(join(engine, ENGINE_ENTRY), WORKER.replace("「## 仮決め」（上の形）", ""));
  assert.throws(() => execFileSync("node", [script, "--repo", REPO, "--engine", engine], { stdio: "pipe" }),
    (e) => e.status === 1 && /worker: 正（headings.json）とずれている。足りない: 仮決め/.test(e.stdout.toString()));

  writeFileSync(join(engine, ENGINE_ENTRY), WORKER);
  writeFileSync(join(engine, ENGINE_RUBRIC), RUBRIC.replace("・背景・問題", ""));
  assert.throws(() => execFileSync("node", [script, "--repo", REPO, "--engine", engine], { stdio: "pipe" }),
    (e) => e.status === 1 && /rubric: known_drift に記録した値から変わった/.test(e.stdout.toString()));

  assert.throws(() => execFileSync("node", [script, "--repo", join(engine, "none"), "--engine", engine], { stdio: "pipe" }),
    (e) => e.status === 2);
});
