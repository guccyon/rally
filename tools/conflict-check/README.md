# 衝突の続きのジョブの検査

既定ブランチとの衝突で supervisor が積む続きのジョブ（continue の reason=conflict）の積み方を、設定 `.factory-config/continuation.yaml` の `conflict` に照らして決定論で（LLM を使わずに）見る。知見 `K-0010`（`.factory-config/knowledge/K-0010.yaml`）の担保で、委員会の議決 `D-0013` と Issue #954 で作った。`events.py` の rule `missing_conflict_files` は知見 `K-0014`（衝突したファイルの記録）の担保（議決 `D-0049`、Issue #1135。`continue.enqueued` の欄 `conflict_files` を読むのは議決 `D-0101`（Issue #1208 の起票時の番号は `D-0073`）、Issue #1208）。`schema.py` を直近の窓のイベント列に対して定期に走らせるひな形 `ci/conflict-schema.yml` も `K-0014` の担保（議決 `D-0101`（Issue #1207 の起票時の番号は `D-0073`）、Issue #1207）。同じひな形の `conflict-record` のジョブ（`record-events.sql` で読んだ直近の窓に積まれた衝突の続きのジョブを `events.py --record-only` で照らす）も `K-0014` の担保（議決 `D-0076`（Issue #1214 の起票時の番号は `D-0074`）、Issue #1214）。`events.py` の rule `mechanical_without_verify` と、同じファイルの機械的な解消の回数は知見 `K-0015`（検査を通さない機械的な解消の繰り返し）の担保（議決 `D-0078`（Issue #1220 の起票時の番号は `D-0077`）、Issue #1220）。`test_open_pr.py` と `open_pr_harness.py`・`engine/open-pr-merge.patch` は知見 `K-0023`（PR を開く前の既定ブランチの取り込み）の担保（議決 `D-0149`、Issue #1383。下の「PR を開く前の既定ブランチの取り込み」）。

PR #659（事例 `C-0005`）では、`CHANGELOG.md` の同じ場所の衝突のたびに、上限なしで tier-large の続きのジョブが 21 回積まれ、セッションを引き継いだ 1 回ごとの費用が $1.95 から $10.36 に増えた。この検査は、その積み方を設定の線にし、Engine の本物の処理と、起きた記録（制御プレーンのイベント列）がそれに合っているかを見る。

```sh
python3 -B -m unittest discover -s tools/conflict-check -p 'test_*.py' -v   # 線のテストと、イベント列の検査・見張りのテスト（どちらも Engine の job.finished の欄を含む）と、Engine の本物の処理のテストと、CI のひな形の形のテスト
python3 -B tools/conflict-check/policy.py [--config <continuation.yaml>]     # 設定を読めて形が正しいか（読めなければ終了コード 2）
python3 -B tools/conflict-check/schema.py --events <イベントの JSON か JSONL> [--defs <定義>] [--since <時刻>] [--json]   # 衝突したファイルの列が載っているか（K-0014。下の「衝突したファイルの列の定義」）
python3 -B tools/conflict-check/events.py --events <イベントの JSON か JSONL> --record-only [--since <時刻>]   # 衝突したファイルがその場で記録されたかだけを見る（K-0014。下の「直近の窓に積まれた衝突の続きのジョブの定期の照合」）
```

依存は Python 3 と PyYAML（Engine の `core-gates/knowledge_check.py` と同じ）。機械的な解消の検査（`verify_mechanical`）とそのテストは git と node も使う（node か git がなければ飛ばす。CI（`CI=true`）では飛ばさない）。`test_engine.py` は bash・jq と Engine のチェックアウトも使う。

## 線（`.factory-config/continuation.yaml` の `conflict`）

| 欄 | 値 | 意味 |
|---|---|---|
| `max_attempts_per_pr` | 3 | 同じ PR の conflict の続きのジョブのうち、エージェントを呼び出して解いた回の上限。機械的に解いた回（`mechanical.verify` の検査に通った回）は数えない。上限に達したら次の衝突では積まない |
| `on_limit` | `stoppage` | 上限を超えて衝突したら積まず、衝突の停止（`ci:<PR の URL>`）を開いて人の判断待ちにする |
| `resume_session_until_attempt` | 1 | セッションを引き継いでよい（agent_resumed=true）のはこの回まで。2 回目以降は新しいセッション |
| `mechanical.paths` | `CHANGELOG.md`・`changelog.d/` | 衝突したファイルがすべてこれに当たるなら、エージェントを使わずに両方の行を残して解く（`git merge-file --union`）。末尾が `/` のものはディレクトリの下のすべて |
| `mechanical.agent_tier` | `none` | 機械的に解くときはエージェントを使わない（tier なし） |
| `mechanical.verify` | `tools/changelog-check/check.mjs` | 機械的に解いてコミットした結果を確かめる決定論の検査（K-0002）。`node <verify> --base <既定ブランチ> --head HEAD` が終了コード 0 のときだけ機械的な解消として扱う。通らなければ（読めなかった 2 も含む）エージェント（`agent_tier`）で解き、その回は上限に数える |
| `agent_tier` | `tier-large` | ほかのファイルも衝突したときにエージェントで解く段 |

判定は `policy.py` の次のもの。

- `enqueue_decision`（次の衝突で積むか、人に回すか）: 渡すのは、これまでにエージェントで解いた回の数（`agent_attempts`）
- `may_resume`（その回が引き継いでよいか）
- `resolution`（衝突したファイルと検査の結果からの解き方と tier、上限に数えるか（`counts`））: 衝突したファイルが `mechanical.paths` だけで、検査に通った（`verified=True`）かまだ検査していない（`None`）なら機械的（数えない）。検査に通らなかった（`False`）か、ほかのファイルも衝突したならエージェント（数える）
- `verify_mechanical`（機械的に解いた結果を `mechanical.verify` の検査にかける）

`test_policy.py` は、次のことを確かめる。

- C-0005 と同じ CHANGELOG だけの衝突が 20 回続いても、機械的に解けて検査に通っていれば毎回積み、人の判断待ちに上がらない
- 機械的に解いた回を挟んでも、エージェントで解いた 3 回のあとの衝突で停止（`stoppage`）にする。CHANGELOG だけでも検査に通らずエージェントで解いた回は数える
- 両方の行を残す解き方（`git merge-file --union`）と K-0002 の検査を一緒に（議決 `D-0023`、起票時の番号は `D-0018`）。一時の git リポジトリで本物の衝突を起こして解き、`check.mjs` にかける
  - 取りまとめどうしの `CHANGELOG.md` の衝突: 両方の行が残り、検査に通るので機械的な解消になる
  - PR が `[Unreleased]` を直接書き換えていた衝突: 検査に通らない（終了コード 1）のでエージェントで解き、上限に数える
  - 同じ名前の断片（`changelog.d/`）どうしの衝突: 両方の行が残り、検査に通る
  - 検査が読めなかった（終了コード 2）ときも、エージェントに回す

## 制御プレーンのイベント列の検査（`events.py`）

起きた記録（制御プレーンの events）を PR ごとに組んで、上の線に照らす。委員会の議決 `D-0023`（Issue #972 の起票時の番号は `D-0018`）と Issue #972 で作った。線は `policy.py` の `enqueue_decision`・`may_resume`・`resolution` をそのまま使う。

```sh
python3 -B tools/conflict-check/events.py --events <イベントの JSON か JSONL> [--pr <PR の URL>] [--config <continuation.yaml>] [--verify <検査の結果の JSON>] [--json]
python3 -B tools/conflict-check/events.py --events tools/conflict-check/fixtures/c0005-pr659-events.json   # C-0005 の流れ。終了コード 1
python3 -B tools/conflict-check/events.py --events tools/conflict-check/fixtures/c0007-pr290-pr295-events.json   # C-0007 の流れ。missing_conflict_files で終了コード 1
python3 -B tools/conflict-check/events.py --events tools/conflict-check/fixtures/c0008-pr678-events.json --verify tools/conflict-check/fixtures/c0008-pr678-events.json   # C-0008 の流れ。mechanical_without_verify で終了コード 1
```

