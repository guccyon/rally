# 知見の一覧（自動生成。手で編集しない）

core-gates/knowledge_check.py --index で作り直す。期限は review_by。

| id | テーマ | 種類 | 確かさ | 状態 | 担保 | 確認日 | 期限 | 主張 |
|---|---|---|---|---|---|---|---|---|
| K-0001 | 4 | causal | hypothesis | active | deterministic | 2026-11-08 | 2026-12-08 | 続きのジョブ（continue。既定ブランチとの衝突の解消など）の報告が、既存の PR 本文への追記になることが実装役に伝わらず、 Engine も見出しの階層 |
| K-0003 | 4 | causal | hypothesis | active | context | 2026-11-01 | 2026-12-01 | judge の verdict「adopt」は「仮決めを認める」と「欠陥を直す対象にする」の 2 つの意味で使われ、incident の adopted に区別 |
| K-0005 | 4 | causal | hypothesis | active | deterministic | 2026-11-07 | 2026-12-07 | PR 本文の見出しの定義が、実装役の報告の型・検査役の rubric（Engine の PR-002）・リポジトリの決まり（CLAUDE.md の PR の説明 |
| K-0007 | 4 | causal | hypothesis | draft | context | 2026-11-04 | 2026-12-04 | 続きの job（continue）が PR 本文に書く自分の報告で、既定ブランチの取り込みやコミットの状態を head を確かめずに書くと、 報告を書いた時点の |
| K-0009 | 4 | causal | hypothesis | active | deterministic | 2026-11-05 | 2026-12-05 | 検査の採用済みの指摘（PR-002 など）と、本文を直す役（body_fix）が決めた本文の書き方の決まりが、次に PR 本文を書き足す役 （続きのジョブの s |
| K-0011 | 4 | causal | hypothesis | active | context | 2026-11-07 | 2026-12-07 | 判定役（judge）のジョブの入力に、検査役の指摘が根拠として引く rubric（PR-002・OV-001 など）の本文が入っていないと、 判定役は指摘の根拠 |
| K-0014 | 4 | causal | hypothesis | active | deterministic | 2026-11-08 | 2026-12-08 | 衝突を解く続きのジョブを積むときに、衝突したファイルを記録しない（pr.conflicted・continue.enqueued・handoff に載せない）と |
| K-0002 | 6 | causal | hypothesis | active | deterministic | 2026-11-08 | 2026-12-08 | 並列で進む PR がすべて CHANGELOG.md の [Unreleased] の同じ場所に 1 行ずつ足す運用だと、コードに関係のない衝突が起きやすく、  |
| K-0004 | 6 | causal | hypothesis | active | deterministic | 2026-11-01 | 2026-12-01 | supervisor が差し戻し（sendback）を「直っていない採用欠陥（defect が null でなく、verdict が adopt）があるか」では |
| K-0006 | 6 | causal | hypothesis | active | deterministic | 2026-11-05 | 2026-12-05 | 差し戻しの直しで PR 本文を末尾への追記だけで直し、テンプレートの順に本文全体を組み直す持ち主がいないと、 見出しの順と階層が崩れて新しい差し戻しを生む。同じ |
| K-0008 | 6 | causal | hypothesis | draft | context | 2026-11-04 | 2026-12-04 | 人の判断待ち（factory:needs-judgment）の停止中に PR が main と衝突（pr.conflicted）しても、衝突を解く contin |
| K-0010 | 6 | causal | hypothesis | active | deterministic | 2026-11-05 | 2026-12-05 | 既定ブランチとの衝突のたびに、上限なしで、セッションを引き継ぐ（agent_resumed）tier-large の続きのジョブを積み直すと、 衝突が機械的（C |
| K-0012 | 6 | causal | hypothesis | active | deterministic | 2026-11-07 | 2026-12-07 | 採用した PR 本文の指摘を直す body_fix（本文だけの直し）が終わる前に supervisor が承認（inspector_approved）を出し、  |
| K-0013 | 6 | causal | hypothesis | active | context | 2026-11-08 | 2026-12-08 | 並行で開いている多くの PR が CHANGELOG.md を変えるとき、pr.conflicted を見るたびに、その PR のマージの順番を待たずに 衝突を |
