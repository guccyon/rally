// 知見 K-0025 の担保（議決 D-0151 の test の案、Issue #1391）。実装役の最後の文（session の output）と、そこから Engine が組んだ PR 本文の
// ## 見出しの並びと数を照らす。受け渡し（summary.md を作る取り出し）で報告の一部しか拾わないと、PR 本文の ## 見出しが報告より欠ける。
// 事例 C-0012（PR #982）の報告そのものは記録にないので、事例に書かれた形（実装役の最後の文は ## 概要 から書かれ、summary.md と PR 本文は
// 「  - カタログにスロットの状態の見本を足した」という箇条書きの途中から始まる）を組み直す。
import { FACTORY_RECORD, headings } from "./check.mjs";

export const ISSUE_979 = 979;
export const PR_URL_982 = "https://github.com/guccyon/rally/pull/982";

/** C-0012: summary.md と PR 本文が始まっていた行（事例の「出来事」）。 */
export const C0012_CUT_LINE = "  - カタログにスロットの状態の見本を足した";

/** 仮決め 1 件（見出しと 6 行と空行の 8 行）。 */
const provisional = (kind, id, question, chosen) => `### ${kind} ${id}: ${question}
- 選んだ: ${chosen}
- 他の案: この PR では見送る
- 根拠: specs/design/governance.yaml の 0.4.0 の migration
- 解釈: Issue の「画面の作り替えは別のタスク」と migration の列挙を並べて読み、この PR の範囲を決めた
- 確信度: 0.6
- 影響: specs/design/ と、admin-web・student-app のカタログ
`;

/**
 * C-0012: 実装役の最後の文（session の output）の形。## 概要 から書かれ、仮決め 5 件を含んで 80 行を超える。
 * 末尾の 80 行は C0012_CUT_LINE から始まる（2026-11-11 の手元の Engine（34a53af）の tail -80 で切ると、事例の summary.md と同じ所から始まる）。
 */
export const C0012_REPORT = `## 概要
デザインシステムの正本を 0.4.0 に上げ、日付の欄とスロットの状態の色を足した。admin-web と student-app のカタログに見本を並べた。

## 対応内容
- specs/design/ の正本を 0.4.0 にした
  - 日付の欄（date-field）の部品を components に足した
  - スロットの状態の色のトークンを tokens に足した
  - governance の 0.4.0 の migration を書いた
- admin-web
  - カタログに日付の欄の見本を足した
  - カタログに状態の色の一覧を足した
  - 公開の欄の状態の表示をトークンの色に置き換えた
- student-app
${C0012_CUT_LINE}
  - スロットの状態の色をトークンから引くようにした
  - 日付の欄の見本をカタログに足した
  - キャプチャの一覧（capture_test.dart）にカタログの画面を足した
  - 状態の色のテストを足した
  - 日付の欄の見本のテストを足した
- admin-web の画面の一覧（screens.ts）にカタログの画面を足した
- changelog.d/979.md を足した

## 影響範囲・互換性
- 画面の見た目は、公開の欄の状態の表示の色だけが変わる
- 既存の画面の日付の入力は置き換えていない（別のタスク）
- トークンの名前は足しただけで、消したものはない
- API と DB は変えていない
- 0.3.0 のトークンを読む画面はそのまま動く
- student-app の画面の文言は変えていない

## 動作確認
- admin-web: npm test と npm run lint
- admin-web: カタログの画面を手元で開いて見本の並びを見た
- student-app: Flutter がないので CI に任せた
- student-app: キャプチャも CI に任せた
- specs/design/ のトークンの JSON が 0.4.0 のスキーマに通る
- tools/design-catalog の検査が通る
- admin-web のキャプチャの一覧に足した画面の名前が画面と合う
- changelog.d/979.md の書き方が changelog-check に通る

## 補足
- 11 の部品の日付の入力の置き換えは別のタスクに回した
- 期間の空の画面のキャプチャは CI で撮る
- 公開の欄の状態の表示の文言は変えていない
- 期間の上限の定数は既存のものを確かめていない
- admin-web の既存の画面の並びは変えていない
- student-app の既存の画面の色は変えていない

## 仮決め
${provisional("scope", "date-field-replace", "日付の欄の置き換えをこの PR でやるか", "カタログの見本だけにし、画面の置き換えは別のタスクに回す")}
${provisional("aesthetic", "slot-state-color", "スロットの状態の色", "空き・予約済み・締め切りをトークンの 3 色にする")}
${provisional("ui", "catalog-order", "カタログの見本の並び", "部品ごとに縦に並べる")}
${provisional("implementation", "slot-state-label", "状態のラベルの置き場所", "カタログの中に書く")}
${provisional("system_interface", "token-name", "状態の色のトークンの名前", "color.slot.<状態> にする")}
## 知見
- 正本の改定と使う側の反映を 1 つのチケットで扱うと、仮決めが増える
- student-app のテストは Flutter のない環境では走らない
- カタログの見本はトークンの名前で色を引く`;

