# overlay/

学習（learn）が書く、この工場固有のルール。base より優先する。形と段取りは Engine の ADR 0013 と `definitions/learning.yaml`。

- `rules/OV-NNN.yaml` に 1 ファイル 1 ルール。id は base の rubric（PR-001 など）と混ぜず、廃止しても再利用しない
- 各ルールに `confidentiality: product | client | public` を必ず付ける
- 段階（`knowledge_stage`）は episodic（似た変更のときだけワーカーに渡す）→ rubric（検査官のルーブリックに足す）→ crystallized（差分の正規表現など決定的な規則）
- 状態は candidate → shadow → adopted → retired。shadow は判定して記録するが差し戻さない。廃止したルールもファイルは消さず `state: retired` にする
- eval ケース（当たるべき差分の正例と、当たってはいけない差分の負例を 1 件以上ずつ）を必ず同梱する。差分は `../evals/rules/<id>/` にファイルで置き、`diff_ref: "file:rules/<id>/positive-1.diff"` のように指す
- 追加・昇格・廃止はルール変更 PR（change_type `factory_rule`）として工場自身のパイプラインを通す。CI の rule-eval（Engine の `core-gates/rule_eval.py`）が緑で、LLM が要る段の eval ケースは工場の検査ジョブが判定する
- manifest.yaml の `factory_rule` の autonomy が `pr_create: shadow` の間は、工場はここに書き込まない

## 例

```yaml
id: OV-001
title: "コーチの一覧はアカウント全件から取らない"
state: shadow
knowledge_stage: crystallized
confidentiality: product
severity: must
lessons: ["github:guccyon/rally#117"]
signature: "rubric:ARCH-002"
check:
  kind: diff_regex
  pattern: "all_accounts\\("
  paths: ["apps/backend/**"]
eval_cases:
  - { id: p1, polarity: positive, expect: fire, diff_ref: "file:rules/OV-001/positive-1.diff" }
  - { id: n1, polarity: negative, expect: silent, diff_ref: "file:rules/OV-001/negative-1.diff" }
```
