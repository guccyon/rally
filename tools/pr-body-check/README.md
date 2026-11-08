# PR 本文の見出しの検査

PR 本文の見出しの構造を決定論で（LLM を使わずに）見る。知見 `K-0001`（`.factory-config/knowledge/K-0001.yaml`）の担保で、委員会の議決 `D-0001` と Issue #826 で作り、議決 `D-0013` の案と Issue #956 で本文の更新の前の関所（`gate.mjs`）を足し、議決 `D-0019` の案と Issue #975 で関所を Engine に配線する差分（`engine/pr-body-gate.patch`）を足した。議決 `D-0032`（Issue #1037 の起票時の番号は `D-0030`。PR #1036）の案と Issue #1037 で、CI のひな形の見張り（`ci.test.mjs`）と必須のチェックの照合（`protection.mjs`）を足した。議決 `D-0044` の案と Issue #1106 で、`protection.mjs` を既定のブランチへの push と毎日走らせ、必須のチェックが外れたら落ちて Issue を開くか更新するひな形（`ci/pr-body-protection.yml`）を足した。知見 `K-0006` の担保として、議決 `D-0006` と Issue #906 で見出しの定義（`template.json`）との照合を足した。知見 `K-0012` の担保として、議決 `D-0033`（Issue #1053 の起票時の番号は `D-0032`。PR #1051）の test の案と Issue #1053 で、本文の直しのあとの再検査を待たずにマージしていないかをイベント列で見る検査（`recheck.mjs`）を足した。

続きのジョブ（衝突の解消・CI 落ちの直しなど）の報告が、新しい PR と同じ見出しの組（`## 概要`・`## 対応内容` など）のまま本文の末尾に足されると、同じ見出しが 2 回出る。これを検査役（LLM）と判定役まで流さず、本文を更新した直後に止めるためのもの。差し戻しの直しで本文の末尾に追記するだけだと、見出しの順が崩れたり必須の見出しが欠けたりする（K-0006）ので、それも見る。

```sh
node check.mjs --body <本文のファイル> [--json <出力先>]
gh pr view <番号> --json body --jq .body | node check.mjs     # --body を省くと標準入力から読む
npm test
```

依存はない（Node.js だけ。`engine.test.mjs` は bash・jq・git・Python 3 と Engine のチェックアウトも使う。`protection.mjs` は branch protection の JSON の読み方を `tools/sendback-check/protection.mjs` から使う）。

## 見ること

| rule | 外れる本文 |
|---|---|
| `duplicate_h2` | 同じ `## ` 見出しが 2 回以上ある |
| `outside_factory_record` | `## 工場の記録` より後ろに `## ` か `# ` の見出しがある（そこから下は工場の記録の中なので `### ` 以下にする） |
| `continuation_outside_record` | 「続きのジョブ」を含む見出しが、`## 工場の記録` の下の `### ` 以下にない |
| `missing_required` | `template.json` で必須（`required: true`）の `## ` 見出しがない |
| `template_order` | `template.json` の `## ` 見出しが定義の順に並んでいない（前の見出しより定義で先のものが後ろに来たら、その見出しを 1 件） |
| `outside_template` | `template.json` にない `## ` か `# ` の見出し（`## 工場の記録`・続きのジョブの報告など）が、定義の見出しの最後のものより前にある |

コードブロック（```` ``` ```` と `~~~`）の中の行は見出しとして数えない。`### ` 以下の見出しは重なってもよく、定義の外でもよい。同じ `## ` 見出しの 2 回目は `duplicate_h2` だけで数え、順の違いや定義の見出しの終わりには数えない。

## 見出しの定義（`template.json`）

PR 本文の `## ` 見出しの順と、必須かどうかの 1 つの定義。順は `概要 / 背景 / 問題 / 対応内容 / 影響範囲・互換性 / 動作確認 / 補足 / 仮決め / 知見`。該当しない見出しは省くので、必須は `概要` と `対応内容` だけ。