/** C-0012: PR #982 に載った本文の形（Engine の PR_BODY の形で、summary.md の C0012_CUT_LINE から始まる）。 */
export const C0012_PR_982 = `Closes #${ISSUE_979}

${C0012_REPORT.split("\n").slice(C0012_REPORT.split("\n").indexOf(C0012_CUT_LINE)).join("\n")}

## 工場の記録
- job: 4931（attempt は dispatcher の記録を参照）
- model: tier-large、turns: 120、cost(API 換算): $8.40
`;

/**
 * 複数のテキストの塊に分かれた実装役の最後の文（80 行に収まる）。塊ごとに ## 見出しの節を持ち、
 * 塊の間はセッションの出力でつないだときの空行 1 つ（BLOCK_SEPARATOR）。最初の塊が ## 概要 から始まる。
 */
export const BLOCK_SEPARATOR = "\n\n";
export const SPLIT_BLOCKS = [
  `## 概要
コーチの様子にスロットの状態の色を足した。`,
  `## 対応内容
- admin-web のコーチの様子に、スロットの状態の色をトークンから引いて付けた
- changelog.d/979.md を足した`,
  `## 動作確認
- admin-web: npm test`,
  `## 仮決め
${provisional("aesthetic", "slot-state-color", "スロットの状態の色", "空き・予約済み・締め切りをトークンの 3 色にする").trimEnd()}`,
  `## 知見
- スロットの状態の色は tokens の color.slot にある`,
];
export const SPLIT_REPORT = SPLIT_BLOCKS.join(BLOCK_SEPARATOR);

/** 文の ## 見出しを出てくる順に返す（コードブロックの中は数えない）。 */
export const h2s = (text) => headings(text).filter((h) => h.level === 2).map((h) => h.text);

/**
 * 実装役の最後の文（report）と PR 本文（body）の ## 見出しの並びと数を照らし、外れの一覧を返す（空ならそろっている）。
 * 本文は Engine が付ける「## 工場の記録」より前だけを見る（そこから下は Engine の記録と続きのジョブの ###）。
 */
export function reportHeadingProblems(report, body) {
  const want = h2s(report);
  const all = h2s(body);
  const record = all.indexOf(FACTORY_RECORD);
  const got = record < 0 ? all : all.slice(0, record);
  const show = (hs) => hs.map((h) => `## ${h}`).join(" / ") || "（なし）";
  if (got.length !== want.length) {
    return [{ rule: "report_heading_count", expected: want, actual: got,
      message: `PR 本文の ## 見出しが ${got.length} 個で、実装役の最後の文の ${want.length} 個とそろわない（報告: ${show(want)}、本文: ${show(got)}）` }];
  }
  const at = want.findIndex((h, i) => h !== got[i]);
  if (at < 0) return [];
  return [{ rule: "report_heading_order", expected: want, actual: got,
    message: `PR 本文の ${at + 1} 番目の ## 見出しが「## ${got[at]}」で、実装役の最後の文の「## ${want[at]}」と違う（報告: ${show(want)}、本文: ${show(got)}）` }];
}
