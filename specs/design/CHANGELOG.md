# デザインシステムの変更履歴

形は `governance.yaml` の `changelog.format`（版・日付・変えたもの・理由・効く画面・承認）。

## 0.1.0（2026-11-01）

- 変えたもの: デザインシステムの最初の正本。コンセプト `rally-court`（人が選んだ）から principles・concept_tone・tokens・components・patterns・content・accessibility・governance を置いた。student-app のテーマ（`lib/ui/theme/`）と基本の部品（`lib/ui/components/`）、admin-web の `@theme`（`src/app/globals.css`）と基本の部品（`src/components/ui/`）を tokens.yaml に合わせた
- 理由: rally にはデザインシステムがなかった（student-app は `colorSchemeSeed: Colors.teal`、admin-web は Tailwind の既定の色だけ）。壊す名前はない
- 効く画面: student-app と admin-web の全画面（`screens.ts` と `capture_test.dart` のすべての id。色・書体・角丸・余白が変わる）。画面の作り替え（部品への置き換え）は別のタスク
- 承認: 人（コンセプトの選択）、工場（基準）

## 0.1.1（2026-11-02）

- 変えたもの: `tokens.yaml` の `typography.families` に `bundling_status` を書き足した（student-app の書体の同梱はまだ。今は名前で指定し、端末に Noto Sans CJK JP がなければ端末の既定のゴシックで出る）。admin-web の `@theme`（`src/app/globals.css`）に書体の段の字間（caption 0.02em・small と body 0.01em・title と headline -0.01em）を写した
- 理由: 正本の「student-app はアプリに同梱」と実際（pubspec に fonts がない）がずれていた。admin-web は大きさと行間だけを写していて字間が抜けていた。同梱は別のタスク
- 効く画面: admin-web の全画面（`screens.ts` のすべての id。字間が少し変わる）。student-app は変わらない
- 承認: 工場（文の直しとトークンの写し）
