# 差し戻しを起こす判定の検査

supervisor が差し戻し（sendback）か続きのジョブを起こす判定と、検査役の承認（`supervisor.inspector_approved`）の basis の書き方を決定論で（LLM を使わずに）見る。知見 `K-0004`（`.factory-config/knowledge/K-0004.yaml`）の担保で、委員会の議決 `D-0004` と Issue #902 で作った。

judge の verdict `adopt` は、仮決めの承認（`defect: null`）と欠陥の採用（`defect` が null でない）の両方に使われる。adopt の件数で差し戻しを起こすと、PR #395（事例 `C-0002`）のように直すもののない差し戻しが記録される。この検査は、判定の線を 1 つの条件にして、記録がそれに合っているかを見る。

```sh
node check.mjs --record <PR の記録の JSON> [--json <出力先>]
node check.mjs < record.json     # --record を省くと標準入力から読む
npm test
```

依存はない（Node.js だけ）。

## 判定の線

差し戻しか続きのジョブを起こすのは、次のどちらかが 1 件以上あるときだけ。**マージ済みかどうかは見ない**（マージの後でも直っていない採用欠陥があれば起こす）。

- 直っていない採用欠陥: `verdict: adopt` で `defect` が null でなく、`fixed` でない。overlay の shadow のルールの指摘（`defect.shadow`）は差し戻さないので数えない（Engine の ADR 0013）
- まだ入っていない仮決めの差し替え: `verdict: replace` で `already_applied` でも `fixed` でもない

`defect: null` の adopt（仮決めの承認）は、何件あっても差し戻しも続きのジョブも起こさない。

## 記録の形

PR の 1 つの head について、制御プレーンの記録（`judgment.judged` と Judgment、`sendback.filed`、続きのジョブの `job.enqueued`、`supervisor.inspector_approved`）から組む。

```json
{
  "pr_url": "https://github.com/guccyon/rally/pull/395",
  "merged": true,
  "judgments": [
    { "subject": "provisional", "verdict": "adopt", "defect": null },
    { "subject": "defect", "verdict": "reject", "defect": { "rubric_id": "PR-002", "location": "PR 本文" } }
  ],
  "followups": [{ "kind": "sendback" }],
  "basis": "judge が 2 件を裁いた（欠陥の採用 0・仮決めの承認 1。採用した欠陥は続きのジョブか差し戻しで直す）"
}
```

- `judgments[].fixed`: その欠陥（か差し替え）が後の head で直ったら `true`
- `followups[].kind`: `sendback`（`sendback.filed`）・`review_fix`（続きのジョブ）・`body_fix`（本文の直し）。ほかの種類は数えない
- `basis`: `supervisor.inspector_approved` を記録したときだけ。なければ basis は見ない

## 見ること

| rule | 外れる記録 |
|---|---|
| `followup_without_defect` | 直っていない採用欠陥も差し替えもないのに、差し戻しか続きのジョブが起きた |
| `missing_followup` | 直っていない採用欠陥か差し替えがあるのに、差し戻しも続きのジョブも起きていない（マージ済みでも外れる） |
| `basis_not_split` | basis に「欠陥の採用 N」と「仮決めの承認 N」が分けて書かれていない（adopt の合計だけ） |
| `basis_mismatch` | basis の 2 つの数が裁きの記録と合わない |

basis の文の形は `approvalBasis()` が返すもの（`judge が N 件を裁いた（欠陥の採用 a・仮決めの承認 p。採用した欠陥は続きのジョブか差し戻しで直す）`）。

## 結果

- 終了コード: 0 = 通る、1 = 外れた、2 = 記録を読めなかった
- 標準出力: 外れた rule ごとに「rule: 理由」と、最後に件数の 1 行
- `--json`: `{ "ok": bool, "errors": [{ "rule", "message" }] }`

## 呼び出す場所

judge の裁きの後（`act_on_judgment`）と検査役の承認の記録の後に、その head の記録で呼ぶ想定。Engine の `supervisor.inspector_approved` の basis は、今は adopt の合計だけ（`採用 N`）を書くので `basis_not_split` で外れる。Engine の判定と basis の文をこの線に合わせることと、呼び出しの配線（Engine の検査か、制御プレーンの定期の検査）は、ワーカーの権限の外なので建設者が入れる。
