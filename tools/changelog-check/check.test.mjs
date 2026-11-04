import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

import { checkChangelog, readFromGit, renderSummary, splitUnreleased } from "./check.mjs";

const BASE = `# 変更履歴

前書き。

## [Unreleased]

### 追加
- 既にある項目（#700）

## [0.1.0] - 2026-10-01

### 追加
- 最初の版（#1）
`;

const FRAG_805 = "### 追加\n- 断片の項目 A（#805）\n";
const FRAG_809 = "- 見出しのない断片の項目 B（#809）\n";

function run({ baseChangelog = BASE, headChangelog = BASE, baseFragments = {}, headFragments = {} } = {}) {
  return checkChangelog({
    baseChangelog,
    headChangelog,
    baseFragments: new Map(Object.entries(baseFragments)),
    headFragments: new Map(Object.entries(headFragments)),
  });
}

test("[Unreleased] の中と外を分ける", () => {
  const { inside, outside } = splitUnreleased(BASE);
  assert.deepEqual(inside, ["### 追加", "- 既にある項目（#700）"]);
  assert.ok(outside.includes("- 最初の版（#1）"));
  assert.equal(splitUnreleased("# 変更履歴\n").inside, null);
});

test("断片だけを足した PR は通る", () => {
  const r = run({ headFragments: { "805.md": FRAG_805 } });
  assert.equal(r.ok, true);
  assert.equal(r.kind, "untouched");
  assert.deepEqual(r.addedFragments, ["805.md"]);
});

test("[Unreleased] に直接足して断片を足していない PR は落ちる", () => {
  const head = BASE.replace("- 既にある項目（#700）", "- 直接足した項目（#810）\n- 既にある項目（#700）");
  const r = run({ headChangelog: head });
  assert.equal(r.ok, false);
  assert.equal(r.kind, "direct_edit");
  assert.deepEqual(r.added, ["- 直接足した項目（#810）"]);
  assert.match(r.message, /changelog\.d\/<Issue 番号>\.md/);
});

test("断片を足していても [Unreleased] を直接書き換えた PR は落ちる（取りまとめで二重になる）", () => {
  const head = BASE.replace("- 既にある項目（#700）", "- 断片の項目 A（#805）\n- 既にある項目（#700）");
  const r = run({ headChangelog: head, headFragments: { "805.md": FRAG_805 } });
  assert.equal(r.ok, false);
  assert.match(r.message, /changelog\.d\/805\.md/);
});

test("[Unreleased] の既存の項目を直す・消す PR は落ちる", () => {
  assert.equal(run({ headChangelog: BASE.replace("既にある項目", "直した項目") }).ok, false);
  assert.equal(run({ headChangelog: BASE.replace("- 既にある項目（#700）\n", "") }).ok, false);
});

test("[Unreleased] の外（前書きや過去の版）だけを変えた PR は通る", () => {
  const r = run({ headChangelog: BASE.replace("前書き。", "前書きを直した。") });
  assert.equal(r.ok, true);
});

test("取りまとめ（断片を消して [Unreleased] に移す）は通る", () => {
  const head = BASE.replace(
    "### 追加\n- 既にある項目（#700）\n",
    "### 追加\n- 断片の項目 A（#805）\n- 既にある項目（#700）\n\n### 変更\n- 見出しのない断片の項目 B（#809）\n",
  );
  const r = run({ headChangelog: head, baseFragments: { "805.md": FRAG_805, "809.md": FRAG_809 } });
  assert.equal(r.ok, true);
  assert.equal(r.kind, "compiled");
  assert.deepEqual(r.removedFragments, ["805.md", "809.md"]);
});

test("取りまとめに断片にない行が混ざっていたら落ちる", () => {
  const head = BASE.replace("- 既にある項目（#700）", "- 断片の項目 A（#805）\n- 紛れ込んだ項目（#811）\n- 既にある項目（#700）");
  const r = run({ headChangelog: head, baseFragments: { "805.md": FRAG_805 } });
  assert.equal(r.ok, false);
  assert.deepEqual(r.added, ["- 断片の項目 A（#805）", "- 紛れ込んだ項目（#811）"]);
});

test("取りまとめで [Unreleased] の既存の項目を消していたら落ちる", () => {
  const head = BASE.replace("- 既にある項目（#700）", "- 断片の項目 A（#805）");
  assert.equal(run({ headChangelog: head, baseFragments: { "805.md": FRAG_805 } }).ok, false);
});

test("版の切り出し（[Unreleased] の項目を版の項に移す）は通る", () => {
  const head = BASE.replace("## [Unreleased]\n\n### 追加\n- 既にある項目（#700）\n", "## [Unreleased]\n\n## [0.2.0] - 2026-11-03\n\n### 追加\n- 既にある項目（#700）\n");
  const r = run({ headChangelog: head });
  assert.equal(r.ok, true);
  assert.equal(r.kind, "released");
});

test("[Unreleased] の見出しを消した PR は落ちる", () => {
  const r = run({ headChangelog: BASE.replace("## [Unreleased]\n", "") });
  assert.equal(r.ok, false);
  assert.equal(r.kind, "unreleased_removed");
});

test("要約に直接足した行を出す", () => {
  const r = run({ headChangelog: BASE.replace("- 既にある項目（#700）", "- 直接足した項目（#810）\n- 既にある項目（#700）") });
  const md = renderSummary(r);
  assert.match(md, /通らない（direct_edit）/);
  assert.match(md, /- 直接足した項目（#810）/);
});

test("git の分かれ目（merge-base）から読む。既定ブランチ側の変更は PR のものとして見ない", () => {
  const dir = mkdtempSync(join(tmpdir(), "changelog-check-"));
  const g = (...args) => execFileSync("git", ["-C", dir, ...args], { encoding: "utf8" });
  const write = (path, text) => {
    mkdirSync(join(dir, path, ".."), { recursive: true });
    writeFileSync(join(dir, path), text);
  };
  try {
    g("init", "-q", "-b", "main");
    g("config", "user.email", "test@example.invalid");
    g("config", "user.name", "test");
    write("CHANGELOG.md", BASE);
    write("changelog.d/README.md", "# 断片\n");
    g("add", "-A");
    g("commit", "-q", "-m", "base");
    g("checkout", "-q", "-b", "pr");
    write("changelog.d/805.md", FRAG_805);
    g("add", "-A");
    g("commit", "-q", "-m", "pr");
    // PR のあとに既定ブランチで取りまとめが入っても、PR の差分には入らない
    g("checkout", "-q", "main");
    write("CHANGELOG.md", BASE.replace("- 既にある項目（#700）", "- 取りまとめた項目（#790）\n- 既にある項目（#700）"));
    g("add", "-A");
    g("commit", "-q", "-m", "compile");

    const input = readFromGit("main", "pr", { cwd: dir });
    assert.deepEqual([...input.headFragments.keys()], ["805.md"]);
    assert.equal(checkChangelog(input).ok, true);

    g("checkout", "-q", "pr");
    write("CHANGELOG.md", BASE.replace("- 既にある項目（#700）", "- 断片の項目 A（#805）\n- 既にある項目（#700）"));
    g("add", "-A");
    g("commit", "-q", "-m", "direct");
    assert.equal(checkChangelog(readFromGit("main", "pr", { cwd: dir })).ok, false);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
