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

## 0.2.0（2026-11-02）

- 変えたもの: `tokens.yaml` の青の値（primary・focus_ring・info.fg #1E3FD0→#3A54B0、primary_pressed #1A2FA8→#2E4590、primary_container・info.container #E6EBFF→#E9EDF8、on_primary_container・info.on_container #14246E→#1E2A5C。scale.blue も同じ段）と contrast_checked の比。`concept_tone.yaml` に `copy`（コピー「コートの熱。次の練習へ、前のめりに。」の 3 語とデザインの結びつき）と `calm_primary`。`components.yaml` の empty_state に variants（screen・section）と基準を足し、文を text_muted から text にした。全部の部品に `preview`（状態ごとの見本）と `preview_css`、`typography.scale` に日本語の `sample` を足した。正本からカタログ（`catalog/catalog.html`・`catalog/catalog.png`）を組み立てる `tools/design-catalog/` を足した。アプリは student-app の `RallyColors`、admin-web の `@theme` を新しい値にし、両アプリの `EmptyState` が variant を受けてボタンの種類を決めるようにした
- 理由: 人が見本のカタログを見て「青が強い」「空の表示のボタンが読みにくい」と指摘し、彩度を下げた値と empty_state の形を決めた（2026-11-02）。名前は変えず値だけ。新しい値も WCAG 2.2 AA を満たす（primary は background で 6.83、surface で 6.15、primary_container で 5.83）
- 効く画面: student-app と admin-web の全画面（`screens.ts` と `capture_test.dart` のすべての id。青が落ち着く）。empty_state は今は両アプリのカタログ（`design-catalog`）だけが使う
- 承認: 人（色の役割の値の変更・concept_tone の変更、2026-11-02）、工場（部品の基準と見本）
