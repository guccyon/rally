# 知見の担保の配線と効き目の検査

知見（`.factory-config/knowledge/K-*.yaml`）の `enforcement.solutions` の各項目が、CI や呼び出しに配線済みか（`wired`）と、本物の記録で効き目を確かめたか（`verified`）を決定論で（LLM を使わずに）見る。知見 `K-0004` の担保で、委員会の議決 `D-0015` と Issue #962 で作り、議決 `D-0021` と Issue #986 で `wired: true` の配線先（`wired_by`）を照らす検査を、議決 `D-0034` と Issue #1059 で `verified: true` の確認の記録（`verified_by`）を照らす検査を、議決 `D-0035` と Issue #1063 で Engine に移す差分を `ENGINE_REF` の版で必ず確かめる CI のジョブ（`engine-test`）とその形の検査（`test_ci.py`）を、議決 `D-0037` と Issue #1080 でそのジョブがテストを飛ばして緑にならない手順を、議決 `D-0039` と Issue #1088 で `protection.mjs` の解決策を手で走らせた記録だけでは `wired` にできない検査（下の「定期の配線の必須」）を、議決 `D-0043` と Issue #1103 で `K-0010` の `EngineFinishedTest` の解決策を PR ごとのジョブから `test_events.py` を呼ばないと `wired` にできない検査（下の「PR ごとの配線の必須」）を、議決 `D-0041` と Issue #1094 で解決策の `decided_by` が指す議決に同じ案があるかを照らす検査を、議決 `D-0042` と Issue #1100 で `K-0010` の衝突の続きのジョブの検査を 1 つのジョブから `test_watch.py`・`test_engine.py` を呼ばないと `wired` にできない検査（下の「呼ぶ検査の必須」）を足した。議決 `D-0066`（起票時の番号は `D-0062`）と Issue #1173 で、ひな形の `enforcement` のジョブが `check.py` の結果で落とす範囲に配線先と検査の置き場を足した（下の「CI のひな形」）。議決 `D-0044` と Issue #1106 で、「定期の配線の必須」の対象に知見 `K-0001` の解決策を足した。議決 `D-0048` と Issue #1129 で、「PR ごとの配線の必須」の対象に、この検査そのもの（`K-0004` の issue 986・1059 の解決策と `check.py`）を足した。議決 `D-0052` と Issue #1139 で、同じ対象に README の一覧を照らすテスト（`K-0004` の issue 1062 の解決策と `test_check.py`）を足した。議決 `D-0048` と Issue #1130 で、`verified_by` の形を Engine でも落とす差分を足した（下の「Engine に移す差分」）。議決 `D-0052` と Issue #1138 で、`.factory-config/knowledge/README.md` の記入済みと未記入の知見の一覧も記録と照らすテストを足した。議決 `D-0053` と Issue #1143 で、ひな形（写してあれば写しも）の `ENGINE_REF` を `knowledge-check.yml` と照らすテストを形の検査から分けて足した（下の `test_ci.py` の段）。議決 `D-0053` と Issue #1144 で、「PR ごとの配線の必須」の対象に `engine-test` のジョブ（`K-0004` の issue 1063・1144 の解決策と `test_engine_patch.py`）を足し、`test_ci.py` が写しのないときに飛ばさず落とす条件を足した（下の「CI のひな形」）。議決 `D-0055` と Issue #1152 で、`test_engine.py`・`test_engine_patch.py` が `CI=true` か `ENGINE_DIR` を渡したときに Engine がなくても飛ばさずに落ちることを子のプロセスで確かめるテスト（`NoSkipTest`）を `test_ci.py` に足した（下の `test_ci.py` の段）。議決 `D-0055` と Issue #1151 で、`test_engine.py` の変異体（`MUTANTS`）が 2 つの改変ちょうどであることを Engine なしで確かめるテストを足した。議決 `D-0057`（Issue の起票時の番号は `D-0055`）と Issue #1154 で、`unittest.TestCase` を定義するファイルがすべて `unittest discover` に拾われ、`engine-test` の出力に走った跡があるかを照らすテストと手順を足した（下の「CI のひな形」）。議決 `D-0059`（Issue #1158 の起票時の番号は `D-0056`）と Issue #1158 で、「定期の配線の必須」と「PR ごとの配線の必須」の両方の対象に、`protection.mjs` を定期と PR ごとに走らせる解決策（`K-0004` の issue 1158 の解決策）を足した。議決 `D-0060`（Issue #1160 の起票時の番号は `D-0056`）と Issue #1160 で、「定期の配線の必須」の対象に `sendback-protection.yml` のひな形の解決策（`K-0004` の issue 1087）を足し、issue 1087 と 1018 の解決策は `.github/workflows/sendback-protection.yml` のジョブでないと数えないようにした。議決 `D-0061`（Issue #1163 の起票時の番号は `D-0058`）と Issue #1163 で、「定期の配線の必須」の `if:` の読み方を、決まった形だけを許す許可リストにした。

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

