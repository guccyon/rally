# 差し戻しを起こす判定の検査

supervisor が差し戻し（sendback）か続きのジョブを起こす判定と、検査役の承認（`supervisor.inspector_approved`）の basis の書き方を決定論で（LLM を使わずに）見る。知見 `K-0004`（`.factory-config/knowledge/K-0004.yaml`）の担保で、委員会の議決 `D-0004` と Issue #902 で作り、議決 `D-0009` と Issue #934 で PR ごとに走らせる検査（制御プレーンのイベント列から組む入口）にし、議決 `D-0011` と Issue #948 で Engine の本物の判定のテストを CI で飛ばさずに走らせる形（`ci/sendback-engine.yml`）にし、議決 `D-0012` と Issue #953 で配線の順序を機械で見張る検査（`order.mjs`）を足し、議決 `D-0014` と Issue #960 で `ENGINE_REF` がコミット SHA であることと写しのずれを見張る手順（`ci.test.mjs`）を足し、議決 `D-0016` と Issue #964 で配線の順序の見張りに `ENGINE_REF` の Engine で走らせた `engine.test.mjs` の結果（(3) と (c) が todo になっていないか）を足した。

judge の verdict `adopt` は、仮決めの承認（`defect: null`）と欠陥の採用（`defect` が null でない）の両方に使われる。adopt の件数で差し戻しを起こすと、PR #395（事例 `C-0002`）のように直すもののない差し戻しが記録される。この検査は、判定の線を 1 つの条件にして、記録がそれに合っているかを見る。

```sh
node check.mjs --record <PR の記録の JSON> [--json <出力先>]
node check.mjs < record.json     # --record を省くと標準入力から読む
node check.mjs --events <制御プレーンのイベントの JSON か JSONL> [--pr <PR の URL>] [--json <出力先>] [--summary "$GITHUB_STEP_SUMMARY"]
node order.mjs [--repo <リポジトリの根>] [--engine-report <engine.test.mjs の TAP>] [--json <出力先>] [--summary "$GITHUB_STEP_SUMMARY"]   # 配線の順序の見張り
npm test                         # Engine の本物の判定のテスト（engine.test.mjs）も走る
node --test ci.test.mjs           # ci/sendback-engine.yml の ENGINE_REF の見張りと、.github/workflows/ の写しとのずれ
```

依存はない（Node.js だけ。`engine.test.mjs` は Python 3 と Engine のチェックアウトも使う）。

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

PR ごとの検査では、記録を手で組まずに制御プレーンの events をそのまま渡す。JSON の配列・`{ "events": [...] }`・JSONL（1 行 1 件）のどれでもよく、古い順（events の id 順）に並べる。1 件は `{ "name", "payload", "judgment"? }`。JSON は整形してあってもよい。`events` が配列でないときと、`name` のない件があるときは読めなかったとして終了コード 2 にする（黙って捨てて通さない）。

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

## Engine の本物の判定のテスト（`engine.test.mjs`）

`check.test.mjs` が確かめるのは `check.mjs` の線（参照の実装）。`engine.test.mjs` は同じ 3 つの場合を Engine の本物の判定で確かめる（議決 `D-0009`・`D-0010`、Issue #935・#944・#949）。`D-0010` のとおり、`check.mjs` を PR ごとに走らせる ci_check の配線より先に、このテストで本物の判定を担保する。