PR ごとに `continue.enqueued`（reason=conflict）の順に 1 件ずつ、何回目か・agent_resumed・tier・衝突したファイルを組む。`job.started`・`job.finished`・`conflict.files` は `job_id` で結ぶ。

| 項目 | 読むところ |
|---|---|
| 何回目か | PR ごとの `continue.enqueued`（reason=conflict）の順番。`payload.attempt` のほうが大きければそれ（イベント列が途中から始まっていても下回らない） |
| agent_resumed | `continue.enqueued` か `job.finished` の `payload.agent_resumed`（真偽） |
| tier | `continue.enqueued` か `job.finished` の `payload.agent_tier` があればそれ。なければ `job.finished` の `cost_usd` が 0 の回は `none`（エージェントを使わなかった。worker の `entrypoint.sh` は機械的に解いた回の費用を 0 にする）。ほかは `job.started` の `model_tier` |
| 衝突したファイル | `conflict.files` の `payload.files`（dispatcher がワーカーのログから記録する。機械的に解く前に衝突していたファイル） |

| rule | 外れる記録 |
|---|---|
| `over_limit` | 同じ PR で `max_attempts_per_pr` を超えて積まれた（今の線では 4 回目以降） |
| `resumed_after_limit` | `resume_session_until_attempt` より後の回（今の線では 2 回目以降）で agent_resumed=true |
| `agent_for_mechanical` | 衝突したファイルが `mechanical.paths`（`CHANGELOG.md` と `changelog.d/`）だけなのに、エージェントの tier（tier-large など）で解いた。その head が `mechanical.verify` の検査に落ちる（`--verify`）なら、エージェントで解くのが線どおりなので外れにしない |
| `mechanical_without_verify` | PR の head が `mechanical.verify` の検査（`tools/changelog-check/check.mjs`。`CHANGELOG.md` を直接書き換えていると落ちる）に落ちるのに、`mechanical.agent_tier`（`none`。機械的）で衝突を解いた（知見 `K-0015` の担保。下の「検査を通さない機械的な解消の検査」） |
| `missing_conflict_files` | `continue.enqueued`（reason=conflict）の時点で、同じ `job_id` の `conflict.files` が載っていない。ないか、backfill で後から足されたもの（`payload.source` が `backfill`）か、ジョブが動き出したあと（`job.started` か `job.finished` より後）に記録されたものしかない（知見 `K-0014` の担保。下の「衝突したファイルの記録の検査」） |

- 記録がなく分からない項目（agent_resumed・tier・衝突したファイル）はその rule を照らさず、PR ごとに件数を出す（`--json` では `unknown`）。`pr.conflicted` は衝突した head の数として出す
  - これは Engine が今の形（`job.finished` に `agent_resumed` と解き方の tier（`agent_tier`）を出さない版）のときだけの扱い（議決 `D-0030`、Issue #1035。下の「Engine の job.finished の欄のテスト」）
- 終了コード: 0 = 通る、1 = 外れた、2 = 記録か設定を読めなかった（JSON・JSONL として読めない、`name` のない件がある、`payload` がオブジェクトでない、`--verify` の形が違う）
- 標準出力: 外れた回ごとに `<PR の URL> attempt=<回> job=<job_id> <rule>: 理由` と、同じファイルの機械的な解消が上限を超えた PR の回数の行と、PR ごとと全体の件数の行。`--json` は `{ "ok", "prs": [{ "pr_url", "attempts", "conflicted_heads", "unknown", "errors", "mechanical": { "repeats", "unverified" } }] }`
- `test_events.py` が、`fixtures/c0005-pr659-events.json`（C-0005 の流れ）で 3 つの rule がすべて落ちること（`over_limit` 17 件・`resumed_after_limit` 19 件・`agent_for_mechanical` 19 件）と、線どおりの積み方が通ることを確かめる
- `fixtures/c0005-pr659-events.json` は `missing_conflict_files` でも 20 回すべて落ちる（13 回目は記録がなく、ほかの回は今の Engine と同じく `job.finished` のあとに記録している）
- `fixtures/c0005-pr659-events.json` は、事例記録（`.factory-config/knowledge/cases/C-0005.md`）にある事実だけを Engine のイベントの形に組み直したもの（制御プレーンの events はワーカーから読めない）。id・job_id・head_sha は仮の値。13 回目は衝突したファイルの記録を置かず、照らせない回にした

### 衝突したファイルの記録の検査（`missing_conflict_files`）

衝突を解く続きのジョブに、衝突したファイルがその場で記録されていない回を落とす。知見 `K-0014`（記録がないと、同じファイルの衝突が繰り返している型が集計で見えない）の担保で、委員会の議決 `D-0049`（PR #1127）と Issue #1135 で作った。

- 線は議決 `D-0049` の test の案どおり「`continue.enqueued`（reason=conflict）の時点で載っているか」。同じ `job_id` の `conflict.files` が、そのジョブの `job.started`・`job.finished` より前（イベント列の順。`id` があれば `id` の順）にあればその場で載ったとみなす
- `continue.enqueued` の payload に衝突したファイルの欄 `conflict_files`が `conflict-files.yaml` の定義の形（空でない文字列の列で 1 件以上）で載っていれば、積んだその場の記録として通し、衝突したファイルにもその列を使う（`conflict.files` より先に見る）。形の判定は `schema.py` の `field_error` を使う（`events.py` では判定を持たない。定義を読めなければ終了コード 2）。形どおりでない（空の列・null・文字列の列でない）ときは載っていないとみなし、`conflict.files` で見る（議決 `D-0101`、Issue #1208。Engine が `engine/conflict-files.patch` どおりに欄を載せたあとも、この検査が落とし続けないように）
- 外れにするのは、`conflict.files` がない回、`payload.source` が `backfill`（`factoryctl learn backfill-conflicts` で後から埋めた）のものしかない回、ジョブが動き出したあと（`job.started` か `job.finished` より後）に記録されたものしかない回。`source` のない記録は位置だけで見る。衝突がなかった回の空の列（`files: []`）は、その場で載っていれば通る
- 終わっていない回・終わらずに止まった回（`job.finished` がない回）も照らす。積んだ時点で載っているはずなので、終わるのを待たない。積んだだけでまだ動き出していない回も、載っていなければ落ちる
- backfill の中身（衝突したファイル）は、上の 3 つの rule（`agent_for_mechanical` など）にはそのまま使う
- `continuation.yaml` の線ではないので、`check` ではなく `record_check` が出す。`events.py` のコマンドの終了コード・標準出力・`--json` の `errors` には一緒に出るが、`watch.py` は `check` だけを使うので、記録の欠けだけでは停止を開かない
- `test_events.py` の `MissingConflictFilesTest` が確かめること: `fixtures/c0007-pr290-pr295-events.json`（C-0007 の流れ）が落ちる（PR #290 のジョブ 309 は backfill の `conflict.files` だけ、PR #295 のジョブ 318・325・338 は記録なし。4 件）。`conflict.files` を `continue.enqueued` のすぐ後ろに載せた列と、`conflict.files` を外して `continue.enqueued` に `conflict_files` を載せた列は通る（形どおりでない `conflict_files` は落ちる）。今の Engine の形（ジョブの終わりに `worker_log` で記録）の回と、終わらずに止まった回で載っていないものは落ちる
- `test_events.py` の `ConflictFilesShapeParityTest` が、`conflict_files` の形の組（通る: 空でない文字列の列で 1 件以上。落ちる: 欄がない・null・空の列・空の文字列や空白だけの要素・文字列でない要素や列でない値）を、`continue.enqueued`（reason=conflict）に載せてほかの記録を置かない列で `schema.py`（`check`）と `events.py` の `missing_conflict_files`（`record_check`）の両方に通し、通る・落ちるがそろって期待どおりであることを確かめる。関数の呼び出しとコマンド（`schema.py --json` と `events.py --record-only --json` が落とす PR の組）の両方で見る。判定の関数は `schema.py` の `field_error` 1 つだが、片方の入口（`schema.check` の null の扱い、`events._inline_files` の読み方）だけを直して 2 つの解決策が食い違うと落ちる（知見 `K-0014` の担保。議決 `D-0112`、Issue #1297）
- `fixtures/c0007-pr290-pr295-events.json` は、事例記録（`.factory-config/knowledge/cases/C-0007.md`）にある事実だけを Engine のイベントの形に組み直したもの。id・head_sha・ブランチ名は仮の値。PR #295 の 3 回の tier・費用・agent_resumed は事例記録にないので置かない
- 2026-11-08 の Engine（`main` の `57069a8`）は、`conflict.files` を `continue.enqueued` や `pr.conflicted` には載せず、ジョブが終わってからワーカーのログから記録する（`dispatcher.py` の `process_learn.record_conflict_files`、`source: worker_log`）。この記録は積んだ時点のものではないので、今の Engine の記録はすべて `missing_conflict_files` で落ちる（K-0014 が指す欠けがまだ残っている）。Engine が積むときに `conflict.files` を記録する（議決 `D-0049` の schema の案。`continue.enqueued`・`pr.conflicted`・handoff に衝突したファイルを必須の項目にする）のは建設者。イベントを出してこの検査を走らせる配線も建設者
- 本物のイベント列に対しては、ひな形 `ci/conflict-schema.yml` の `conflict-record` のジョブが毎日と手動で走らせる（下の「直近の窓に積まれた衝突の続きのジョブの定期の照合」。議決 `D-0076`、Issue #1214）
- handoff（`handoff.saved`）はこの検査では見ない（積んだ時点ではなくジョブの終わりに出るため）。衝突の続きの `handoff.saved` に衝突したファイルが載っているかは `schema.py` が見る（下の「衝突したファイルの列の定義」。議決 `D-0076`、Issue #1213）。`pr.conflicted` の `conflict_files` も見ない（続きのジョブとは `job_id` で結べないため。欄があるかは `schema.py` が見る）

