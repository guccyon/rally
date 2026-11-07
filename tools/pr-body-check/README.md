# PR 本文の見出しの検査

PR 本文の見出しの構造を決定論で（LLM を使わずに）見る。知見 `K-0001`（`.factory-config/knowledge/K-0001.yaml`）の担保で、委員会の議決 `D-0001` と Issue #826 で作り、議決 `D-0013` の案と Issue #956 で本文の更新の前の関所（`gate.mjs`）を足した。知見 `K-0006` の担保として、議決 `D-0006` と Issue #906 で見出しの定義（`template.json`）との照合を足した。

続きのジョブ（衝突の解消・CI 落ちの直しなど）の報告が、新しい PR と同じ見出しの組（`## 概要`・`## 対応内容` など）のまま本文の末尾に足されると、同じ見出しが 2 回出る。これを検査役（LLM）と判定役まで流さず、本文を更新した直後に止めるためのもの。差し戻しの直しで本文の末尾に追記するだけだと、見出しの順が崩れたり必須の見出しが欠けたりする（K-0006）ので、それも見る。

```sh
node check.mjs --body <本文のファイル> [--json <出力先>]
gh pr view <番号> --json body --jq .body | node check.mjs     # --body を省くと標準入力から読む
npm test
```

依存はない（Node.js だけ。`engine.test.mjs` は bash・Python 3 と Engine のチェックアウトも使う）。

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

PR 本文の `## ` 見出しの順と、必須かどうかの 1 つの定義。CLAUDE.md の「PR の説明は 概要 / 対応内容 / 影響範囲・互換性 / 動作確認 / 補足 / 知見 の見出しで」に、実装役の報告の型にある `仮決め` を `補足` と `知見` の間に入れた。該当しない見出しは省くので、必須は `概要` と `対応内容` だけ。

`check.mjs` は読み込むときに定義を検証する（`h2` が空でない配列で、各要素は前後に空白のない 1 行の `text` と真偽値の `required` だけを持ち、`text` が重ならない）。外れたら読み込みの時点で投げ、検査を始めない。検証した定義は配列も要素も凍結した `TEMPLATE` として公開するので、後から書き換えられない。

見出しの正をどこに置くか（CLAUDE.md・実装役の報告の型・Engine の rubric の PR-002 のどれを正にするか）は、知見 `K-0005` の線で人が決める。決まったらこのファイルをその正に合わせる。

## 結果

- 終了コード: 0 = 通る、1 = 外れた、2 = 本文を読めなかった
- 標準出力: 外れた箇所ごとに「N 行目: 理由」（本文全体の外れ、つまり `missing_required` は「本文: 理由」）と、最後に件数の 1 行
- `--json`: `{ "ok": bool, "errors": [{ "rule", "line", "heading", "message" }] }`。`missing_required` の `line` は `null`

## 本文の更新の前の関所（`gate.mjs`）

知見 `K-0001` を決定論で担保する配線の入口（議決 `D-0013` の案、Issue #956）。続きのジョブの summary を本文に組み込んだあと、`gh pr edit` で書く前の本文を `checkBody` に照らし、外れたら本文を書かずに本文の直し（body_fix）に回す。

```sh
printf '%s' "${NEW_BODY}" | node tools/pr-body-check/gate.mjs --pr-url "${PR_URL}"   # リポジトリの根から
```

- 終了コード: 0 = 本文を書いてよい、1 = 本文を書かずに body_fix に回す、2 = 読めなかった（本文は書かない）
- 1 のときの標準出力: body_fix の入力の JSON 1 行。Engine の `dispatcher.enqueue_body_fix` が `job.enqueued` の `input` に入れる形（`{ pr_url, defects: [{ rubric_id, severity, location, evidence, remediation_hint }] }`）
  - `rubric_id` は本文の構成の指摘 `PR-002`（Engine の `core-gates/rubric.yaml`）、`severity` は `should`
  - `location` は「PR 本文 N 行目」（Engine の `is_body_only` が本文だけの指摘と読む形）。`evidence` は「pr-body-check <rule>: 理由」
- 外れた箇所は標準エラーに「N 行目: 理由」で出す。`--json <出力先>` で `{ ok, errors, body_fix }` を書く

## テスト

`npm test` で 3 つを走らせる。

