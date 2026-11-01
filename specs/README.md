# specs/

プロダクト共通の SSOT。P1 入口のブートストラップで Blueprint から骨格を生成する。

**最初に `product-context.md` を読む**（Blueprint の要点。事業の形・Gen1 の範囲・不変条件・用語の注意）。

- `glossary.yaml`: ドメイン用語（Blueprint の domain_glossary）
  - 骨格を配置済み（中核の 12 語）。構造は `terms:` の配列で、各項目は `id`（英小文字スネークケース）・`ja`・`definition`（1 文）・`context`（context-map.yaml の id）を持つ
- `context-map.yaml`: 境界づけられたコンテキスト（初期案を配置済み。Gen1 のコアは identity / scheduling / entitlement。glossary の context はこの id を使う）
- `contracts/`: OpenAPI / AsyncAPI。5 アプリ分の境界を最初から引き、実装するのは Gen1 のクライアントのみ
  - 枝を配置済み（`api/` に student-app / admin-web / coach-app / coach-web / golf-app、`events/`）。`api/student-app/openapi.yaml` に Gen1 一周シナリオの最小 API を置いた。規約と互換ポリシーは `contracts/README.md`
- `analytics/`: 行動イベントの定義とセマンティック層
  - `events.yaml` を配置済み（Gen1 一周シナリオの行動イベント 11 件と、チャットの `message.sent`）。構造は `events:` の配列で、各項目は `name`（`<対象>.<出来事>`）・`ja`・`context`（context-map.yaml の id）・`actor`（student | coach | system）・`required`（必須属性。`occurred_at` と `student_id` / `coach_id` のどちらかを必ず含む）・`kpi`（KPI 名 or null）を持つ。すべてのイベントに付けられる共通の任意属性はトップレベルの `common_optional:`（各項目は `name`・`ja`・`description`・`example`）に置き、今は `release_id`（版（Release）の ID。例: `guccyon/rally@0.1.0`）と `app_version`（アプリの版。例: `0.1.0`）。利用者の声や不具合の報告を版に結ぶ入口で、発火する側は値が分かるときだけ付け、分からなければ付けない（欠けていても発火は止めない）。`required` には入れない
  - `kpi.yaml` を配置済み（セマンティック層。KPI 6 件、うち north star 1 件）。構造は `kpis:` の配列で、各項目は `id`・`ja`（events.yaml の `kpi` が指す名前）・`north_star`・`formula`（events.yaml のイベント名と属性だけで書く擬似 SQL。書き方と語彙 `vocabulary:` はファイル冒頭）・`inputs`（式が読むイベント名）・`window`（`<日数>d`）・`window_on`・`direction`（up_is_good | down_is_good）・`note` を持つ。通話品質スコアの式（docs/adr/0002 Q1）もここだけに置く
  - 検査は `apps/backend/crates/application/tests/` の `kpi_semantic_layer.rs`（式が events.yaml にある名前だけを参照し、`inputs` と一致すること）と `analytics_emission.rs`（events.yaml の全イベントに発火するユースケースがあること。イベント名と発火箇所の対応表）。実際の集計ジョブとダッシュボードは P5
- `design/`: デザインシステム（P4・ADR 0025「画面はデザインシステムの部品で組む」）
  - `components.yaml`: 部品の一覧（正本）。構造は `components:` の配列で、各項目は `id`・`ja`・`purpose`（用途）・`variants`（種類。各項目は `id`・`ja`・`purpose`。なければ省く）・`states`（要る状態。語彙はトップレベルの `state_vocabulary:`）・`tokens`（`tokens.yaml` のトークンの名前。入れ子のキーをドットでつないだもの。`tokens.yaml` はデザインシステムの正本の PR #505 が置き、名前はそれに揃えてある。この一覧は #505 のあとにマージする）を持つ。admin-web は `src/components/ui/`、student-app は `lib/ui/` に同じ id の部品を置き、部品 × 種類 × 状態をすべてカタログ（admin-web の `/design`、student-app の `DesignCatalogScreen`）に並べる。対応は admin-web `tests/designSystem.test.tsx` と student-app `test/ui/design_catalog_test.dart` が確かめる
  - `patterns.yaml`: 画面の型（一覧・詳細・入力・送る前の確認）。構造は `patterns:` の配列で、各項目は `id`・`ja`・`purpose`・`layout`（上からの並び。各段は `component`（components.yaml の id）・任意の `variant` / `when` / `optional` / `note`）・`components`（使う部品の id）・`examples`（近い今の画面のキャプチャの id）を持つ
- `change-scenarios.yaml`: L3 の固定変更シナリオ
  - 初版を配置済み（Blueprint のシナリオの種から 7 件、すべて `status: candidate`）。構造は `scenarios:` の配列で、各項目は `id`・`title`・`description`・`expected_contexts`（context-map.yaml の id）・`rationale`・`status` を持つ