制御プレーンから出すときの例（その PR の conflict の続きのジョブと、それに結ぶジョブのイベント）:

```sql
select coalesce(json_agg(json_build_object('id', e.id, 'name', e.name, 'job_id', e.job_id, 'correlation_id', e.correlation_id, 'payload', e.payload) order by e.id), '[]')
from events e
where (e.name in ('pr.conflicted', 'continue.enqueued') and e.payload->>'pr_url' = :'pr_url')
   or (e.name in ('job.started', 'job.finished', 'conflict.files')
       and e.job_id in (select c.job_id from events c where c.name = 'continue.enqueued' and c.payload->>'reason' = 'conflict'
                                                         and c.payload->>'pr_url' = :'pr_url'));
```

2026-11-07 の Engine（`main` の `03974c0`）の記録との違い:

- agent_resumed は制御プレーンの events に載っていない（ワーカーのログの `[worker] agent resumed=` の行と、チケットのボリュームの `ticket.json` だけ）。載るまでは `resumed_after_limit` は照らせず、件数に出る。`job.finished` に `agent_resumed` を足すのは建設者。足した版からは照らせない件数が 1 以上で `EngineFinishedTest` が落ちる
- tier は `continuation.enqueue` が常に tier-large で積むので、機械的に解いた回は `cost_usd` が 0 のことでしか `none` と分からない。解き方の tier（`agent_tier`）を記録（`job.finished`）に足すのも建設者。足した版からは照らせない件数が 1 以上で `EngineFinishedTest` が落ちる
- この検査を supervisor の周期ごとに走らせる呼び出しは `watch.py`（下の「supervisor の周期ごとの見張り」）、PR ごとのテストの CI はひな形 `ci/conflict-check.yml` に置いた（議決 `D-0030`、Issue #1034）。どちらもまだ配線していない（Engine の supervisor と `.github/workflows/` は建設者）

`over_limit` は、上限に数えるのをエージェントの回だけにした線（Issue #1010）にまだ合わせていない。機械的に解いた回も含めて `continue.enqueued` の順番で数える。合わせるのは別のチケット。`agent_for_mechanical` は、`--verify` で検査に落ちると分かった head をエージェントで解いた回は外れにしない（Issue #1220）。検査の結果を渡さない回は、今までどおり K-0002 の検査に通らずにエージェントで解いた回も外れとして出す。

### 検査を通さない機械的な解消の検査（`mechanical_without_verify`）

PR の中に衝突の元（`CHANGELOG.md` の直接の書き換え）が残っているのに、`mechanical.verify` の検査を経ずに機械的な解消が繰り返された回を落とす。知見 `K-0015`（機械的な解消は上限に数えないので、原因の直しに切り替わらず繰り返す）の担保で、委員会の議決 `D-0078`（Issue #1220 の起票時の番号は `D-0077`。PR #1212）と Issue #1220 で作った。

- 制御プレーンの events には検査の結果が載らない（Engine は機械的に解く前に `mechanical.verify` を呼んでいない）ので、head ごとの `node tools/changelog-check/check.mjs --base <既定ブランチ> --head <head_sha>` の終了コードを `--verify` で渡す。形は `{ "<PR の URL>": { "<head_sha>": <終了コード> } }` か、それを `verify` に持つオブジェクト（fixture は `events` と同じファイルに置く）。head は `continue.enqueued` の `head_sha`（衝突した head）
- 外れにするのは、その head の終了コードが 0 でない（読めなかった 2 も含む。`policy.py` の `verify_mechanical` と同じ）のに、tier が `mechanical.agent_tier`（`none`）だった回。衝突がなかった回（`files: []`）は解いていないので見ない。`--verify` にない head は照らさず、機械的に解いた回のうち照らせなかった回の数を出す（`--json` では `mechanical.unverified`）
- あわせて、同じ PR で同じファイルの機械的な解消（tier が `none` の回の衝突したファイル）が `max_attempts_per_pr`（3 回）を超えたら、ファイルごとの回数を出す（`--json` では `mechanical.repeats`）。外れにはしない（線は機械的な解消を上限に数えないので、検査に通った回の繰り返しは線の中）。衝突したファイルの記録がない回は数えない
- `continuation.yaml` の線なので `check` が出し、`watch.py` の停止にも使われる（`watch.py` は `--verify` を渡さないので、今は照らさない）
- `test_events.py` の `MechanicalWithoutVerifyTest` が確かめること: `fixtures/c0008-pr678-events.json`（C-0008 の流れ）が 13 件落ち（ほかに `over_limit` 10 件と `missing_conflict_files` 13 件）、`CHANGELOG.md` 13 回と出る。同じ CHANGELOG.md だけの衝突でも、検査に落ちてエージェント（tier-large）で解き、3 回で停止した線どおりの積み方は通る。検査に通った head の機械的な解消と衝突のなかった回は落ちず、`--verify` の形が違えば終了コード 2
- `fixtures/c0008-pr678-events.json` は、事例記録（`.factory-config/knowledge/cases/C-0008.md`）にある事実（`CHANGELOG.md` だけの機械的な解消が 13 回、ジョブ 3414〜3822。直接の書き換えは review_fix 3873 まで残った）だけを Engine のイベントの形に組み直したもの。最初と最後のほかの job_id、id・head_sha は仮の値。機械的に解いた回は今の Engine と同じく `cost_usd` を 0 にし、衝突したファイルはジョブの終わりに `worker_log` で記録した形。agent_resumed は事例記録にないので置かない。`verify` の終了コード 1 は事例記録からの読みで、本物の検査の結果ではない
- 機械的に解く前に検査を呼ぶこと、その結果をイベントに出すか、この検査に渡して走らせる配線は建設者

### Engine の job.finished の欄のテスト（`test_events.py` の `EngineFinishedTest`）

「記録がない項目は照らさずに件数を出す」を、Engine が今の形のときだけに狭める（議決 `D-0030`（Issue #1035 の起票時の番号は `D-0029`。PR #1033）、Issue #1035。K-0004 の `engine.test.mjs` の todo と同じ形）。

