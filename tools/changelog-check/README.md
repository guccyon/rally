# CHANGELOG の直接の書き換えの検査

PR が `CHANGELOG.md` の `[Unreleased]` を直接書き換えていたら落とす。利用者に見える変更は `changelog.d/<Issue 番号>.md` の断片に書き、工場が取りまとめる（`changelog.d/README.md`、factory-engine の ADR 0027）。知見 `K-0002` を決定論で担保するための検査（議決 `D-0001`、#827）。決定的で、依存は node と git だけ（`npm ci` は要らない）。

```sh
node check.mjs --base origin/main --head HEAD [--summary "$GITHUB_STEP_SUMMARY"]
npm test
```

終了コードは 0 = 通る、1 = 通らない、2 = 読めなかった。通らないときは `::error file=CHANGELOG.md::…` を出す。

## 見る範囲

`git diff base...head` と同じく、base と head の分かれ目（merge-base）と head を比べる。既定ブランチ側で入った取りまとめは PR の変更として見ない。比べるのは `[Unreleased]` の項の行（空行を除く）で、前書きや過去の版の項は見ない。

| 変更 | 判定 |
|---|---|
| `[Unreleased]` を変えていない（断片だけを足した PR など） | 通る |
| 取りまとめ（`core-gates/changelog_compile.py`）: `[Unreleased]` に足した行がすべて、この PR で消した断片の行か種類の見出し（`### 追加` など）で、消した行がない | 通る |
| 版の切り出し: `[Unreleased]` から消した行がすべて版の項に移っていて、足した行がない | 通る |
| それ以外で `[Unreleased]` を変えた（断片を足していても、取りまとめで二重になるので） | 落とす |
| `## [Unreleased]` の見出しを消した | 落とす |

取りまとめの PR はラベルや作り手ではなく、差分の中身（消した断片の行が移っただけか）で見分ける。

## CI への組み込み

ワーカーは `.github/workflows/` を変えられないので、ワークフローは建設者が足す。PR ごとに走らせ、通らなければジョブを落とす。

```yaml
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - name: CHANGELOG.md を直接書き換えていないか
        run: node tools/changelog-check/check.mjs --base "origin/${{ github.base_ref }}" --head HEAD --summary "$GITHUB_STEP_SUMMARY"
```