- `engine_harness.py` が Engine のチェックアウトから `orchestrator/dispatcher/judgment.py` の `record_judged`、`dispatcher.py` の `act_on_judgment`・`file_sendback`・`enqueue_body_fix`・`is_body_only`、`supervisor.py` の `inspector_approved` を取り出し、DB（events・artifacts・jobs）と GitHub・続きのジョブ・停止を差し替えて動かす。関数の中身には手を入れない。起きた後続と basis を `check.mjs` の `checkRecord` に照らす
- (1) PR #395 と同じ入力では後続を起こさない、(2) 直っていない採用欠陥があればマージの前（`review_fix`）でも後（`sendback`）でも起こす、shadow のルールの指摘は起こさない、(3) basis に欠陥の採用数と仮決めの承認数を分けて書く
- Engine のチェックアウトは環境変数 `ENGINE_DIR` で渡す（なければリポジトリの隣の `../engine`）。どちらもなければ飛ばす。ただし `ENGINE_DIR` を渡したときと CI（`CI=true`）では飛ばさずに落とす（議決 `D-0011`、Issue #948）
- (0) は `engine_harness.py --check` で、`record_judged`・`act_on_judgment`・`file_sendback`・`enqueue_body_fix`・`is_body_only`・`inspector_approved` をすべて取り出せるかを見る。チェックアウトがない・ファイルがない・関数の名前や置き場が変わったときは落ちる
- (3) は必須のテスト（議決 `D-0011`、Issue #949）。ただし Engine が今の形の basis（`judge が N 件を裁いた（採用 N。採用した指摘は続きのジョブか差し戻しで直す）`。`LEGACY_BASIS`）を書くときだけは `todo` にする（外れても落ちないが結果には出る）。Engine の basis がこの形から変わったら、その版からは `approvalBasis()` と合わないと落ちる（分けて書いても数や文がずれれば落ちる）。建設者が Engine の basis を `approvalBasis()` の形に合わせ、`ENGINE_REF` をその版に上げたら、`LEGACY_BASIS` と `todoWhileLegacy` も消してよい。`todo` のうちに `check.mjs` の ci_check を配線すると、すべての承認が `basis_not_split` で落ちるので、配線は Engine の basis を直して (3) が通った後にする
- イベントの経路（Issue #945）: `engine_harness.py` は Engine が記録したイベントを、上の「制御プレーンから出すときの例」の SQL と同じ形（同じ名前に絞り、`judgment.judged` に Judgment を添える）でも返す（`events`）。`supervisor.inspector_approved` と `supervisor.merged` は supervisor の周期が記録する形で足す。テストはそれを ci_check と同じ呼び方（`--events --pr --summary`、`GITHUB_ACTIONS=true`）で `check.mjs` に渡し、(a) 上の場合のイベント列が 1 つの head に組めて後続の判定が線に合うこと、(b) PR #395 と同じ入力のイベント列に Engine の形の `sendback.filed` を足した流れが `followup_without_defect` で落ちること（`::error` と要約も）を確かめる
- (c) Engine の正しい判定のイベント列が `check.mjs --events` で通る（終了コード 0）。(3) と同じく、Engine が今の形の basis を書くときだけ `todo` にする（今の Engine では `basis_not_split` で落ちる）。これが通ることが ci_check を配線してよい合図
- 取り出す関数の名前が Engine で変わると `engine_harness.py` は終了コードを 0 以外にして落ちる。偽の cursor が知らない問い合わせが来たときも落ちる（黙って通さない）

```sh
ENGINE_DIR=/path/to/factory-engine node --test engine.test.mjs
python3 engine_harness.py --check /path/to/factory-engine   # 関数を取り出せるかだけを見る
```

### CI への組み込み

PR ごとに Engine を固定した版（`ENGINE_REF`）でチェックアウトして `ENGINE_DIR` を向け、`engine.test.mjs` を走らせるワークフローを `ci/sendback-engine.yml` に置いた。同じファイルの `wiring-order` のジョブが、下の配線の順序の見張り（`order.mjs`）を PR ごとに走らせる。ワーカーは `.github/workflows/` を変えられないので、建設者が `.github/workflows/sendback-engine.yml` に写す。写すときに変えてよいのは `ENGINE_REF` の行だけ。Engine を上げるときは `ENGINE_REF` も上げ、手元で `ENGINE_DIR` を向けて `engine.test.mjs` が通ることを確かめる。

- `ENGINE_REF` は 40 桁のコミット SHA（小文字の 16 進）に限る。ブランチ名やタグは動くので、同じ PR でも結果が変わる。`engine-test` のジョブの最初の手順（どのチェックアウトよりも前）で確かめ、違えば `::error title=sendback-engine engine_ref_not_sha::…` を出して Engine を取らずに落とす（議決 `D-0014`、Issue #960）。手順の振る舞いは `ci.test.mjs` が、ワークフローから取り出して bash で動かして確かめる
- `wiring-order` のジョブは `ci.test.mjs` も走らせる。`.github/workflows/sendback-engine.yml` の写しが `ENGINE_REF` の行のほかで `ci/sendback-engine.yml` とずれていれば落ちる（写しがなければその場合は飛ばす）。ワークフローを直すときは先に `ci/sendback-engine.yml` を直してから写し直す
- 写したら、branch protection で `engine-test` と `wiring-order` を必須のチェックにする。写すこと・必須にすること・PR #395 と同じ入力で本物の判定が起こさないことが CI で通った記録を Issue #948 に残すことは建設者が行い、そのあとで `K-0004` の `enforcement.solutions` の what を「効き目を確かめた」に直す

## 呼び出す場所

PR ごと（PR の更新か、検査役の承認の後）と、supervisor が差し戻しか続きのジョブを起こしたとき（`sendback.filed`・`continue.enqueued` の `review_fix`・`body_fix` の `job.enqueued` を記録した後）に、その PR のイベント列で呼び、1 なら落とす。Engine の `supervisor.inspector_approved` の basis は、今は adopt の合計だけ（`採用 N`）を書くので `basis_not_split` で外れる。Engine の判定と basis の文をこの線に合わせること、制御プレーンからイベントを出す経路、呼び出しの配線（`.github/workflows/` か Engine の検査）は、ワーカーの権限の外なので建設者が入れる。

配線の順序（議決 `D-0010`）:

1. Engine の `inspector_approved` の basis を `approvalBasis()` の形にし、`ENGINE_REF` をその版に上げて `engine.test.mjs` の (3) と (c) が通ることを確かめる（(3) と (c) は Issue #949 で、今の形の basis のときだけ `todo` にする必須のテストにした）
2. CI で Engine を固定の版（`ENGINE_REF`）で取って `ENGINE_DIR` を向け、`npm test` を走らせる
3. 上の SQL で PR のイベント列を `events.json` に出す手順と、下の `check.mjs --events` の手順を足す（PR ごとと、supervisor が差し戻し・続きのジョブを起こしたとき）
4. 本物の PR #395 のイベント列で `followup_without_defect` で落ちることを確かめ、`K-0004` の `enforcement.solutions` の what を「効き目を確かめた」に直す