- `engine_harness.py finished <Engine のチェックアウト>` が、Engine の `orchestrator/dispatcher/dispatcher.py` のうちワーカーを動かし終えたジョブに出す `job.finished`（payload に `cost_usd` を持つ `record_event`）の欄を構文木から読む（動かさない）。payload が辞書の式でないか、そういう呼び出しがなければ落ちる
- テストのイベント列（エージェントで解いた回・機械的に解いた回・引き継がない回で、記録がそろったもの）の `job.finished` を Engine が出す欄だけにし、`job.started` の `model_tier`（積んだ段。解き方の tier ではない）も外して `events.py` で組み、`agent_resumed` と `tier` の照らせない件数が 0 かを見る
- Engine が今の形（`agent_resumed` も `agent_tier` も出さない。`legacy_finished`）のときだけ `todo:` の理由で飛ばす。どちらかを出す版からは必須で、片方だけを出す版はもう片方が照らせずに落ちる。欄の名前を変えて出した版も今の形と見るので、その時は `FINISHED_FIELDS` を合わせる
- Engine のチェックアウトの渡し方と飛ばし方は `test_engine.py` と同じ（`ENGINE_DIR` を渡したときと `CI=true` では飛ばさずに落とす）。ひな形 `ci/conflict-check.yml` の `unittest discover` で一緒に走る
- 建設者が `job.finished` に `agent_resumed` と `agent_tier` を足して `ENGINE_REF` を上げたら、`legacy_finished` の todo の呼び出しを消す（`test_watch.py` の同じ todo も。下の「Engine の job.finished の欄で見張るテスト」）
- todo が黙って残らないよう、todo の理由に照らせない件数（`agent_resumed=3・tier=3` など）を添え、知見 `K-0010` の `review_by`（`.factory-config/knowledge/K-0010.yaml` から読む。2026-11-08 の時点で 2026-12-05）を過ぎても Engine が今の形なら、todo にせずに落とす（議決 `D-0043`、Issue #1104）。期限の日までは todo、翌日から落ちる（`core-gates/knowledge_check.py` の「過ぎた」と同じ）。`review_by` を読めないときも落ちる。直し方は、Engine を直して `ENGINE_REF` を上げる。Engine が今の形のまま `review_by` を前の版より後ろへ動かす PR は、`tools/knowledge-enforcement-check/check.py` の「review_by の先送りの照合」が落とす（延ばすだけで期限切れの失敗を先送りしない。議決 `D-0067`（起票時の番号は `D-0066`）、Issue #1178）。今日の日付は環境変数 `CONFLICT_CHECK_TODAY`（`YYYY-MM-DD`）で差し替えられる。期限の判定と todo の理由の組み立ては `test_events.py` の `todo_until_review_by` の 1 か所に置き、`test_watch.py` の `EngineFinishedTest` もそれを呼ぶ

## supervisor の周期ごとの見張り（`watch.py`）

`events.py` で外れた PR に、衝突の停止（`ci:<PR の URL>`）を開いて人の判断待ちにする。委員会の議決 `D-0030`（Issue #1034 の起票時の番号は `D-0029`。PR #1033）と Issue #1034 で作った。外れたかの判定は `events.py` の `records_from_events`・`check` をそのまま使う。

```sh
python3 -B tools/conflict-check/watch.py --events <イベントの JSON か JSONL> [--pr <PR の URL>] [--config <continuation.yaml>] [--json]
python3 -B tools/conflict-check/watch.py --events tools/conflict-check/fixtures/c0005-pr659-events.json   # C-0005 の流れ。PR #659 に停止を 1 つ開く。終了コード 1
```

- `stoppages(policy, events)`: `over_limit`・`resumed_after_limit`・`agent_for_mechanical` のどれかが出た PR ごとに、開く停止を 1 つ返す。key は `ci:<PR の URL>`、category は `ticket`（`ci_failure_step` が開く衝突の停止と同じ）、`detected_by` は `conflict-check`。`what` には外れた rule と最初に外れた回の理由、`job_id` はその回のジョブ、`correlation_id` はその回の `continue.enqueued` の `correlation_id`（イベント列にあれば）。外れのない PR と、記録がなく照らせなかっただけの PR には開かない
- `apply(cur, stoppage, found)`: Engine の `orchestrator/dispatcher/stoppage.py` の `open_stoppage` で開き、新しく開いた key を返す。同じ key がすでに開いていれば Engine が開き直さない（`stoppage.opened:<key>` の dedupe）ので、周期ごとに呼んでよい
- 終了コード: 0 = 開く停止がない、1 = 開く停止がある、2 = 記録か設定を読めなかった（`events.py` と同じ）。`--json` は `{ "stoppages": [...] }`
- `test_watch.py` が、C-0005 の流れで PR #659 に停止を 1 つ開くこと、3 つの rule のどれか 1 つでも開くこと、線どおりの積み方・照らせないだけの PR には開かないこと、次の周期で開き直さないことを確かめる。Engine の本物の `open_stoppage` に渡して動かすのは `test_engine.py` の (4)
- 「照らせないだけの PR には開かない」も、Engine が今の形（`job.finished` に `agent_resumed` と `agent_tier` を出さない版）のときだけの扱い（議決 `D-0042`、Issue #1101。下の「Engine の job.finished の欄で見張るテスト」）

### Engine の job.finished の欄で見張るテスト（`test_watch.py` の `EngineFinishedTest`）

`test_events.py` の `EngineFinishedTest` と同じ形で、`watch.py` が照らせずに停止を開かなかった外れの件数が 1 以上なら落とす（議決 `D-0042`、Issue #1101。K-0004 の `engine.test.mjs` の todo と同じ形）。

- Engine の `job.finished` の欄は `engine_harness.py finished` で読む。イベント列は、照らすのに `agent_resumed` が要る外れ（2 回目以降で引き継いだ。`resumed_after_limit`）の PR と、`tier` が要る外れ（`CHANGELOG.md` だけの衝突を tier-large で解いた。`agent_for_mechanical`）の PR と、線どおりの PR（`test_events.py` の `FULL`）を 1 本にし、`job.finished` を Engine が出す欄だけに、`job.started` の `model_tier` も外す（`test_events.py` の `engine_shaped`）
- `watch.stoppages` が外れの PR にその rule で停止を開かなかった件数（照らすのに要る項目ごと）が 0 かを見る。線どおりの PR に開いたら落ちる
- todo（`legacy_finished`）と Engine のチェックアウトの渡し方・飛ばし方は `test_events.py` の `EngineFinishedTest` と同じ（todo の理由に見逃した件数を添えて `test_engine.py` の `todo` で数え、`K-0010` の `review_by` を過ぎたら落とすのも同じ。議決 `D-0043`、Issue #1104）。片方だけを出す版はもう片方の外れを見逃して落ちる
- 建設者が `job.finished` に `agent_resumed` と `agent_tier` を足して `ENGINE_REF` を上げたら、`test_events.py` と `test_watch.py` の `legacy_finished` の todo の呼び出しを消す
- todo の前に、`watch.py` が `events.py` と別の扱いに変わっていないか（`finished_diverged`）を見て、食い違いがあれば Engine が今の形でも todo にせずに落とす（議決 `D-0066`（Issue #1174 の起票時の番号は `D-0062`）、Issue #1174）。同じイベント列で、`watch.stoppages` が開く停止の rule が PR ごとに `events.check` の rule と同じか、`watch.py` が見逃した外れはどれも `events.py` の照らせない件数（`events.unknown`）が 1 以上の項目か、を見る。イベント列には、`job.finished` の欄によらず照らせる外れの PR（`agent_resumed` と `agent_tier` を `continue.enqueued` に載せて 4 回積む。3 つの rule がすべて出る）も混ぜるので、今の形の Engine でも `watch.py` が rule を照らさなくなったり、照らせない回を外れにしたりすれば落ちる。`FinishedFieldsTest` が、Engine なしで 4 通りの欄（今の形・片方・両方）で食い違わないことと、`watch.py` の照らし方を変えた形ではどの欄でも食い違うことを確かめる

