# overlay/

学習（learn）が書く、この工場固有のルール。base より優先する。

- 各ルールに `confidentiality: product | client | public` を必ず付ける
- ルールの状態は candidate → shadow → adopted → retired。追加・昇格・廃止はルール変更 PR として工場自身のパイプラインを通り、`evals/` の改善と退行なしがマージ条件
- P0 では自己進化を動かさないので、ここには書き込まない（manifest.yaml の change_type `factory_rule` は pr_create も shadow）