見出しの正はこのファイルに置く（知見 `K-0005` の線でオーナーが決めた。Issue #936）。CLAUDE.md の「書き方」の「PR の説明は … の見出しで」は同じ順にそろえてあり、`check.test.mjs` が両者の一致を確かめる。Engine の rubric の `PR-002`（`core-gates/rubric.yaml`）の見出しの列（概要・背景・問題・対応内容・影響範囲・動作確認・補足）は同じ順だが、`影響範囲` の名前と `仮決め`・`知見` がない点がまだ食い違う。Engine は別のリポジトリなので、建設者がこの定義に合わせる。

`check.mjs` は初めて照らすときに定義を読んで検証する（`h2` が空でない配列で、各要素は前後に空白のない 1 行の `text` と真偽値の `required` だけを持ち、`text` が重ならない）。読めない・JSON でない・外れたら専用の `TemplateError` を投げ、本文を照らさない。モジュールの読み込み時には投げないので、表示と終了コード（3）は境界の `main` が組む。検証した定義は配列も要素も凍結して `loadTemplate()` が返すので、後から書き換えられない。

## 結果

- 終了コード: 0 = 通る、1 = 外れた、2 = 本文を読めなかった、3 = 見出しの定義（`template.json`）が不正（本文は照らしていない。本文を直しても通らないので、定義を直す）
- 3 のときは標準エラーに「pr-body-check: 見出しの定義が不正: 理由」を出し、標準出力と `--json` には何も書かない
- 標準出力: 外れた箇所ごとに「N 行目: 理由」（本文全体の外れ、つまり `missing_required` は「本文: 理由」）と、最後に件数の 1 行
- `--json`: `{ "ok": bool, "errors": [{ "rule", "line", "heading", "message" }] }`。`missing_required` の `line` は `null`

## 本文の更新の前の関所（`gate.mjs`）

知見 `K-0001` を決定論で担保する配線の入口（議決 `D-0013` の案、Issue #956）。続きのジョブの summary を本文に組み込んだあと、`gh pr edit` で書く前の本文を `checkBody` に照らし、外れたら本文を書かずに本文の直し（body_fix）に回す。

```sh
printf '%s' "${NEW_BODY}" | node tools/pr-body-check/gate.mjs --pr-url "${PR_URL}"   # リポジトリの根から
```

- 終了コード: 0 = 本文を書いてよい、1 = 本文を書かずに body_fix に回す、2 = 読めなかった（本文は書かない）、3 = 見出しの定義が不正（本文は書かない。body_fix には回さず、定義を直す）
- 1 のときの標準出力: body_fix の入力の JSON 1 行。Engine の `dispatcher.enqueue_body_fix` が `job.enqueued` の `input` に入れる形（`{ pr_url, defects: [{ rubric_id, severity, location, evidence, remediation_hint }] }`）
  - `rubric_id` は本文の構成の指摘 `PR-002`（Engine の `core-gates/rubric.yaml`）、`severity` は `should`
  - `location` は「PR 本文 N 行目」（Engine の `is_body_only` が本文だけの指摘と読む形）。`evidence` は「pr-body-check <rule>: 理由」
- 外れた箇所は標準エラーに「N 行目: 理由」で出す。`--json <出力先>` で `{ ok, errors, body_fix }` を書く

## 本文の直しのあとの再検査（`recheck.mjs`）

知見 `K-0012` の担保（議決 `D-0033` の test の案、Issue #1053）。本文だけの直し（body_fix）が終わる前か、直した本文を検査する前に supervisor がマージしていないかを、制御プレーンのイベント列で見る。

```sh
node recheck.mjs --events <制御プレーンのイベントの JSON か JSONL> [--pr <PR の URL>] [--json <出力先>]
```

- イベント列は古い順に並べて渡す。形と読み方は `tools/sendback-check/check.mjs --events` と同じ（`parseEvents` を使う）。本文の直しの積みと終わりは `job_id`（events の列）で結ぶ
- 同じ PR の `supervisor.merged` ごとに、それより前を見て外す:

