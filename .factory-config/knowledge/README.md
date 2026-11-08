# knowledge/

工場が学んだ知見の置き場。形と手順は Engine の ADR 0024 と `definitions/knowledge.yaml`。

- `K-NNNN.yaml` に 1 件 1 主張・1 ファイル。最初はフラットに置き、テーマは中の `theme` で引く。ディレクトリの分け方は委員会がオーナーとして見直す
- `cases/C-NNNN.md` は事例記録（なぜなぜの過程）。知見の根拠として参照される
- `committee/D-NNNN.yaml` は委員会の議決。反対意見も消さずに残す
- `INDEX.md` は自動生成（`core-gates/knowledge_check.py --index`）。手で編集しない
- 確かさは hypothesis（仮説、30 日）・confirmed（確認済み、90 日）・settled（確定、期限なし）。期限（`review_by`）を過ぎた知見は要再確認になり、エージェントに渡さない
- 足すときは変更セット: 新しい知見に、影響を受ける既存の知見の扱い（直す・置き換える・廃止する・再確認する・そのまま）を必ず添えて 1 本の PR にする。ファイルは消さず状態だけ変える。id は再利用しない
- **個人情報は置かない**。利用者を指す情報（氏名・連絡先・利用者 ID・本文の引用など）は抽象化して切る（「生徒 A さん」ではなく「予約の確定待ちの生徒」）。CI の knowledge-check が形を検出したらマージしない。利用者の情報をどうしても含む知見は、工場の制限付きの保管先に置き、ここからは `restricted: R-NNNN` で指すだけにする
- 決定論で担保した知見も消さず、どの解決策で担保したかを `enforcement.solutions` に残す
- `enforcement.solutions` の項目に `wired`（CI や呼び出しに配線済みか）と `verified`（本物の記録で効き目を確かめたか）を真偽で書く。`wired: true` には配線先の `wired_by`（`.github/workflows/` のファイル・job・呼ぶ検査の道）が要り、`check.py` がその job が検査を呼んでいるかを照らす（議決 `D-0021`、Issue #986）。`verified: true` には確かめた本物の記録の `verified_by`（`evidence` と同じ形の `case`・`pr`・`event` の列で、`cases/` にある事例記録を 1 件以上含む）が要り、`check.py` が事例記録の実在と `wired: true` を照らす（議決 `D-0034`、Issue #1059）。書くのは足したり直したりした知見から順に（2026-11-08 の時点ですべての解決策に書いたのは `K-0001`・`K-0002`・`K-0004`・`K-0005`・`K-0010`・`K-0011`・`K-0012`。`K-0006`・`K-0009` には未記入の解決策が残っていて、次にその知見を直す変更セットで実際の配線と確認の状態を書く。この 2 つの一覧は `tools/knowledge-enforcement-check/test_check.py` が `check.py` の数えた結果と順まで照らし、合わなければ落ちる。議決 `D-0052`、Issue #1138）。書いていない項目は落とさず `false`（未記入）として数え、未記入の件数を別に出す。`tools/knowledge-enforcement-check/check.py` の表示では、すべて `wired: false`（未記入を含む）の知見の担保を `deterministic` ではなく `未配線` と出す。`INDEX.md` の担保の列は Engine の `core-gates/knowledge_check.py` が同じ表示にするまで `enforcement.level` のまま（`deterministic`）出る
