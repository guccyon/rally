# 知見の担保の配線と効き目の検査

知見（`.factory-config/knowledge/K-*.yaml`）の `enforcement.solutions` の各項目が、CI や呼び出しに配線済みか（`wired`）と、本物の記録で効き目を確かめたか（`verified`）を決定論で（LLM を使わずに）見る。知見 `K-0004` の担保で、委員会の議決 `D-0015` と Issue #962 で作り、議決 `D-0021` と Issue #986 で `wired: true` の配線先（`wired_by`）を照らす検査を、議決 `D-0034` と Issue #1059 で `verified: true` の確認の記録（`verified_by`）を照らす検査を、議決 `D-0035` と Issue #1063 で Engine に移す差分を `ENGINE_REF` の版で必ず確かめる CI のジョブ（`engine-test`）とその形の検査（`test_ci.py`）を、議決 `D-0037` と Issue #1080 でそのジョブがテストを飛ばして緑にならない手順を足した。

`K-0004` の解決策は 10 件になったが、どれも「効き目はまだ確かめていない」を `what` の文で書くだけで、`INDEX.md` の担保の列は `deterministic` と出ていた。配線も確認もしていない担保を決定論の担保と読み違えないように、真偽の項目にして機械で数える。

```sh
python3 tools/knowledge-enforcement-check/check.py [--repo .] [--json <出力先>] [--summary "$GITHUB_STEP_SUMMARY"]
python3 -m unittest discover -s tools/knowledge-enforcement-check   # テスト
ENGINE_DIR=<factory-engine のチェックアウト> python3 -m unittest discover -s tools/knowledge-enforcement-check   # Engine に当てる差分のテストも
```

依存は Python 3 と PyYAML（なければ ruby で読む。Engine の `core-gates/knowledge_check.py` と同じ）。終了コードは 0 = 緑、1 = 通らない、2 = 読めなかった。

## 項目

`enforcement.solutions` の各項目に書く。

| 項目 | 値 | 意味 |
|---|---|---|
| `wired` | `true` / `false` | CI（`.github/workflows/`）や Engine の呼び出しから実際に走るか。ひな形（`ci/*.yml`）を置いただけ、テストを書いただけは `false` |
| `wired_by` | `{workflow, job, runs}` の列 | `wired: true` のときに必須。`workflow` は `.github/workflows/` の直下のファイル、`job` はその `jobs` の id、`runs` は呼ぶ検査の道（リポジトリの根から。例: `tools/sendback-check/order.mjs`）。`wired` が `true` でなければ見ない（議決 `D-0021`、Issue #986） |
| `verified` | `true` / `false` | 本物の記録（実際の PR やイベント列）で、外れたときに落ちる・正しいときに通ることを確かめたか。写しや作った入力だけで確かめたものは `false` |
| `verified_by` | `case`・`pr`・`event` の参照の列 | `verified: true` のときに必須。確かめた本物の記録を、`evidence` と同じく 1 項目に 1 つの参照で書く（例: `- case: C-0001`・`- pr: 395`・`- event: case.analyzed:40376`）。`case` を 1 件以上含め、その事例記録が `.factory-config/knowledge/cases/` にあること。`verified` が `true` でなければ見ない（議決 `D-0034`、Issue #1059） |

書いていない項目は落とさず `false`（未記入）として数え、表示も `未配線` 側に倒す（`K-0002` の `config` のように実際は効いているかもしれない解決策も、書くまでは配線済みと出さない）。未記入の件数は出力の「未記入」に別に出すので、書いていないのか `false` と書いたのかは見分けられる。項目を書くのは足したり直したりした知見から順に（2026-11-08 の時点ですべての解決策に書いたのは `K-0001`・`K-0002`・`K-0004`・`K-0010`・`K-0012`。`K-0006`・`K-0009` の未記入の解決策は次にその知見を直す変更セットで書く）。この 2 つの一覧は `test_check.py` が `check.py` の数えた結果（解決策があって未記入が 0 件の知見と、未記入が 1 件以上の知見）と照らし、合わなければ落ちる（議決 `D-0035`、Issue #1062）。知見に項目を書き足したら、同じ変更セットでこの一覧を直す。

## 落とすもの

