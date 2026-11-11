# judge の入力の rubric の本文の検査

判定役（judge）のジョブの入力（エージェントに渡すプロンプト）に、検査役の指摘が根拠として引くすべての rubric_id（`PR-002`・`OV-001` など）について、rubric の本文が入っているかを決定論で（LLM を使わずに）見る。1 つでも欠けたら外れる。知見 `K-0011`（`.factory-config/knowledge/K-0011.yaml`）の担保で、委員会の議決 `D-0033` と Issue #1054 で作った。テストは Issue #1054 の PR #1117 で書かれたもので、議決 `D-0146`（Issue #1320 の起票時の番号は `D-0119`。PR #1117）の ci_check の案と Issue #1320 で CI に組み込むために、今の Engine で読み込めるよう `engine_harness.py` の import の道の直しだけを足して取り込んだ（下の「CI への組み込み（建設者）」）。

事例 `C-0006`（PR #821）では、検査役が `OV-001`（背景・問題の見出しを求める overlay のルール）と `PR-002`（本文の構成）を引いて指摘したが、判定役の解釈には「根拠にした rubric の本文もない」とあり、`OV-001` を決まりの外として reject した。Engine の `core-gates/judge.py` は指摘が引く rubric の本文を Engine の `core-gates/rubric.yaml` からだけ引くので、製品の overlay のルール（`.factory-config/overlay/rules/OV-*.yaml`）の本文は渡らない。

```sh
npm test                         # 線のテスト（line.test.mjs）と、Engine の本物の入力の組み立てのテスト（engine.test.mjs）
ENGINE_DIR=/path/to/factory-engine node --test engine.test.mjs
python3 engine_harness.py --check /path/to/factory-engine   # judge.py を差し替えて読み込めるかだけを見る
```

依存は Node.js と Python 3（`engine.test.mjs` は PyYAML と Engine のチェックアウトも使う）。

## 線（`line.mjs`）

judge の入力の `defects[].rubric_id`（出た順に 1 回ずつ。rubric_id のない指摘は数えない）のそれぞれについて、正本の rubric の本文が入力にあることを求める。入力の中の本文は JSON に埋め込まれるので、空白と JSON の逃がし（`\n`・`\"`・`\\`）を落として比べる。id や題（title）だけがあっても、本文の一部だけでも外れる。

- 正本の本文: Engine の `core-gates/rubric.yaml` の `rules[].check`。なければ製品の `.factory-config/overlay/rules/<id>.yaml` の `check.text`（`check` が文字列ならそれ）
- 指摘がなければ何も求めない

| rule | 外れる入力 |
|---|---|
| `unknown_rubric` | 指摘が引く rubric_id の正本の本文が、core にも overlay にも見つからない（本文を確かめられないので通さない） |
| `missing_rubric_body` | 正本の本文が入力にない |

## Engine の本物の入力の組み立てのテスト（`engine.test.mjs`）

`line.test.mjs` が確かめるのは線。`engine.test.mjs` は Engine の本物の `judge.py` の `main` で入力を組み、線に照らす。

- `engine_harness.py` が Engine のチェックアウトから `core-gates/judge.py` を読み込み、`main` をそのまま動かす。エージェントの呼び出し（`agent_runner`）はプロンプトを覚えて空の裁きを返し、差分（`pr_diff`）は決まった文字列にし、`gh pr view` は入力の `pr` を返し、`git` は何もしない。PR の head のクローン（`/work/product`）は製品のチェックアウト（既定はこのリポジトリ）に読み替える。ほかのコマンドを呼んだら標準エラーに `harness_unknown_call` を出して終了コード 3 で落ちる
- fixtures は `fixtures/c0006.json`。`C-0006` の指摘 7 件（実装役の仮決め 5 件と、rubric を引く `PR-002`・`OV-001` の 2 件）を judge の入力の形にしたもの。仮決めは PR #821 の本文の「## 仮決め」から、指摘の evidence は `C-0006` の「出来事」から組み直した（制御プレーンの原文ではない）
- (0) judge.py を差し替えて読み込める。(1) C-0006 の入力に、`PR-002` と `OV-001` の本文がどちらも入っている。(2) fixtures が core と overlay の rubric を引き、正本の本文を引ける（(1) が黙って通らないため）。(3) Engine の `rubric.yaml` から `PR-002` を消した写しでは todo にならずに外れる
- (1) は必須のテスト。ただし Engine が今の形（core の本文はすべて渡し、overlay の本文を 1 つも渡さない。`isLegacyRubric`）のときだけ `todo` にする（外れても落ちないが結果に出る）。core の本文が欠けたときと、overlay の本文を一部だけ渡すようになったときは落ちる。`tools/sendback-check` の `todoWhileLegacy`（議決 `D-0011`）と同じ扱い
- Engine のチェックアウトは環境変数 `ENGINE_DIR` で渡す（なければリポジトリの隣の `../engine`）。どちらもなければ飛ばす。ただし `ENGINE_DIR` を渡したときと CI（`CI=true`）では飛ばさずに落とす

