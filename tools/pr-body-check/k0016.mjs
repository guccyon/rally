// 知見 K-0016 の事例の本文の頭を組む（担保のテストで使う。議決 D-0078、Issue #1219）。
// 実装役の報告（handoff の summary.md）から最初の PR 本文を組んだときに、本文の頭（## 概要 と ## 対応内容）が欠けたり、
// ## 概要 の前に余計な文が混じったりした形。本文そのものは記録にないので、事例 C-0008（PR #678）と C-0006（PR #821）に書かれた形を組み直す。

export const PR_URL_678 = "https://github.com/guccyon/rally/pull/678";
export const PR_URL_821 = "https://github.com/guccyon/rally/pull/821";

/** C-0008: job 3324 の報告。「  - テストで次を確かめます。」から始まり、セッションで書いた ## 概要 と ## 対応内容 の頭が抜けている。 */
export const C0008_JOB_3324 = `  - テストで次を確かめます。
    - トークンの JSON が 0.2.0 のスキーマに通る
    - カタログの見本がトークンの名前で色を引く

## 影響範囲・互換性
- 画面の見た目は変わらない（トークンとカタログを足しただけ）

## 動作確認
- npm test

## 仮決め
### aesthetic catalog-layout: カタログの並び
- 選んだ: 種類ごとに縦に並べる`;

/** C-0008: PR #678 に載った本文の頭の形。「Closes #670」の直後に見出しのない箇条書きで始まり、## 概要 が後ろに来ている。 */
export const C0008_PR_678 = `- トークンの JSON を 0.2.0 に上げた
- カタログに見本を並べた

## 概要
デザインシステムの正本を 0.2.0 にし、カタログを足す。

## 対応内容
- specs/design/ のトークンを 0.2.0 にした

## 動作確認
- npm test`;

/** C-0008: job 3873 の報告。中身の見出しはそろっているが、## 概要 の前に余計な 1 文が付いている。 */
export const C0008_JOB_3873 = `CHANGELOG.md の直接の書き換えを changelog.d/670.md に移しました。

## 概要
デザインシステムの正本を 0.2.0 にし、カタログを足す。

## 対応内容
- specs/design/ のトークンを 0.2.0 にした
- CHANGELOG.md の直接の書き換えをやめ、changelog.d/670.md に移した

## 動作確認
- npm test`;

/** C-0006: PR #821 の報告。箇条書きから始まり、冒頭の ## 概要 の段落と ## 対応内容 の見出しが抜けている。 */
export const C0006_PR_821 = `- 本部の予約の一覧に、確定待ちのまま開始が近い予約の強調を足した
- 強調の閾値を 24 時間にした

## 影響範囲・互換性
- 応答の形は変わらない

## 動作確認
- npm test`;

/** 事例の報告の表（表示の名前・Issue・報告・ほかに出る外れの rule）。どれも本文の先頭の外れ（head_not_first_heading）で落ちる。 */
export const BROKEN_HEADS = [
  ["C-0008 の job 3324 の報告（「  - テストで次を確かめます。」から始まる）", 670, C0008_JOB_3324, ["missing_required", "missing_required"]],
  ["C-0008 の PR #678 の本文（Closes の直後に見出しのない箇条書き、## 概要 が後ろ）", 670, C0008_PR_678, []],
  ["C-0008 の job 3873 の報告（## 概要 の前に 1 文）", 670, C0008_JOB_3873, []],
  ["C-0006 の PR #821 の報告（冒頭の段落と ## 対応内容 の欠け）", 815, C0006_PR_821, ["missing_required", "missing_required"]],
];

/**
 * 報告から最初の PR 本文を組む（Engine の worker/entrypoint.sh の PR_BODY の形。engine.test.mjs は Engine から組む）。
 * incomplete なら途中終了の未完了の印と Refs を付ける。
 */
export function firstPrBody(issue, summary, { incomplete = false } = {}) {
  const mark = incomplete ? "**未完了**: ワーカーの Claude Code が途中で終了しました（exit 1、turns 200、max_turns 200）。\n\n" : "";
  return `${mark}${incomplete ? "Refs" : "Closes"} #${issue}

${summary}

## 工場の記録
- job: 3324（attempt は dispatcher の記録を参照）
- model: tier-large、turns: 40、cost(API 換算): $1.95
`;
}