| rule | 外れる列 |
|---|---|
| `merged_without_body_recheck` | `pr.body_amended` のあと、本文の検査（`gate.evaluated`）がないまま `supervisor.merged` が来た（事例 `C-0006`、PR #821: 直し 14:25:04、マージ 14:25:06） |
| `merged_before_body_fix_finished` | `job.enqueued`（`kind: body_fix`）のあと、その終わり（`pr.body_amended` か `pr.body_amend_failed`）より前に `supervisor.merged` が来た（事例 `C-0005`、PR #659: マージ 20:08:43、直し 20:08:50） |
| `merged_after_body_fix_failed` | 本文の直しが失敗で終わり（`pr.body_amend_failed`）、そのあと直しが成功（`pr.body_amended`）しないまま `supervisor.merged` が来た |

- 本文の検査は `gate.evaluated` で数え、`gate` は問わない（検査役の `rubric` も、`check.mjs` の結果を `gate: "pr-body-check"` で記録したものも数える）。直す前の検査とほかの PR の検査は数えない
- 終了コード: 0 = 通る、1 = 外れた、2 = イベント列を読めなかった。標準出力に外れごとに「PR の URL rule: 理由」と、最後に件数の 1 行。`--json` は `{ ok, errors: [{ rule, pr_url, message }] }`
- まだどこからも呼んでいない（CI にも Engine にも配線していない）。テストは `npm test` で走る

## テスト

`npm test` で 6 つを走らせる。