supervisor への配線（建設者が入れる）: `merge_step` の `ci_failure_step` の後（または `conflict_step` で続きのジョブを積んだ直後）に、その PR のイベント列（上の SQL）を引いて `watch.stoppages` と `watch.apply` を呼ぶ。停止が開けば `waiting_for_human` が `ci:` を見るので、その PR は同時に流す枠から外れる。

今の Engine との違い:

- `ci_failure_step` は、次の head で CI が通ったときと人の判断待ちの間に同じ `ci:` の停止を閉じる。閉じたあとは `stoppage.opened:<key>` の dedupe で同じ key を開き直せない（Engine の `ci:` の停止すべてに共通）。見張りの停止を閉じないようにするか、key を分けるかは配線のときに建設者が決める
- `conflict_step` は停止が開いていても衝突を解く続きのジョブを積む（`awaiting_decision` は `ci:` を見ない）。見張りの停止で積むのを止めるのも配線のときに入れる

## 衝突したファイルの列の定義（`conflict-files.yaml`・`schema.py`）

知見 `K-0014` の担保で、委員会の議決 `D-0049` と Issue #1136 で作った。衝突を解く続きのジョブを積むときに衝突したファイルを記録しないと、同じファイルの衝突が繰り返している型が集計で見えない（事例 `C-0007` では衝突の解消のジョブの記録に衝突したファイルがなく、事後の backfill（`conflict.files` の `source: backfill`）が要った）。そこで、衝突にかかわる 3 つの制御プレーンのイベントに、衝突したファイルの列 `conflict_files` を必須の欄として定義した。

| イベント | いつ要るか |
|---|---|
| `pr.conflicted` | いつも |
| `continue.enqueued` | `reason` が `conflict` のとき |
| `handoff.saved` | `continue_reason` が `conflict` のとき |

- 欄の形は `paths`（空でない文字列（ファイルの道）の列）で 1 件以上。欄がない・`null` は `missing_field`、空の列・文字列の列でないものは `bad_field` で落とす（要らないイベントでも、載せたなら形を見る）。ほかのイベント（`conflict.files` など）は照らさない
- 定義は `conflict-files.yaml`（Engine の `definitions/events.yaml` の `events` と同じ形の抜き書き。`required`・`required_when`・`fields`）。`schema.py` はこの定義か、`--defs` で渡した Engine の `events.yaml` でイベント列を照らし、外れがあれば終了コード 1、記録か定義を読めなければ 2
- Engine は別のリポジトリなので、Engine の定義に同じ欄を足す差分を `engine/conflict-files.patch` に置いた。`test_schema.py` の `EngineDefsTest` が、Engine のチェックアウトの `definitions/events.yaml` の写しに差分を当て（もう入っていればそのまま）、Engine の定義が `conflict-files.yaml` とそろうことを確かめる。Engine のチェックアウトの渡し方と飛ばし方は `test_engine.py` と同じ。ひな形 `ci/conflict-check.yml` の `unittest discover` で一緒に走る
- C-0005 の流れ（`fixtures/c0005-pr659-events.json`）は、今の Engine の形どおりイベントに衝突したファイルが載らず（`conflict.files` を `job_id` で結ぶだけ）、`pr.conflicted` と `continue.enqueued`（reason=conflict）のすべてが `missing_field` で落ちる
- handoff は `events.py` の `missing_conflict_files` では見ず、事例記録の fixtures にも `handoff.saved` がないので、`test_schema.py` の `EngineHandoffTest` が Engine の `orchestrator/dispatcher/dispatcher.py` の `handoff.saved` の payload の欄を `engine_harness.py handoff` で構文木から読み（動かさない）、`conflict-files.yaml` の `required_when` の欄（`continue_reason`）を Engine が出していること（欄の名前がずれると、衝突の続きの `handoff.saved` が `conflict_files` なしで黙って通るため）と、その欄の形のイベントが衝突の続きなら `conflict_files` がない・空の列で落ち、載せれば通り、衝突でない続きと続きでないジョブでは要らないことを確かめる（議決 `D-0076`（Issue #1213 の起票時の番号は `D-0074`。PR #1203）、Issue #1213）。payload が辞書の式でないか `handoff.saved` を出す呼び出しがなければ落ちる。Engine のチェックアウトの渡し方と飛ばし方は `EngineDefsTest` と同じ

配線は建設者が入れる。差分を Engine に当てることと、Engine がこの欄を載せてイベントを出すこと（`pr_events.py` の `pr.conflicted` は GitHub の `mergeable_state` からしか衝突を知らないので、衝突したファイルを求める手順が要る。`continuation.enqueue` と `dispatcher.py` の `handoff.saved` も同じ）、欄がない・空のイベントを出さないこと、`schema.py` を定期に走らせること（下の「直近の窓のイベント列の定期の照合」）が要る。入るまでは `K-0014` の解決策は `wired: false`・`verified: false`。

### 直近の窓のイベント列の定期の照合（`ci/conflict-schema.yml`・`window-events.sql`）

`schema.py` を、制御プレーンの直近の窓のイベント列に対して毎日と手動で走らせるワークフローのひな形。議決 `D-0101`（Issue #1207 の起票時の番号は `D-0073`。PR #1204）と Issue #1207 で作った。

```sh
psql "$DATABASE_URL" -X -q -A -t -v ON_ERROR_STOP=1 -v since=<差分を当てた時刻> -v window_days=8 -f tools/conflict-check/window-events.sql > conflict-events.json
python3 -B tools/conflict-check/schema.py --events conflict-events.json --since <差分を当てた時刻>
```

- `window-events.sql` は、`conflict-files.yaml` の 3 つのイベントのうち、`occurred_at` が差分を当てた時刻（`since`）と直近 `window_days` 日の始まりの遅いほうから後のものを、`schema.py` の `--events` に渡す JSON の配列（`id`・`name`・`occurred_at`・`job_id`・`correlation_id`・`payload`）で出す
- `schema.py` の `--since` は、`occurred_at` がそれより前のイベントを照らさない（SQL で絞ったあとも同じ線で絞る）。差分の前の Engine はこの欄を出さないので、前のイベントまで照らすとすべて `missing_field` で落ちる。`--since` を渡したのに `occurred_at` がない・時差つきの ISO 8601 で読めないイベントがあれば、照らすかを決められないので終了コード 2。`--json` は照らさなかった件数を `skipped` に出す
- ひな形の `conflict-schema` のジョブは、`CONFLICT_FILES_SINCE` が空か時差つきの ISO 8601 の時刻でなければ最初の手順で `conflict_files_since_missing`、シークレット `CONFLICT_EVENTS_DATABASE_URL` がないか events を読めなければ `events_unreadable` で落ちる（照らさずに緑にしない）。外れがあれば `schema.py` の終了コードで落ち、結果を手順の要約に出す。窓（`WINDOW_DAYS`）は 8 日（毎日の実行が 1 回飛んでも見落とさない幅）
- PR には紐づかない記録を見るので、PR ごとには走らせない（落ちても PR を止めない。直すのは Engine）
- `test_schema_ci.py` が、ひな形の形（`schedule` の cron と `workflow_dispatch`・`if:` と `continue-on-error` なし）、時刻の確かめ、シークレットのないとき、外れの終了コードを握りつぶさないこと、`window-events.sql` の照らすイベントが定義と同じこと、写しがあれば `CONFLICT_FILES_SINCE` の行のほかがひな形と同じでその値が時刻であることを見る。`WindowSqlTest` は SQL を `DATABASE_URL` の Postgres の使い捨ての schema で走らせ、窓の中の `pr.conflicted`・`continue.enqueued`・`handoff.saved` を fixture の行どおり（`id` で名指す）に出し、差分を当てる前・直近 8 日より前・ほかのイベントを出さないことを見る（SQL の誤りで空の列や欠けた列を照らして緑にならないように）。psql か `DATABASE_URL` がなければ飛ばすが、`CONFLICT_SQL_REQUIRED=true` のときは飛ばさずに `conflict_sql_unavailable` で落ちる（下の「CI への組み込み」の `conflict-sql` のジョブ）。どちらもひな形 `ci/conflict-check.yml` の `unittest discover` で一緒に走る

