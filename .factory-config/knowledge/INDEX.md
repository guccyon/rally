# 知見の一覧（自動生成。手で編集しない）

core-gates/knowledge_check.py --index で作り直す。期限は review_by。

| id | テーマ | 種類 | 確かさ | 状態 | 担保 | 確認日 | 期限 | 主張 |
|---|---|---|---|---|---|---|---|---|
| K-0001 | 4 | causal | hypothesis | active | context | 2026-11-03 | 2026-12-03 | 続きのジョブ（continue。既定ブランチとの衝突の解消など）の報告が、既存の PR 本文への追記になることが実装役に伝わらず、 Engine も見出しの階層 |
| K-0004 | 4 | causal | hypothesis | active | context | 2026-11-03 | 2026-12-03 | PR 本文の本体の節と、既定ブランチの取り込みの記録を同じ見出し（## 概要・## 対応内容など）で書くと、 書く役（衝突解消の implement）・直す役（ |
| K-0002 | 6 | causal | hypothesis | active | deterministic | 2026-11-03 | 2026-12-03 | 並列で進む PR がすべて CHANGELOG.md の [Unreleased] の同じ場所に 1 行ずつ足す運用だと、コードに関係のない衝突が起きやすく、  |
| K-0003 | 6 | causal | hypothesis | active | context | 2026-11-03 | 2026-12-03 | 長く開いた PR の衝突解消（continue、reason=conflict）には回数の上限も人への引き継ぎもないため、 既定ブランチが動き続けると衝突解消が |
| K-0005 | 6 | causal | hypothesis | active | context | 2026-11-03 | 2026-12-03 | 同じ PR の本文を書き換えるジョブ（衝突解消の implement の continue と body_fix）を順に並べずに動かすと、 片方の直しを片方が上 |