- `check.test.mjs`: 見出しの検査（`check.mjs`）
- `gate.test.mjs`: 関所。事例 `C-0005`（PR #659）の 21 回の追記（conflict が 20 回、ci_failed が 1 回。どれも新しい PR と同じ `## 概要`・`## 対応内容`・`## 動作確認` を持つ summary）と同じ入力で落ち、1 回目の追記でもう落ちることを確かめる。入力は `c0005.mjs` が組む（本文そのものは記録にないので、事例に書かれた形を組み直したもの）。通る側の写しとして、同じ 21 回の報告を「## 工場の記録」の下の `### 続きのジョブ（job N）の記録` に、見出しを `###`・`####` に下げて、または `## ` 見出しのない形で足した本文では、毎回 `gate` が書いてよいと返し、CLI が 0 で何も出さないことも確かめる（誤検知がないこと。議決 `D-0032` の test の案、Issue #1038）
- `engine.test.mjs`: Engine の本物で確かめる。Engine のチェックアウトは環境変数 `ENGINE_DIR` で渡す（なければリポジトリの隣の `../engine`）。どちらもなければ飛ばす。ただし `ENGINE_DIR` を渡したときと CI（`CI=true`）では飛ばさずに落とす（`tools/sendback-check` と同じ。議決 `D-0011`）
  - (0) Engine の `orchestrator/worker/entrypoint.sh` から、新しい PR の本文（`PR_BODY`）・続きのジョブの組み込み（`SECTION_TITLE` から `NEW_BODY` まで）・本文の更新（`gh pr view` から `continued` のログまで）の節を取り出せ、`engine_harness.py` が `dispatcher.py` の `is_body_only`・`enqueue_body_fix` を取り出せる
  - (1) Engine の組み込みで C-0005 の 21 回の追記をした本文が関所で落ちる（Engine が組み込み方を直して重ならなくなったら、それでよい）
  - (2) 関所の body_fix の入力が Engine の `is_body_only` で本文だけの指摘と読まれ、`enqueue_body_fix` で body_fix のジョブが積まれる
  - (3) Engine の本文の更新の節を、偽の `gh`（`pr view` と `pr edit --body` だけを真似る）で C-0005 の 21 回分動かし、関所に外れる本文を一度も書かない。必須のテストで、`todo` はない（議決 `D-0019`、Issue #975 で外した）。Engine の `entrypoint.sh` が関所（`tools/pr-body-check/gate.mjs`）を呼ばない形なら落ちるので、`engine/pr-body-gate.patch` を入れていない Engine では落ちる
  - (4) `ENGINE_DIR` の Engine の写しに `engine/pr-body-gate.patch` を当て（もう関所を呼んでいれば当てない）、(3) と同じ 21 回で外れる本文を一度も書かず、外れた回ごとに引き継ぎ（`HANDOFF_DIR/jobs/<job>/body_gate.json`）に関所の結果（`rc`・`body_fix`・書かなかった報告の全文 `pending_entry`・そのファイルの場所 `handoff`）を残す。差分が当たらなければ落ちる
  - (5) 同じ写しで、`## ` 見出しを持たない続きのジョブの報告は止めずに「## 工場の記録」の下の `### 続きのジョブの記録` に足して書く（誤検知しない）
  - (6) 同じ写しの dispatcher の `act_on_body_gate` が、`rc` 1 の結果で body_fix を 1 件積み（関所の指摘に、書かなかった報告の全文と引き継ぎの場所を「PR 本文 末尾」の指摘として足す）、`rc` 2・3 では積まずに人に回す（停止を開く）
  - (7) 4000 字を超える報告も切らずに、引き継ぎの `body_gate.json` と body_fix の指摘に全文が渡る
  - (8) 未完了の続きが仕上がった回（`CONTINUE_REASON=incomplete`・`INCOMPLETE` が空）に関所で外れても、報告を足さない本文で仕上げ（未完了の印を消す・`Refs` を `Closes` に直す・題を戻す・ドラフトを外す）だけは済ませ、`finished.applied` を true で残す。報告は body_fix に回し、人には回さない
  - (9) 同じ回で報告を足さない本文も関所で外れたら、仕上げも書かず `finished.applied` を false で残し、dispatcher が停止（`body_gate_finish:<PR の URL>`、human）を開いて人に回す
  - (10) `ENGINE_DIR`（CI では `ENGINE_REF` の版）の Engine の `entrypoint.sh` そのものの本文の更新の節で（差分を当てた写しではなく）、C-0005 の 21 回の続きのジョブの報告の見出しを `###`・`####` に下げたか `## ` 見出しを持たない形なら、毎回関所を 1 回呼んで 0 が返り（`node` を包んで終了コードを残す）、`body_gate.json` を残さずに本文を書き、書いた本文が関所に通る（正しい更新まで止める誤検知がないこと。議決 `D-0032` の test の案、Issue #1038。議決 `D-0045` の test の案、Issue #1110 で写しから本物に替えた）。必須のテストで、関所を呼ばない Engine では (3) と同じく落ちる。議決 `D-0044` の test の案（Issue #1107）で、この通る側を必須のテストとして担保に記録した
  - (11) Engine がまだ関所を呼ばない間だけ、(10) と同じことを `engine/pr-body-gate.patch` を当てた写しで確かめる（差分そのものの誤検知がないこと）。Engine が関所を呼ぶようになったら (10) が本物で確かめるので飛ばす。Engine の行が変わって `engine/pr-body-gate.patch` が当たらなくなると (4)〜(9) と (11) がそろって落ち、差分を確かめられなくなるので、そのときは同じ PR で差分を今の Engine に合わせて作り直す
- `ci.test.mjs`: CI のひな形 `ci/pr-body-engine.yml` の形と、`ci/pr-body-protection.yml` の形と手順の振る舞い（下の「配線の手順（建設者）」と「定期の照合」）
- `protection.test.mjs`: 必須のチェックの照合（`protection.mjs`）
- `recheck.test.mjs`: 本文の直しのあとの再検査（`recheck.mjs`）。事例 `C-0005`（PR #659）と `C-0006`（PR #821）の記録に書かれた時刻・job・順をイベントの形に組み直した列が、それぞれ `merged_before_body_fix_finished` と `merged_without_body_recheck` で落ち、直しの終わりと再検査を待ってからマージした列は通ることを確かめる
- 節の取り出し方（`snippets`）は `entrypoint.sh` の今の行の形に合わせている。Engine で形が変わって取り出せなくなったら (0) が落ちるので、同じ PR で合わせる

