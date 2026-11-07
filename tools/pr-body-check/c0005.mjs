// 事例 C-0005（PR #659）の続きのジョブの入力を組む（知見 K-0001 の担保のテストで使う。議決 D-0013、Issue #956）。
// PR #659 では、main の取り込みのたびに続きのジョブ（conflict が 20 回、ci_failed が 1 回）が走り、その summary が新しい PR と同じ
// ## の見出しの組（概要・対応内容…）を持ったまま本文に足された。本文そのものは記録にないので、事例に書かれた形を組み直す。

export const PR_URL = "https://github.com/guccyon/rally/pull/659";
export const ISSUE = 648;

/** 新しい PR のジョブの summary（CLAUDE.md の見出しの組）。 */
export const FIRST_SUMMARY = `## 概要
コーチの様子に返却待ちの動画添削の件数を足す。

## 対応内容
- getCoachOverview の応答に返却待ちの件数を足した

## 影響範囲・互換性
- 応答に項目が増えるだけ

## 動作確認
- cargo test

## 仮決め
### implementation id: 件数を数える置き場
- 選んだ: infra の問い合わせで数える`;

/** 続きのジョブの理由の並び（conflict が 20 回、ci_failed が 1 回。ci_failed は別の PR との 13 ファイルの衝突のあと）。 */
export const REASONS = [...Array(10).fill("conflict"), "ci_failed", ...Array(10).fill("conflict")];

/** 続きのジョブ（i 回目）の summary。続きのジョブも新しい PR と同じ ## の見出しの組で書いた。 */
export function continuationSummary(reason, i) {
  const what = reason === "ci_failed"
    ? "- 取り込みのあとに落ちた CI のテストを直した"
    : "- CHANGELOG.md の [Unreleased]「追加」の衝突を解いた（両方の行を残した）";
  return `## 概要
既定ブランチを取り込み、${reason === "ci_failed" ? "CI 落ちを直した" : "衝突を解いた"}（${i + 1} 回目）。

## 対応内容
${what}

## 動作確認
- cargo test`;
}

/** 新しい PR の本文（Engine の worker/entrypoint.sh の PR_BODY の形。engine.test.mjs は Engine から組む）。 */
export function newPrBody() {
  return `Closes #${ISSUE}

${FIRST_SUMMARY}

## 工場の記録
- job: 2790（attempt は dispatcher の記録を参照）
- model: tier-large、turns: 40、cost(API 換算): $1.95
`;
}

/** PR #659 のころの組み込み（summary を ## の見出しのまま本文の末尾に足す）で、21 回の追記のあとの本文を返す。 */
export function legacyBody(newPrBody) {
  return REASONS.reduce((body, reason, i) => `${body}\n\n${continuationSummary(reason, i)}\n`, newPrBody);
}

// 通る側の写し（議決 D-0032 の test の案、Issue #1038）。同じ 21 回の続きのジョブを、報告が「## 工場の記録」の下に正しく入る形で組む。
// 関所がこれを止めない（誤検知がない）ことを gate.test.mjs と engine.test.mjs が確かめる。

/** 続きのジョブ（i 回目）の summary の ## 見出しを、depth の階層（既定は ###）に下げた報告。 */
export function demotedSummary(reason, i, depth = 3) {
  return continuationSummary(reason, i).replace(/^## /gm, `${"#".repeat(depth)} `);
}

/** 続きのジョブ（i 回目）の、## 見出しを持たない報告（見出しなしの文と箇条書きだけ）。 */
export function plainSummary(reason, i) {
  return continuationSummary(reason, i).split("\n").filter((l) => !l.startsWith("## ") && l !== "").join("\n");
}

/** 21 回の続きのジョブの報告を、1 回ごとに「### 続きのジョブ（job N）の記録」の下へ足した本文の並び（i 回目の追記のあとの本文が i 番目）。 */
export function recordedBodies(newPrBody, report) {
  const bodies = [];
  REASONS.reduce((body, reason, i) => {
    const next = `${body}\n### 続きのジョブ（job ${2800 + i}）の記録\n\n${report(reason, i)}\n`;
    bodies.push(next);
    return next;
  }, newPrBody);
  return bodies;
}
