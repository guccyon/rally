# 衝突の続きのジョブの検査

既定ブランチとの衝突で supervisor が積む続きのジョブ（continue の reason=conflict）の積み方を、設定 `.factory-config/continuation.yaml` の `conflict` に照らして決定論で（LLM を使わずに）見る。知見 `K-0010`（`.factory-config/knowledge/K-0010.yaml`）の担保で、委員会の議決 `D-0013` と Issue #954 で作った。

PR #659（事例 `C-0005`）では、`CHANGELOG.md` の同じ場所の衝突のたびに、上限なしで tier-large の続きのジョブが 21 回積まれ、セッションを引き継いだ 1 回ごとの費用が $1.95 から $10.36 に増えた。この検査は、その積み方を設定の線にし、Engine の本物の処理と、起きた記録（制御プレーンのイベント列）がそれに合っているかを見る。

```sh
python3 -B -m unittest discover -s tools/conflict-check -p 'test_*.py' -v   # 線のテストと、イベント列の検査・見張りのテスト（どちらも Engine の job.finished の欄を含む）と、Engine の本物の処理のテストと、CI のひな形の形のテスト
python3 -B tools/conflict-check/policy.py [--config <continuation.yaml>]     # 設定を読めて形が正しいか（読めなければ終了コード 2）
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
python3 -B tools/conflict-check/events.py --events <イベントの JSON か JSONL> [--pr <PR の URL>] [--config <continuation.yaml>] [--json]
python3 -B tools/conflict-check/events.py --events tools/conflict-check/fixtures/c0005-pr659-events.json   # C-0005 の流れ。終了コード 1
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
| `agent_for_mechanical` | 衝突したファイルが `mechanical.paths`（`CHANGELOG.md` と `changelog.d/`）だけなのに、エージェントの tier（tier-large など）で解いた |

- 記録がなく分からない項目（agent_resumed・tier・衝突したファイル）はその rule を照らさず、PR ごとに件数を出す（`--json` では `unknown`）。`pr.conflicted` は衝突した head の数として出す
  - これは Engine が今の形（`job.finished` に `agent_resumed` と解き方の tier（`agent_tier`）を出さない版）のときだけの扱い（議決 `D-0030`、Issue #1035。下の「Engine の job.finished の欄のテスト」）
- 終了コード: 0 = 通る、1 = 外れた、2 = 記録か設定を読めなかった（JSON・JSONL として読めない、`name` のない件がある、`payload` がオブジェクトでない）
- 標準出力: 外れた回ごとに `<PR の URL> attempt=<回> job=<job_id> <rule>: 理由` と、PR ごとと全体の件数の行。`--json` は `{ "ok", "prs": [{ "pr_url", "attempts", "conflicted_heads", "unknown", "errors" }] }`
- `test_events.py` が、`fixtures/c0005-pr659-events.json`（C-0005 の流れ）で 3 つの rule がすべて落ちること（`over_limit` 17 件・`resumed_after_limit` 19 件・`agent_for_mechanical` 19 件）と、線どおりの積み方が通ることを確かめる
- `fixtures/c0005-pr659-events.json` は、事例記録（`.factory-config/knowledge/cases/C-0005.md`）にある事実だけを Engine のイベントの形に組み直したもの（制御プレーンの events はワーカーから読めない）。id・job_id・head_sha は仮の値。13 回目は衝突したファイルの記録を置かず、照らせない回にした

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

`over_limit` と `agent_for_mechanical` は、上限に数えるのをエージェントの回だけにした線（Issue #1010）にまだ合わせていない。`over_limit` は機械的に解いた回も含めて `continue.enqueued` の順番で数え、`agent_for_mechanical` は K-0002 の検査に通らずにエージェントで解いた回も外れとして出す。合わせるのは別のチケット。

### Engine の job.finished の欄のテスト（`test_events.py` の `EngineFinishedTest`）

「記録がない項目は照らさずに件数を出す」を、Engine が今の形のときだけに狭める（議決 `D-0030`（Issue #1035 の起票時の番号は `D-0029`。PR #1033）、Issue #1035。K-0004 の `engine.test.mjs` の todo と同じ形）。

- `engine_harness.py finished <Engine のチェックアウト>` が、Engine の `orchestrator/dispatcher/dispatcher.py` のうちワーカーを動かし終えたジョブに出す `job.finished`（payload に `cost_usd` を持つ `record_event`）の欄を構文木から読む（動かさない）。payload が辞書の式でないか、そういう呼び出しがなければ落ちる
- テストのイベント列（エージェントで解いた回・機械的に解いた回・引き継がない回で、記録がそろったもの）の `job.finished` を Engine が出す欄だけにし、`job.started` の `model_tier`（積んだ段。解き方の tier ではない）も外して `events.py` で組み、`agent_resumed` と `tier` の照らせない件数が 0 かを見る
- Engine が今の形（`agent_resumed` も `agent_tier` も出さない。`legacy_finished`）のときだけ `todo:` の理由で飛ばす。どちらかを出す版からは必須で、片方だけを出す版はもう片方が照らせずに落ちる。欄の名前を変えて出した版も今の形と見るので、その時は `FINISHED_FIELDS` を合わせる
- Engine のチェックアウトの渡し方と飛ばし方は `test_engine.py` と同じ（`ENGINE_DIR` を渡したときと `CI=true` では飛ばさずに落とす）。ひな形 `ci/conflict-check.yml` の `unittest discover` で一緒に走る
- 建設者が `job.finished` に `agent_resumed` と `agent_tier` を足して `ENGINE_REF` を上げたら、`legacy_finished` の todo の呼び出しを消す（`test_watch.py` の同じ todo も。下の「Engine の job.finished の欄で見張るテスト」）
- todo が黙って残らないよう、todo の理由に照らせない件数（`agent_resumed=3・tier=3` など）を添え、知見 `K-0010` の `review_by`（`.factory-config/knowledge/K-0010.yaml` から読む。2026-11-08 の時点で 2026-12-05）を過ぎても Engine が今の形なら、todo にせずに落とす（議決 `D-0043`、Issue #1104）。期限の日までは todo、翌日から落ちる（`core-gates/knowledge_check.py` の「過ぎた」と同じ）。`review_by` を読めないときも落ちる。直し方は、Engine を直して `ENGINE_REF` を上げるか、委員会で `K-0010` を再確認して `review_by` を動かす。今日の日付は環境変数 `CONFLICT_CHECK_TODAY`（`YYYY-MM-DD`）で差し替えられる。期限の判定と todo の理由の組み立ては `test_events.py` の `todo_until_review_by` の 1 か所に置き、`test_watch.py` の `EngineFinishedTest` もそれを呼ぶ

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

supervisor への配線（建設者が入れる）: `merge_step` の `ci_failure_step` の後（または `conflict_step` で続きのジョブを積んだ直後）に、その PR のイベント列（上の SQL）を引いて `watch.stoppages` と `watch.apply` を呼ぶ。停止が開けば `waiting_for_human` が `ci:` を見るので、その PR は同時に流す枠から外れる。

今の Engine との違い:

- `ci_failure_step` は、次の head で CI が通ったときと人の判断待ちの間に同じ `ci:` の停止を閉じる。閉じたあとは `stoppage.opened:<key>` の dedupe で同じ key を開き直せない（Engine の `ci:` の停止すべてに共通）。見張りの停止を閉じないようにするか、key を分けるかは配線のときに建設者が決める
- `conflict_step` は停止が開いていても衝突を解く続きのジョブを積む（`awaiting_decision` は `ci:` を見ない）。見張りの停止で積むのを止めるのも配線のときに入れる

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

写したら、K-0010 の ci_check の解決策（issue 1034）を `wired: true` にし、`wired_by` に `{workflow: .github/workflows/conflict-check.yml, job: conflict-check, runs: tools/conflict-check/test_watch.py}` と `runs: tools/conflict-check/test_engine.py` の 2 項目を書く。`tools/knowledge-enforcement-check/check.py` が、1 つのジョブからこの 2 つのテストをすべて呼ぶ項目がなければ落とす（議決 `D-0042`、Issue #1100。`check.py` の README の「呼ぶ検査の必須」）。
