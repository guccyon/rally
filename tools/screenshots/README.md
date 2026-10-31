# 画面のキャプチャの比較

PR の前（ベース）と後の画面のキャプチャを比べ、変わった画面だけを見比べられる形にまとめる。CI（`.github/workflows/screenshots.yml`）が使う。撮るのは各アプリ（`apps/admin-web/screenshots/`、`apps/student-app/screenshots/`）。

```sh
npm ci
node compare.mjs --app admin-web --base <前のキャプチャ> --head <後のキャプチャ> --out <出力先>
npm test
```

## 入力

キャプチャのフォルダに、画面ごとに `<id>.png` と `<id>.json`（`{ "id", "title", "ok" }`）。前のフォルダがない・空なら、取れた画面はすべて「新しい画面」。

## 出力（CI の成果物 `screenshots-<アプリ>` の中身）

| ファイル | 中身 |
|---|---|
| `manifest.json` | `app` と、`changed`（変わった。`diff_ratio` は違う画素の割合）・`added`（新しい）・`removed`（消えた）・`failed`（撮れなかった）・`unchanged`（変わらない）の画面の一覧。各項目は `id`・`title` と画像のパス（`images/` からの相対） |
| `index.html` | 変わった画面の前・後・差分を並べたページ |
| `summary.md` | PR の CI の要約に足す表 |
| `images/` | `<id>.base.png` / `<id>.head.png` / `<id>.diff.png` |

違う画素が 0.05% 以下（描画の揺れ）なら変わっていない扱い。大きさが違えば変わった扱い。

## PR からのたどり方

CI は PR の先頭のコミットにチェック「画面のキャプチャ / <アプリ>」（状態は常に success。見た目は人が判断するのでマージを止めない）を付け、その詳細のリンクを成果物の URL にする。説明は「見た目が変わった画面 N」など。工場が読むときは、そのコミットの状態（commit status）の `target_url` から成果物を取り、`manifest.json` を読む。アプリに変更がない PR では撮らず、チェックも付かない。
