# PR 本文の見出しの検査

PR 本文の見出しの構造を決定論で（LLM を使わずに）見る。知見 `K-0001`（`.factory-config/knowledge/K-0001.yaml`）の担保で、委員会の議決 `D-0001` と Issue #826 で作った。

続きのジョブ（衝突の解消・CI 落ちの直しなど）の報告が、新しい PR と同じ見出しの組（`## 概要`・`## 対応内容` など）のまま本文の末尾に足されると、同じ見出しが 2 回出る。これを検査役（LLM）と判定役まで流さず、本文を更新した直後に止めるためのもの。

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

コードブロック（```` ``` ```` と `~~~`）の中の行は見出しとして数えない。`### ` 以下の見出しは重なってもよい。

## 結果

- 終了コード: 0 = 通る、1 = 外れた、2 = 本文を読めなかった
- 標準出力: 外れた箇所ごとに「N 行目: 理由」と、最後に件数の 1 行
- `--json`: `{ "ok": bool, "errors": [{ "rule", "line", "heading", "message" }] }`

## 見出しの定義の照らし合わせ（headings.mjs）

知見 `K-0005`（`.factory-config/knowledge/K-0005.yaml`）の担保で、委員会の議決 `D-0006` と Issue #905 で作った。PR 本文の `## ` 見出しの正を `headings.json` の `headings` に 1 つだけ置き、次の 3 つがそれと一致するかを見る。ずれていると、決まりに沿って書いた本文が検査役に差し戻されたり、判定役が同じ指摘を決まりの外として退けたりして、PR ごとに判断が割れる。

| 名前 | 場所 | 読む人 |
|---|---|---|
| `claude_md` | このリポジトリの `CLAUDE.md` の「PR の説明は … の見出しで」 | 判定役（決まりとして読む）・人 |
| `worker` | Engine の `orchestrator/worker/entrypoint.sh` の「最後の出力を次の見出しだけで構成する」 | 実装役（報告の型） |
| `rubric` | Engine の `core-gates/rubric.yaml` の `PR-002` の `check` の「見出し（…）」 | 検査役 |

```sh
node headings.mjs [--repo <リポジトリ>] [--engine <factory-engine の置き場>]   # 終了コード: 0 = 揃っている、1 = ずれた、2 = 読めなかった
```

- Engine の置き場は `--engine`、環境変数 `FACTORY_ENGINE_DIR`、ワーカーに載っている `/work/engine` の順に探す。見つからないときは `claude_md` だけを見る（テストでは Engine の照らし合わせを skip にする）
- Engine 側のずれで、直すのに Engine の人の承認が要るものは `headings.json` の `known_drift` に今の値をそのまま書く。その値のときだけ通し、それ以外に変わったら（直って正と一致したときも）落として記録を直させる。今は `rubric`（`背景`・`問題` があり、`影響範囲・互換性` が `影響範囲`、`仮決め`・`知見` がない）が記録されている
- 正を変えるときは `headings.json` と `CLAUDE.md` の行を同じ PR で直し、Engine の 2 か所は Engine の PR で揃える

## 呼び出す場所

- `check.mjs`: PR 本文の更新（`pr.head_updated`・`handoff.saved`）のあと、検査役に回す前に呼び、1 なら検査役に回さずに止める想定
- `headings.mjs`: `npm test` に入っている。Engine を固定した版で取って（ほかのワークフローと同じ `ENGINE_REF`）PR ごとに走らせるか、Engine を上げるときに走らせる想定

どちらも呼び出しの配線（Engine の検査の前段か、このリポジトリの CI）は、ワーカーの権限の外なので建設者が入れる。
