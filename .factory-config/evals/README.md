# evals/

この製品の Golden Set とルール検証。汎用部分は Engine の `eval-corpus/` に置き、ここは差分だけを持つ。

- `golden/`: ペア比較形式の採点。1 件ごとに「どちらが望ましいか」と採点理由 1 行を残す。正例・負例を 1:1 に近づけ、負例には意図的なアンチパターンを含める
- 評価仕様は Engine の `definitions/evalspec.yaml`（`l4-pairwise`）
- P0b の目標は 30 件（pr-merge-criteria の判断の記録 3 件を含む）。30 件は初期試行で、自動差し戻しの解放条件ではない