建設者が入れること（ワーカーは `.github/workflows/` を変えられず、Engine は別のリポジトリ）: 差分を Engine に当てて、Engine が衝突したファイルの列を載せてイベントを出すようにする。そのあとこのひな形を `.github/workflows/conflict-schema.yml` に写し、`CONFLICT_FILES_SINCE` に差分を当てた時刻を書く（写すときに変えてよいのはその行だけ）。events を読むだけの権限の接続先をシークレット `CONFLICT_EVENTS_DATABASE_URL` に置く。手動で走らせて通ったら、`K-0014` の issue 1136 と 1207 の解決策を `wired: true` にし、`wired_by` に `{workflow: .github/workflows/conflict-schema.yml, job: conflict-schema, runs: tools/conflict-check/schema.py}` と、`{workflow: .github/workflows/knowledge-enforcement.yml, job: enforcement, runs: tools/knowledge-enforcement-check/check.py}`（`tools/knowledge-enforcement-check/README.md` の「どの解決策にも求める `check.py` の配線」）を書く。`tools/knowledge-enforcement-check/check.py` が、`.github/workflows/conflict-schema.yml` の schedule でも走るジョブから `schema.py` を呼ぶ項目がなければ落とす（「定期の配線の必須」）。

### 直近の窓に積まれた衝突の続きのジョブの定期の照合（`ci/conflict-schema.yml` の `conflict-record`・`record-events.sql`）

`events.py` の rule `missing_conflict_files` を、制御プレーンの直近の窓に積まれた衝突の続きのジョブに対して毎日と手動で走らせるジョブ。上の `conflict-schema` と同じひな形に置き、`CONFLICT_FILES_SINCE`・`WINDOW_DAYS`・シークレット `CONFLICT_EVENTS_DATABASE_URL` を分け合う（Engine が積む時点で衝突したファイルを載せるのは同じ `engine/conflict-files.patch` なので、照らし始める時刻も同じ）。議決 `D-0076`（Issue #1214 の起票時の番号は `D-0074`。PR #1203）の ci_check の案と Issue #1214 で作った。

```sh
psql "$DATABASE_URL" -X -q -A -t -v ON_ERROR_STOP=1 -v since=<差分を当てた時刻> -v window_days=8 -f tools/conflict-check/record-events.sql > conflict-record-events.json
python3 -B tools/conflict-check/events.py --events conflict-record-events.json --since <差分を当てた時刻> --record-only
```

- `record-events.sql` は、`occurred_at` が差分を当てた時刻（`since`）と直近 `window_days` 日の始まりの遅いほうから後の `continue.enqueued`（reason=conflict）と、同じ `job_id` の `job.started`・`job.finished`・`conflict.files` を、`events.py` の `--events` に渡す JSON の配列（`window-events.sql` と同じ列）で `id` の順に出す。結ぶイベントは窓の前でも出す（積んだ時点より前か後かは `events.py` が `id` の順で見る）
- `events.py` の `--record-only` は `record_check`（`missing_conflict_files`）だけを出し、`continuation.yaml` の線（`over_limit` など。`check`）は照らさない（そちらは `watch.py` の見張り。ここで落とすと記録の欠けと積み方の外れが混ざる）
- `events.py` の `--since` は `schema.py` の `--since` と同じ線で、`occurred_at` がそれより前のイベントを組まない。`occurred_at` がない・時差つきの ISO 8601 で読めないイベントがあれば終了コード 2
- ジョブの手順と落ち方は `conflict-schema` と同じ（`CONFLICT_FILES_SINCE` が時刻でなければ `conflict_files_since_missing`、シークレットがないか events を読めなければ `events_unreadable`、外れがあれば `events.py` の終了コード 1 で落ち、結果を手順の要約に出す）。`needs` はなく、`conflict-schema` が落ちても照らす
- `test_events.py` の `RecordOnlySinceTest` が、C-0007 の流れ（`fixtures/c0007-pr290-pr295-events.json`）が `--record-only` で `missing_conflict_files` の 4 件だけで落ちること、同じ形の衝突でも `continue.enqueued` に `conflict_files` を載せた列は通ること（PR #290 の K-0010 の `agent_for_mechanical` も出さない）、`--since` の絞り方と読めない時刻を確かめる。`test_schema_ci.py` の `RecordTemplateTest` がジョブの形と手順の振る舞いを、`RecordSqlTest` が SQL を `DATABASE_URL` の Postgres の使い捨ての schema で走らせて、差分を当てる前・窓の外・衝突でない続き・ほかのイベントを出さず、出した列を `events.py` が組めることを見る（psql か `DATABASE_URL` がなければ飛ばす。`CONFLICT_SQL_REQUIRED=true` のときは `WindowSqlTest` と同じく落ちる）
- `test_events.py` の `SinceParityTest` が、同じイベント列と `--since` の時刻の組を `schema.py --since`（`conflict-schema`）と `events.py --record-only --since`（`conflict-record`）の両方に通し、外すイベントの件数（標準出力の「より前の N 件は照らさない」）・落とす PR の組（`--json`）・終了コードがそろって期待どおりであることを確かめる。組は、同じ時刻の書き方違いの `--since` 4 通りと、時差を直すと字面の大小と前後が逆になる時刻・ちょうど・1 秒前・1 ミリ秒前・前後に空白のある `occurred_at`、`occurred_at` がない・null・数・空・日付だけ・時差がない・時刻でない列（どちらも終了コード 2。`--since` がなければどちらも見ない）、時差つきの時刻でない・値がない `--since`（どちらも終了コード 2）。判定の関数は `schema.py` の `parse_time` と `before` だが、片方の入口（`main` での `--since` の読み方、`before` の結果の使い方）だけを直して 2 つのジョブの窓が食い違うと落ちる（知見 `K-0014` の担保。議決 `D-0113`、Issue #1299）

建設者が入れること: 上の `conflict-schema` と同じく、差分を Engine に当てて積むときに衝突したファイルを載せるようにしてから、ひな形を `.github/workflows/conflict-schema.yml` に写す（`conflict-record` のジョブも一緒に写る）。手動で走らせて通ったら、`K-0014` の issue 1214 の解決策を `wired: true` にし、`wired_by` に `{workflow: .github/workflows/conflict-schema.yml, job: conflict-record, runs: tools/conflict-check/events.py}` と、`{workflow: .github/workflows/knowledge-enforcement.yml, job: enforcement, runs: tools/knowledge-enforcement-check/check.py}` を書く。`tools/knowledge-enforcement-check/check.py` が、`.github/workflows/conflict-schema.yml` の schedule でも走るジョブから `events.py` を呼ぶ項目がなければ落とす（「定期の配線の必須」）。そのあと、C-0007 と同じ形の衝突（`CHANGELOG.md` だけの衝突の続きのジョブ）が窓に入った回で落ちなかったことを事例記録に残し、`verified: true` と `verified_by` を書く。

## Engine の本物の処理のテスト（`test_engine.py`）

`test_policy.py` が確かめるのは `policy.py` の線（参照の実装）。`test_engine.py` は `engine_harness.py` で Engine の本物の処理を動かし、その結果を線に照らす。関数と手順の中身には手を入れない。