書いていない項目は落とさず `false`（未記入）として数え、表示も `未配線` 側に倒す（`K-0002` の `config` のように実際は効いているかもしれない解決策も、書くまでは配線済みと出さない）。未記入の件数は出力の「未記入」に別に出すので、書いていないのか `false` と書いたのかは見分けられる。項目を書くのは足したり直したりした知見から順に（2026-11-08 の時点ですべての解決策に書いたのは `K-0001`・`K-0002`・`K-0004`・`K-0005`・`K-0010`・`K-0012`・`K-0014`。`K-0006`・`K-0009` の未記入の解決策は次にその知見を直す変更セットで書く）。この 2 つの一覧は `test_check.py` が `check.py` の数えた結果（解決策があって未記入が 0 件の知見と、未記入が 1 件以上の知見）と照らし、合わなければ落ちる（議決 `D-0035`、Issue #1062）。`.factory-config/knowledge/README.md` の同じ 2 つの一覧（「すべての解決策に書いたのは …」「… には未記入の解決策が残っていて」）も同じく順まで照らす（議決 `D-0052`、Issue #1138）。知見に項目を書き足したら、同じ変更セットで両方の一覧を直す。

## 落とすもの

- `wired` か `verified` が真偽でない（`"true"`・`yes` など）
- `wired` が `true` でないのに `verified` が `true`（配線してから本物の記録で確かめる）
- `verified` が `true` なのに、`verified_by` がない・空・列でない、項目が 1 つの参照でない・種類が `case`・`pr`・`event` でない、`case` が `C-NNNN` でないか `.factory-config/knowledge/cases/` にない、`pr` が正の整数でない、`event` が `<種類>:<id>` でない、`case` が 1 件もない（`pr`・`event` は手元で本物か照らせないので、事例記録に辿れることを求める）
- `wired` が `true` なのに、`wired_by` がない・空・項目が欠けている、`workflow` が `.github/workflows/` の直下でない（ひな形の `ci/*.yml` は配線ではない）、`workflow`・`job`・`runs` のどれかが実在しない、その job が `runs` の検査を呼んでいない（下の「配線先の照合」）
- `REQUIRED_SCHEDULE` の解決策（`K-0004` の issue 1018・1087・1158、`K-0001` の issue 1106）が `wired: true` なのに、`wired_by` に schedule のジョブからその検査（`tools/sendback-check/protection.mjs`、`tools/pr-body-check/protection.mjs`）を呼ぶ項目がない（issue 1018・1087 は `.github/workflows/sendback-protection.yml` のジョブに限る。下の「定期の配線の必須」）
- `REQUIRED_PR` の解決策（`K-0010` の issue 1035・1103、`K-0004` の issue 986・1059）が `wired: true` なのに、`wired_by` に pull_request のジョブからその検査（`tools/conflict-check/test_events.py`、`tools/knowledge-enforcement-check/check.py`）を呼ぶ項目がない（下の「PR ごとの配線の必須」）
- `REQUIRED_RUNS` の解決策（`K-0010` の issue 1034・1100）が `wired: true` なのに、`wired_by` に 1 つのジョブから `tools/conflict-check/test_watch.py`・`test_engine.py` をすべて呼ぶ項目がない（下の「呼ぶ検査の必須」）
- `decided_by` を書いた解決策で、`committee:D-NNNN` の形でない、`.factory-config/knowledge/committee/D-NNNN.yaml` がないか読めない、その議決の `enforcement` に同じ知見の id（`knowledge`）と同じ種類（`solution_kind` が解決策の `kind`）の案がない（`decision_mismatch`。下の「議決との照合」）

## 配線先の照合

`wired` は書き手の申告なので、配線していないのに `true` と書かれても見抜けなかった（議決 `D-0021`、Issue #986）。`check.py` は `wired_by` の job の手順（`run`）が `runs` を呼んでいるかを、`tools/sendback-check/order.mjs` と同じやり方で照らす。