```sh
npm test
ENGINE_DIR=/path/to/factory-engine node --test engine.test.mjs
python3 engine_harness.py --check /path/to/factory-engine   # 関数を取り出せるかだけを見る
python3 engine_harness.py --body-gate /path/to/factory-engine < gate.json   # act_on_body_gate を動かす（差分を当てた Engine）
```

## マージ前の必須チェック（建設者）

head_sha の変わらない本文だけの直しも通すため、CI では `pull_request` の `opened`・`synchronize`・`reopened` に加えて `edited` でも起動させ、マージ前の必須チェックにする（議決 D-0006）。本文は `github.event.pull_request.body` を環境変数で受けて標準入力に渡す（`${{ }}` で run に直接埋めない）。

```yaml
on:
  pull_request:
    types: [opened, synchronize, reopened, edited]
jobs:
  pr-body-check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - env:
          BODY: ${{ github.event.pull_request.body }}
        run: printf '%s' "$BODY" | node tools/pr-body-check/check.mjs
```

なお、Engine の `orchestrator/worker/entrypoint.sh` は今、続きのジョブの報告を `## 工場の記録` の後ろに `## 続きのジョブの記録` として足すので、そのままでは `outside_factory_record`・`continuation_outside_record` で外れる。必須チェックにする前に、Engine 側でその見出しを `### ` に下げる（K-0001 の線。Engine への作用なので人の承認が要る）。下げる直しは `engine/pr-body-gate.patch` に入れてある。

このワークフローとブランチ保護の必須チェックは、ワーカーの権限の外なので建設者が入れる。

## Engine に入れること（建設者）

Engine は別のリポジトリなので、次は建設者が入れる。そのための差分を `engine/pr-body-gate.patch` に置いた（Engine の `orchestrator/worker/entrypoint.sh`・`orchestrator/dispatcher/dispatcher.py`・`definitions/events.yaml`。Engine のチェックアウトの根で `git apply` する）。入れて `ENGINE_REF` を上げるまで、`engine.test.mjs` の (3) は落ちる。

1. `worker/entrypoint.sh` の続きのジョブの節で、`NEW_BODY` を組んだあと `gh pr edit` の前に、リポジトリに `tools/pr-body-check/gate.mjs` があれば `NEW_BODY` を照らす。1（か 2）なら `gh pr edit` で本文を書かず、標準出力の body_fix の入力と組んだ本文（続きのジョブの報告）を引き継ぎ（`HANDOFF_DIR`）と `FACTORY_RESULT` に残す。3（見出しの定義が不正）なら本文を書かず、body_fix には回さずに人に回す（本文を直しても通らない）。push した head（`pr.head_updated`）と引き継ぎ（`handoff.saved`）は今のまま
2. dispatcher が、その結果を見て `enqueue_body_fix` で body_fix を積む（判定役を待たない。検査役にも回さない）。body_fix には、書かなかった続きのジョブの報告を「## 工場の記録」の下の `###` に入れて本文を組み直すよう渡す
3. body_fix が直した本文も関所に照らし、まだ外れたら人に回す（同じ直しを繰り返さない）

差分が入れるのは 1 と 2（3 は入れていない）:

