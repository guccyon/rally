# judge の裁きと差し戻しの incident の形の検査

judge の裁き（`judgment.judged`）と、supervisor が差し戻しとして数える incident の形を、スキーマ（`schema/`）で決定論に（LLM を使わずに）見る。知見 `K-0003`（`.factory-config/knowledge/K-0003.yaml`）の担保で、委員会の議決 `D-0004` と Issue #903 で作った。

judge の `verdict: adopt` は、仮決めを認める裁きと、欠陥を直す対象にする裁きの両方に使われる。incident の `adopted` にその 2 つを混ぜた合計を書くと、数から直すべき欠陥の数が読めなくなる（事例 `C-0002`、PR #395）。これを、イベントを書く・数える段で形として止めるためのもの。

```sh
node check.mjs --event judgment.judged --file <JSON Lines か JSON 配列> [--json <出力先>]
node check.mjs --event incident.sendback < incidents.jsonl     # --file を省くと標準入力から読む
npm test
```

依存はない（Node.js だけ）。

## 見ること

| イベント | スキーマ | 外れる形 |
|---|---|---|
| `judgment.judged` | `schema/judgment.judged.schema.json` | `verdict` が `adopt` なのに `subject`（`provisional` = 仮決めの承認、`defect` = 欠陥の採用）がない。Engine の必須の欄（`artifact_id`・`category`・`verdict`・`confidence`・`basis`・`mode`・`pr_url`）がない。`verdict`・`subject` が決まった値のどれでもない |
| `incident.sendback` | `schema/incident.sendback.schema.json` | `adopted_defects`（欠陥の採用の数）と `approved_provisionals`（仮決めの承認の数）のどちらかがない。2 つを混ぜた合計の `adopted` がある |

- `subject` は Engine が artifacts の Judgment にすでに書いている欄（`orchestrator/dispatcher/judgment.py`）と同じ名前・同じ値にした。イベントにもそれを必須で載せる形
- 1 行が events の行の形（`{ "name": …, "payload": { … } }`）なら `payload` を見る
- `check.mjs` はスキーマのキーワードのうち `type`・`enum`・`const`・`required`・`properties`・`minimum`・`maximum`・`minLength`・`if`/`then`/`else`・`not`・`allOf` だけを解釈する。スキーマにほかのキーワードを足すときは `check.mjs` も直す
- 欄の名前と値（`subject` の `provisional`・`defect`、incident の `adopted_defects`・`approved_provisionals`）は案で、まだ確定していない。`approve_provisional`・`adopt_defect` などの言葉の定義（`specs/glossary.yaml`）と合わせて、承認の線（Engine の ADR 0023・0024）を経てから確定する
- 仮決めの採用が Issue や specs/ の補足などの続きの作業を求める場合（議決 `D-0004` の反対の票）が `provisional`・`defect` の 2 つに収まるかは、まだ確かめていない。配線の前に事例 `C-0002` とほかの判定で確かめる

## 結果

- 終了コード: 0 = 通る、1 = 外れた、2 = 読めなかった（知らないイベントの名前、JSON でない行）
- 標準出力: 外れた箇所ごとに「N 行目 <欄の位置>: 理由」（JSON の配列なら N は何件目か）と、最後に件数の 1 行
- `--json`: `{ "ok": bool, "errors": [{ "event", "line", "path", "message" }] }`

## 呼び出す場所

Engine が `judgment.judged` を書く前と、supervisor が差し戻しの incident を数える前（`orchestrator/dispatcher/case.py`）に呼び、1 なら書かずに止める想定。今の Engine は `judgment.judged` に `subject` を載せず、incident には合計の `adopted` を書くので、このスキーマには外れる。Engine の書く側を合わせることと呼び出しの配線は、ワーカーの権限の外なので建設者が入れる。それまでは何も止めていないので、`K-0003` の担保は `context` のままにし、この検査は予定の解決策（`state: planned`）として記録している。
