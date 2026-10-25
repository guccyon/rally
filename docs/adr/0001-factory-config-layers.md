# ADR 0001: .factory-config の層と P0 の手置き

- 状態: 提案（2026-10-25、Tetsuro の了承待ち）
- 関係: factory-engine ADR 0001、プロジェクト憲章 04 節、Blueprint v0.4 の autonomy

## 決定

1. 工場の設定は `.factory-config/` に `base/`・`overlay/`・`evals/`・`manifest.yaml` の 4 つで持つ
2. P0〜P5 は base を生成せず、manifest.yaml の `engine.version` で Engine の版を指す
3. manifest.yaml の autonomy は「環境（操作: pr_create / merge / deploy_test / deploy_prod）× 変更種別」の表で持つ。真偽値 1 つで auto_merge を表さない
4. 変更種別はアカウントスキル pr-merge-criteria の基準から起こす（docs_only / design_decision / large_integration / dependency_update / code_change / factory_rule / constitution）
5. P0 の工場は pr_create まで。人間の手元で自動マージしている種別（docs_only・dependency_update）も、工場の実行としては merge を shadow にして判定だけ記録し、人間の判断との一致を解放の根拠にする

## 選ばなかった案

- **docs_only を P0 から工場でも auto merge**: 現行の運用と同じで手間は減るが、開始定義の「P0 の権限は pr_create まで」と衝突し、解放の根拠になる一致の実績が取れない
- **manifest を Blueprint から自動生成**: meta-controller がない P0 では手書きのほうが速く、レビューもしやすい。Blueprint との一致は人が確認する