- supervisor（積む側）: `orchestrator/dispatcher/supervisor.py` の `conflict_step`・`ci_failure_step` を取り出し、`continuation.py`（`enqueue`・`attempts`・`pending`）をそのまま読み込んで、DB（events・jobs・tickets）と停止・GitHub を差し替えて動かす。同じ PR が head ごとに衝突し続ける流れを supervisor の周期（`merge_step` と同じ順）で回す
- supervisor の続きのジョブの解き方は、`job.finished` の `agent_tier`・`cost_usd` に残す（`--resolved agent` は tier-large、`--resolved mechanical` は none と費用 0）
- worker（解く側）: `orchestrator/worker/entrypoint.sh` の衝突を機械的に解く手順（`SEEN_CONFLICTS=` の行から `[worker] continue` の echo の前まで）を、一時の git リポジトリで本物の衝突を起こしてから bash で動かす。変えるのは一時ファイルの置き場（`/work/cl.*`）だけ。機械的に解けたら取り込みをコミットし、`policy.py` の `verify_mechanical`（K-0002 の検査）にかけた結果を返す。`CHANGELOG.md` の衝突は `--changelog compiled`（PR が断片を取りまとめた。検査に通る）か `--changelog direct`（PR が `[Unreleased]` を直接書き換えた。検査に通らない）で起こす
- worker（引き継ぎ）: `entrypoint.sh` の `STATE_ARGS` の行を conflict と ci_failed の環境で動かし、エージェントに `--state-dir`（セッションの引き継ぎ）を渡すかを見る
- watch（見張り）: `orchestrator/dispatcher/stoppage.py` をそのまま読み込み、`watch.py` の `apply` から本物の `open_stoppage` を呼ぶ。偽の cursor は `on conflict (dedupe_key) do nothing` の insert を真似る

| テスト | 確かめること |
|---|---|
| (0) | 取り出す関数と手順がすべて Engine にある。名前や置き場が変わったら落ちる |
| (1) | エージェントで解いた回が上限（3 回）を超えた衝突では続きのジョブを積まず、衝突の停止（`ci:`）を開いて人の判断待ちにする。上限までは tier-large で積み、停止は開かない。機械的に解いた回は 6 回続いても数えず、停止を開かない |
| (2) | 2 回目以降の conflict の続きのジョブはセッションを引き継がない |
| (3) | `CHANGELOG.md` だけの衝突、`changelog.d/` を含む CHANGELOG だけの衝突はエージェントを使わずに解く（衝突も衝突の印も残らず、K-0002 の検査に通る）。解いた結果が検査に通らなければエージェントに渡す。ほかのファイルも衝突したらエージェントに渡す |
| (4) | `watch.py` の `apply` を `stoppage.py` の本物の `open_stoppage`（`psycopg` の `Jsonb` を差し替え、通知はログだけ）に渡し、C-0005 のイベント列で `ci:` の停止を category `ticket` で 1 つ開き、もう一度回しても開き直さない（議決 `D-0030`、Issue #1034） |

- Engine のチェックアウトは環境変数 `ENGINE_DIR` で渡す（なければリポジトリの隣の `../engine`）。どちらもなければ飛ばす。ただし `ENGINE_DIR` を渡したときと CI（`CI=true`）では飛ばさずに落とす（`tools/sendback-check/engine.test.mjs` と同じ。議決 `D-0011`）
- 偽の cursor が知らない問い合わせが来たとき、取り出した手順が落ちたときも落ちる（黙って通さない）

```sh
ENGINE_DIR=/path/to/factory-engine python3 -B -m unittest discover -s tools/conflict-check -p 'test_*.py' -v
python3 -B tools/conflict-check/engine_harness.py --check /path/to/factory-engine   # 取り出せるかだけを見る
```

### 今の Engine

2026-11-05 の Engine（`main` の `9fee9c2`）で走らせた結果（2026-11-07 に `main` の `2490db6` でも同じ）。外れている線は、Engine が今の形のときだけ `todo:` の理由で飛ばす（結果に出る）。Engine の振る舞いが今の形から変わったら、その版からは線に合わないと落ちる。

| 線 | 今の Engine | テスト |
|---|---|---|
| 回数の上限（3 回）と人への上げ | 外れる。conflict は head ごとに 1 回で、回数では切らない（ADR 0016、`continuation.py` の `enqueue`）。6 回の衝突で 6 回積み、停止は開かない | (1) の上限は `todo` |
| 機械的に解いた回は上限に数えない | 合う（回数で切らないので）。上限を入れるときに `continue.enqueued` の件数で数えると外れる（`continuation.attempts` のままでは機械的に解いた回も数える） | (1) の機械的な回は必須 |
| 2 回目以降はセッションを引き継がない | 合う。conflict のときは `--state-dir` を渡さない（2026-11-02 から。引き継がない分、1 回目も新しいセッション） | (2) は必須 |
| CHANGELOG.md だけの衝突を機械的に解く | 合う（2026-11-02 から） | (3) は必須 |
| changelog.d/ を含む CHANGELOG だけの衝突を機械的に解く | 外れる。機械的に解くのは衝突が `CHANGELOG.md` だけのときだけ | (3) の断片は `todo` |
| 機械的に解いた結果を K-0002 の検査で確かめる | 外れる。両方の行を残して解いたら、検査にかけずにそのまま機械的な解消にする（`[Unreleased]` を直接書き換えていた PR でも） | (3) の検査に通らない衝突は `todo` |

建設者が Engine を線に合わせる（`continuation.enqueue` か `conflict_step` に `max_attempts_per_pr` の上限を入れて、超えたら積まずに `ci_failure_step` の停止に任せる。数えるのはエージェントで解いた回だけ。`entrypoint.sh` の機械的な解消を `mechanical.paths` に広げ、解いた結果を `mechanical.verify` の検査にかけて、通らなければ衝突の状態に戻してエージェントに渡す）。そのあと `test_engine.py` の `legacy_supervisor`・`legacy_worker`・`legacy_unverified` と `todo_while_legacy` の呼び出しを消す。Engine がこの設定を読む配線（今は値を Engine 側の定数に写すことになる）も建設者が決める。

### CI への組み込み

PR ごとのワークフローのひな形を `ci/conflict-check.yml` に置いた（議決 `D-0030`、Issue #1034）。Engine を固定した版（`ENGINE_REF`。2026-11-07 に走らせた `main` の `2490db6`）でチェックアウトして `ENGINE_DIR` を向け、`CI=true` で `unittest discover` を走らせる（`test_policy.py`・`test_events.py`・`test_watch.py`・`test_engine.py`・`test_ci.py`）。`CI=true` と `ENGINE_DIR` を渡すので、fixtures が読めない・Engine のチェックアウトがない・`engine_harness.py` が関数や手順を取り出せない・node か git がないときは飛ばさずに落ちる。Engine が今の形のときだけの `todo` は飛ばしとして結果に出る。

todo で飛ばしたテスト（`test_engine.py` の `todo_while_legacy` と `test_events.py`・`test_watch.py` の `EngineFinishedTest`）は `test_engine.py` の `todo` で数え、テストが終わったあと標準エラーに `conflict-check: todo で飛ばしたテスト N 件` の行と 1 件ずつの中身を出す。GitHub Actions（`GITHUB_ACTIONS=true`）では 1 件ずつ `::warning title=conflict-check todo::` の注記にもする（`OK (skipped=N)` に紛れて黙って残らないように。議決 `D-0043`、Issue #1104）。ほかの理由の飛ばし（写しがない・Engine のチェックアウトがない）は数えない

ワーカーは `.github/workflows/` を変えられないので、建設者が `.github/workflows/conflict-check.yml` に写す。写すときに変えてよいのは `ENGINE_REF` の行だけ。`test_ci.py` がひな形の形（絞り込みのない `pull_request`・`if:` と `continue-on-error` なし・`ENGINE_REF` が 40 桁の SHA で最初の手順で確かめる・`CI=true` と `ENGINE_DIR` を渡してテストを走らせる）と、写しがあれば `ENGINE_REF` の行のほかがひな形と同じかを見る。写したら `conflict-check` を必須のチェックにするのも建設者。写して `job.finished` に `agent_resumed` と `agent_tier` を足した Engine に `ENGINE_REF` を上げたら、`K-0010` の `EngineFinishedTest` の解決策（issue 1035）を `wired: true` にし、`wired_by` に `{workflow: .github/workflows/conflict-check.yml, job: conflict-check, runs: tools/conflict-check/test_events.py}` を書く。`tools/knowledge-enforcement-check/check.py` が、そのジョブが pull_request で走って `test_events.py` を呼んでいるかを照らし、なければ落とす（議決 `D-0043`、Issue #1103。`tools/knowledge-enforcement-check/README.md` の「PR ごとの配線の必須」）。

