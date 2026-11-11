# 知見の一覧（自動生成。手で編集しない）

core-gates/knowledge_check.py --index で作り直す。期限は review_by。

| id | テーマ | 種類 | 確かさ | 状態 | 担保 | 確認日 | 期限 | 主張 |
|---|---|---|---|---|---|---|---|---|
| K-0001 | 4 | causal | hypothesis | active | deterministic | 2026-11-08 | 2026-12-08 | 続きのジョブ（continue。既定ブランチとの衝突の解消など）の報告が、既存の PR 本文への追記になることが実装役に伝わらず、 Engine も見出しの階層 |
| K-0003 | 4 | causal | hypothesis | active | context | 2026-11-01 | 2026-12-01 | judge の verdict「adopt」は「仮決めを認める」と「欠陥を直す対象にする」の 2 つの意味で使われ、incident の adopted に区別 |
| K-0005 | 4 | causal | hypothesis | active | deterministic | 2026-11-07 | 2026-12-07 | PR 本文の見出しの定義が、実装役の報告の型・検査役の rubric（Engine の PR-002）・リポジトリの決まり（CLAUDE.md の PR の説明 |
| K-0007 | 4 | causal | hypothesis | draft | context | 2026-11-04 | 2026-12-04 | 続きの job（continue）が PR 本文に書く自分の報告で、既定ブランチの取り込みやコミットの状態を head を確かめずに書くと、 報告を書いた時点の |
| K-0009 | 4 | causal | hypothesis | active | deterministic | 2026-11-05 | 2026-12-05 | 検査の採用済みの指摘（PR-002 など）と、本文を直す役（body_fix）が決めた本文の書き方の決まりが、次に PR 本文を書き足す役 （続きのジョブの s |
| K-0011 | 4 | causal | hypothesis | active | deterministic | 2026-11-07 | 2026-12-07 | 判定役（judge）のジョブの入力に、検査役の指摘が根拠として引く rubric（PR-002・OV-001 など）の本文が入っていないと、 判定役は指摘の根拠 |
| K-0014 | 4 | causal | hypothesis | active | deterministic | 2026-11-08 | 2026-12-08 | 衝突を解く続きのジョブを積むときに、衝突したファイルを記録しない（pr.conflicted・continue.enqueued・handoff に載せない）と |
| K-0016 | 4 | causal | hypothesis | active | deterministic | 2026-11-08 | 2026-12-08 | 実装役の報告（handoff の summary.md）から PR 本文を組み立てる受け渡しで、本文の頭（## 概要 と ## 対応内容）が欠けたり、 ## 概 |
| K-0019 | 4 | causal | hypothesis | active | context | 2026-11-09 | 2026-12-09 | 実装役が契約や API の振る舞いの意味を解釈して仮決めしたときに、backend の実装・テストでその振る舞いを確かめた場所（ファイル名と関数名）を 根拠に添 |
| K-0021 | 4 | causal | hypothesis | active | context | 2026-11-09 | 2026-12-09 | Issue が「いまの絞り込み」「ほかの期間の欄」のように今の画面を前提にした言葉で書かれているのに、指示役が前提にしている既存の欄や 手本の画面をファイル名で |
| K-0025 | 4 | causal | hypothesis | draft | context | 2026-11-11 | 2026-12-11 | 実装役の報告（最後の文）から handoff の summary.md を作り PR 本文に渡す取り出しが、報告の一部しか拾わないと、実装役が ## 概要 から |
| K-0002 | 6 | causal | hypothesis | active | deterministic | 2026-11-08 | 2026-12-08 | 並列で進む PR がすべて CHANGELOG.md の [Unreleased] の同じ場所に 1 行ずつ足す運用だと、コードに関係のない衝突が起きやすく、  |
| K-0004 | 6 | causal | hypothesis | active | deterministic | 2026-11-01 | 2026-12-01 | supervisor が差し戻し（sendback）を「直っていない採用欠陥（defect が null でなく、verdict が adopt）があるか」では |
| K-0006 | 6 | causal | hypothesis | active | deterministic | 2026-11-05 | 2026-12-05 | 差し戻しの直しで PR 本文を末尾への追記だけで直し、テンプレートの順に本文全体を組み直す持ち主がいないと、 見出しの順と階層が崩れて新しい差し戻しを生む。同じ |
| K-0008 | 6 | causal | hypothesis | draft | context | 2026-11-04 | 2026-12-04 | 人の判断待ち（factory:needs-judgment）の停止中に PR が main と衝突（pr.conflicted）しても、衝突を解く contin |
| K-0010 | 6 | causal | hypothesis | active | deterministic | 2026-11-05 | 2026-12-05 | 既定ブランチとの衝突のたびに、上限なしで、セッションを引き継ぐ（agent_resumed）tier-large の続きのジョブを積み直すと、 衝突が機械的（C |
| K-0012 | 6 | causal | hypothesis | active | deterministic | 2026-11-07 | 2026-12-07 | 採用した PR 本文の指摘を直す body_fix（本文だけの直し）が終わる前に supervisor が承認（inspector_approved）を出し、  |
| K-0013 | 6 | causal | hypothesis | active | context | 2026-11-08 | 2026-12-08 | 並行で開いている多くの PR が CHANGELOG.md を変えるとき、pr.conflicted を見るたびに、その PR のマージの順番を待たずに 衝突を |
| K-0015 | 6 | causal | hypothesis | active | deterministic | 2026-11-08 | 2026-12-08 | 衝突の元が PR の中に残っている（PR が CHANGELOG.md を直接書き換えている）とき、エージェントを使わない機械的な衝突の解消が 同じ PR・同じ |
| K-0017 | 6 | causal | hypothesis | active | context | 2026-11-08 | 2026-12-08 | チケットの大きさ（触る領域の数・契約や schema の差分・予想の行数）を投入の前に測り、1 ジョブのターン予算の近くの大きさのものを 領域ごとの子チケットに |
| K-0018 | 6 | causal | hypothesis | active | context | 2026-11-08 | 2026-12-08 | 契約（specs/contracts）の変更と 2 つ以上のアプリ（apps/*）をまたぐ機能のチケットを起こすときに、契約の変更を先行の子チケットにし、 各ア |
| K-0020 | 6 | causal | hypothesis | active | context | 2026-11-09 | 2026-12-09 | 仮決めの escalate で人の判断を待つ PR が衝突の多いファイルに触っているのに、待つ間に同じファイルに触る次の変更の投入を止めないと、 待ちの間に既定 |
| K-0022 | 6 | causal | hypothesis | active | context | 2026-11-11 | 2026-12-11 | どのチケットも同じ場所に追記する共有ファイル（apps/backend/README.md の機能ごとの節、apps/backend/crates/server |
| K-0023 | 6 | causal | hypothesis | active | context | 2026-11-11 | 2026-12-11 | 実装のジョブが PR を開く前に、最新の既定ブランチを同じセッションで取り込んで衝突を確かめないと、実装の間に main が進んだときに PR は開いた時点で衝 |
| K-0024 | 6 | causal | hypothesis | active | context | 2026-11-11 | 2026-12-11 | よく衝突するファイル（apps/backend/README.md・apps/backend/crates/server/src/lib.rs・specs/co |
| K-0026 | 6 | causal | hypothesis | draft | context | 2026-11-11 | 2026-12-11 | supervisor がマージの判断（supervisor.merge_decided）を変更のファイルの分類（表）だけで検査と裁きの前に出し、採用した検査の指 |
