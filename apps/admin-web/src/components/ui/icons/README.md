# 同梱のアイコン

`specs/design/tokens.yaml` の `iconography.set` に従い、admin-web は Material Symbols Rounded の SVG を同梱する（外から読み込まない）。使う分だけを置く。

- 出どころ: [google/material-design-icons](https://github.com/google/material-design-icons) の `symbols/web/<名前>/materialsymbolsrounded/<名前>_24px.svg`（取得 2026-11-01）。中身は変えていない
- ライセンス: Apache License 2.0（同じフォルダの `LICENSE`）
- 部品からは `paths.ts`（SVG の path の写し）を `Icon` で描く。SVG を足す・差し替えるときは `paths.ts` も直す（`tests/designIcons.test.ts` が一致を確かめる）

| ファイル | 使うところ |
|---|---|
| `info.svg` | 状態 info（StatusTag・Notice） |
| `check_circle.svg` | 状態 success |
| `schedule.svg` | 状態 warning（確定待ちなど） |
| `error.svg` | 状態 danger と、入力欄の誤り（TextField） |
| `do_not_disturb_on.svg` | 状態 neutral（キャンセル済みなど） |
| `chevron_right.svg` | 一覧の行の詳細へ（ListRow） |
