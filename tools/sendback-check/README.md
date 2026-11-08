# 差し戻しを起こす判定の検査

supervisor が差し戻し（sendback）か続きのジョブを起こす判定と、検査役の承認（`supervisor.inspector_approved`）の basis の書き方を決定論で（LLM を使わずに）見る。知見 `K-0004`（`.factory-config/knowledge/K-0004.yaml`）の担保で、委員会の議決 `D-0004` と Issue #902 で作り、議決 `D-0009` と Issue #934 で PR ごとに走らせる検査（制御プレーンのイベント列から組む入口）にし、議決 `D-0011` と Issue #948 で Engine の本物の判定のテストを CI で飛ばさずに走らせる形（`ci/sendback-engine.yml`）にし、議決 `D-0012` と Issue #953 で配線の順序を機械で見張る検査（`order.mjs`）を足し、議決 `D-0014` と Issue #960 で `ENGINE_REF` がコミット SHA であることと写しのずれを見張る手順（`ci.test.mjs`）を足し、議決 `D-0017` と Issue #969 でひな形が必須のチェックにできる形かを見張る手順を `ci.test.mjs` に足し、議決 `D-0016` と Issue #964 で配線の順序の見張りに `ENGINE_REF` の Engine で走らせた `engine.test.mjs` の結果（(3) と (c) が todo になっていないか）を足した、議決 `D-0014` と Issue #961 で Engine の basis が直ったのに todo が残っていれば落とすテストを `engine.test.mjs` に足し、議決 `D-0020` と Issue #977 で `engine.test.mjs` を Engine の既定のブランチの先頭でも定期に走らせて落ちたら知らせる形（`ci/sendback-engine-head.yml`）を足した、議決 `D-0026` と Issue #1018 で写しのジョブが branch protection の必須のチェックに入っているかを照らす検査（`protection.mjs`）を足し、議決 `D-0028` と Issue #1024 で `ENGINE_REF` の行を変える PR で `engine-test` が必ずその版で走るかを見張る手順を `ci.test.mjs` に足し、同じ議決と Issue #1025 で Engine が別の言い回しで adopt の合計だけを書いても (3) が必須で落ちることを Engine の写しで確かめるテストを `engine.test.mjs` に足し、議決 `D-0031` と Issue #1046 で `engine_harness.py` の偽の cursor が知らない問い合わせを `harness_unknown_query` で落とし、`engine-head` がそれを `engine_head_failed`（判定の変化）と分けて知らせる形にし、議決 `D-0031` と Issue #1047 で `engine-head` が通ったときも Issue #948 に記録し、その記録が残っているかを照らす検査（`head-record.mjs`）を足した、議決 `D-0039` と Issue #1087 で `protection.mjs` を branch protection を読めるトークンで毎日走らせ、必須のチェックが後で外されたら Issue を開くか更新する形（`ci/sendback-protection.yml`）を足した。

judge の verdict `adopt` は、仮決めの承認（`defect: null`）と欠陥の採用（`defect` が null でない）の両方に使われる。adopt の件数で差し戻しを起こすと、PR #395（事例 `C-0002`）のように直すもののない差し戻しが記録される。この検査は、判定の線を 1 つの条件にして、記録がそれに合っているかを見る。

