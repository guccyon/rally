# 差し戻しを起こす判定の検査

supervisor が差し戻し（sendback）か続きのジョブを起こす判定と、検査役の承認（`supervisor.inspector_approved`）の basis の書き方を決定論で（LLM を使わずに）見る。知見 `K-0004`（`.factory-config/knowledge/K-0004.yaml`）の担保で、委員会の議決 `D-0004` と Issue #902 で作り、議決 `D-0009` と Issue #934 で PR ごとに走らせる検査（制御プレーンのイベント列から組む入口）にした。

judge の verdict `adopt` は、仮決めの承認（`defect: null`）と欠陥の採用（`defect` が null でない）の両方に使われる。adopt の件数で差し戻しを起こすと、PR #395（事例 `C-0002`）のように直すもののない差し戻しが記録される。この検査は、判定の線を 1 つの条件にして、記録がそれに合っているかを見る。

```sh
node check.mjs --record <PR の記録の JSON> [--json <出力先>]
node check.mjs < record.json     # --record を省くと標準入力から読む
node check.mjs --events <制御プレーンのイベントの JSON か JSONL> [--pr <PR の URL>] [--json <出力先>] [--summary "$GITHUB_STEP_SUMMARY"]
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

## イベント列から組む（`--events`）

PR ごとの検査では、記録を手で組まずに制御プレーンの events をそのまま渡す。JSON の配列・`{ "events": [...] }`・JSONL（1 行 1 件）のどれでもよく、古い順（events の id 順）に並べる。1 件は `{ "name", "payload", "judgment"? }`。

- head の区切り: `supervisor.judge_enqueued`（`head_sha`）。そこから次の `judge_enqueued` までを、その head の記録にする。最初の `judge_enqueued` より前は裁きがないので見ない
- `judgment.judged`: 中身（`subject`・`defect`・`already_applied`）は artifacts の Judgment（stage=judged）にあるので、`payload.artifact_id` で引いて `judgment` に添える。添えていなければ `payload.category` が `inspection` なら欠陥、ほかは仮決めとして数える（shadow のルールの指摘は見分けられない）
- 後続: `sendback.filed`（`origin_pr`）は `sendback`、`continue.enqueued` は `reason`（`review_fix` だけ数える。`conflict`・`ci_failed` は数えない）、`job.enqueued` の `kind: body_fix` は `body_fix`
- `supervisor.inspector_approved`: `basis` を `head_sha` の同じ区切りに付ける
- `supervisor.merged` か `pr.merged`（`pr_url` があるもの）があれば、その PR の記録は `merged: true`
- PR はイベントの `pr_url`（`sendback.filed` は `origin_pr`）で分ける。`--pr` を付けるとその PR だけを見る

制御プレーンから出すときの例（その PR のイベントに Judgment を添える）:

```sql
select coalesce(json_agg(json_build_object('name', e.name, 'payload', e.payload, 'judgment', a.payload) order by e.id), '[]')
from events e
left join artifacts a on e.name = 'judgment.judged' and a.id = (e.payload->>'artifact_id')::bigint
where coalesce(e.payload->>'pr_url', e.payload->>'origin_pr') = :'pr_url'
  and e.name in ('supervisor.judge_enqueued', 'judgment.judged', 'sendback.filed', 'continue.enqueued', 'job.enqueued',
                 'supervisor.inspector_approved', 'supervisor.merged');
```

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
- 標準出力: 外れた rule ごとに「rule: 理由」（`--events` では前に `<PR の URL>@<head>`）と、最後に件数の 1 行。`GITHUB_ACTIONS=true` のときは `::error title=sendback-check <rule>::…` の形で出す
- `--json`: `{ "ok": bool, "errors": [{ "rule", "message" }] }`（`--events` では各件に `pr_url`・`head_sha` も）
- `--summary`: head ごとの通る・外れたと理由を Markdown で足す（`$GITHUB_STEP_SUMMARY` を渡す）

## 呼び出す場所

PR ごと（PR の更新か、検査役の承認の後）と、supervisor が差し戻しか続きのジョブを起こしたとき（`sendback.filed`・`continue.enqueued` の `review_fix`・`body_fix` の `job.enqueued` を記録した後）に、その PR のイベント列で呼び、1 なら落とす。Engine の `supervisor.inspector_approved` の basis は、今は adopt の合計だけ（`採用 N`）を書くので `basis_not_split` で外れる。Engine の判定と basis の文をこの線に合わせること、制御プレーンからイベントを出す経路、呼び出しの配線（`.github/workflows/` か Engine の検査）は、ワーカーの権限の外なので建設者が入れる。

ワークフローの例（イベントを出す手順は建設者が決める。ここでは前の手順で `events.json` に出してある前提）:

```yaml
      - uses: actions/checkout@v4
      - name: 差し戻し・続きのジョブの判定と承認の basis（K-0004）
        run: node tools/sendback-check/check.mjs --events events.json --pr "${{ github.event.pull_request.html_url }}" --summary "$GITHUB_STEP_SUMMARY"
```
