# Rally

株式会社コートサイド（架空）のオンラインスポーツレッスン「Rally」の Product Monorepo。
自律型ソフトウェア事業部（Factory of Factories）の 1 号機（PoC）で、[guccyon/factory-engine](https://github.com/guccyon/factory-engine) をマウントして動く工場兼プロダクトになる。

> 企業名・数値・現状指標はすべて架空。外部連携（Stripe / Twilio / S3 / 行動トラッキング / プッシュ通知）は Gen1 ではすべてモック。

- 現在の Phase: **P0（計測基盤）Week 1**。工場の設定の骨格だけがあり、アプリコードはまだない（P1 入口のブートストラップで Walking Skeleton を立ち上げる）
- Gen1 の範囲: 生徒アプリ 1 本 ＋ 最小の admin-web。一周シナリオは「体験予約 → 初回レッスン → 完了記録」

## 構成

| パス | 役割 | 状態 |
|---|---|---|
| `.factory-config/manifest.yaml` | 憲法レベルの設定。autonomy（環境 × 変更種別）・予算・互換 | P0 起案（署名待ち） |
| `.factory-config/base/` | Engine が生成する設定（編集禁止）。P0〜P5 は Engine の版を指すだけ | 骨格 |
| `.factory-config/overlay/` | 学習で育つルール。各ルールに confidentiality（product / client / public）必須。P0 は書き込まない | 骨格 |
| `.factory-config/evals/` | この製品の Golden Set とルール検証。`golden/` にペア比較の採点を置く | 骨格 |
| `specs/` | SSOT（glossary・context-map・db-schemas・contracts・analytics） | P1 |
| `apps/` | backend（Rust / axum。`apps/backend/` に層ごとの Cargo ワークスペースと `/healthz`、CI のコマンドは `apps/backend/README.md`）・student-app（Flutter）・admin-web（Next.js） | P1（backend の骨格を配置済み） |
| `apps/admin-web/` | 最小の管理画面（Next.js App Router・TypeScript）。コーチ（枠の公開・レッスンメニューの登録と編集・添削の返却・生徒とのチャット（添削の依頼をチャットの中から返す）・通話参加・自分の評価の件数と平均と一覧（評価への返信）・自己紹介の編集・ホームに今月と先月のレッスンの実績・お知らせ（未読の数と一覧。行から予約・添削・チャットへ））と本部（一覧と、スポーツ別の予約枠の需給 `/headquarters/slot-supply`。`getSlotSupply`。コーチを一覧から選んで評価の件数と平均と一覧 `/headquarters/coach-review-summary`。`listCoaches` / `getCoachReviewSummary` / `listCoachReviews`。コーチの一覧で受付を止める・再開する `/headquarters/coaches`。`suspendCoach` / `resumeCoach`。一覧の行から開くコーチ 1 人の様子（受付の状態・評価の件数と平均・今月と先月の実績・公開中のメニューの数） `/headquarters/coach-overview`。`getCoachOverview`。コーチのレッスンメニューの横断の一覧と、行から非公開にする・公開に戻す `/headquarters/lesson-menus`。`listLessonMenus` / `unpublishLessonMenu` / `republishLessonMenu`。生徒の一覧（スポーツで絞り込み） `/headquarters/students`。`listStudents`。行から生徒 1 人の利用の様子 `/headquarters/students/activity`。`getStudentActivity`。日付の期間で予約の一覧（状態の色分けと、確定待ちのまま開始が近い予約の強調） `/headquarters/bookings`。`listBookings`）をロールで出し分ける。API の型は admin-web の OpenAPI から生成。コマンドは `apps/admin-web/README.md` | P1（最小の画面を配置済み） |
| `apps/student-app/` | 生徒アプリの最小（Flutter・Riverpod）。アカウント登録 → スロット検索 → 体験予約 → 確定 → 通話（モック）か動画の送付（モック）→ 添削結果 → 完了記録 → 評価 → 月額契約の画面と、自分の予約（一覧とキャンセル）と、自分の動画（送った動画と添削の状態の一覧。返却済みから添削結果）と、担当コーチとのチャット（スレッドの一覧・メッセージの一覧と送信・既読）と、通知の受け取り設定と、お知らせ（一覧と既読。ホームに未読のしるし。行から予約・動画・チャットへ）と、生徒のスポーツのコーチを探す画面（評価の平均と件数・レッスンメニューの数）とコーチの紹介（自己紹介・評価・最近の評価・レッスンメニュー。そのコーチの空き枠へ。お気に入りの切り替え）と、ホームのお気に入りのコーチと、自分のプロフィール（表示名の変更）と、受けたレッスン（完了したレッスンの一覧。未評価の行から評価、動画添削の行から添削結果へ）。API のスキーマは student-app の OpenAPI に揃えて `lib/api/` に置く。確認は `flutter analyze` と `flutter test` まで（実機ビルドは対象外）。コマンドは `apps/student-app/README.md` | P1（最小の画面を配置済み） |
| `docs/adr/` | 設計判断の記録 | 随時 |
| `docs/plans/` | Phase ごとの実行計画（P1 入口の Walking Skeleton: `p1-walking-skeleton.md`） | Phase 境界 |

## Blueprint

この工場の入力は Blueprint（`blueprint_version: 0.4`）。正本はプロジェクトの資料フォルダにあり、決定は `decisions:` に日付付きで追記する。
`manifest.yaml` の autonomy・budgets・compat は Blueprint の同名の節と一致させる。

## 利用について

この工場は「仮想開発事業部」の 1 号機で、実行には Engine の実行体を含む。工場で育った固有の知識（`.factory-config/overlay/` `evals/`）はプロダクト側に帰属し、Engine の知財は建設者に帰属する（factory-engine ADR 0001）。開発中のため public で公開しているが、ライセンスはまだ定めていない。
