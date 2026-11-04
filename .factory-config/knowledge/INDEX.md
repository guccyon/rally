# 知見の一覧（自動生成。手で編集しない）

core-gates/knowledge_check.py --index で作り直す。期限は review_by。

| id | テーマ | 種類 | 確かさ | 状態 | 担保 | 確認日 | 期限 | 主張 |
|---|---|---|---|---|---|---|---|---|
| K-0001 | 4 | causal | hypothesis | active | context | 2026-11-03 | 2026-12-03 | 続きのジョブ（continue。既定ブランチとの衝突の解消など）の報告が、既存の PR 本文への追記になることが実装役に伝わらず、 Engine も見出しの階層 |
| K-0003 | 4 | causal | hypothesis | active | context | 2026-11-02 | 2026-12-02 | 続きの job（continue）が自分の報告を PR 本文の末尾に追記すると、見出しが二重になり、取り込みやコミットの状態も head と食い違いやすい。本文 |
| K-0002 | 6 | causal | hypothesis | active | deterministic | 2026-11-03 | 2026-12-03 | 並列で進む PR がすべて CHANGELOG.md の [Unreleased] の同じ場所に 1 行ずつ足す運用だと、コードに関係のない衝突が起きやすく、  |
| K-0004 | 6 | causal | hypothesis | active | context | 2026-11-02 | 2026-12-02 | 人の判断待ち（factory:needs-judgment）の停止中に PR が main と衝突しても、衝突を解く continue を判断の後まで起こさない |