## Engine に入れること（建設者）

Engine は別のリポジトリなので、次は建設者が入れる。入れて `ENGINE_DIR` の版を上げたら、`engine.test.mjs` の `isLegacyRubric` と `todo` を消してよい。

1. `core-gates/judge.py` が指摘の rubric の本文を組むとき、PR の head のクローンの `.factory-config/overlay/rules/*.yaml` も読み、`id` ごとに `check.text` を `rubric` に入れる（core と同じ id があれば core を優先する）
2. rubric を読めなかったとき（PyYAML がないなど）に黙って空にせず、記録に残す

## CI への組み込み（建設者）

知見 `K-0011` の担保（議決 `D-0146`（Issue #1320 の起票時の番号は `D-0119`。PR #1117）の ci_check の案、Issue #1320）。`ci/judge-input-engine.yml` は、Engine を `.factory-config/engine.lock` の版で取って `ENGINE_DIR` を向け、このディレクトリのテスト（`line.test.mjs`・`engine.test.mjs`）を PR ごとに走らせる CI のひな形。

- Engine の `core-gates/judge.py`・`core-gates/rubric.yaml` の変化は `engine.lock` の版上げの PR で、製品の overlay のルール（`.factory-config/overlay/rules/`）の変化はその PR のチェックアウトで、どちらもこのジョブに入る。必須のチェックにするので `paths`・`branches` で絞らない（絞ると走らない PR で必須のチェックが報告されず止まる）
- Engine の版は `engine.lock` の 1 行から読む（CLAUDE.md の「Engine の版上げはワークフローを触らず `.factory-config/engine.lock` の 1 行で行う」）。40 桁の commit SHA でなければ `engine_ref_not_sha` で落ちる
- `ENGINE_DIR` を渡し `CI=true` なので、Engine のチェックアウトがないときや `engine_harness.py` が `judge.py` を読み込めないときは飛ばさずに落ちる。(1) は今の Engine の形のときだけ `todo`（上の「Engine の本物の入力の組み立てのテスト」）
- `engine_harness.py` は `python3 -I` で走るので、`judge.py` が同じ `core-gates/` から import するモジュール（`knowledge_context` など）のために `core-gates/` を import の道に足す

ワーカーは `.github/workflows/` を変えられないので、建設者が次を行う。

1. `ci/judge-input-engine.yml` を `.github/workflows/judge-input-engine.yml` にそのまま写し、`judge-input-engine-test` が通ることを確かめる（id のままの `engine-test` はほかのワークフローも報告するので、チェックの名前を分けてある。議決 `D-0038` と同じ理由）
2. `judge-input-engine-test` をブランチ保護の必須のチェックにする
3. 同じ変更セットで、知見 `K-0011` の Issue #1320 の解決策を `wired: true` にし、`wired_by` に `{workflow: .github/workflows/judge-input-engine.yml, job: engine-test, runs: …}` を `runs` が `tools/judge-input-check/line.test.mjs` と `tools/judge-input-check/engine.test.mjs` の 2 項目で書く。この解決策の `requires_check` に 2 つを写しのワークフローで書いたので、どちらかが欠けると `tools/knowledge-enforcement-check/check.py` が `requires_check_unwired` で落とす（`line.mjs` と `engine_harness.py` はテストの中で使われるので、呼び出しはテストの道で数える）
