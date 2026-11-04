# 知見の一覧（自動生成。手で編集しない）

core-gates/knowledge_check.py --index で作り直す。期限は review_by。

| id | テーマ | 種類 | 確かさ | 状態 | 担保 | 確認日 | 期限 | 主張 |
|---|---|---|---|---|---|---|---|---|
| K-0001 | 4 | causal | hypothesis | active | context | 2026-11-03 | 2026-12-03 | 続きのジョブ（continue。既定ブランチとの衝突の解消など）の報告が、既存の PR 本文への追記になることが実装役に伝わらず、 Engine も見出しの階層 |
| K-0003 | 4 | causal | hypothesis | draft | context | 2026-11-01 | 2026-12-01 | 実装役が書く報告（summary.md）の型と、検査役が PR-002 で見る PR テンプレートの見出しが揃っていないと、 報告がそのまま PR 本文になるの |
| K-0002 | 6 | causal | hypothesis | active | deterministic | 2026-11-03 | 2026-12-03 | 並列で進む PR がすべて CHANGELOG.md の [Unreleased] の同じ場所に 1 行ずつ足す運用だと、コードに関係のない衝突が起きやすく、  |
| K-0004 | 6 | causal | hypothesis | draft | context | 2026-11-01 | 2026-12-01 | 差し戻しの直しで PR 本文を末尾への追記だけで直し、テンプレートの順に本文全体を組み直す持ち主がいないと、 見出しの順と階層が崩れて新しい差し戻しを生む。同じ |
