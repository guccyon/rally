# specs/

プロダクト共通の SSOT。P1 入口のブートストラップで Blueprint から骨格を生成する。

- `glossary.yaml`: ドメイン用語（Blueprint の domain_glossary）
  - 骨格を配置済み（12 語）。構造は `terms:` の配列で、各項目は `id`（英小文字スネークケース）・`ja`・`definition`（1 文）・`context`（境界づけられたコンテキスト名）
- `context-map.yaml`: 境界づけられたコンテキスト（初期案は Blueprint の bounded_contexts_hint。Gen1 のコアは identity / scheduling / entitlement）
- `contracts/`: OpenAPI / AsyncAPI。5 アプリ分の境界を最初から引き、実装するのは Gen1 のクライアントのみ
- `analytics/`: 行動イベントの定義とセマンティック層
- `change-scenarios.yaml`: L3 の固定変更シナリオ
