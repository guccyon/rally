# 知見の一覧（自動生成。手で編集しない）

core-gates/knowledge_check.py --index で作り直す。期限は review_by。

| id | テーマ | 種類 | 確かさ | 状態 | 担保 | 確認日 | 期限 | 主張 |
|---|---|---|---|---|---|---|---|---|
| K-0001 | 4 | causal | hypothesis | active | deterministic | 2026-11-05 | 2026-12-05 | 続きのジョブ（continue。既定ブランチとの衝突の解消など）の報告が、既存の PR 本文への追記になることが実装役に伝わらず、 Engine も見出しの階層 |
| K-0003 | 4 | causal | hypothesis | active | context | 2026-11-01 | 2026-12-01 | judge の verdict「adopt」は「仮決めを認める」と「欠陥を直す対象にする」の 2 つの意味で使われ、incident の adopted に区別 |
| K-0005 | 4 | causal | hypothesis | active | context | 2026-11-04 | 2026-12-04 | PR 本文の見出しの定義が、実装役の報告の型・検査役の rubric（Engine の PR-002）・リポジトリの決まり（CLAUDE.md の PR の説明 |
| K-0007 | 4 | causal | hypothesis | draft | context | 2026-11-04 | 2026-12-04 | 続きの job（continue）が PR 本文に書く自分の報告で、既定ブランチの取り込みやコミットの状態を head を確かめずに書くと、 報告を書いた時点の |
| K-0010 | 4 | causal | hypothesis | draft | context | 2026-11-05 | 2026-12-05 | 検査の指摘を直す続きのジョブ（review_fix）が差分のコードを変えた（衝突の印の解消を超える変更をした）のに、PR 本文に自分の要約を新しい節として足すだ |
| K-0011 | 5 | causal | hypothesis | draft | context | 2026-11-05 | 2026-12-05 | 既定ブランチの取り込み（conflict の continue）の検査が衝突の印と fmt・clippy・テストだけだと、取り込んだ変更で共有モジュールの約束  |
| K-0002 | 6 | causal | hypothesis | active | deterministic | 2026-11-05 | 2026-12-05 | 並列で進む PR がすべて CHANGELOG.md の [Unreleased] の同じ場所に 1 行ずつ足す運用だと、コードに関係のない衝突が起きやすく、  |
| K-0004 | 6 | causal | hypothesis | active | deterministic | 2026-11-01 | 2026-12-01 | supervisor が差し戻し（sendback）を「直っていない採用欠陥（defect が null でなく、verdict が adopt）があるか」では |
| K-0006 | 6 | causal | hypothesis | active | context | 2026-11-05 | 2026-12-05 | 差し戻しの直しで PR 本文を末尾への追記だけで直し、テンプレートの順に本文全体を組み直す持ち主がいないと、 見出しの順と階層が崩れて新しい差し戻しを生む。同じ |
| K-0008 | 6 | causal | hypothesis | draft | context | 2026-11-04 | 2026-12-04 | 人の判断待ち（factory:needs-judgment）の停止中に PR が main と衝突（pr.conflicted）しても、衝突を解く contin |
| K-0009 | 6 | causal | hypothesis | draft | context | 2026-11-05 | 2026-12-05 | 既定ブランチとの衝突（conflict）を解く続きのジョブ（continue）に回数や費用の上限もエスカレーションも無いと、 人のマージを待つ間に同じ形の衝突の |