同じひな形のジョブ `conflict-sql` は、Postgres のサービスコンテナ（`postgres:16`。合言葉はジョブの中だけの使い捨て）と psql（`postgresql-client`）を用意して `DATABASE_URL` と `CONFLICT_SQL_REQUIRED=true` を渡し、`test_schema_ci.py` の `WindowSqlTest`・`RecordSqlTest` を `unittest discover -k` で走らせる（知見 `K-0014` の担保。議決 `D-0111`（PR #1294）、Issue #1295）。`conflict-check` のジョブには Postgres がないのでこの 2 つは飛ぶが、`conflict-sql` では psql か `DATABASE_URL` がなければ `setUpClass` が `conflict_sql_unavailable` で落とし、飛ばしで緑にならない。`test_schema_ci.py` の `SqlRequiredTest` がその落ち方を、`test_ci.py` の `SqlJobTest` がジョブの形（絞り込みなし・サービスコンテナ・psql を入れる手順・`DATABASE_URL` と `CONFLICT_SQL_REQUIRED`・`-k` の名前が今のテストの組の名前であること）と、`DATABASE_URL` がないと手順が落ちることを見る。写すときは `conflict-check` と一緒に `.github/workflows/conflict-check.yml` に写り、`conflict-sql` も必須のチェックにする（建設者）

写したら、K-0010 の ci_check の解決策（issue 1034）を `wired: true` にし、`wired_by` に `{workflow: .github/workflows/conflict-check.yml, job: conflict-check, runs: tools/conflict-check/test_watch.py}` と `runs: tools/conflict-check/test_engine.py` の 2 項目を書く。`tools/knowledge-enforcement-check/check.py` が、1 つのジョブからこの 2 つのテストをすべて呼ぶ項目がなければ落とす（議決 `D-0042`、Issue #1100。`check.py` の README の「呼ぶ検査の必須」）。

## PR を開く前の既定ブランチの取り込み（`test_open_pr.py`）

知見 `K-0023`（`.factory-config/knowledge/K-0023.yaml`）の担保（議決 `D-0149`（Issue #1383 の起票時の番号は `D-0148`。PR #1382）、Issue #1383）。Engine の `orchestrator/worker/entrypoint.sh` は、続きのジョブでは既定ブランチを取り込むが、新しい PR では取り込まずに push して PR を開く。実装の間に既定ブランチが進んでいると PR は開いた時点で衝突し、文脈を持たない衝突の解消のジョブ（tier-large）で解くことになる（事例 `C-0011`: PR #345 は開いて 4 秒後に `continue.enqueued`(reason=conflict) が出て、解消のジョブは実装より高かった）。

```sh
ENGINE_DIR=/path/to/factory-engine python3 -B -m unittest discover -s tools/conflict-check -p 'test_open_pr.py' -v
python3 -B tools/conflict-check/open_pr_harness.py order <entrypoint.sh>                                    # 新しい PR の道の中の fetch・取り込み・push・gh pr create の行の位置
python3 -B tools/conflict-check/open_pr_harness.py run <entrypoint.sh> --main conflict --agent leave        # 動かした結果（PR を開いたか・その時点で main の最新を含むか・終了コード）
```

Engine は別のリポジトリなので、Engine に当てる差分を `engine/open-pr-merge.patch` に置いた。新しい PR の道で、実装をコミットしたあと push と `gh pr create` の前に次を行う。

- origin の既定ブランチを fetch する（通信の失敗は 0・2・4・8 秒の間をおいて 4 回まで。取れなければ取り込まずに開く。前と同じで、衝突は続きのジョブで解く）
- 既定ブランチを取り込む（`git merge --no-edit origin/${DEFAULT_BRANCH}`）。衝突したら、同じエージェントのセッション（`STATE_ARGS` の `--state-dir`）で衝突だけを解かせる。費用と turns は実装の分に足す
- 衝突の印が残ったか、取り込みを中止された（既定ブランチの最新が HEAD に入っていない）ら、push も PR 作成もせず、`FACTORY_RESULT`（`pr_url` は null、`reason` は `conflict_unresolved`、`conflicts` は衝突したファイル）を出して終了コード 5 で終える（dispatcher は `failed`（`exit=5`）にする）。作業はボリュームの `diff.patch`（`save_handoff`）に残る

`open_pr_harness.py` が、`entrypoint.sh` の `stage_all` と新しい PR の道（`[worker] continued pr=` の echo のあとの `else` から、`gh pr create` のあとの最初の `fi` の前まで）をそのまま bash で動かす。一時の bare リポジトリを origin にして既定ブランチ（main）を浅く（`--depth 1`）クローンし、作業のブランチに実装の変更を置いてから、main を進める（`none` = 進めない、`advance` = ほかのファイル、`conflict` = 実装と同じ行）。`gh` は偽物で、`gh pr create` が呼ばれた時点の origin で PR のブランチが push 済みか・main の最新を含むか・衝突の印を含むかを記録する。エージェントも偽物で、`resolve` = 両方の行を残して解く、`leave` = 何もしない、`abort` = 取り込みを中止する。変えるのはファイルの置き場（`/work/` を一時ディレクトリに）だけ。bash・git・jq を使う。

| テスト | 確かめること |
|---|---|
| (0) | 新しい PR の道と `stage_all` を取り出せる。置き場が変わったら落ちる |
| (1) 取り込みが gh pr create より前にある | 新しい PR の道で、fetch → 既定ブランチの取り込み → push → `gh pr create` の順に行がある。main が進んだとき、`gh pr create` の時点で PR のブランチが main の最新を含み、エージェントは呼ばない |
| (2) 取り込みが衝突したまま PR を開かない | 衝突をエージェントが解けない・取り込みを中止したとき、`gh pr create` も push もせず、終了コード 0 以外と `reason: conflict_unresolved` で終える |
| (3) | 衝突は同じセッション（`--state-dir`）で 1 回解かせてから、main の最新を含み衝突の印のないブランチで開く |
| (4) | main が進んでいなければ、エージェントを呼ばずにそのまま開く |

- `PatchedEngineTest` は差分を Engine の `entrypoint.sh` の写しに当てて（もう入っていればそのまま）動かす。todo にしない。差分が当たらなければ落ちる
- `EngineTest` は Engine のチェックアウトの `entrypoint.sh` をそのまま動かす。今の Engine の形（新しい PR の道に fetch も取り込みもない）のときだけ、`test_engine.py` の `todo` で飛ばして件数に数え、`K-0023` の `review_by`（2026-12-11）を過ぎたら落とす（期限の日までは todo、翌日から落ちる。`CONFLICT_CHECK_TODAY` で今日を差し替えられる。`test_events.py` の `EngineFinishedTest` と同じ。議決 `D-0043`）。Engine が今の形から変わったら、その版からは todo にならず線に合わないと落ちる
- 2026-11-11 の Engine（`main` の `34a53af`）と `ENGINE_REF`（`2490db6`）のどちらでも、`EngineTest` の (1)〜(3) は todo（(0) と (4) は通る）、`PatchedEngineTest` はすべて通る。解けないときの `exit 5` を消した写しでは (2) が落ちることを確かめた
- Engine のチェックアウトの渡し方と飛ばし方は `test_engine.py` と同じ。ひな形 `ci/conflict-check.yml` の `unittest discover -s tools/conflict-check -p 'test_*.py'` で一緒に走る

配線は建設者が入れる。差分を Engine に当てて `ENGINE_REF` を上げること、ひな形を `.github/workflows/conflict-check.yml` に写して `conflict-check` を必須のチェックにすること。入るまでは `K-0023` の解決策は `wired: false`・`verified: false`。効き目は、配線の後の窓で `pr.opened` と `continue.enqueued`(reason=conflict) が数秒差の PR の件数を数えて確かめる（それまで `verified` は `false`）。