- `entrypoint.sh`: 続きのジョブの記録の見出しを `### 続きのジョブの記録`（「## 工場の記録」の下）にする（前の `## 続きのジョブの記録` がある本文にはその下に足す）。未完了の印を外す直しのあとの `NEW_BODY` を、`gh pr edit` の前に関所に照らし、0 以外なら `gh pr edit`・`gh pr ready` を呼ばず、`{ rc, body_fix, pending_entry, finished, handoff }`（`pending_entry` は書かなかった報告の全文で切らない。`finished` は未完了の続きが仕上がった回だけで、仕上げを済ませたか（`applied`）と題・Issue の番号。`handoff` はそれを残した引き継ぎのファイル）を `FACTORY_RESULT` の `body_gate` と引き継ぎの `jobs/<job>/body_gate.json` に残す。仕上がった回は、報告を足さない本文が関所に通れば、それで仕上げ（未完了の印・`Closes`・題・`gh pr ready`）だけを先に書く。関所のないリポジトリでは今までどおり書く
- `dispatcher.py`: 続きのジョブの結果に `body_gate` があれば `act_on_body_gate` を呼ぶ。`rc` 1 は判定役を待たずに `enqueue_body_fix` で body_fix を積み、書かなかった報告を切らずに全文で、引き継ぎの場所を添えて「PR 本文 末尾」の指摘として渡す。`rc` 2・3 は停止（`body_gate:<PR の URL>`、human）を開く。仕上げを書けなかった（`finished.applied` が false）ときは、body_fix では行を消したりドラフトを外したりできないので、停止（`body_gate_finish:<PR の URL>`、human）を開いてやることを待ち事項に書く。どちらも `pr.body_gate_blocked` を残す
- `definitions/events.yaml`: `pr.body_gate_blocked` を足す

`ci/pr-body-engine.yml` は、Engine を固定の版で取って `ENGINE_DIR` を向け、このディレクトリのテストを PR ごとに走らせる CI。ワーカーは `.github/workflows/` を変えられないので、建設者が `.github/workflows/pr-body-engine.yml` に写し、`engine-test`（チェックの名前は `pr-body-engine-test`）をブランチ保護の必須チェックにする（議決 `D-0019`。手順と見張りは下の「配線の手順（建設者）」）。(3) に `todo` がないので、関所を呼ばない Engine の版（今の `ENGINE_REF`）のままだと落ちる。差分を Engine に入れて `ENGINE_REF` を上げるのと同じ PR で写す。この CI では PR 本文そのものは見ない（落ちると `ci_failed` の続きのジョブが積まれ、その summary がまた本文に足されて、この知見の形を増やすため）。本文は上の「マージ前の必須チェック」で見るが、同じ理由で、そのチェックが落ちたときは `ci_failed` の続きのジョブを積まず、関所と同じく body_fix に回す（建設者が Engine の dispatcher で分ける）。

## 配線の手順（建設者）

議決 `D-0032`（Issue #1037）の案。Engine に差分を入れて `ENGINE_REF` を上げ、同じ PR で `ci/pr-body-engine.yml` を `.github/workflows/pr-body-engine.yml` に写して、`engine-test` のジョブを必須のチェックにする。ワーカーの権限で入れたのは、その配線が効く形かの見張り（`ci.test.mjs`）と、必須になったかの照合（`protection.mjs`）まで。

`ci.test.mjs`（`engine-test` の `node --test` の中で走る）が、ひな形と写しで次を見て、外れたら落とす。

- `on` が絞り込みのない `pull_request` だけ（`paths`・`paths-ignore`・`branches`・`types` は中身を問わず外れ）。`if:`・`continue-on-error`・`engine-test` の `needs` がない
- `ENGINE_REF` がワークフローの `env` に 40 桁のコミット SHA で 1 行だけあり（ジョブや手順の `env`・`GITHUB_ENV` で上書きしない）、Engine を取る手順の `ref` が `${{ env.ENGINE_REF }}`
- テストの手順が `tools/pr-body-check` で `ENGINE_DIR` を渡し、道を渡さずに `node --test` で全部のテストを走らせ、`exit $rc` で落ちる
- `engine-test` の `name:` が `pr-body-engine-test`。`tools/knowledge-enforcement-check/ci/knowledge-enforcement.yml` にも id のままの `engine-test` のジョブがあるので（`tools/sendback-check/ci/sendback-engine.yml` の `engine-test` は `sendback-engine-test` を報告する。議決 `D-0038`）、同じ名前だと必須のチェックの「engine-test」がどちらの報告でも満たされ、この写しを消しても止まらない
- ひな形の `ENGINE_REF` が `tools/sendback-check/ci/sendback-engine.yml` と同じ版。写しがあれば、`ENGINE_REF` の行のほかがひな形と同じで、`.github/workflows/sendback-engine.yml` の写しとも同じ版。GitHub Actions で写しがなければ落ちる

