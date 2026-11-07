# 知見の担保の配線と効き目の検査

知見（`.factory-config/knowledge/K-*.yaml`）の `enforcement.solutions` の各項目が、CI や呼び出しに配線済みか（`wired`）と、本物の記録で効き目を確かめたか（`verified`）を決定論で（LLM を使わずに）見る。知見 `K-0004` の担保で、委員会の議決 `D-0015` と Issue #962 で作った。

`K-0004` の解決策は 10 件になったが、どれも「効き目はまだ確かめていない」を `what` の文で書くだけで、`INDEX.md` の担保の列は `deterministic` と出ていた。配線も確認もしていない担保を決定論の担保と読み違えないように、真偽の項目にして機械で数える。

```sh
python3 tools/knowledge-enforcement-check/check.py [--repo .] [--json <出力先>] [--summary "$GITHUB_STEP_SUMMARY"]
python3 -m unittest discover -s tools/knowledge-enforcement-check   # テスト
```

依存は Python 3 と PyYAML（なければ ruby で読む。Engine の `core-gates/knowledge_check.py` と同じ）。終了コードは 0 = 緑、1 = 通らない、2 = 読めなかった。

## 項目

`enforcement.solutions` の各項目に書く。

| 項目 | 値 | 意味 |
|---|---|---|
| `wired` | `true` / `false` | CI（`.github/workflows/`）や Engine の呼び出しから実際に走るか。ひな形（`ci/*.yml`）を置いただけ、テストを書いただけは `false` |
| `verified` | `true` / `false` | 本物の記録（実際の PR やイベント列）で、外れたときに落ちる・正しいときに通ることを確かめたか。写しや作った入力だけで確かめたものは `false` |

書いていない項目は落とさず `false`（未記入）として数え、表示も `未配線` 側に倒す（`K-0002` の `config` のように実際は効いているかもしれない解決策も、書くまでは配線済みと出さない）。未記入の件数は出力の「未記入」に別に出すので、書いていないのか `false` と書いたのかは見分けられる。項目を書くのは足したり直したりした知見から順に（2026-11-05 の時点で書いたのは `K-0004` だけ。`K-0001`・`K-0002`・`K-0009` は次にその知見を直す変更セットで書く）。

## 落とすもの

- `wired` か `verified` が真偽でない（`"true"`・`yes` など）
- `wired` が `true` でないのに `verified` が `true`（配線してから本物の記録で確かめる）

## 担保の表示

`check.py` の出力に出す値（`label()`）。Engine の `knowledge_check.py` に移したら（下の「Engine に移す差分」）`INDEX.md` の担保の列もこの値になる予定で、それまで `INDEX.md` は `enforcement.level` のまま出る。

| 強さ（`enforcement.level`） | 解決策 | 表示 |
|---|---|---|
| `context`・`checklist` | 問わない | 強さのまま |
| `deterministic` | `wired: true` が 1 件もない（解決策がない場合も） | `未配線` |
| `deterministic` | `wired: true` はあるが、`wired: true` かつ `verified: true` が 1 件もない | `deterministic（未確認）` |
| `deterministic` | `wired: true` かつ `verified: true` が 1 件以上 | `deterministic` |

`check.py` の表示では、すべて `wired: false`（未記入を含む）の知見を `deterministic` と出さない。

## Engine に移す差分

Engine（別のリポジトリ）に入れる変更を `engine/knowledge-enforcement.patch` に置いた（議決 `D-0017`、Issue #968）。ワーカーは Engine を変えられないので、差分として置き、建設者が Engine に当てる。

- `definitions/knowledge.yaml` の `knowledge.enforcement` に `solution_flags`（`wired`・`verified`）と `index_labels`（上の表示の 3 つ）を足す
- `core-gates/knowledge_check.py` に `enforcement_label()`（`check.py` の `label()` と同じ線）を足し、`index_md()` の担保の列をその値にする。`check_item()` で、`wired`・`verified` が真偽でないときと、`wired` でないのに `verified` が `true` のときに落とす

`test_engine.py` が、Engine のチェックアウトの写しに差分を当てて（Engine がもう取り込んでいれば当てずに）、担保の表示が `label()` と同じになること、`INDEX.md` で `K-0004` が `未配線` と出ること、真偽の検査が落ちる・通ること、このリポジトリの知見が作り直した `INDEX.md` で通ることを確かめる。Engine のチェックアウトは環境変数 `ENGINE_DIR` で渡す（なければリポジトリの隣の `../engine`）。どちらもなければ飛ばし、`ENGINE_DIR` を渡したときと CI（`CI=true`）では飛ばさずに落とす。差分が Engine に当たらなくなったら今の Engine に合わせて作り直す。

```sh
ENGINE_DIR=/path/to/factory-engine python3 -m unittest discover -s tools/knowledge-enforcement-check
```

## まだ入っていないもの（建設者が入れる）

- `engine/knowledge-enforcement.patch` を Engine に当てること。製品の CI（`knowledge-check`）は Engine の固定の版（`ENGINE_REF`）の `knowledge_check.py` で `INDEX.md` を作り直して比べるので、当てた版に `ENGINE_REF` を上げるまで `INDEX.md` の担保の列は `deterministic` のまま。`ENGINE_REF` を上げる PR で `--index` を作り直す（`K-0004` などが `未配線` になる）。上げたあとは `test_engine.py` は差分を当てずに Engine の版をそのまま確かめるので、差分のファイルは消してよい
- `ci/knowledge-enforcement.yml` を `.github/workflows/` に写して PR ごとに走らせること。写すまでこの検査は未配線。写すときは `ENGINE_REF` を `knowledge-check.yml` と同じ版にそろえる
