# specs/

プロダクト共通の SSOT。P1 入口のブートストラップで Blueprint から骨格を生成する。

**最初に `product-context.md` を読む**（Blueprint の要点。事業の形・Gen1 の範囲・不変条件・用語の注意）。

- `glossary.yaml`: ドメイン用語（Blueprint の domain_glossary）
  - 骨格を配置済み（中核の 12 語）。構造は `terms:` の配列で、各項目は `id`（英小文字スネークケース）・`ja`・`definition`（1 文）・`context`（context-map.yaml の id）を持つ
- `context-map.yaml`: 境界づけられたコンテキスト（初期案を配置済み。Gen1 のコアは identity / scheduling / entitlement。glossary の context はこの id を使う）
- `contracts/`: OpenAPI / AsyncAPI。5 アプリ分の境界を最初から引き、実装するのは Gen1 のクライアントのみ
  - 枝を配置済み（`api/` に student-app / admin-web / coach-app / coach-web / golf-app、`events/`）。`api/student-app/openapi.yaml` に Gen1 一周シナリオの最小 API を置いた。規約と互換ポリシーは `contracts/README.md`
- `analytics/`: 行動イベントの定義とセマンティック層
  - `events.yaml` を配置済み（Gen1 一周シナリオの行動イベント 11 件）。構造は `events:` の配列で、各項目は `name`（`<対象>.<出来事>`）・`ja`・`context`（context-map.yaml の id）・`actor`（student | coach | system）・`required`（必須属性。`occurred_at` と `student_id` / `coach_id` のどちらかを必ず含む）・`kpi`（KPI 名 or null）を持つ
  - `kpi.yaml` を配置済み（セマンティック層。KPI 6 件、うち north star 1 件）。構造は `kpis:` の配列で、各項目は `id`・`ja`（events.yaml の `kpi` が指す名前）・`north_star`・`formula`（events.yaml のイベント名と属性だけで書く擬似 SQL。書き方と語彙 `vocabulary:` はファイル冒頭）・`inputs`（式が読むイベント名）・`window`（`<日数>d`）・`window_on`・`direction`（up_is_good | down_is_good）・`note` を持つ。通話品質スコアの式（docs/adr/0002 Q1）もここだけに置く
  - 検査は `apps/backend/crates/application/tests/` の `kpi_semantic_layer.rs`（式が events.yaml にある名前だけを参照し、`inputs` と一致すること）と `analytics_emission.rs`（events.yaml の全イベントに発火するユースケースがあること。イベント名と発火箇所の対応表）。実際の集計ジョブとダッシュボードは P5
- `change-scenarios.yaml`: L3 の固定変更シナリオ
  - 初版を配置済み（Blueprint のシナリオの種から 7 件、すべて `status: candidate`）。構造は `scenarios:` の配列で、各項目は `id`・`title`・`description`・`expected_contexts`（context-map.yaml の id）・`rationale`・`status` を持つ