`protection.mjs` は、`gh api` で取り出した branch protection（古い形か rulesets）の JSON を読み、写しの `engine-test` が報告するチェックの名前（`pr-body-engine-test`）が必須のチェックにあるかを照らす。branch protection は PR の `GITHUB_TOKEN` では読めないので、PR ごとには走らせず建設者が走らせる。

```sh
gh api repos/{owner}/{repo}/branches/main/protection > protection.json   # 古い形
gh api repos/{owner}/{repo}/rules/branches/main > rules.json             # rulesets
node tools/pr-body-check/protection.mjs --protection protection.json --protection rules.json   # リポジトリの根から
```

`--protection` は何度でも渡せる。古い形と rulesets の両方を渡すと、どちらかで必須になっているチェックを必須に数える（議決 `D-0044`、Issue #1106）。

| rule | 外れる形 |
|---|---|
| `workflow_not_copied` | `.github/workflows/pr-body-engine.yml` がないか、`engine-test` のジョブがない |
| `check_name_changed` | `engine-test` のチェックの名前が `pr-body-engine-test` でない |
| `required_check_missing` | そのチェックの名前が必須のチェックにない（`engine-test` の id だけを必須にしても外れる） |
| `check_name_shared` | `.github/workflows/` のほかのジョブも同じチェックの名前を報告する |

終了コードは 0 = 通る、1 = 外れた、2 = 読めなかった。今のリポジトリには写しがないので `workflow_not_copied` で落ちる。

順序:

1. 写す前に、今の `ENGINE_REF`（関所を呼ばない Engine）を `ENGINE_DIR` にして `engine.test.mjs` の (3) が落ちることを記録する（`CI=true ENGINE_DIR=<Engine> node --test engine.test.mjs`）
2. `engine/pr-body-gate.patch` を Engine に入れ、`ENGINE_REF` をその版に上げる（`ci/pr-body-engine.yml` と `tools/sendback-check/ci/sendback-engine.yml` の両方。どちらのテストも通る版にする）。同じ PR で `ci/pr-body-engine.yml` を写し、`pr-body-engine-test` が通ることを確かめる
3. `pr-body-engine-test` を必須のチェックにし、`protection.mjs` が 0 で終わることを記録する。`ci/pr-body-protection.yml` を `.github/workflows/pr-body-protection.yml` に写してシークレット `PR_BODY_PROTECTION_TOKEN` を置き、手動で走らせて通ることを確かめる（下の「定期の照合」。議決 `D-0044`、Issue #1106）
4. 本物の続きのジョブで、関所に外れた本文が書かれず（`pr.body_gate_blocked`）、body_fix が積まれた（`job.enqueued`）ことを記録する
5. 1〜4 の記録を Issue #1037（と Issue #1106）に残してから、知見 `K-0001` のこの解決策の `wired` を `true`（`wired_by` に `.github/workflows/pr-body-engine.yml`・`engine-test`・`tools/pr-body-check/engine.test.mjs`）に、`verified` を `true` にする
6. 上げた `ENGINE_REF` の Engine で `engine.test.mjs` の (10)（3 通りの形の 21 回で関所が毎回 0 を返して本文を書く）が通り、(11) が飛ぶ（差分の写しではなく本物で確かめた）ことを記録してから、知見 `K-0001` の Issue #1038 と Issue #1110 の解決策の `wired` と `verified` を `true` にする（議決 `D-0045`）

