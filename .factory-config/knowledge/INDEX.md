# 知見の一覧（自動生成。手で編集しない）

core-gates/knowledge_check.py --index で作り直す。期限は review_by。

| id | テーマ | 種類 | 確かさ | 状態 | 担保 | 確認日 | 期限 | 主張 |
|---|---|---|---|---|---|---|---|---|
| K-0001 | 4 | causal | hypothesis | active | context | 2026-11-04 | 2026-12-04 | 続きのジョブ（continue。既定ブランチとの衝突の解消など）の報告が、既存の PR 本文への追記になることが実装役に伝わらず、 Engine も見出しの階層 |
| K-0003 | 4 | causal | hypothesis | active | context | 2026-11-01 | 2026-12-01 | judge の verdict「adopt」は「仮決めを認める」と「欠陥を直す対象にする」の 2 つの意味で使われ、incident の adopted に区別 |
| K-0005 | 4 | causal | hypothesis | draft | context | 2026-11-04 | 2026-12-04 | 続きの job（continue）が PR 本文に書く自分の報告で、既定ブランチの取り込みやコミットの状態を head を確かめずに書くと、 報告を書いた時点の |
| K-0002 | 6 | causal | hypothesis | active | deterministic | 2026-11-03 | 2026-12-03 | 並列で進む PR がすべて CHANGELOG.md の [Unreleased] の同じ場所に 1 行ずつ足す運用だと、コードに関係のない衝突が起きやすく、  |
| K-0004 | 6 | causal | hypothesis | active | context | 2026-11-01 | 2026-12-01 | supervisor が差し戻し（sendback）を「直っていない採用欠陥（defect が null でなく、verdict が adopt）があるか」では |
| K-0006 | 6 | causal | hypothesis | draft | context | 2026-11-04 | 2026-12-04 | 人の判断待ち（factory:needs-judgment）の停止中に PR が main と衝突（pr.conflicted）しても、衝突を解く contin |