- shell の注釈を落とし、行末の `\` でつないだ行を 1 行にして見る
- 道は手順の `working-directory`（なければ job の `defaults.run.working-directory`）と、手順の中の `cd` の下か根から引く。`$GITHUB_WORKSPACE`・`${{ github.workspace }}` は根にする
- ワークフロー・job・手順の `env` と shell の代入の変数（`$NAME`・`${NAME}`・`${{ env.NAME }}`）は値に置き換える
- `npm`・`pnpm` の `run`・`run-script`・`test` は、そのディレクトリ（`--prefix` も）の `package.json` の scripts の中身を辿る。シェルのラッパー（`.sh`・`.bash`）は中身を読んで辿る（どちらも 3 段まで）
- `runs` がテストのファイル（`test_*.py`・`*.test.mjs` など）のときは、そのディレクトリで `python3 -m unittest discover -s <ディレクトリ>` や、ファイルを名指ししない `node --test` を走らせる形も呼び出しに数える
- `order.mjs` は取りこぼすと見張れないので迷う形を呼ぶ側に数えるが、ここで見逃すと配線していない担保を配線済みと読ませるので、逆に迷う形は呼んでいない側に数える（scripts にない名前・読めないラッパー・別の job の手順・`uses:` の手順・別のワークフローを呼ぶ job は数えない）。字面で偽の `if:`（`false`・`${{ false }}`）の job と手順も走らないので数えない
- Engine の呼び出しだけで走る解決策は今の `wired_by` では書けない。書けるようになるまで `wired: false` のまま置く

## 定期の配線の必須

branch protection の必須のチェックは PR の `GITHUB_TOKEN` では読めないので、`tools/sendback-check/protection.mjs` は建設者が手で走らせて記録するだけになりやすく、必須のチェックが後で外されても誰も気づかない（議決 `D-0039`、Issue #1088）。`check.py` の `REQUIRED_SCHEDULE` に挙げた解決策（今は `K-0004` の issue 1018 の解決策と `tools/sendback-check/protection.mjs` の組と、`K-0001` の issue 1106 の解決策と `tools/pr-body-check/protection.mjs` の組（議決 `D-0044`。`tools/pr-body-check/ci/pr-body-protection.yml` を写すと満たす）と、`K-0004` の issue 1158 の解決策と `tools/sendback-check/protection.mjs` の組（議決 `D-0059`。下の「PR ごとの配線の必須」にも挙げる）と、`K-0004` の issue 1087 の解決策と `tools/sendback-check/protection.mjs` の組（議決 `D-0060`、Issue #1160））は、`wired: true` にするとき、`wired_by` に次をすべて満たす項目が 1 つ以上ないと落とす。

- `runs` がその検査で、上の「配線先の照合」でジョブがそれを呼んでいる
- `workflow` の `on.schedule` に空でない `cron` が 1 つ以上ある（`workflow_dispatch` だけ・`pull_request` だけでは足りない）
- ジョブと呼ぶ手順の `if:` が、無いか、`github.event_name == '<event>'` を `||` でつないだだけで `github.event_name == 'schedule'` を含む形（`${{ }}` で包んでもよい）である。それ以外の式（`&&` でほかの条件をつなぐ・`needs.*` や `always()` を見る・ほかの条件を `||` でつなぐ・`!=` や `contains()` で見る・真偽の値）は schedule で飛ばしうるので、`schedule` を含んでいても走らない側に数える許可リストにしている（議決 `D-0061`（Issue #1163 の起票時の番号は `D-0058`）、Issue #1163）
- `check.py` の `REQUIRED_SCHEDULE_WORKFLOW` に挙げた解決策（今は `K-0004` の issue 1087 と 1018）は、`workflow` が `.github/workflows/sendback-protection.yml` である（議決 `D-0060`（起票時の番号は `D-0056`）、Issue #1160）。ほかのワークフローの schedule のジョブから `protection.mjs` を呼ぶだけでは、ひな形 `tools/sendback-check/ci/sendback-protection.yml`（issue 1087 の解決策）を写していないのに `wired` と読ませてしまうため。ひな形をそのまま写した `protection` のジョブで通ること、同じ中身でも別の名前のワークフロー・schedule を外した写し・`protection-pr` のジョブで落ちること、issue 1158 と `K-0001` の解決策はワークフローの名前を問わないことは `test_check.py` が確かめる

`wired` が `true` でない間は見ない。

## PR ごとの配線の必須

`K-0010` の `EngineFinishedTest`（`tools/conflict-check/test_events.py`）は、`job.finished` に `agent_resumed` と `agent_tier` を出さない Engine の版では todo で飛ぶので、ひな形 `tools/conflict-check/ci/conflict-check.yml` を写さずにほかの検査（`test_ci.py` など）を走らせるジョブを書いただけで `wired` にすると、照らせない件数が見えないまま配線済みと読ませてしまう（議決 `D-0043`、Issue #1103）。`check.py` の `REQUIRED_PR` に挙げた解決策（今は `K-0010` の issue 1035 と 1103 の解決策と `tools/conflict-check/test_events.py` の組と、`K-0004` の issue 986 と 1059 の解決策と `tools/knowledge-enforcement-check/check.py` の組（議決 `D-0048`、Issue #1129）と、`K-0004` の issue 1062 の解決策と `tools/knowledge-enforcement-check/test_check.py` の組（議決 `D-0052`、Issue #1139）と、`K-0004` の issue 1063・1144 の解決策と `tools/knowledge-enforcement-check/test_engine_patch.py` の組（議決 `D-0053`、Issue #1144）と、`K-0004` の issue 1158 の解決策と `tools/sendback-check/protection.mjs` の組（議決 `D-0059`、Issue #1158））は、`wired: true` にするとき、`wired_by` に次をすべて満たす項目が 1 つ以上ないと落とす。

- `runs` がその検査で、上の「配線先の照合」でジョブがそれを呼んでいる（ひな形の `unittest discover -s tools/conflict-check` の形も `test_events.py` を呼んでいる側に数える）
- `workflow` の `on` に `pull_request` がある（`schedule`・`workflow_dispatch`・`push` だけでは足りない。必須のチェックは PR ごとに走るものにする）
- ジョブと呼ぶ手順の `if:` が pull_request で走らないと読めない形でない。字面で偽か、`github.event_name` を見るのに `== 'pull_request'` がない・`!= 'pull_request'` がある形は走らない側に数える

`wired` が `true` でない間は見ない。ひな形を `.github/workflows/conflict-check.yml` に写して必須のチェックにし、`ENGINE_REF` を `job.finished` に 2 つの欄を足した Engine に上げたら、`K-0010` の issue 1035 の項目を `wired: true` にし、`wired_by` に `{workflow: .github/workflows/conflict-check.yml, job: conflict-check, runs: tools/conflict-check/test_events.py}` を書く。

`K-0004` の issue 986（`wired_by` の照合）と issue 1059（`verified_by` の照合）の解決策は、この `check.py` そのものの担保なので、`ci/knowledge-enforcement.yml` を写して手で走らせたり定期に走らせたりするだけでは `wired` にできない（議決 `D-0048`、Issue #1129）。ひな形を `.github/workflows/knowledge-enforcement.yml` に写し、`enforcement` のジョブを branch protection の必須チェックに入れたら、2 つの項目を `wired: true` にし、`wired_by` に `{workflow: .github/workflows/knowledge-enforcement.yml, job: enforcement, runs: tools/knowledge-enforcement-check/check.py}` を書く（ひな形をそのまま写したジョブで通ることは `test_check.py` が確かめる。`engine-test` のジョブは `check.py` を呼ばないので数えない）。

`K-0004` の issue 1062 の解決策（上の「項目」の 2 つの一覧を `K-*.yaml` と照らす `test_check.py` のテスト）も、手で走らせただけでは一覧がずれた PR を止められないので、pull_request のジョブから `test_check.py` を呼ばないと `wired` にできない（議決 `D-0052`、Issue #1139）。ひな形を写して `enforcement` のジョブを必須チェックに入れ、一覧と `K-*.yaml` がずれた PR でそのジョブの `test_check.py` が落ちた実行を事例記録（`.factory-config/knowledge/cases/`）に残してから、その項目を `wired: true`・`verified: true` にし、`wired_by` に `{workflow: .github/workflows/knowledge-enforcement.yml, job: enforcement, runs: tools/knowledge-enforcement-check/test_check.py}` を、`verified_by` にその事例記録を書く（ひな形をそのまま写したジョブで通ること、`check.py` を指す項目・`test_check.py` を呼ばないジョブ・pull_request で走らないジョブで落ちることは `test_check.py` が確かめる）。

`K-0004` の issue 1063（`engine-test` のジョブで差分を `ENGINE_REF` の版で確かめる）と issue 1144（写しのないときに `test_ci.py` が落ちる）の解決策も、`engine-test` のジョブを PR ごとに走らせないと差分が当たらなくなった PR を止められないので、pull_request のジョブから `test_engine_patch.py` を呼ばないと `wired` にできない（議決 `D-0053`、Issue #1144）。ひな形の `engine-test` の `unittest discover -s tools/knowledge-enforcement-check` がこれを満たし、`check.py`・`test_check.py`・`test_ci.py` を名指しで呼ぶ `enforcement` のジョブでは満たさない。ひな形を写して `engine-test` を必須チェックに入れ、`engine/knowledge-enforcement.patch` が `ENGINE_REF` に当たらない PR で `engine-test` が落ちた実行を事例記録に残してから、2 つの項目を `wired: true`・`verified: true` にし、`wired_by` に `{workflow: .github/workflows/knowledge-enforcement.yml, job: engine-test, runs: tools/knowledge-enforcement-check/test_engine_patch.py}` を、`verified_by` にその事例記録を書く（ひな形をそのまま写したジョブで通ること、`enforcement` のジョブ・`check.py` を指す項目・テストを走らせないジョブ・pull_request で走らないジョブで落ちることは `test_check.py` が確かめる）。

`K-0004` の issue 1158 の解決策（`tools/sendback-check/protection.mjs` を定期と PR ごとに走らせ、必須のチェックが外れていれば落とす）は、定期だけでは外れている間の PR を止められず、PR ごとだけでは PR のない日に外されても気づけないので、「定期の配線の必須」と「PR ごとの配線の必須」の両方を満たさないと `wired` にできない（議決 `D-0059`（起票時の番号は `D-0056`）、Issue #1158）。`tools/sendback-check/ci/sendback-protection.yml` を `.github/workflows/sendback-protection.yml` に写し、`protection-pr` が PR で落ちる実行と `protection` が schedule で走った実行を Issue #948 と事例記録に残してから、その項目を `wired: true`・`verified: true` にし、`wired_by` に `{workflow: .github/workflows/sendback-protection.yml, job: protection, runs: tools/sendback-check/protection.mjs}` と `{workflow: .github/workflows/sendback-protection.yml, job: protection-pr, runs: tools/sendback-check/protection.mjs}` を、`verified_by` にその事例記録を書く（ひな形をそのまま写した 2 つのジョブで通ること、片方のジョブだけ・`pull_request` か `schedule` を外した写し・pull_request で走らない `protection-pr`・`protection.mjs` を呼ばない項目で落ちることは `test_check.py` が確かめる）。

## 呼ぶ検査の必須

`K-0010` の衝突の続きのジョブの検査は、ひな形 `tools/conflict-check/ci/conflict-check.yml` を `.github/workflows/conflict-check.yml` に写して配線する。見張り（`watch.py`）と Engine の本物の処理（`test_engine.py`）のテストを呼ばないジョブ（`test_policy.py` だけなど）を `wired_by` に書いて `wired` にしても、上の「配線先の照合」は書いた `runs` を呼ぶかしか見ないので通ってしまう（議決 `D-0042`、Issue #1100）。`check.py` の `REQUIRED_RUNS` に挙げた解決策（今は `K-0010` の issue 1034 と issue 1100 の解決策と、`tools/conflict-check/test_watch.py`・`tools/conflict-check/test_engine.py` の組）は、`wired: true` にするとき、`wired_by` に次を満たす項目がないと落とす。

- 同じ `workflow`（`.github/workflows/` の直下にあるファイル）・`job` で、`runs` が挙げた検査の項目がそれぞれある（検査ごとに 1 項目）
- そのジョブが、上の「配線先の照合」で挙げた検査をすべて呼んでいる（`unittest discover -s tools/conflict-check` の形も数える。別のジョブに分けて呼ぶ形は数えない）

ひな形をそのまま写したジョブ（`conflict-check`）はこれを満たす（`test_check.py` で確かめる）。`wired` が `true` でない間は見ない。

この照合が PR を止めるのは、`check.py` を `ci/knowledge-enforcement.yml` の `enforcement` のジョブから走らせたときだけ。そのジョブは記録を変えた PR に加えて `.github/workflows/` と `tools/conflict-check/` を変えた PR でも `check.py` の結果で落とす（下の「CI のひな形」。議決 `D-0066`、Issue #1173）。ひな形を `.github/workflows/knowledge-enforcement.yml` に、`tools/conflict-check/ci/conflict-check.yml` を `.github/workflows/conflict-check.yml` に写して両方を必須のチェックに入れ、issue 1034 と issue 1100 の項目を `wired: true` にして `wired_by` に `{workflow: .github/workflows/conflict-check.yml, job: conflict-check, runs: tools/conflict-check/test_watch.py}` と同じ workflow・job の `tools/conflict-check/test_engine.py` を書くのは建設者。

## 議決との照合

解決策の `decided_by` は書き手が Issue から写すので、Issue と議決の番号がずれると（Issue #1017・#1018・#1024・#1025 で続いた）別の議決を指したまま残る（議決 `D-0041`、Issue #1094）。`check.py` は `decided_by` を書いた解決策ごとに、指す議決のファイル（`.factory-config/knowledge/committee/D-NNNN.yaml`）があり、その `enforcement` の列に `knowledge` がその知見の `id`、`solution_kind` がその解決策の `kind` の案があるかを照らし、なければ `decision_mismatch` で落とす。`decided_by` を書いていない解決策（`K-0002` の `config` など）は見ない。知見そのものの `decided_by`（`enforcement` の外）は Engine の `knowledge_check.py` が議決の実在を見るので、ここでは見ない。

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

Engine（別のリポジトリ）に入れる変更を `engine/knowledge-enforcement.patch` に置いた（議決 `D-0016`・`D-0017`、Issue #965・#968。同じ案を 2 つの Issue で作ったので差分は 1 本にまとめた）。議決 `D-0021` の schema の案（Issue #987）も同じ案なので、新しい差分は作らずこの差分で満たす。議決 `D-0034` の schema の案（Issue #1060。`wired_by` を Engine の定義に書き、Engine の検査でも同じ形を落とす）もこの差分に足した。議決 `D-0048` の schema の案（Issue #1130。`wired_by` に加えて `verified_by` を Engine の定義に書き、`verified` が `true` のときは `verified_by` を必須・`wired` が `true` を条件とし、Engine の検査でも同じ形を落とす）も、`wired_by` の分は入っていたので `verified_by` の分をこの差分に足した。ワーカーは Engine を変えられないので、差分として置き、建設者が Engine に当てる。

- `definitions/knowledge.yaml` の `knowledge.enforcement` に `solution_flags`（`wired`・`verified`）と `index_labels`（上の表示の 3 つ）を足す
- `core-gates/knowledge_check.py` に `enforcement_label()`（`check.py` の `label()` と同じ線）を足し、`index_md()` の担保の列をその値にする。`check_item()` で、`wired`・`verified` が真偽でないときと、`wired` でないのに `verified` が `true` のときに落とす
- `definitions/knowledge.yaml` の `knowledge.enforcement` に `solution_wired_by`（`required_when: {wired: true}` と、`items` の `workflow`（`.github/workflows/` の直下のファイルの形）・`job`・`runs`）を足す
- `core-gates/knowledge_check.py` に `check_wired_by()` を足し、`check_item()` で `wired` が `true` の解決策の `wired_by` の形を落とす（列でない・空、項目が `workflow`・`job`・`runs` の空でない文字列でない、`workflow` が `.github/workflows/` の直下でない）。ファイル・job の実在と job が `runs` を呼ぶかは Engine では見ず、`check.py` の「配線先の照合」が照らす
- `definitions/knowledge.yaml` の `knowledge.enforcement` に `solution_verified_by`（`required_when: {verified: true}`・`requires: {wired: true}`・`items` の `case`（`C-NNNN`）・`pr`（正の整数）・`event`（`<種類>:<id>`）・`min_cases: 1`）を足す
- `core-gates/knowledge_check.py` に `check_verified_by()` を足し、`check_item()` で `verified` が `true` の解決策の `verified_by` を落とす（列でない・空、項目が 1 つの参照でないか種類が `case`・`pr`・`event` でない、`case` が `C-NNNN` でないか `cases/` にない、`pr` が正の整数でない、`event` が `<種類>:<id>` でない、`case` が 1 件もない）。`check.py` の「確認の記録の照合」と同じ線。`wired` でないのに `verified` が `true` は上の真偽の検査で落とす
- `core-gates/tests/test_knowledge_check.py` に、その 4 つの Engine の側のテストを足す

`test_engine.py` が、Engine のチェックアウトの写しに差分を当てて（Engine がもう取り込んでいれば当てずに）、担保の表示が `label()` と同じになること（手で選んだ組に加え、解決策の `wired`・`verified` の値（未記入・`true`・`false`・真偽でない値）の組を強さと `enforcement` の形ごとに総当たりで。議決 `D-0024`、Issue #1015。総当たりの効き目を確かめた 2 つの改変（`wired` の `"true"` を配線済みに数える、`verified` を真偽でなく値の有無で見る）を差分を当てた Engine の写しの `enforcement_label()` にさらに当てた変異体（`MUTANTS`）でも走らせ、どの変異体でも食い違いが 1 件以上出なければ落とす。置き換える字面が Engine になくなったときも落ちるので `MUTANTS` を今の書き方に合わせて直す。議決 `D-0037`、Issue #1079。`MUTANTS` の名前の集合がこの 2 つの改変ちょうどで、どれも字面を置き換えることは Engine のチェックアウトがなくても毎回確かめ、変異体が消されたり減らされたりしたら落とす。議決 `D-0055`、Issue #1151）、`INDEX.md` で `K-0004` が `未配線` と出ること、真偽の検査が落ちる・通ること、`wired_by` の形で落とすかが `check.py` の形の誤り（`wired_by` がない・項目が欠ける・`.github/workflows/` の直下でない）と `wired`・`wired_by` の値の組ごとに一致すること、`verified_by` で落とすかが `check.py` の `check_verification()` と `verified`・`verified_by` の値の組ごとに一致すること（議決 `D-0048`、Issue #1130）、定義の `solution_wired_by`・`solution_verified_by` が `check.py` と Engine の値とそろうこと、差分で Engine に足したテストが通ること（pytest があるとき）、このリポジトリの知見が作り直した `INDEX.md` で通ることを確かめる。Engine のチェックアウトは環境変数 `ENGINE_DIR` で渡す（なければリポジトリの隣の `../engine`）。どちらもなければ飛ばし、`ENGINE_DIR` を渡したときと CI（`CI=true`）では飛ばさずに落とす。Engine のファイルは、チェックアウトに `knowledge-check.yml` の `ENGINE_REF` の版があればその版から取る（チェックアウトの今の中身が別の版でも、製品の CI が使う版で確かめる）。その版がなければ `ENGINE_DIR` を渡したときと CI では落とし、`ENGINE_DIR` を渡さず `../engine` を使う手元ではチェックアウトの今の中身で確かめる。差分で足したテストは pytest がないと `ENGINE_DIR` を渡さない手元では飛ばすが、`ENGINE_DIR` を渡したときと CI では落とす（議決 `D-0035`・`D-0037`、Issue #1063・#1080）。差分が Engine に当たらなくなったら今の Engine に合わせて作り直す（Issue #1060 で `knowledge-check.yml` の `ENGINE_REF`（`9d3713a`）に合わせて作り直し、`ci/knowledge-enforcement.yml` の `ENGINE_REF` もその版にそろえた）。

```sh
ENGINE_DIR=/path/to/factory-engine python3 -m unittest discover -s tools/knowledge-enforcement-check
```

### 差分が古くなっていないか（`patch_stale`）

`test_engine.py` は Engine のチェックアウトの今の中身に差分を当てるので、製品の CI が使う版（`knowledge-check.yml` の `ENGINE_REF`）と違う版を渡すと、その版では当たらないことに気づけない（議決 `D-0024`、Issue #1014）。`test_engine_patch.py` は `.github/workflows/knowledge-check.yml` の `ENGINE_REF` を読み、`ENGINE_DIR` のチェックアウトからその版を名指しで取り出して `git apply --check` で差分を当て、次のときに `patch_stale` で落とす（GitHub Actions では `::error` も出す）。

- 差分が当たらず、しかも Engine 側にまだ入っていない（`git apply --reverse --check` も通らず、`core-gates/knowledge_check.py` に差分で足した `enforcement_label`・`check_wired_by`・`check_verified_by` もない）

差分で足した関数（`enforcement_label`・`check_wired_by`・`check_verified_by` のどれか）はあるのに `git apply --reverse --check` が通らない版（差分の一部だけが入った・入れたあと変わった）は、もう入っているとは数えずに `patch_partially_applied` で落とす（GitHub Actions では `::error` も出す。議決 `D-0036`、Issue #1077）。関数の名前があるだけでは、一部だけ入った版や名前だけが別の中身で入った版でも通ってしまうため。落ちたら差分を今の Engine に合わせて作り直す（全部入れて `ENGINE_REF` を上げたなら差分を消す）。

当たるか、もう入っていれば（`git apply --reverse --check` が通るときだけ）通る。チェックアウトに `ENGINE_REF` の版がないときは飛ばすが、`ENGINE_DIR` を渡したときと CI では落とす（`ci/knowledge-enforcement.yml` の `ENGINE_REF` が `knowledge-check.yml` とそろっていないとこれで落ちる）。判定そのものは、作った git の版（当たる・入っている・一部だけ入った・入れたあと変わった・どれでもない）で Engine なしに確かめる。`ci/knowledge-enforcement.yml` の `engine-test` のジョブで `test_engine.py` と一緒に走る。

## CI のひな形（`ci/knowledge-enforcement.yml`）

ジョブは 2 つ。

- `engine-test`: `ENGINE_REF` の版の Engine を取り、`python3 -m unittest discover -s tools/knowledge-enforcement-check`（`test_engine_patch.py`・`test_engine.py` を含むすべてのテスト）を `CI=true`・`ENGINE_DIR` を渡し、pytest を入れて走らせ、出力を `knowledge-enforcement-unittest.log` に残す。差分が `ENGINE_REF` に当たらない（`patch_stale`）か一部だけ入っている（`patch_partially_applied`）か、差分で足したテストや担保の表示の比べが落ちれば、どの PR でもジョブが落ちる。次の手順（`飛ばしたテストがないか`）が、その出力の要約の `skipped=` が 1 件以上か、走らせたテストの件数（`Ran N tests`）が出ていなければ `tests_skipped`・`unittest_not_run` で落とす（飛ばす条件の取りこぼしで緑にしない。議決 `D-0037`、Issue #1080）。写したらこのジョブを branch protection の必須チェックに入れて PR を止める（議決 `D-0035`、Issue #1063）
- `enforcement`: Engine の要らないテスト（`test_check.py`・`test_ci.py`）と `check.py` を走らせる。`check.py` の結果で落とすのは、PR が `.factory-config/knowledge/` か `tools/knowledge-enforcement-check/` を変えたときと、配線先（`.github/workflows/`）か `check.py` が呼ぶことを求める検査の置き場（`tools/conflict-check/`・`tools/sendback-check/`・`tools/pr-body-check/`）を変えたとき。記録を変えない PR でも、`wired: true` の解決策の配線先のジョブから検査を外す（`conflict-check.yml` から `test_watch.py`・`test_engine.py` を外すなど）と「呼ぶ検査の必須」などが落とすので、それを止められるようにした（議決 `D-0066`（起票時の番号は `D-0062`）、Issue #1173）。`test_ci.py` の `EnforcementScopeTest` が、ひな形の範囲の手順を偽の `git` で動かして、`check.py` の `REQUIRED_SCHEDULE`・`REQUIRED_SCHEDULE_WORKFLOW`・`REQUIRED_PR`・`REQUIRED_RUNS` に挙げた検査とワークフロー、`K-*.yaml` の `wired_by` の `workflow`・`runs` のどれを変えても範囲に入り、ほかの道だけなら入らないことと、`K-0010` の issue 1034 を `wired: true` にした一時のリポジトリで、ひな形の `conflict-check.yml` の写しなら落とす手順が通り、`test_policy.py` だけを呼ぶ形に変えると範囲の中では落ち、範囲の外では落とさないことを確かめる。検査の置き場を `check.py` の一覧に足したら、この範囲にも足す（足さないと `EnforcementScopeTest` が落ちる）

`unittest discover` は既定の `-p test*.py` に合わないファイル（`check_x.py`・`x_test.py`・`test-x.py`）と、`__init__.py` のない下のディレクトリのファイルを黙って拾わないので、テストを書き足しても走らないまま緑になりうる（議決 `D-0057`、Issue #1154）。`test_ci.py` の `DiscoveryTest` が、このディレクトリ（下も含む）で `unittest.TestCase` を継ぐクラスを定義する `.py` がすべてその形に合うかを照らす。`engine-test` は飛ばした件数の手順のすぐ後で `python3 -B tools/knowledge-enforcement-check/test_ci.py --log knowledge-enforcement-unittest.log` を走らせ、拾われないファイルか、出力に（飛ばさずに）走ったテストが 1 件もないファイル（読み込めずに `_FailedTest` になった・`setUpClass` で落ちた・すべて飛ばした）があれば `::error`（`tests_not_discovered`）を出して落とす。この手順があることも `test_ci.py` の形の検査で見る。

`test_ci.py` がひな形（写してあれば写しも）の `engine-test` の形を見て、外れたら落とす。絞り込みのない `pull_request` だけで走る、ジョブと手順に `if:`・`continue-on-error:`・`needs:` がない、`ENGINE_REF` がワークフローの `env` に 1 行だけあって `knowledge-check.yml` と同じ版、最初の手順で 40 桁の SHA かを確かめる、Engine を `ref: ${{ env.ENGINE_REF }}`・`path: .engine` で取る、テストの手順が `CI: "true"`・`ENGINE_DIR` を渡して pytest を入れ、`unittest discover -s tools/knowledge-enforcement-check` を `-p`・`-k` で絞らずに走らせて 2 つのテストを（`check.py` の配線先の照合で）呼び、`shell: bash`・`set -o pipefail` で出力を `tee knowledge-enforcement-unittest.log` に残し、失敗を `|| true` などで握りつぶさない。そのすぐ後に、その出力の `skipped=` を見て落とす手順が bash で 1 つある。写しは `ENGINE_REF` の行のほかがひな形と同じであること（`ENGINE_REF` も `knowledge-check.yml` とそろえるので、実際は同じ中身）。ひな形を 25 とおりに書き換えた写しで、それぞれ外れることも確かめる。`ENGINE_REF` は、形の検査とは別のテスト（`EngineRefTest`）でも、ひな形と（あれば）写しの `ENGINE_REF` の行が `knowledge-check.yml` の `ENGINE_REF` と同じ版の 1 行かを字面で照らし、片方だけ上げた PR を落とす（写しがなくても飛ばさない。議決 `D-0053`、Issue #1143）。`knowledge-check.yml` だけ・ひな形だけ・写しだけを上げた組で落ち、3 つをそろえて上げた組で通ることを一時ディレクトリに置いたファイルで確かめる。`skipped=` を見る手順は、作った出力（`skipped` なし・あり・失敗と一緒・テストが 0 件・出力がない）を渡して、飛ばしたかテストが走っていないときだけ落ちることを確かめる。この手順はひな形から走らせたときしか効かないので、飛ばす条件そのものも別のテスト（`NoSkipTest`）で照らす。`test_engine.py`・`test_engine_patch.py` を、Engine のない一時の置き場（隣の `../engine` もない）に写して子のプロセスで走らせ、`CI=true`・空のディレクトリかないディレクトリの `ENGINE_DIR`・Engine のファイルはあるが `ENGINE_REF` の版のない `ENGINE_DIR`（`CI=true` と一緒も）のどれでも、飛ばさずに Engine のチェックアウトの文で落ちることと、どちらも渡さないときだけ飛ぶこと（対照）を確かめる（議決 `D-0037` の ci_check の案を議決 `D-0055` で採り直したもの、Issue #1152）。`REQUIRED` から `CI` か `ENGINE_DIR` を外す・版がないときに飛ばす・`skipUnless` から `REQUIRED` を外す・チェックアウトがないときに黙って抜ける改変で落ちることを手元で確かめた。pytest がないときに落ちるかは Engine の本物の中身が要るのでここでは見ない。

写し（`.github/workflows/knowledge-enforcement.yml`）がないとき、`test_ci.py` は CI（`CI=true`）か、`K-*.yaml` の `wired: true` の解決策が `wired_by` に写しを書いているときは飛ばさずに落とす（建設者が写したあとで写しを消した・名前を変えた PR を止める。`engine-test` は `CI=true` で走るので、そこでも飛ばして緑にならない。議決 `D-0053`、Issue #1144）。どちらでもない手元では、まだ写していないとして飛ばす。写しがある・CI・記録が写しを書いている・どれでもないときの振る舞いは `test_ci.py` 自身が確かめる。

### Engine の先頭で定期に見る（`ci/knowledge-enforcement-head.yml`）

`test_engine_patch.py` は `ENGINE_REF` の版だけを見るので、`ENGINE_REF` が古いまま Engine の既定のブランチの先頭だけが進んで差分が当たらなくなっても気づけない（議決 `D-0036`、Issue #1076）。環境変数 `ENGINE_PATCH_REF` に版を渡すと、`test_engine_patch.py` は `ENGINE_REF` の代わりにその版に差分を当てる（`patch_stale` の文も `ENGINE_PATCH_REF <版>` と出す）。

`ci/knowledge-enforcement-head.yml`（`tools/sendback-check/ci/sendback-engine-head.yml` と同じ形）は、PR では走らせず 1 日 1 回と手動で、Engine を `ref` なしで既定のブランチの先頭から取り、`ENGINE_PATCH_REF` を先頭の版にして `test_engine_patch.py` を走らせる。

- 通ったら記録しない
- `patch_stale` なら `::error` を出し、Issue #968 に先頭の版・その時の `knowledge-check.yml` の `ENGINE_REF`（`test_engine_patch.py` の `engine_ref()` で読む。読めなければ「読めなかった」と書く）・実行の URL・当たらなかったところを記録する。最初の行の印（`<!-- knowledge-enforcement-head: <先頭の版> patch_stale -->`）が #968 にすでにあれば足さない。これまでの記録が読めなければ足す側に倒し、記録に失敗したらジョブを落とす
- `patch_stale` でない落ち方（Engine を取れなかった・テストが壊れた）は `::error`（`head_check_failed`）を出してジョブを落とすが、#968 には記録しない

`test_ci_head.py` が、ワークフローの形（定期と手動だけ・`ref` なし・`issues: write`）と、作った Engine と偽の `gh` で手順を動かした振る舞い、`.github/workflows/` の写しがあればひな形と同じことを確かめる。

```sh
ENGINE_DIR=/path/to/factory-engine ENGINE_PATCH_REF=$(git -C /path/to/factory-engine rev-parse HEAD) \
  python3 -m unittest discover -s tools/knowledge-enforcement-check -p test_engine_patch.py