### 定期の照合（`ci/pr-body-protection.yml`）

`ci.test.mjs` は守りたい `engine-test` の中で走るので、写しが消えたり `pr-body-engine-test` が必須のチェックから外されたりしたときは自分も走らず、その崩れを見張れない。建設者が一度 `protection.mjs` を走らせるだけでも、後で外されたことに気づけない（議決 `D-0044`、Issue #1106）。`ci/pr-body-protection.yml` の `protection` のジョブ（チェックの名前 `pr-body-protection`）が、`protection.mjs` を既定のブランチ（`main`）への push・1 日 1 回（UTC 20:30、JST 5:30）・手動（`workflow_dispatch`）で走らせる。push で走るので、写しを消す PR がマージされた直後にも落ちる。

- branch protection は `GITHUB_TOKEN` では読めないので、読める権限のトークン（GitHub App か `administration:read` の fine-grained PAT）をシークレット `PR_BODY_PROTECTION_TOKEN` に置く（建設者。`tools/sendback-check/ci/sendback-protection.yml` の `SENDBACK_PROTECTION_TOKEN` と同じ値でよい）。トークンを渡すのは branch protection を読む手順だけ
- `main` の古い形（`branches/main/protection`）と rulesets（`rules/branches/main`）の両方を読み、`--protection` に 2 つとも渡す。古い形が保護なし（404 の `Branch not protected`）のときはそのまま渡す（rulesets にもなければ `required_check_missing`）
- トークンがない・読めなかった（権限がないなど）ときは `::error title=pr-body-protection protection_unreadable::…` で落ちる。必須のチェックの有無が分からないので Issue は開かない
- `workflow_not_copied`・`check_name_changed`・`required_check_missing`・`check_name_shared` のどれかで外れたら、ジョブは落ち、題が `ISSUE_TITLE` の開いている Issue がなければ開き、あればコメントで足す。記録の最初の行は外れ方ごとの印（`<!-- pr-body-protection: <外れの行の sha256 の先頭 16 桁> -->`）で、同じ印が Issue の本文かコメントにすでにあれば足さない。開いている Issue を探せなかったときは開く側に倒し、Issue を開けない・コメントできないときはその手順も落ちる
- 通ったら、開いている Issue に通ったことを書いて閉じる
- PR では走らせない（PR のコードをトークンと `issues: write` で動かさない）。建設者が `.github/workflows/pr-body-protection.yml` にそのまま写す。`ci.test.mjs` がこのひな形の形（push は `main` だけ・schedule・手動のほかで走らない・トークンは読む手順だけ・2 つの JSON を渡す・Issue の手順は `if: always()`・チェックの名前が `pr-body-engine-test` と重ならない）と、手順を偽の `gh` で動かした振る舞い（そろっていれば通る・rulesets だけでも通る・4 つの外れ方のどれでも Issue を開く・開いていればコメントで足し同じ外れ方は足さない・通ったら閉じる・読めなければ照らさない）、写しがあればひな形と同じかを見る
- 写してシークレットを置き、手動で 1 回走らせて通ったら、`K-0001` の Issue #1106 の解決策の `wired` を `true`（`wired_by` に `.github/workflows/pr-body-protection.yml` の `protection` と `tools/pr-body-check/protection.mjs`、`.github/workflows/pr-body-engine.yml` の `engine-test` と `tools/pr-body-check/engine.test.mjs`）にする。`tools/knowledge-enforcement-check/check.py` の `REQUIRED_SCHEDULE` に入れたので、schedule のジョブから `protection.mjs` を呼ぶ `wired_by` がないと `wired: true` にできない。上の「順序」の 1 と 4（今の `ENGINE_REF` で (3) が落ちること、本物の続きのジョブで外れた本文が書かれず body_fix が積まれたこと）を Issue #1106 に記録してから `verified` を `true` にする