```sh
node check.mjs --record <PR の記録の JSON> [--json <出力先>]
node check.mjs < record.json     # --record を省くと標準入力から読む
node check.mjs --events <制御プレーンのイベントの JSON か JSONL> [--pr <PR の URL>] [--json <出力先>] [--summary "$GITHUB_STEP_SUMMARY"]
node order.mjs [--repo <リポジトリの根>] [--engine-report <engine.test.mjs の TAP>] [--json <出力先>] [--summary "$GITHUB_STEP_SUMMARY"]   # 配線の順序の見張り
npm test                         # Engine の本物の判定のテスト（engine.test.mjs）も走る
node --test ci.test.mjs           # ci/sendback-engine.yml の ENGINE_REF の見張り・必須のチェックにできる形か・.github/workflows/ の写しとのずれ、ci/sendback-engine-head.yml の形と記録の手順、ci/sendback-protection.yml の形と Issue の手順
node protection.mjs --protection <gh api の branch protection の JSON> [--protection <rulesets の JSON>] [--repo <リポジトリの根>] [--json <出力先>] [--summary "$GITHUB_STEP_SUMMARY"]   # 必須のチェックの照合
node head-record.mjs --comments <gh api の Issue #948 のコメントの JSON> [--json <出力先>] [--summary "$GITHUB_STEP_SUMMARY"]   # engine-head の記録の照合
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
- (3) は必須のテスト（議決 `D-0011`、Issue #949）。ただし Engine が今の形の basis（`judge が N 件を裁いた（採用 N。採用した指摘は続きのジョブか差し戻しで直す）`。`LEGACY_BASIS`）を書くときだけは `todo` にする（外れても落ちないが結果には出る）。Engine の basis がこの形から変わったら、その版からは `approvalBasis()` と合わないと落ちる（分けて書いても数や文がずれれば落ちる）。建設者が Engine の basis を `approvalBasis()` の形に合わせ、`ENGINE_REF` をその版に上げる PR で、`LEGACY_BASIS` と `todoWhileLegacy` と todo を消して必須のテストにする（議決 `D-0014`、Issue #961）。Engine が今の形の basis を書かなくなったのに `engine.test.mjs` に todo（`order.mjs` の `todoLines` と同じ数え方）が残っていると、「(3) Engine の basis が今の形（adopt の合計だけ）でなくなったら、同じ PR で engine.test.mjs の todo を外す」のテストで落ちる（今の形のうちは何も見ずに通る）。`engine_harness.py` の偽の cursor は `judgment.judged` の問い合わせに payload の項目（`verdict` のほか `category` など）をすべて返すので、Engine が欠陥（`category` が `inspection`）と仮決めを分けて数える版でも同じ形で動く。`todo` のうちに `check.mjs` の ci_check を配線すると、すべての承認が `basis_not_split` で落ちるので、配線は Engine の basis を直して (3) が通った後にする
- (3) の今の形かどうかは `LEGACY_BASIS` の文と合うかで見るので、Engine が別の言い回しで adopt の合計だけを書くように変わると今の形と見なされない（議決 `D-0028`、Issue #1025）。そのとき todo を外すことを求められ、外すと (3) が必須で落ちる流れを、Engine の `orchestrator/dispatcher` を一時ディレクトリに写して `inspector_approved` の basis の文を `LEGACY_BASIS` と違う adopt の合計だけの言い回し（`ADOPT_TOTAL_VARIANTS`）に書き換えた写しで確かめる。写しごとに、basis が `LEGACY_BASIS` に合わず `basis_not_split` で外れること、todo が残っていれば「todo を外す」のテストの中身が落ち、todo の行を除けば通ること、(3) の中身が todo にならずに落ちることを見る。比べのために、今の形の文のままの写しでは (3) が todo になることも見る。Engine が今の形の basis を書かなくなったら写しは作れないので何も見ずに通る（`LEGACY_BASIS`・`todoWhileLegacy` と一緒にこのテストも消す）
- イベントの経路（Issue #945）: `engine_harness.py` は Engine が記録したイベントを、上の「制御プレーンから出すときの例」の SQL と同じ形（同じ名前に絞り、`judgment.judged` に Judgment を添える）でも返す（`events`）。`supervisor.inspector_approved` と `supervisor.merged` は supervisor の周期が記録する形で足す。テストはそれを ci_check と同じ呼び方（`--events --pr --summary`、`GITHUB_ACTIONS=true`）で `check.mjs` に渡し、(a) 上の場合のイベント列が 1 つの head に組めて後続の判定が線に合うこと、(b) PR #395 と同じ入力のイベント列に Engine の形の `sendback.filed` を足した流れが `followup_without_defect` で落ちること（`::error` と要約も）を確かめる
- (c) Engine の正しい判定のイベント列が `check.mjs --events` で通る（終了コード 0）。(3) と同じく、Engine が今の形の basis を書くときだけ `todo` にする（今の Engine では `basis_not_split` で落ちる）。これが通ることが ci_check を配線してよい合図
- 取り出す関数の名前が Engine で変わると `engine_harness.py` は終了コードを 0 以外にして落ちる。偽の cursor が知らない問い合わせが来たときも落ちる（黙って通さない）。そのときは判定の結果を出さず、標準エラーに `harness_unknown_query: <問い合わせ>` を出して終了コード 3 で落ちる（Engine が例外を握りつぶしても、知らない問い合わせを引いたことで落とす）。Engine の判定の変化ではなくハーネスのずれの印で、`engine-head` がこれを `engine_head_failed` と分ける（議決 `D-0031`、Issue #1046）。`judgment.pending_fixes` の問い合わせ（人が直す答えを出した判断のうち、まだ同じ担当に渡していないもの）には空を返す（Issue #1025）

```sh
ENGINE_DIR=/path/to/factory-engine node --test engine.test.mjs
python3 engine_harness.py --check /path/to/factory-engine   # 関数を取り出せるかだけを見る
```

### CI への組み込み

PR ごとに Engine を固定した版（`ENGINE_REF`）でチェックアウトして `ENGINE_DIR` を向け、`engine.test.mjs` を走らせるワークフローを `ci/sendback-engine.yml` に置いた。同じファイルの `wiring-order` のジョブが、下の配線の順序の見張り（`order.mjs`）を PR ごとに走らせる。ワーカーは `.github/workflows/` を変えられないので、建設者が `.github/workflows/sendback-engine.yml` に写す。写すときに変えてよいのは `ENGINE_REF` の行だけ。Engine を上げるときは `ENGINE_REF` も上げ、手元で `ENGINE_DIR` を向けて `engine.test.mjs` が通ることを確かめる。

- `ENGINE_REF` は 40 桁のコミット SHA（小文字の 16 進）に限る。ブランチ名やタグは動くので、同じ PR でも結果が変わる。`engine-test` のジョブの最初の手順（どのチェックアウトよりも前）で確かめ、違えば `::error title=sendback-engine engine_ref_not_sha::…` を出して Engine を取らずに落とす（議決 `D-0014`、Issue #960）。手順の振る舞いは `ci.test.mjs` が、ワークフローから取り出して bash で動かして確かめる
- `wiring-order` のジョブは `ci.test.mjs` も走らせる。`.github/workflows/sendback-engine.yml` の写しが `ENGINE_REF` の行のほかで `ci/sendback-engine.yml` とずれていれば落ちる（写しがなければその場合は飛ばす）。ワークフローを直すときは先に `ci/sendback-engine.yml` を直してから写し直す
- `ci.test.mjs` は、`ci/sendback-engine.yml` が必須のチェックにできる形かも見る（議決 `D-0017`、Issue #969）。`on` が絞り込み（`paths`・`branches`・`types`）のない `pull_request` だけで、`if:` と `continue-on-error` がなく、`wiring-order` のジョブが `order.mjs` と `ci.test.mjs` を、`engine-test` のジョブが `ENGINE_DIR` を向けて `engine.test.mjs` を走らせること。写しとのずれも見るので、見張りを外す・飛ばす変更はひな形と写しを一緒に直しても `wiring-order` で落ちる。GitHub Actions（`GITHUB_ACTIONS=true`）では写しが `.github/workflows/sendback-engine.yml` にないと落ちる（別の名前に写すとずれを見張れないため）
- `ci.test.mjs` は、`ENGINE_REF` の行を変える PR（Engine を上げる PR）で `engine-test` が必ず走り、その版の Engine を確かめる形かも見る（議決 `D-0028`、Issue #1024）。`paths`・`paths-ignore` で絞ると写しを含めても書き方しだいで走らない PR が出るので、絞り込みの中身は問わず上の見張りで外れにする。そのほかに、`ENGINE_REF` がワークフローの `env` に 1 行だけあること（ジョブや手順の `env`・`GITHUB_ENV` で上書きしない。ずれの比べ方は `ENGINE_REF` の行を無視するので、2 行目で上書きされると見逃すため）、`engine-test` に `needs` がないこと（待つジョブが落ちると飛ばされる）、最初の手順が `ENGINE_REF` の見張りで、Engine を取る手順の `ref` が `${{ env.ENGINE_REF }}` であることを、ひな形と写しの両方で見る
- 写したら、branch protection で `engine-test` と `wiring-order` を必須のチェックにする。写しを消す・ジョブの名前を変える PR は、必須のチェックが報告されないのでそこで止まる。写すこと・必須にすること・PR #395 と同じ入力で本物の判定が起こさないことが CI で通った記録を Issue #948 に残すことは建設者が行い、そのあとで `K-0004` の `enforcement.solutions` の what を「効き目を確かめた」に直す。必須にしたことは下の `protection.mjs` で照らす

### 必須のチェックの照合（`protection.mjs`）

写しを消す PR（写しがなければ `wiring-order` も走らない）や、`ci.test.mjs` ごと見張りを書き換える PR は、`engine-test` と `wiring-order` が必須のチェックで報告されないことでしか止まらない。必須にし忘れると見張りを外す PR がそのまま通り、ジョブに `name:` があるのに id で必須にすると、報告されない名前を待ってすべての PR が止まる（議決 `D-0026`、Issue #1018）。`protection.mjs` が、`.github/workflows/sendback-engine.yml` の 2 つのジョブが報告するチェックの名前と、既定のブランチの必須のチェックを照らす。

| rule | 外れる状態 |
|---|---|
| `workflow_not_copied` | `.github/workflows/sendback-engine.yml` がないか、`engine-test`・`wiring-order` のジョブがない |
| `required_check_missing` | そのジョブが報告するチェックの名前（ジョブの `name:` があればそれ、なければ id）が必須のチェックにない |

- branch protection は PR の `GITHUB_TOKEN` では読めないので、PR ごとには走らせない。建設者が写して必須にしたあと、`gh api repos/{owner}/{repo}/branches/main/protection`（rulesets なら `gh api repos/{owner}/{repo}/rules/branches/main`）の出力を `--protection` に渡して走らせる。どちらの形も読む（古い形は `required_status_checks` の `contexts` と `checks[].context`、rulesets は `type: required_status_checks` の規則）。保護がない（404 の `{ "message": … }`）ときは必須のチェックが空として外れる
- 終了コードは 0 = 通る、1 = 外れた、2 = 読めなかった（JSON でない・`--protection` がない）。`GITHUB_ACTIONS=true` では `::error title=sendback-check <rule>::…` の形で出す
- `--protection` は何度でも渡せる。古い形と rulesets の両方を渡すと、どちらかで必須になっているチェックを必須に数える（議決 `D-0039`、Issue #1087）
- 通った出力と、本物の配線の PR で `wiring-order` が落ちる・通る実行の URL を Issue #948 に記録してから、`K-0004` の議決 `D-0026` の解決策（Issue #1018）の `wired` を `true`（`wired_by` に `.github/workflows/sendback-engine.yml` の `wiring-order` と `tools/sendback-check/order.mjs`、`engine-test` と `tools/sendback-check/engine.test.mjs`）にし、`verified` を `true` にする

#### 定期の照合（`ci/sendback-protection.yml`）

建設者が一度 `protection.mjs` を走らせるだけでは、必須にしたあとで `engine-test`・`wiring-order` が必須のチェックから外されても気づけない（議決 `D-0039`、Issue #1087）。`ci/sendback-protection.yml` の `protection` のジョブが、`protection.mjs` を 1 日 1 回（UTC 20:00、JST 5:00）と手動（`workflow_dispatch`）で走らせる。

- branch protection は `GITHUB_TOKEN` では読めないので、読める権限のトークン（GitHub App か `administration:read` の fine-grained PAT）をシークレット `SENDBACK_PROTECTION_TOKEN` に置く（建設者）。トークンを渡すのは branch protection を読む手順だけ
- `main` の古い形（`branches/main/protection`）と rulesets（`rules/branches/main`）の両方を読み、`--protection` に 2 つとも渡す。古い形が保護なし（404 の `Branch not protected`）のときはそのまま渡す（rulesets にもなければ `required_check_missing`）
- トークンがない・読めなかった（権限がないなど）ときは `::error title=sendback-protection protection_unreadable::…` で落ちる。必須のチェックの有無が分からないので Issue は開かない
- `workflow_not_copied` か `required_check_missing` で外れたら、ジョブは落ち、題が `ISSUE_TITLE` の開いている Issue がなければ開き、あればコメントで足す。記録の最初の行は外れ方ごとの印（`<!-- sendback-protection: <外れの行の sha256 の先頭 16 桁> -->`）で、同じ印が Issue の本文かコメントにすでにあれば足さない（毎日同じ記録を積まない）。開いている Issue を探せなかったときは開く側に倒し、Issue を開けない・コメントできないときはその手順も落ちる
- 通ったら、開いている Issue があれば通ったことを書いて閉じる
- PR では走らせない（PR のコードをトークンと `issues: write` で動かさない）。建設者が `.github/workflows/sendback-protection.yml` にそのまま写す。`ci.test.mjs` がこのひな形の形（schedule と手動だけ・トークンは読む手順だけ・2 つの JSON を渡す・Issue の手順は `if: always()`）と、手順を偽の `gh` で動かした振る舞い（そろっていれば通る・rulesets だけでも通る・外されたら Issue を開く・開いていればコメントで足し同じ外れ方は足さない・通ったら閉じる・読めなければ照らさない）、写しがあればひな形と同じかを見る
- 写してシークレットを置き、手動で 1 回走らせて通ったら、`K-0004` の Issue #1087 の解決策の `wired` を `true`（`wired_by` に `.github/workflows/sendback-protection.yml` の `protection` と `tools/sendback-check/protection.mjs`）にする。外れたときに Issue が開くことを確かめてから `verified` を `true` にする

### Engine の先頭での定期の実行（`ci/sendback-engine-head.yml`）

`engine-test` は Engine を `ENGINE_REF` の固定の版で取るので、Engine の判定が変わっても `ENGINE_REF` を上げるまで誰も気づかない（議決 `D-0020`、Issue #977）。`ci/sendback-engine-head.yml` の `engine-head` のジョブが、同じ `engine.test.mjs` を Engine の既定のブランチの先頭（`ref` を付けないチェックアウト）で 1 日 1 回（UTC 19:30、JST 4:30）と手動（`workflow_dispatch`）で走らせる。

- 記録の手順は、テストの手順が通っても落ちても走る（`if: always()`）。テストの手順（`id: head-test`）の結果（`steps.head-test.outcome`）が `success` なら `::error` を出さず、Issue #948 に `engine_head_passed` として先頭の版・その時の `ENGINE_REF`・実行の URL を記録する。`success` でない（落ちた・飛ばされた・取り消された）ときは下の落ちたときの記録にする（議決 `D-0031`、Issue #1047。落ちたときだけ記録していると、先頭で通り続ける限り「先頭の Engine での実行の記録」が残らず、Issue #977 の解決策の `wired`・`verified` を確かめられない）
- 落ちたら `::error title=sendback-engine-head engine_head_failed::…` を出し、Issue #948 に先頭の版・その時の `ENGINE_REF`（`.github/workflows/sendback-engine.yml` の写しがあればその版、なければひな形の版）・落ちたテスト（TAP の `not ok` の行）・実行の URL を記録する。`ENGINE_REF` を上げる前に、判定がどう変わったかをそこで確かめる
- `engine.test.mjs` の出力（`engine-head.txt`。`engine_harness.py` の標準エラーも入る）に `harness_unknown_query` の印があれば、`engine_head_failed` ではなく `::error title=sendback-engine-head harness_unknown_query::…` を出し、Issue #948 にハーネスのずれとして、知らない問い合わせ（5 件まで）と一緒に記録する。Engine の判定の変化ではないので、`engine_harness.py` を先頭に合わせてから判定を確かめる（議決 `D-0031`、Issue #1046）。印があればほかのテストの落ち方に関わらずこちらにする（ハーネスが合わないうちは判定のテストの結果を信じられない）
- 同じ先頭の版の同じ種類の記録（`<!-- sendback-engine-head: <SHA> <engine_head_passed・engine_head_failed・harness_unknown_query のどれか> -->` の印）が #948 にすでにあれば足さない（毎日同じ記録を積まない）。先頭の版が取れなかった（Engine を取れなかった）ときと、これまでの記録が読めなかったときは毎回足す。記録に失敗したらその手順も落ちる
- PR では走らせない。必須のチェックにもしない（先頭の Engine の変化で PR を止めない）。Issue に記録するので `issues: write` が要り、PR のコードをその権限で動かさないため `sendback-engine.yml` とは別のワークフローにした
- 建設者が `.github/workflows/sendback-engine-head.yml` にそのまま写す。`ci.test.mjs` がこのひな形の形（schedule と手動だけ・Engine を `ref` なしで取る・`ENGINE_DIR` を向けて `engine.test.mjs` を走らせる・通ったときも落ちたときも記録の手順を走らせる）と、記録の手順を偽の `gh` で動かした振る舞い（通ったら `engine_head_passed` で記録し、通っていなければそう記録しない）、写しがあればひな形と同じかを見る。偽の Engine（ハーネスの知らない `judgment.decided` の問い合わせを引く・その例外を握りつぶす・判定だけが変わる）で、`engine_harness.py` から `engine.test.mjs`、記録の手順までを通して、`harness_unknown_query` と `engine_head_failed` が分かれて出ることも見る

#### 記録の照合（`head-record.mjs`）

議決 `D-0031` は、建設者が `ci/sendback-engine-head.yml` を `.github/workflows/` に写して schedule で走らせ、先頭の Engine での実行の記録（通った・落ちた）が Issue #948 に 1 件残ってから、Issue #977 の解決策の `wired` と `verified` を `true` にすると決めた。その「1 件残ってから」を `head-record.mjs` が照らす（Issue #1047）。

| rule | 外れる状態 |
|---|---|
| `head_record_missing` | Issue #948 に `engine-head` の記録がない。記録に数えるのは、`github-actions[bot]` が書いたコメントで、最初の行が `<!-- sendback-engine-head: <40 桁の SHA> <engine_head_passed・engine_head_failed・harness_unknown_query> -->` で、本文に実行の URL（`/actions/runs/<番号>`）があるものだけ（人が印を写したコメントと、先頭の版が取れなかった `unknown` の記録は数えない） |

- Issue のコメントは PR ごとには読まないので、建設者が写して schedule か手動で 1 回走らせたあと、`gh api repos/{owner}/{repo}/issues/948/comments --paginate --jq '.[]'` の出力（1 行 1 件。JSON の配列でもよい）を `--comments` に渡して走らせる
- 終了コードは 0 = 通る、1 = 外れた、2 = 読めなかった（JSON でない・`--comments` がない）。`GITHUB_ACTIONS=true` では `::error title=sendback-check <rule>::…` の形で出す。通ると記録ごとに種類・先頭の版・実行の URL を 1 行ずつ出す
- 通ってから、`K-0004` の Issue #977 の解決策の `wired` を `true`（`wired_by` に `.github/workflows/sendback-engine-head.yml` の `engine-head` と `tools/sendback-check/engine.test.mjs`）にし、`verified` を `true` にする。`head-record.mjs` の出力の実行の URL を what に書き足す

## 呼び出す場所

PR ごと（PR の更新か、検査役の承認の後）と、supervisor が差し戻しか続きのジョブを起こしたとき（`sendback.filed`・`continue.enqueued` の `review_fix`・`body_fix` の `job.enqueued` を記録した後）に、その PR のイベント列で呼び、1 なら落とす。Engine の `supervisor.inspector_approved` の basis は、今は adopt の合計だけ（`採用 N`）を書くので `basis_not_split` で外れる。Engine の判定と basis の文をこの線に合わせること、制御プレーンからイベントを出す経路、呼び出しの配線（`.github/workflows/` か Engine の検査）は、ワーカーの権限の外なので建設者が入れる。

配線の順序（議決 `D-0010`）:

1. Engine の `inspector_approved` の basis を `approvalBasis()` の形にし、`ENGINE_REF` をその版に上げて `engine.test.mjs` の (3) と (c) が通ることを確かめる。同じ PR で `LEGACY_BASIS`・`todoWhileLegacy` と todo を消す（消し忘れると `engine.test.mjs` が落ちる。Issue #961）（(3) と (c) は Issue #949 で、今の形の basis のときだけ `todo` にする必須のテストにした）
2. CI で Engine を固定の版（`ENGINE_REF`）で取って `ENGINE_DIR` を向け、`npm test` を走らせる
3. 上の SQL で PR のイベント列を `events.json` に出す手順と、下の `check.mjs --events` の手順を足す（PR ごとと、supervisor が差し戻し・続きのジョブを起こしたとき）
4. 本物の PR #395 のイベント列で `followup_without_defect` で落ちることを確かめ、`K-0004` の `enforcement.solutions` の what を「効き目を確かめた」に直す

### 配線の順序の見張り（`order.mjs`）

上の順序を人の注意ではなく機械で守る（議決 `D-0012`、Issue #953）。`.github/workflows/` に `check.mjs --events` を呼ぶ手順があるときだけ、次のどれかで落とす。呼ぶ手順がなければ（今のリポジトリ）何も見ずに通る。

| rule | 外れる状態 |
|---|---|
| `engine_test_todo` | `engine.test.mjs` に todo（`t.todo(…)`・`test.todo(…)`・`{ todo: … }`）が残っている（basis の場合の `todoWhileLegacy` を含む）。`engine.test.mjs` がないときも |
| `engine_test_not_wired` | `.github/workflows/` に `engine.test.mjs` を走らせる手順がない（`engine.test.mjs` を名指しした `node --test` か、`tools/sendback-check` で `npm test`・ファイルを名指ししない `node --test`）。手順かジョブに `if:`（中身を問わない）か `continue-on-error`（`false` のほか）を付けたものは、飛ばせるか落ちても通るので数えない（議決 `D-0025`、Issue #1017） |
| `engine_test_not_required` | `engine.test.mjs` を走らせるジョブと同じワークフローにある `check.mjs --events` のジョブが、それを `needs` で待たない（間に別のジョブを挟んで待つのはよい。同じジョブなら `engine.test.mjs` の手順より後で呼ぶ）。別のワークフローの間は `needs` で待てないので見ない（`engine-test` を必須のチェックにして止める）。議決 `D-0025`、Issue #1017 |
| `engine_basis_not_passed` | `--engine-report` で渡した `engine.test.mjs` の結果（`ENGINE_REF` の Engine で走らせた TAP）で、(3) か (c) が通っていない（`LEGACY_BASIS` の todo になった・飛ばされた・落ちた・結果にない）。議決 `D-0016`、Issue #964 |

- 呼ぶ手順は、注釈を除いた手順（`- ` で始まる列の要素）の中に `--events` と check.mjs の呼び出しが並ぶもの。check.mjs の呼び出しは `tools/sendback-check/check.mjs` の道か、`sendback-check` が出てくるワークフロー・手順（`working-directory`・`cd`・`npm --prefix`）での `check.mjs`・`npm`（`pnpm`）の `run`・`run-script`。`--events` は手順のどこにあってもよい（引数の順序を変えた形、継ぎ行、配列や関数で渡す形）。取りこぼすと見張れないので、迷う形は呼ぶ側に数える
- 包んだ呼び方も辿る（議決 `D-0017`、Issue #967）。`npm run` は `tools/sendback-check/package.json` の scripts の中身を引き（中に `check.mjs` があれば呼び出し、`--events` があればそれも数える。scripts にない名前は `check` で始まれば呼び出しに数える）、手順の行に出てくるスクリプトのファイル（`.sh`・`.bash`・`.mjs`・`.cjs`・`.js`。テストと `check.mjs`・`order.mjs` は除く）はリポジトリの中にあれば中身を読んで同じ線で見る（3 段まで）。`tools/sendback-check/check.mjs` の道を入れた変数（`env` や shell の代入）を `$NAME`・`${NAME}`・`env.NAME` で使う手順も呼び出しに数える。どの形で落ちるかは `order.test.mjs` のワークフローの写しで確かめている
- 走らせる場所は 2 つ。`ci/sendback-engine.yml` の `wiring-order` のジョブ（`node tools/sendback-check/order.mjs`）と、`check.mjs --events` を GitHub Actions（`GITHUB_ACTIONS=true`）で動かしたとき（`GITHUB_WORKSPACE` のリポジトリを見て、外れたら記録が通っても終了コード 1）。後者があるので、`ci/sendback-engine.yml` を写さずに `check.mjs --events` だけを配線しても落ちる
- `engine_basis_not_passed` は書いたもの（todo の行）ではなく、走らせた結果を見る。`ci/sendback-engine.yml` の `engine-test` のジョブが `engine.test.mjs` の結果を TAP で `engine-test.tap` にも出し、`engine.test.mjs` と同じ手順の中で `node order.mjs --engine-report engine-test.tap` を走らせて、失敗を `rc` に反映する（テストが落ちても結果を見る。`if:` は使わない。ひな形に `if:` があると `ci.test.mjs` の見張り（議決 `D-0017`、Issue #969）で落ちるので、それと両立させるため）。(3) と (c) の名前は `order.mjs` の `BASIS_TESTS` に置き、`engine.test.mjs` もそれを使う。結果のファイルがない（Engine のテストまで進まなかった）ときは、配線していれば「結果にない」で落ちる
- 通すには、Engine の basis を直して `ENGINE_REF` を上げ、`LEGACY_BASIS` と `todoWhileLegacy` を消し、`ci/sendback-engine.yml` を `.github/workflows/` に写してから `check.mjs --events` の手順を足す
- 終了コード: 0 = 通る、1 = 外れた、2 = 読めなかった。`GITHUB_ACTIONS=true` のときは `::error title=sendback-check <rule>::…` の形で出す

ワークフローの例（イベントを出す手順は建設者が決める。ここでは前の手順で `events.json` に出してある前提）:

```yaml
      - uses: actions/checkout@v4
      - name: 差し戻し・続きのジョブの判定と承認の basis（K-0004）
        run: node tools/sendback-check/check.mjs --events events.json --pr "${{ github.event.pull_request.html_url }}" --summary "$GITHUB_STEP_SUMMARY"
```