- `check.test.mjs`: 見出しの検査（`check.mjs`）
- `gate.test.mjs`: 関所。事例 `C-0005`（PR #659）の 21 回の追記（conflict が 20 回、ci_failed が 1 回。どれも新しい PR と同じ `## 概要`・`## 対応内容`・`## 動作確認` を持つ summary）と同じ入力で落ち、1 回目の追記でもう落ちることを確かめる。入力は `c0005.mjs` が組む（本文そのものは記録にないので、事例に書かれた形を組み直したもの）
- `engine.test.mjs`: Engine の本物で確かめる。Engine のチェックアウトは環境変数 `ENGINE_DIR` で渡す（なければリポジトリの隣の `../engine`）。どちらもなければ飛ばす。ただし `ENGINE_DIR` を渡したときと CI（`CI=true`）では飛ばさずに落とす（`tools/sendback-check` と同じ。議決 `D-0011`）
  - (0) Engine の `orchestrator/worker/entrypoint.sh` から、新しい PR の本文（`PR_BODY`）・続きのジョブの組み込み（`SECTION_TITLE` から `NEW_BODY` まで）・本文の更新（`gh pr view` から `continued` のログまで）の節を取り出せ、`engine_harness.py` が `dispatcher.py` の `is_body_only`・`enqueue_body_fix` を取り出せる
  - (1) Engine の組み込みで C-0005 の 21 回の追記をした本文が関所で落ちる（Engine が組み込み方を直して重ならなくなったら、それでよい）
  - (2) 関所の body_fix の入力が Engine の `is_body_only` で本文だけの指摘と読まれ、`enqueue_body_fix` で body_fix のジョブが積まれる
  - (3) Engine の本文の更新の節を、偽の `gh`（`pr view` と `pr edit --body` だけを真似る）で C-0005 の 21 回分動かし、関所に外れる本文を一度も書かない。必須のテストだが、Engine の `entrypoint.sh` が関所（`tools/pr-body-check/gate.mjs`）を呼んでいない今の形のときだけ `todo` にする（外れても落ちないが結果に出る。`tools/sendback-check` の `todoWhileLegacy` と同じ扱い）。Engine が関所を呼ぶようになったら、その版からは外れた本文を書くと落ちる
- 節の取り出し方（`snippets`）は `entrypoint.sh` の今の行の形に合わせている。Engine で形が変わって取り出せなくなったら (0) が落ちるので、同じ PR で合わせる

```sh
npm test
ENGINE_DIR=/path/to/factory-engine node --test engine.test.mjs
python3 engine_harness.py --check /path/to/factory-engine   # 関数を取り出せるかだけを見る
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

なお、Engine の `orchestrator/worker/entrypoint.sh` は今、続きのジョブの報告を `## 工場の記録` の後ろに `## 続きのジョブの記録` として足すので、そのままでは `outside_factory_record`・`continuation_outside_record` で外れる。必須チェックにする前に、Engine 側でその見出しを `### ` に下げる（K-0001 の線。Engine への作用なので人の承認が要る）。

このワークフローとブランチ保護の必須チェックは、ワーカーの権限の外なので建設者が入れる。

## Engine に入れること（建設者）

Engine は別のリポジトリなので、次は建設者が入れる。入れて `ENGINE_DIR` の版を上げたら、`engine.test.mjs` の `isLegacyEntrypoint` と `todo` を消してよい。

1. `worker/entrypoint.sh` の続きのジョブの節で、`NEW_BODY` を組んだあと `gh pr edit` の前に、リポジトリに `tools/pr-body-check/gate.mjs` があれば `NEW_BODY` を照らす。1（か 2）なら `gh pr edit` で本文を書かず、標準出力の body_fix の入力と組んだ本文（続きのジョブの報告）を引き継ぎ（`HANDOFF_DIR`）と `FACTORY_RESULT` に残す。push した head（`pr.head_updated`）と引き継ぎ（`handoff.saved`）は今のまま
2. dispatcher が、その結果を見て `enqueue_body_fix` で body_fix を積む（判定役を待たない。検査役にも回さない）。body_fix には、書かなかった続きのジョブの報告を「## 工場の記録」の下の `###` に入れて本文を組み直すよう渡す
3. body_fix が直した本文も関所に照らし、まだ外れたら人に回す（同じ直しを繰り返さない）

`ci/pr-body-engine.yml` は、Engine を固定の版で取って `ENGINE_DIR` を向け、このディレクトリのテストを PR ごとに走らせる CI。ワーカーは `.github/workflows/` を変えられないので、建設者が `.github/workflows/pr-body-engine.yml` に写す。この CI では PR 本文そのものは見ない（落ちると `ci_failed` の続きのジョブが積まれ、その summary がまた本文に足されて、この知見の形を増やすため）。本文は上の「マージ前の必須チェック」で見るが、同じ理由で、そのチェックが落ちたときは `ci_failed` の続きのジョブを積まず、関所と同じく body_fix に回す（建設者が Engine の dispatcher で分ける）。
