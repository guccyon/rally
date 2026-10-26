# evals/

この製品の Golden Set とルール検証。汎用部分は Engine の `eval-corpus/` に置き、ここは差分だけを持つ。

- `golden/`: ペア比較形式の採点。1 件ごとに「どちらが望ましいか」と採点理由 1 行を残す。正例・負例を 1:1 に近づけ、負例には意図的なアンチパターンを含める
- 評価仕様は Engine の `definitions/evalspec.yaml`（`l4-pairwise`）
- P0b の目標は 30 件（pr-merge-criteria の判断の記録 3 件を含む）。30 件は初期試行で、自動差し戻しの解放条件ではない

## 記録

| id | Ticket | 対象 | 選好 | 状態 |
|---|---|---|---|---|
| 0001 | #1 glossary の骨格 | PR #2（a）vs PR #4（b） | b | confirmed（2026-10-26） |
| 0002 | #8 change-scenarios | PR #11（a, tier-large）vs PR #14（b, tier-small） | a（low） | builder_candidate |
| 0003 | #9 analytics events | PR #12（a, tier-large）vs PR #15（b, tier-small） | a（medium） | builder_candidate |
| 0004 | #10 contracts の空枝 | PR #13（a, tier-large）vs PR #16（b, tier-small） | a（medium） | builder_candidate |
