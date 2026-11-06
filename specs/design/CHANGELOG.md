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

## 0.4.0（2026-11-05）

- 変えたもの: `components.yaml` に `date_field`（日付の欄。種類は `date`・`date_range`、状態は通常・押下・フォーカス・無効・読み込み中・誤り、誤りの文言の型は空の欄・終わりが始まりより前・31 日を超える）を足し、`text_field` の `not_for` に書き足した。`tokens.yaml` に `color.slot_state`（open は info、booked は success、closed は neutral）・`state_rules`・info の比を足した。`status_tag` にスロットの状態の見本、`governance.yaml` の lint に `date-field-only`・`state-color-map`、`requests` に `checkbox`（保留）を足した。admin-web に `DateField`・`DateRangeField`（`src/components/ui/DateField.tsx`）と誤りの型 `dateRangeError`（`src/components/ui/dateRange.ts`）、状態の色の対応 `SLOT_STATE_STATUS` を足し、コーチの予約枠の一覧の状態を `StatusTag` にした。student-app に `RallySlotState` を足した。両アプリの neutral の状態のタグに border の 1px の輪郭を付けた（`state_rules`）。両アプリのカタログに見本を足した（student-app は `date_field` を `not_for` により置かない）
- 理由: #455 の画面の設計で部品が足りなかった（admin-web は 11 の部品が `input type=date` を直に書き、スロットの状態に色の対応がなかった）。`date_field` は 2 つの欄にまたがる誤りと既定の期間を読む loading があるので、`text_field` の種類にせず別の部品にした。既存の名前と値は消していない
- 効く画面: 正本の上では admin-web の期間の欄のある画面（`hq-bookings*`・`hq-cancelled-bookings*`・`hq-live-sessions*`・`hq-returned-video-reviews*`・`hq-slot-supply*`・`hq-coach-reviews*`・`hq-low-reviews*`・`hq-students*`・`hq-announcements*`・`hq-coach-overview`・`coach-slots*`）と `design-catalog`。この版で画面が変わるのは両アプリの `design-catalog`、`coach-slots*`（状態がタグになる）。11 の部品の `input type=date` の `DateRangeField` への置き換えと `periodError` の文言の置き換えは別のタスク
- 承認: 工場（新しい部品・should の基準・対応の追加）

## 0.5.0（2026-11-06）

- 変えたもの: 「スコアボード」の方向に見直した（#981）。`tokens.yaml` の色の役割の値（primary・on_primary_container・text・on_secondary・inverse_surface #3A54B0／#10131A→インク #0E1013、primary_pressed #2E4590→#33372F、primary_container #E9EDF8→#E4E6E0、surface #F1F3F8→#F2F3EE、border #DDE1EA→#E4E6E0、border_strong #7D8599→#7A7F74、text_muted #4A5163→#5B6058、on_inverse_surface_muted #B8BFCC→#B9BDB3、focus_ring #3A54B0→#667F00、scrim の色、info を青からインクと surface に）と `contrast_checked`・`not_allowed_pairs`。`primary_container_pressed`（#D6D9D0）・`scale.ink` の 200 と 800・`scale.lime` の 700 を足し、`scale.blue` を deprecated にした（1.0.0 で消す）。数字の書体 `typography.families.numeric`（Barlow Condensed、SIL OFL 1.1）と `weights.extrabold`、書体の段 `figure`・`figure_sm` を足し、display（44px 900→numeric 64px 800）・headline（28px→32px・-0.03em）・title（20px 700→17px 900）の値と admin-web の headline（24px→28px・-0.02em）を変えた。`shape.usage` でボタンを pill から md に、elevation のカードを surface から background と border の線にした。`components.yaml` の button（角丸 md・高さ 56px・太さ 900、secondary を塗りに、text に下線、focus の状態）・card（variants の plain と booking の日付のコマ）・status_tag（点と字、urgent）・notice（inline を白地と左の線に、success の濃い面と完了の印）・list_row（selected）・data_table（包みの面をやめ、見出しの下線を primary の 2px に）・confirm_dialog（cancel を secondary に）と `preview_css`。`concept_tone.yaml` の `direction`・`calm_primary`・`copy`・`difference`、`principles.yaml` の one-hot-spot（ライムの用途を 3 つに）と not_doing、`patterns.yaml` の common_layout（Section を面で囲わない）、`accessibility.yaml` の例、`governance.yaml` の lint（font-family の allow、deprecated-token・button-shape・status-tag-shape を足した）。0.4.0 の `slot_state`・`state_rules`（neutral を点と字に合わせた）・`date_field`・lint・`requests` は残した。両アプリのテーマと基本の部品を新しい値と形にし、Barlow Condensed を両アプリに同梱した（student-app は `assets/fonts/` と pubspec の fonts と `LicenseRegistry`、admin-web は `public/fonts/` と `@font-face`、どちらも `OFL.txt`）。カタログ（`catalog/catalog.html`・`catalog.png`）を組み立て直した
- 理由: 人がカタログ 0.3.0 を見て「往年の Bootstrap 感があって垢抜けない」と指摘し、3 つの方向を同じ画面で比べて案 A「スコアボード」を選んだ（2026-11-05）。出どころは青のピル型の主ボタンと線の副ボタン、淡い色の札と知らせ、青みの灰、全部を同じ灰の面で囲うこと、字の大小差の小ささ、数字らしさのなさ。名前は消さず値と形だけを変えた。新しい値も WCAG 2.2 AA を満たす（primary は background で 19.05、text_muted は surface で 5.78、focus_ring は白で 4.56・インクの上で 4.17）。0.x の間は 0.2.0 と同じく、名前を保った値の変更と追加を minor とした。デザイナー役の正本は 0.4.0 として書かれていたが、0.4.0 は date_field（#455）で使っていたので 0.5.0 にした
- 効く画面: student-app と admin-web の全画面（`screens.ts` と `capture_test.dart` のすべての id。主なボタン・リンク・選択中・フォーカスがインクに、灰が中立に、ボタンが角丸 md に、カードと admin-web の Section が白地と線に、状態のタグが点と字に、知らせが白地と左の線（成功は濃い面）に変わる）。予約のカードの日付のコマ（`RallyBookingCard`）は部品とカタログ（`design-catalog`）だけで、ホームなどの画面への置き換えは別のタスク
- 承認: 人（色の役割の値・concept_tone と principles の変更・書体の追加、2026-11-05）、工場（部品の基準・lint・見本）