- `wired` か `verified` が真偽でない（`"true"`・`yes` など）
- `wired` が `true` でないのに `verified` が `true`（配線してから本物の記録で確かめる）
- `verified` が `true` なのに、`verified_by` がない・空・列でない、項目が 1 つの参照でない・種類が `case`・`pr`・`event` でない、`case` が `C-NNNN` でないか `.factory-config/knowledge/cases/` にない、`pr` が正の整数でない、`event` が `<種類>:<id>` でない、`case` が 1 件もない（`pr`・`event` は手元で本物か照らせないので、事例記録に辿れることを求める）
- `wired` が `true` なのに、`wired_by` がない・空・項目が欠けている、`workflow` が `.github/workflows/` の直下でない（ひな形の `ci/*.yml` は配線ではない）、`workflow`・`job`・`runs` のどれかが実在しない、その job が `runs` の検査を呼んでいない（下の「配線先の照合」）

## 配線先の照合

`wired` は書き手の申告なので、配線していないのに `true` と書かれても見抜けなかった（議決 `D-0021`、Issue #986）。`check.py` は `wired_by` の job の手順（`run`）が `runs` を呼んでいるかを、`tools/sendback-check/order.mjs` と同じやり方で照らす。

- shell の注釈を落とし、行末の `\` でつないだ行を 1 行にして見る
- 道は手順の `working-directory`（なければ job の `defaults.run.working-directory`）と、手順の中の `cd` の下か根から引く。`$GITHUB_WORKSPACE`・`${{ github.workspace }}` は根にする
- ワークフロー・job・手順の `env` と shell の代入の変数（`$NAME`・`${NAME}`・`${{ env.NAME }}`）は値に置き換える
- `npm`・`pnpm` の `run`・`run-script`・`test` は、そのディレクトリ（`--prefix` も）の `package.json` の scripts の中身を辿る。シェルのラッパー（`.sh`・`.bash`）は中身を読んで辿る（どちらも 3 段まで）
- `runs` がテストのファイル（`test_*.py`・`*.test.mjs` など）のときは、そのディレクトリで `python3 -m unittest discover -s <ディレクトリ>` や、ファイルを名指ししない `node --test` を走らせる形も呼び出しに数える
- `order.mjs` は取りこぼすと見張れないので迷う形を呼ぶ側に数えるが、ここで見逃すと配線していない担保を配線済みと読ませるので、逆に迷う形は呼んでいない側に数える（scripts にない名前・読めないラッパー・別の job の手順・`uses:` の手順・別のワークフローを呼ぶ job は数えない）。字面で偽の `if:`（`false`・`${{ false }}`）の job と手順も走らないので数えない
- Engine の呼び出しだけで走る解決策は今の `wired_by` では書けない。書けるようになるまで `wired: false` のまま置く

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

Engine（別のリポジトリ）に入れる変更を `engine/knowledge-enforcement.patch` に置いた（議決 `D-0016`・`D-0017`、Issue #965・#968。同じ案を 2 つの Issue で作ったので差分は 1 本にまとめた）。議決 `D-0021` の schema の案（Issue #987）も同じ案なので、新しい差分は作らずこの差分で満たす。議決 `D-0034` の schema の案（Issue #1060。`wired_by` を Engine の定義に書き、Engine の検査でも同じ形を落とす）もこの差分に足した。ワーカーは Engine を変えられないので、差分として置き、建設者が Engine に当てる。

- `definitions/knowledge.yaml` の `knowledge.enforcement` に `solution_flags`（`wired`・`verified`）と `index_labels`（上の表示の 3 つ）を足す
- `core-gates/knowledge_check.py` に `enforcement_label()`（`check.py` の `label()` と同じ線）を足し、`index_md()` の担保の列をその値にする。`check_item()` で、`wired`・`verified` が真偽でないときと、`wired` でないのに `verified` が `true` のときに落とす
- `definitions/knowledge.yaml` の `knowledge.enforcement` に `solution_wired_by`（`required_when: {wired: true}` と、`items` の `workflow`（`.github/workflows/` の直下のファイルの形）・`job`・`runs`）を足す
- `core-gates/knowledge_check.py` に `check_wired_by()` を足し、`check_item()` で `wired` が `true` の解決策の `wired_by` の形を落とす（列でない・空、項目が `workflow`・`job`・`runs` の空でない文字列でない、`workflow` が `.github/workflows/` の直下でない）。ファイル・job の実在と job が `runs` を呼ぶかは Engine では見ず、`check.py` の「配線先の照合」が照らす
- `core-gates/tests/test_knowledge_check.py` に、その 3 つの Engine の側のテストを足す

`test_engine.py` が、Engine のチェックアウトの写しに差分を当てて（Engine がもう取り込んでいれば当てずに）、担保の表示が `label()` と同じになること（手で選んだ組に加え、解決策の `wired`・`verified` の値（未記入・`true`・`false`・真偽でない値）の組を強さと `enforcement` の形ごとに総当たりで。議決 `D-0024`、Issue #1015）、`INDEX.md` で `K-0004` が `未配線` と出ること、真偽の検査が落ちる・通ること、`wired_by` の形で落とすかが `check.py` の形の誤り（`wired_by` がない・項目が欠ける・`.github/workflows/` の直下でない）と `wired`・`wired_by` の値の組ごとに一致すること、定義の `solution_wired_by` が `check.py` と Engine の値とそろうこと、差分で Engine に足したテストが通ること（pytest があるとき）、このリポジトリの知見が作り直した `INDEX.md` で通ることを確かめる。Engine のチェックアウトは環境変数 `ENGINE_DIR` で渡す（なければリポジトリの隣の `../engine`）。どちらもなければ飛ばし、`ENGINE_DIR` を渡したときと CI（`CI=true`）では飛ばさずに落とす。Engine のファイルは、チェックアウトに `knowledge-check.yml` の `ENGINE_REF` の版があればその版から取る（チェックアウトの今の中身が別の版でも、製品の CI が使う版で確かめる）。その版がなければ `ENGINE_DIR` を渡したときと CI では落とし、`ENGINE_DIR` を渡さず `../engine` を使う手元ではチェックアウトの今の中身で確かめる。差分で足したテストは pytest がないと `ENGINE_DIR` を渡さない手元では飛ばすが、`ENGINE_DIR` を渡したときと CI では落とす（議決 `D-0035`・`D-0037`、Issue #1063・#1080）。差分が Engine に当たらなくなったら今の Engine に合わせて作り直す（Issue #1060 で `knowledge-check.yml` の `ENGINE_REF`（`9d3713a`）に合わせて作り直し、`ci/knowledge-enforcement.yml` の `ENGINE_REF` もその版にそろえた）。

```sh
ENGINE_DIR=/path/to/factory-engine python3 -m unittest discover -s tools/knowledge-enforcement-check
```

### 差分が古くなっていないか（`patch_stale`）

`test_engine.py` は Engine のチェックアウトの今の中身に差分を当てるので、製品の CI が使う版（`knowledge-check.yml` の `ENGINE_REF`）と違う版を渡すと、その版では当たらないことに気づけない（議決 `D-0024`、Issue #1014）。`test_engine_patch.py` は `.github/workflows/knowledge-check.yml` の `ENGINE_REF` を読み、`ENGINE_DIR` のチェックアウトからその版を名指しで取り出して `git apply --check` で差分を当て、次のときに `patch_stale` で落とす（GitHub Actions では `::error` も出す）。

- 差分が当たらず、しかも Engine 側にまだ入っていない（`git apply --reverse --check` も通らず、`core-gates/knowledge_check.py` に差分で足した `enforcement_label`・`check_wired_by` もない）

差分で足した関数（`enforcement_label`・`check_wired_by` のどれか）はあるのに `git apply --reverse --check` が通らない版（差分の一部だけが入った・入れたあと変わった）は、もう入っているとは数えずに `patch_partially_applied` で落とす（GitHub Actions では `::error` も出す。議決 `D-0036`、Issue #1077）。関数の名前があるだけでは、一部だけ入った版や名前だけが別の中身で入った版でも通ってしまうため。落ちたら差分を今の Engine に合わせて作り直す（全部入れて `ENGINE_REF` を上げたなら差分を消す）。

当たるか、もう入っていれば（`git apply --reverse --check` が通るときだけ）通る。チェックアウトに `ENGINE_REF` の版がないときは飛ばすが、`ENGINE_DIR` を渡したときと CI では落とす（`ci/knowledge-enforcement.yml` の `ENGINE_REF` が `knowledge-check.yml` とそろっていないとこれで落ちる）。判定そのものは、作った git の版（当たる・入っている・一部だけ入った・入れたあと変わった・どれでもない）で Engine なしに確かめる。`ci/knowledge-enforcement.yml` の `engine-test` のジョブで `test_engine.py` と一緒に走る。

## CI のひな形（`ci/knowledge-enforcement.yml`）

ジョブは 2 つ。

- `engine-test`: `ENGINE_REF` の版の Engine を取り、`python3 -m unittest discover -s tools/knowledge-enforcement-check`（`test_engine_patch.py`・`test_engine.py` を含むすべてのテスト）を `CI=true`・`ENGINE_DIR` を渡し、pytest を入れて走らせ、出力を `knowledge-enforcement-unittest.log` に残す。差分が `ENGINE_REF` に当たらない（`patch_stale`）か一部だけ入っている（`patch_partially_applied`）か、差分で足したテストや担保の表示の比べが落ちれば、どの PR でもジョブが落ちる。次の手順（`飛ばしたテストがないか`）が、その出力の要約の `skipped=` が 1 件以上か、走らせたテストの件数（`Ran N tests`）が出ていなければ `tests_skipped`・`unittest_not_run` で落とす（飛ばす条件の取りこぼしで緑にしない。議決 `D-0037`、Issue #1080）。写したらこのジョブを branch protection の必須チェックに入れて PR を止める（議決 `D-0035`、Issue #1063）
- `enforcement`: Engine の要らないテスト（`test_check.py`・`test_ci.py`）と `check.py` を走らせる。`check.py` の結果で落とすのは、PR が `.factory-config/knowledge/` か `tools/knowledge-enforcement-check/` を変えたときだけ

`test_ci.py` がひな形（写してあれば写しも）の `engine-test` の形を見て、外れたら落とす。絞り込みのない `pull_request` だけで走る、ジョブと手順に `if:`・`continue-on-error:`・`needs:` がない、`ENGINE_REF` がワークフローの `env` に 1 行だけあって `knowledge-check.yml` と同じ版、最初の手順で 40 桁の SHA かを確かめる、Engine を `ref: ${{ env.ENGINE_REF }}`・`path: .engine` で取る、テストの手順が `CI: "true"`・`ENGINE_DIR` を渡して pytest を入れ、`unittest discover -s tools/knowledge-enforcement-check` を `-p`・`-k` で絞らずに走らせて 2 つのテストを（`check.py` の配線先の照合で）呼び、`shell: bash`・`set -o pipefail` で出力を `tee knowledge-enforcement-unittest.log` に残し、失敗を `|| true` などで握りつぶさない。そのすぐ後に、その出力の `skipped=` を見て落とす手順が bash で 1 つある。写しは `ENGINE_REF` の行のほかがひな形と同じであること（`ENGINE_REF` も `knowledge-check.yml` とそろえるので、実際は同じ中身）。ひな形を 22 とおりに書き換えた写しで、それぞれ外れることも確かめる。`skipped=` を見る手順は、作った出力（`skipped` なし・あり・失敗と一緒・テストが 0 件・出力がない）を渡して、飛ばしたかテストが走っていないときだけ落ちることを確かめる。

## まだ入っていないもの（建設者が入れる）

- `engine/knowledge-enforcement.patch` を Engine に当てること。製品の CI（`knowledge-check`）は Engine の固定の版（`ENGINE_REF`）の `knowledge_check.py` で `INDEX.md` を作り直して比べるので、当てた版に `ENGINE_REF` を上げるまで `INDEX.md` の担保の列は `deterministic` のまま。`ENGINE_REF` を上げる PR で `--index` を作り直す（`K-0004` などが `未配線` になる）。上げたあとは `test_engine.py` は差分を当てずに Engine の版をそのまま確かめるので、差分のファイルは消してよい
- `ci/knowledge-enforcement.yml` を `.github/workflows/` に写して PR ごとに走らせ、`engine-test` のジョブを branch protection の必須チェックに入れること。写すまでこの検査（`test_engine_patch.py` の `patch_stale` を含む）は未配線。写すときは `ENGINE_REF` を `knowledge-check.yml` と同じ版にそろえる（ずれていると `test_ci.py` が落ちる）
