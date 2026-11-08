import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { EXIT_TEMPLATE_ERROR, TemplateError, checkBody, headings, loadTemplate, parseTemplate } from "./check.mjs";

const TEMPLATE = loadTemplate();

/** 新しい PR の本文（Engine の worker/entrypoint.sh が組む形）。 */
const NEW_PR = `Closes #541

## 概要
本部が予約 1 件の詳細を見る。

## 対応内容
- getBooking を足した

## 動作確認
- cargo test

## 工場の記録
- job: 1499
- model: tier-large
`;

const rules = (body) => checkBody(body).map((e) => e.rule);

test("新しい PR の本文は通る", () => {
  assert.deepEqual(checkBody(NEW_PR), []);
});

test("続きのジョブの記録が「## 工場の記録」の下の ### にあれば通る", () => {
  const body = `${NEW_PR}
### 続きのジョブ（job 1504）の記録
<details><summary>既定ブランチの取り込みと衝突の解消</summary>

- CHANGELOG.md の衝突を解いた
</details>
`;
  assert.deepEqual(checkBody(body), []);
});

test("続きのジョブが同じ見出しの組を末尾に足すと外れる（C-0001 の PR #546）", () => {
  const body = `${NEW_PR}
## 概要
既定ブランチを取り込んだ。

## 対応内容
- CHANGELOG.md の衝突を解いた
`;
  const errors = checkBody(body);
  assert.deepEqual(errors.filter((e) => e.rule === "duplicate_h2").map((e) => e.heading), ["概要", "対応内容"]);
  assert.ok(errors.some((e) => e.rule === "outside_factory_record" && e.heading === "概要"));
  assert.match(errors[0].message, /「## 概要」が 2 回以上ある（3 行目と 16 行目）/);
});

test("「## 工場の記録」がなくても同じ ## 見出しの重なりは外れる", () => {
  assert.deepEqual(rules("## 概要\na\n\n## 対応内容\nb\n\n## 補足\nc\n\n## 概要\nd\n"), ["duplicate_h2"]);
});

test("「## 工場の記録」より後ろの ## 見出しは、重なっていなくても外れる", () => {
  assert.deepEqual(rules(`${NEW_PR}\n## 続きのジョブの記録\n\n<details><summary>CI 落ちの直し</summary>\n</details>\n`),
    ["outside_factory_record"]);
  assert.deepEqual(rules(`${NEW_PR}\n# 補足\n`), ["outside_factory_record"]);
});

test("続きのジョブの見出しが「## 工場の記録」より前にあると外れる", () => {
  const body = NEW_PR.replace("## 工場の記録", "### 続きのジョブ（job 1504）の記録\n- 衝突を解いた\n\n## 工場の記録");
  assert.deepEqual(rules(body), ["continuation_outside_record"]);
});

test("### の見出しは重なっても外れない", () => {
  assert.deepEqual(checkBody("## 概要\nz\n\n## 対応内容\n### 画面\n- a\n\n## 仮決め\n### 画面\n- b\n"), []);
});

test("コードブロックの中の見出しは数えない", () => {
  const body = "## 概要\nこう書く:\n\n```markdown\n## 概要\n```\n\n~~~\n## 概要\n~~~\n\n## 対応内容\n";
  assert.deepEqual(headings(body).map((h) => h.line), [1, 12]);
  assert.deepEqual(checkBody(body), []);
});

test("見出しの末尾の # と空白は無視して比べる", () => {
  assert.deepEqual(rules("## 概要\n\n## 対応内容\n\n##  概要 ##\n"), ["duplicate_h2"]);
});

/** 実装役の報告の型どおりの見出しを全部使った本文。 */
const FULL_PR = `Closes #906

## 概要
a

## 対応内容
- b

## 影響範囲・互換性
c

## 動作確認
- d

## 補足
e

## 仮決め
### implementation 1: f
- 選んだ: g

## 知見
- h

## 工場の記録
- job: 4656
`;

test("定義の見出しを全部使った本文は通る", () => {
  assert.deepEqual(checkBody(FULL_PR), []);
});

test("定義は CLAUDE.md の「PR の説明」の見出しの順と同じ", () => {
  assert.deepEqual(TEMPLATE.map((t) => t.text),
    ["概要", "背景", "問題", "対応内容", "影響範囲・互換性", "動作確認", "補足", "仮決め", "知見"]);
  const claude = readFileSync(new URL("../../CLAUDE.md", import.meta.url), "utf8").match(/PR の説明は (.+?) の見出しで/);
  assert.deepEqual(claude[1].split(" / "), TEMPLATE.map((t) => t.text));
  assert.deepEqual(TEMPLATE.filter((t) => t.required).map((t) => t.text), ["概要", "対応内容"]);
});

test("定義は凍結してあり、生成した後で書き換えられない", () => {
  assert.equal(loadTemplate(), TEMPLATE);
  assert.ok(Object.isFrozen(TEMPLATE));
  assert.ok(TEMPLATE.every((t) => Object.isFrozen(t)));
  assert.throws(() => { TEMPLATE.push({ text: "背景", required: true }); }, TypeError);
  assert.throws(() => { TEMPLATE[0].required = false; }, TypeError);
  assert.throws(() => { TEMPLATE[0].text = "背景"; }, TypeError);
});

test("不正な定義は生成するときに外す", () => {
  const ok = { text: "概要", required: true };
  for (const [data, why] of [
    [null, /h2 が空でない配列でない/],
    [{}, /h2 が空でない配列でない/],
    [{ h2: [] }, /h2 が空でない配列でない/],
    [{ h2: ["概要"] }, /h2\[0\] がオブジェクトでない/],
    [{ h2: [{ ...ok, extra: 1 }] }, /知らない欄がある（extra）/],
    [{ h2: [{ text: "", required: true }] }, /h2\[0\]\.text/],
    [{ h2: [{ text: " 概要", required: true }] }, /h2\[0\]\.text/],
    [{ h2: [{ text: "概\n要", required: true }] }, /h2\[0\]\.text/],
    [{ h2: [{ text: 1, required: true }] }, /h2\[0\]\.text/],
    [{ h2: [{ text: "概要", required: "yes" }] }, /h2\[0\]\.required/],
    [{ h2: [ok, ok] }, /h2\[1\]\.text「概要」が重なっている/],
  ]) {
    assert.throws(() => parseTemplate(data), (e) => e instanceof TemplateError && why.test(e.message));
  }
});

test("定義を検証しても元のデータは凍結しない", () => {
  const data = { h2: [{ text: "概要", required: true }] };
  const t = parseTemplate(data);
  assert.deepEqual(t, [{ text: "概要", required: true }]);
  assert.ok(!Object.isFrozen(data.h2[0]));
});

test("必須の見出しが欠けると外れる", () => {
  const errors = checkBody(NEW_PR.replace("## 対応内容\n- getBooking を足した\n\n", ""));
  assert.deepEqual(errors.map((e) => [e.rule, e.heading, e.line]), [["missing_required", "対応内容", null]]);
  // 見出しのない文から始まるので、本文の先頭の外れ（K-0016）も 1 件
  assert.deepEqual(rules("本文に見出しがない\n").sort(), ["head_not_first_heading", "missing_required", "missing_required"]);
});

test("必須でない見出しは省いてよい", () => {
  assert.deepEqual(checkBody("## 概要\na\n\n## 対応内容\nb\n"), []);
});

test("定義の見出しの順が違うと外れる（C-0003 の PR #396 のような末尾への追記の直し）", () => {
  const body = NEW_PR.replace("## 工場の記録", "## 補足\n- 追記した\n\n## 影響範囲・互換性\n- なし\n\n## 工場の記録");
  const errors = checkBody(body);
  assert.deepEqual(errors.map((e) => [e.rule, e.heading]), [["template_order", "影響範囲・互換性"]]);
  assert.match(errors[0].message, /「## 影響範囲・互換性」が「## 補足」（12 行目）より後ろにある/);
});

test("順の違いは、戻った見出しのたびに 1 件ずつ数える", () => {
  // 「## 知見」から始まるので、本文の先頭の外れ（K-0016）も 1 件
  assert.deepEqual(rules("## 知見\n\n## 概要\n\n## 対応内容\n").map(String), ["head_not_first_heading", "template_order", "template_order"]);
});

test("重なりの 2 回目は順の違いに数えない", () => {
  assert.deepEqual(rules("## 概要\n\n## 対応内容\n\n## 補足\n\n## 対応内容\n"), ["duplicate_h2"]);
});

test("定義にない ## 見出しが定義の見出しの間にあると外れる", () => {
  const body = FULL_PR.replace("## 補足", "## 続きのジョブの報告\n- 衝突を解いた\n\n## 補足");
  assert.deepEqual(rules(body), ["continuation_outside_record", "outside_template"]);
  assert.deepEqual(rules("## 概要\n\n## 参考\n\n## 対応内容\n"), ["outside_template"]);
});

test("「## 工場の記録」が定義の見出しより前にあると外れる", () => {
  const body = "## 概要\na\n\n## 工場の記録\n- job: 1\n\n## 対応内容\nb\n";
  assert.deepEqual(rules(body), ["outside_template", "outside_factory_record"]);
});

test("定義にない見出しは、定義の見出しの最後より後ろか ### 以下なら通る", () => {
  assert.deepEqual(checkBody("## 概要\n### 参考\n\n## 対応内容\n\n## 参考\n- x\n"), []);
});

test("背景・問題は定義の見出しなので、概要と対応内容の間に置けば通り、省いてもよい", () => {
  assert.deepEqual(checkBody("## 概要\na\n\n## 背景\nb\n\n## 問題\nc\n\n## 対応内容\nd\n"), []);
  assert.deepEqual(checkBody("## 概要\na\n\n## 問題\nc\n\n## 対応内容\nd\n"), []);
  assert.deepEqual(rules("## 概要\na\n\n## 対応内容\nd\n\n## 背景\nb\n"), ["template_order"]);
  assert.deepEqual(rules("## 概要\na\n\n## 問題\nc\n\n## 背景\nb\n\n## 対応内容\nd\n"), ["template_order"]);
});

test("CLI は外れたら 1 で終わり、JSON を書く", () => {
  const dir = mkdtempSync(join(tmpdir(), "pr-body-"));
  const script = fileURLToPath(new URL("./check.mjs", import.meta.url));
  writeFileSync(join(dir, "ok.md"), NEW_PR);
  writeFileSync(join(dir, "bad.md"), `${NEW_PR}\n## 概要\n`);
  execFileSync("node", [script, "--body", join(dir, "ok.md")]);
  assert.throws(() => execFileSync("node", [script, "--body", join(dir, "bad.md"), "--json", join(dir, "out.json")]),
    (e) => e.status === 1);
  const out = JSON.parse(readFileSync(join(dir, "out.json"), "utf8"));
  assert.equal(out.ok, false);
  assert.deepEqual(out.errors.map((e) => e.rule), ["duplicate_h2", "outside_factory_record"]);
  writeFileSync(join(dir, "missing.md"), "## 概要\n");
  assert.throws(() => execFileSync("node", [script, "--body", join(dir, "missing.md")], { encoding: "utf8" }),
    (e) => e.status === 1 && /本文: 必須の見出し「## 対応内容」がない/.test(e.stdout));
  const stdin = execFileSync("node", [script], { input: NEW_PR, encoding: "utf8" });
  assert.match(stdin, /外れ 0 件/);
  assert.throws(() => execFileSync("node", [script, "--body", join(dir, "none.md")], { stdio: "pipe" }), (e) => e.status === 2);
});

/** check.mjs を、壊した template.json と一緒に一時ディレクトリへ写す。 */
function brokenCopy(template) {
  const dir = mkdtempSync(join(tmpdir(), "pr-body-template-"));
  copyFileSync(fileURLToPath(new URL("./check.mjs", import.meta.url)), join(dir, "check.mjs"));
  writeFileSync(join(dir, "template.json"), template);
  writeFileSync(join(dir, "body.md"), NEW_PR);
  return dir;
}

test("定義が不正なら、読み込みでは止まらず、CLI が本文を照らさずに 3 で終わる（外れの 1 と見分ける）", () => {
  assert.equal(EXIT_TEMPLATE_ERROR, 3);
  for (const [template, why] of [
    [JSON.stringify({ h2: [{ text: "概要", required: true, extra: 1 }] }), /見出しの定義が不正: h2\[0\] に知らない欄がある（extra）/],
    [JSON.stringify({ h2: [] }), /見出しの定義が不正: h2 が空でない配列でない/],
    ["{ not json", /見出しの定義が不正: template\.json を読めなかった/],
  ]) {
    const dir = brokenCopy(template);
    const script = join(dir, "check.mjs");
    execFileSync("node", ["--input-type=module", "-e", `await import(${JSON.stringify(`file://${script}`)})`]);
    assert.throws(() => execFileSync("node", [script, "--body", join(dir, "body.md"), "--json", join(dir, "out.json")], { stdio: "pipe", encoding: "utf8" }),
      (e) => e.status === EXIT_TEMPLATE_ERROR && why.test(e.stderr) && !/^\s+at /m.test(e.stderr) && e.stdout === "");
    assert.throws(() => readFileSync(join(dir, "out.json")), { code: "ENOENT" });
  }
});
