# specs/

プロダクト共通の SSOT。P1 入口のブートストラップで Blueprint から骨格を生成する。

**最初に `product-context.md` を読む**（Blueprint の要点。事業の形・Gen1 の範囲・不変条件・用語の注意）。

- `glossary.yaml`: ドメイン用語（Blueprint の domain_glossary）
  - 骨格を配置済み（中核の 12 語）。構造は `terms:` の配列で、各項目は `id`（英小文字スネークケース）・`ja`・`definition`（1 文）・`context`（context-map.yaml の id）を持つ
- `context-map.yaml`: 境界づけられたコンテキスト（初期案を配置済み。Gen1 のコアは identity / scheduling / entitlement。glossary の context はこの id を使う）
- `contracts/`: OpenAPI / AsyncAPI。5 アプリ分の境界を最初から引き、実装するのは Gen1 のクライアントのみ
- `analytics/`: 行動イベントの定義とセマンティック層
  - `events.yaml` を配置済み（Gen1 一周シナリオの行動イベント 11 件）。構造は `events:` の配列で、各項目は `name`（`<対象>.<出来事>`）・`ja`・`context`（context-map.yaml の id）・`actor`（student | coach | system）・`required`（必須属性。`occurred_at` と `student_id` / `coach_id` のどちらかを必ず含む）・`kpi`（KPI 名 or null）を持つ
- `change-scenarios.yaml`: L3 の固定変更シナリオ
  - 初版を配置済み（Blueprint のシナリオの種から 7 件、すべて `status: candidate`）。構造は `scenarios:` の配列で、各項目は `id`・`title`・`description`・`expected_contexts`（context-map.yaml の id）・`rationale`・`status` を持つ
