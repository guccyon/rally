# CHANGELOG の断片

利用者（生徒・コーチ・本部）や API の利用者に見える変更は、PR ごとにこのディレクトリへ 1 ファイルで書く。`CHANGELOG.md` は直接書き換えない。

同時に流れる PR がどれも `CHANGELOG.md` の同じ箇所に追記すると、1 件マージされるたびにほかの PR が衝突する（2026-11-02 は衝突の解消が 1 日 345 回）。断片はファイルが PR ごとに分かれるので衝突しない。工場が断片を `CHANGELOG.md` の `[Unreleased]` に取りまとめる（factory-engine の ADR 0027、`core-gates/changelog_compile.py`）。

## 書き方

- ファイル名は `<Issue 番号>.md`（例: `705.md`）。1 PR 1 ファイル。Issue のない変更は内容の分かる名前でよい
- 中身は `[Unreleased]` に入れる形のまま。種類の見出し（`### 追加`・`### 変更`・`### 修正`・`### 削除`）の下に項目を書く。見出しを省いた行は `### 変更` に入る
- 項目の書き方はこれまでの `CHANGELOG.md` と同じ（何が変わったか、利用者から見た理由、契約・DB・画面への影響、末尾に `（#Issue 番号）`）

```markdown
### 追加
- 本部が送ったお知らせの一覧を期間で絞る: admin-web の `GET /announcements`（`listAnnouncements`）に任意のクエリ `from`・`to` を足した。…（#647）
```

## 取りまとめ

- 工場が、断片が 10 件たまったときと、出荷の列車（17 時）の前に、`factory:changelog` の Issue を起こし、断片を `CHANGELOG.md` に移す PR を作る。移した断片は消える
- 取りまとめの PR だけは `CHANGELOG.md` を書き換える