```

## まだ入っていないもの（建設者が入れる）

- `engine/knowledge-enforcement.patch` を Engine に当てること。製品の CI（`knowledge-check`）は Engine の固定の版（`ENGINE_REF`）の `knowledge_check.py` で `INDEX.md` を作り直して比べるので、当てた版に `ENGINE_REF` を上げるまで `INDEX.md` の担保の列は `deterministic` のまま。`ENGINE_REF` を上げる PR で `--index` を作り直す（`K-0004` などが `未配線` になる）。上げたあとは `test_engine.py` は差分を当てずに Engine の版をそのまま確かめるので、差分のファイルは消してよい
- `ci/knowledge-enforcement.yml` を `.github/workflows/` に写して PR ごとに走らせ、`engine-test` と `enforcement` のジョブを branch protection の必須チェックに入れること。そのあとで `K-0004` の issue 986・1059 の解決策を `wired: true` にする（上の「PR ごとの配線の必須」。議決 `D-0048`、Issue #1129）。README の一覧と `K-*.yaml` がずれた PR で `enforcement` のジョブの `test_check.py` が落ちる実行を記録してから、issue 1062 の解決策を `wired: true`・`verified: true` にする（議決 `D-0052`、Issue #1139）。`engine/knowledge-enforcement.patch` が `ENGINE_REF` に当たらない PR で `engine-test` が落ちる実行を記録してから、issue 1063・1144 の解決策を `wired: true`・`verified: true` にする（上の「PR ごとの配線の必須」。議決 `D-0053`、Issue #1144）。写すまでこの検査（`test_engine_patch.py` の `patch_stale` を含む）は未配線。写すときは `ENGINE_REF` を `knowledge-check.yml` と同じ版にそろえる（ずれていると `test_ci.py` が落ちる）
- `ci/knowledge-enforcement-head.yml` を `.github/workflows/knowledge-enforcement-head.yml` にそのまま写すこと。写すまで Engine の先頭での `patch_stale` は定期に見ていない
