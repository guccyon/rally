# デザインシステムのカタログ

デザインシステムの正本（`specs/design/`）だけから、部品の見本を並べたカタログを組み立てる。出力は `specs/design/catalog/catalog.html` と `catalog.png`（人が PR や手元で見る）。

```sh
npm ci
npm run build       # catalog.html だけ
npm run build:png   # catalog.html と catalog.png（Chromium で撮る。初回は npx playwright install chromium）
npm test            # 置いてある catalog.html が今の正本と同じか・すべての部品に見本があるか
```

## 何から作るか

| カタログの欄 | 正本 |
|---|---|
| 冒頭のコピーとコンセプトの表 | `concept_tone.yaml` の `copy` |
| 色・書体・余白と角丸 | `tokens.yaml`（書体の見本は `typography.scale` の `sample`） |
| 部品 | `components.yaml` の各部品の `preview` と `preview_css` |

CSS の変数は admin-web の `@theme` と同じ名前（`--color-primary`・`--radius-md`・`--space-sm`。状態の色は `--color-<状態>-fg` / `-container` / `-on-container`）で `tokens.yaml` から作る。書体の段は `.type-<段>` の class。Web フォントは読まず、書体は `typography.families.sans` の名前で指定するだけなので、`catalog.png` は撮った端末に Noto Sans CJK JP（か Noto Sans JP）がないと別のゴシックで出る。

正本を直したら作り直して同じ変更に入れる。`catalog.html` と `catalog.png` は手で直さない。
