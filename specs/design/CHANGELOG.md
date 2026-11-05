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

## 0.3.0（2026-11-05）

- 変えたもの: `tokens.yaml` の `spacing.grid.pc` を書き換えた（「admin-web は 1 段で最大 48rem、一覧の表は 1 段の幅に収める」をやめ、読む画面は `reading` の最大 768px、本部の一覧の画面は `wide` の最大 1280px。どちらも左右 lg 24px で中央寄せ）。`spacing.grid.admin_web_narrow`（600〜1023px は幅いっぱいで左右 24px、600px 未満は左右 16px。表が収まらないときは表の包みだけを横に動かす）・`spacing.content_width_px`・`spacing.wide_rules` を足し、`typography.max_line_length` の注を合わせた。`components.yaml` に `data_table`（表）を足した（セルは単語の途中で折り返さない、見出し・日時・数字・名前・状態・種類は 1 行、列のあいだは spacing の段でセルの左右 sm 8px・列のあいだ md 16px、横スクロールは表の包みだけでページ全体は動かさない）。`patterns.yaml` の `common_layout.admin_web` と `list` に幅と表の基準、`accessibility.yaml` の `text-resize` に表の包みの例外（WCAG 1.4.10）、`governance.yaml` の lint に `table-no-page-scroll` を足した。admin-web に表の部品 `DataTable`（`src/components/ui/DataTable.tsx`）を足し、カタログ（`/design`）に見本を置いた。student-app のカタログには置かない（`not_for` に student-app とある）
- 理由: 本部の一覧の表が 48rem の枠（RoleGate の `max-w-3xl` と Section の padding で表は 550〜690px ほど）に押し込まれ、見出しや値が単語の途中で折り返し、列が `py-1` の余白だけでくっついていた（#896・#919）。既存の名前は消さず、`grid.pc` の値（文）だけを変えた。`reading` の 768px は今の 48rem と同じなので、読む画面の幅は変わらない
- 効く画面: 正本の上では admin-web の本部の一覧の画面（`screens.ts` の `hq-bookings*`・`hq-cancelled-bookings*`・`hq-students*`・`hq-coaches*`・`hq-lesson-menus*`・`hq-slot-supply*`・`hq-incomplete-coach-profiles*`・`hq-pending-video-reviews*`・`hq-returned-video-reviews*`・`hq-live-sessions*`・`hq-overdue-live-sessions*`・`hq-unanswered-messages*`）。表のセルの決まりは coach-* の表と本部の詳細の表にも効く（幅は reading のまま）。この版で画面が変わるのはカタログ（`design-catalog`）だけで、各一覧の RoleGate の幅と表の `DataTable` への置き換えは #896 の実装役のタスク
- 承認: 工場（新しい部品と should の基準、レイアウトの値の文の直し。must の追加と色の値の変更はない）
