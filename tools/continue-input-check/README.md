# 続きのジョブの入力の検査

continue（`conflict`・`ci_failed`）の implement ジョブがワーカーに渡す入力に、その PR の未解消の採用済みの指摘（`PR-002` など）の id と、本文を直す役（body_fix）が残した本文の決まりが入っているかを決定論で（LLM を使わずに）見る。知見 `K-0009`（`.factory-config/knowledge/K-0009.yaml`）の担保で、委員会の議決 `D-0013` と Issue #955 で作った。

事例 `C-0005`（PR #659）では、judge が採用した `PR-002`（本文の構成）を body_fix が直しても、衝突のたびに積まれた続きのジョブには指摘も決まりも渡らず、その summary が同じ `##` の見出しのまま本文に足されて、同じ指摘が採用され続けた。この検査は、続きのジョブを積んだ時点の記録から入力に求めるものを組み、渡す入力がそれを満たすかを見る。

```sh
npm test                         # 線のテスト（line.test.mjs）と、Engine の本物の積み方のテスト（engine.test.mjs）
ENGINE_DIR=/path/to/factory-engine node --test engine.test.mjs
python3 engine_harness.py --check /path/to/factory-engine   # 関数を取り出せるかだけを見る
```

依存はない（Node.js だけ。`engine.test.mjs` は Python 3 と Engine のチェックアウトも使う）。採用欠陥の数え方と、イベント列を head ごとに組む処理は `tools/sendback-check/check.mjs`（`K-0004` の担保）のものを使う。

## 線（`line.mjs`）

採用済みの指摘か本文の決まりがある PR で continue（`conflict`・`ci_failed`）の implement ジョブを積んだら、ワーカーが標準入力で受け取る JSON に次がなければ外れる。どちらもなければ何も求めない（入力がなくても通る）。`review_fix` は直す指摘そのもの（`defects`）を渡すので見ない。

- `adopted_defects[].rubric_id`: その PR の未解消の採用済みの指摘の id がすべてある
  - 未解消の採用済みの指摘: 最後に裁いた head（`supervisor.judge_enqueued` の区切り。裁きのない head は数えない）の裁きのうち、`verdict: adopt` で `defect` が null でなく、`fixed` でないもの。overlay の shadow のルールの指摘（`defect.shadow`）は除く。前の head の裁きは後の head の裁きで置き換わる
- `body_rules[]`: body_fix が残した本文の決まりがすべてある
  - body_fix が残した本文の決まり: その PR の `pr.body_amended` の `rules`（文字列の配列）。古い順に、同じ文は 1 回だけ

求めるものはイベント列（`tools/sendback-check/README.md` の「制御プレーンから出すときの例」の SQL の形。`judgment.judged` に Judgment を添える）に `pr.body_amended` を足したものから組む（`requiredInput(events, prUrl)`）。

| rule | 外れる入力 |
|---|---|
| `input_missing` | 求めるものがあるのに、入力を渡していない（JSON のオブジェクトでない） |
| `missing_defect_id` | 未解消の採用済みの指摘の id が `adopted_defects` にない |
| `missing_body_rule` | body_fix が残した本文の決まりが `body_rules` にない |

## Engine の本物の積み方のテスト（`engine.test.mjs`）

`line.test.mjs` が確かめるのは線（参照の実装）。`engine.test.mjs` は Engine の本物の関数で続きのジョブを積み、ワーカーに渡す入力を線に照らす。

- `engine_harness.py` が Engine のチェックアウトから `judgment.py` の `record_judged`、`dispatcher.py` の `act_on_judgment`・`enqueue_body_fix`・`is_body_only`・`file_sendback`、`supervisor.py` の `conflict_step`・`ci_fix_step`、`continuation.py`（`enqueue`・`SIGNALED`）をそのまま動かす。DB（events・artifacts・jobs）と GitHub は差し替える。偽の cursor と judge の入出力の組み立ては `tools/sendback-check/engine_harness.py` のものを使う
- 流れ: judge が `PR-002` を採用 → body_fix に回る → body_fix が本文を直して `pr.body_amended` に `rules` を残す → 衝突（か CI 落ち）で supervisor が続きのジョブを積む
- ワーカーに渡す入力は、`dispatcher.run_job` と同じく、reason が `continuation.SIGNALED` にあるときだけ `job.enqueued` の `input`（なければ `{}`）とし、ほかは渡さない（`null`）とする
- (0) 関数をすべて取り出せる。(1) 採用済みの指摘がある PR で continue を積むと、入力に `PR-002` と本文の決まりがある。(2) Engine の記録から求めるもの（`PR-002` と決まり）を組める（(1) が黙って通らないため）。(3) 採用済みの指摘がない PR では何も求めない
- (1) は必須のテスト。ただし Engine の入力が今の形（`conflict` は入力を渡さない、`ci_failed` は CI の合図の `head_sha`・`failed` だけ。`isLegacyInput`）のときだけ `todo` にする（外れても落ちないが結果に出る）。今の形から変わったら、その版からは指摘の id と決まりがなければ落ちる。`tools/sendback-check` の `todoWhileLegacy`（議決 `D-0011`）と同じ扱い
- Engine のチェックアウトは環境変数 `ENGINE_DIR` で渡す（なければリポジトリの隣の `../engine`）。どちらもなければ飛ばす。ただし `ENGINE_DIR` を渡したときと CI（`CI=true`）では飛ばさずに落とす
- 取り出す関数の名前が Engine で変わると `engine_harness.py` は終了コードを 0 以外にして落ちる。偽の cursor が知らない問い合わせが来たときも落ちる（黙って通さない）。Engine が続きのジョブの入力を組むために新しい問い合わせを足したら、偽の cursor も同じ PR で合わせる

## Engine に入れること（建設者）

Engine は別のリポジトリなので、次は建設者が入れる。入れて `ENGINE_DIR` の版を上げたら、`engine.test.mjs` の `isLegacyInput` と `todo` を消してよい。

1. `continuation.enqueue`（か supervisor の `conflict_step`・`ci_fix_step`）で、`conflict`・`ci_failed` の続きのジョブの `input` に `adopted_defects`（未解消の採用済みの指摘。`rubric_id`・`location`・`evidence`・`remediation_hint`）と `body_rules` を入れ、`conflict` も標準入力で渡す（`SIGNALED` に足す）
2. body_fix が本文を直したら、その決まり（「続きのジョブの報告は `##` を足さない」など）を `pr.body_amended` の `rules` に残す
3. ワーカー（`entrypoint.sh`）が入力の `adopted_defects` と `body_rules` を実装役に渡す

CI への組み込み（`tools/sendback-check/ci/sendback-engine.yml` のように Engine を固定の版で取って `ENGINE_DIR` を向ける）は、まだない。