### 配線の順序の見張り（`order.mjs`）

上の順序を人の注意ではなく機械で守る（議決 `D-0012`、Issue #953）。`.github/workflows/` に `check.mjs --events` を呼ぶ手順があるときだけ、次のどちらかで落とす。呼ぶ手順がなければ（今のリポジトリ）何も見ずに通る。

| rule | 外れる状態 |
|---|---|
| `engine_test_todo` | `engine.test.mjs` に todo（`t.todo(…)`・`test.todo(…)`・`{ todo: … }`）が残っている（basis の場合の `todoWhileLegacy` を含む）。`engine.test.mjs` がないときも |
| `engine_test_not_wired` | `.github/workflows/` に `engine.test.mjs` を走らせる手順がない（`engine.test.mjs` を名指しした `node --test` か、`tools/sendback-check` で `npm test`・ファイルを名指ししない `node --test`） |
| `engine_basis_not_passed` | `--engine-report` で渡した `engine.test.mjs` の結果（`ENGINE_REF` の Engine で走らせた TAP）で、(3) か (c) が通っていない（`LEGACY_BASIS` の todo になった・飛ばされた・落ちた・結果にない）。議決 `D-0016`、Issue #964 |

- 呼ぶ手順は、注釈を除いた手順（`- ` で始まる列の要素）の中に `--events` と check.mjs の呼び出しが並ぶもの。check.mjs の呼び出しは `tools/sendback-check/check.mjs` の道か、`sendback-check` が出てくるワークフロー・手順（`working-directory`・`cd`・`npm --prefix`）での `check.mjs`・`npm`（`pnpm`）の `run`・`run-script`。`--events` は手順のどこにあってもよい（引数の順序を変えた形、継ぎ行、配列や関数で渡す形）。取りこぼすと見張れないので、迷う形は呼ぶ側に数える
- 包んだ呼び方も辿る（議決 `D-0017`、Issue #967）。`npm run` は `tools/sendback-check/package.json` の scripts の中身を引き（中に `check.mjs` があれば呼び出し、`--events` があればそれも数える。scripts にない名前は `check` で始まれば呼び出しに数える）、手順の行に出てくるスクリプトのファイル（`.sh`・`.bash`・`.mjs`・`.cjs`・`.js`。テストと `check.mjs`・`order.mjs` は除く）はリポジトリの中にあれば中身を読んで同じ線で見る（3 段まで）。`tools/sendback-check/check.mjs` の道を入れた変数（`env` や shell の代入）を `$NAME`・`${NAME}`・`env.NAME` で使う手順も呼び出しに数える。どの形で落ちるかは `order.test.mjs` のワークフローの写しで確かめている
- 走らせる場所は 2 つ。`ci/sendback-engine.yml` の `wiring-order` のジョブ（`node tools/sendback-check/order.mjs`）と、`check.mjs --events` を GitHub Actions（`GITHUB_ACTIONS=true`）で動かしたとき（`GITHUB_WORKSPACE` のリポジトリを見て、外れたら記録が通っても終了コード 1）。後者があるので、`ci/sendback-engine.yml` を写さずに `check.mjs --events` だけを配線しても落ちる
- `engine_basis_not_passed` は書いたもの（todo の行）ではなく、走らせた結果を見る。`ci/sendback-engine.yml` の `engine-test` のジョブが `engine.test.mjs` の結果を TAP で `engine-test.tap` にも出し、次の手順（`if: ${{ !cancelled() }}`。テストが落ちても走る）で `node order.mjs --engine-report engine-test.tap` に渡す。(3) と (c) の名前は `order.mjs` の `BASIS_TESTS` に置き、`engine.test.mjs` もそれを使う。結果のファイルがない（Engine のテストまで進まなかった）ときは、配線していれば「結果にない」で落ちる
- 通すには、Engine の basis を直して `ENGINE_REF` を上げ、`LEGACY_BASIS` と `todoWhileLegacy` を消し、`ci/sendback-engine.yml` を `.github/workflows/` に写してから `check.mjs --events` の手順を足す
- 終了コード: 0 = 通る、1 = 外れた、2 = 読めなかった。`GITHUB_ACTIONS=true` のときは `::error title=sendback-check <rule>::…` の形で出す

ワークフローの例（イベントを出す手順は建設者が決める。ここでは前の手順で `events.json` に出してある前提）:

```yaml
      - uses: actions/checkout@v4
      - name: 差し戻し・続きのジョブの判定と承認の basis（K-0004）
        run: node tools/sendback-check/check.mjs --events events.json --pr "${{ github.event.pull_request.html_url }}" --summary "$GITHUB_STEP_SUMMARY"
```
