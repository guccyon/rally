# PR 本文の見出しの検査

PR 本文の見出しの構造を決定論で（LLM を使わずに）見る。知見 `K-0001`（`.factory-config/knowledge/K-0001.yaml`）の担保で、委員会の議決 `D-0001` と Issue #826 で作った。知見 `K-0006` の担保として、議決 `D-0006` と Issue #906 で見出しの定義（`template.json`）との照合を足した。

続きのジョブ（衝突の解消・CI 落ちの直しなど）の報告が、新しい PR と同じ見出しの組（`## 概要`・`## 対応内容` など）のまま本文の末尾に足されると、同じ見出しが 2 回出る。これを検査役（LLM）と判定役まで流さず、本文を更新した直後に止めるためのもの。差し戻しの直しで本文の末尾に追記するだけだと、見出しの順が崩れたり必須の見出しが欠けたりする（K-0006）ので、それも見る。

```sh
node check.mjs --body <本文のファイル> [--json <出力先>]
gh pr view <番号> --json body --jq .body | node check.mjs     # --body を省くと標準入力から読む
npm test
```

依存はない（Node.js だけ）。

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

## 呼び出す場所

PR 本文の更新（`pr.head_updated`・`handoff.saved`・`pr.body_amended`）のあと、検査役に回す前に呼び、1 なら検査役に回さずに止める想定。

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

呼び出しの配線（`.github/workflows/` のワークフロー・ブランチ保護の必須チェック・Engine の `pr.body_amended` の後段）は、ワーカーの権限の外なので建設者が入れる。
