# 衝突の続きのジョブの検査

既定ブランチとの衝突で supervisor が積む続きのジョブ（continue の reason=conflict）の積み方を、設定 `.factory-config/continuation.yaml` の `conflict` に照らして決定論で（LLM を使わずに）見る。知見 `K-0010`（`.factory-config/knowledge/K-0010.yaml`）の担保で、委員会の議決 `D-0013` と Issue #954 で作った。

PR #659（事例 `C-0005`）では、`CHANGELOG.md` の同じ場所の衝突のたびに、上限なしで tier-large の続きのジョブが 21 回積まれ、セッションを引き継いだ 1 回ごとの費用が $1.95 から $10.36 に増えた。この検査は、その積み方を設定の線にし、Engine の本物の処理と、起きた記録（制御プレーンのイベント列）がそれに合っているかを見る。

```sh
python3 -B -m unittest discover -s tools/conflict-check -p 'test_*.py' -v   # 線のテストと、イベント列の検査のテストと、Engine の本物の処理のテスト
python3 -B tools/conflict-check/policy.py [--config <continuation.yaml>]     # 設定を読めて形が正しいか（読めなければ終了コード 2）
```

依存は Python 3 と PyYAML（Engine の `core-gates/knowledge_check.py` と同じ）。`test_engine.py` は git・bash・jq と Engine のチェックアウトも使う。

## 線（`.factory-config/continuation.yaml` の `conflict`）

| 欄 | 値 | 意味 |
|---|---|---|
| `max_attempts_per_pr` | 3 | 同じ PR で積む conflict の続きのジョブの上限。数えるのは `continue.enqueued`（reason=conflict）の件数で、機械的に解いた回も数える |
| `on_limit` | `stoppage` | 上限を超えて衝突したら積まず、衝突の停止（`ci:<PR の URL>`）を開いて人の判断待ちにする |
| `resume_session_until_attempt` | 1 | セッションを引き継いでよい（agent_resumed=true）のはこの回まで。2 回目以降は新しいセッション |
| `mechanical.paths` | `CHANGELOG.md`・`changelog.d/` | 衝突したファイルがすべてこれに当たるなら、エージェントを使わずに両方の行を残して解く（`git merge-file --union`）。末尾が `/` のものはディレクトリの下のすべて |
| `mechanical.agent_tier` | `none` | 機械的に解くときはエージェントを使わない（tier なし） |
| `agent_tier` | `tier-large` | ほかのファイルも衝突したときにエージェントで解く段 |

判定は `policy.py` の `enqueue_decision`（次の衝突で積むか、人に回すか）・`may_resume`（その回が引き継いでよいか）・`resolution`（衝突したファイルからの解き方と tier）。

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
- 終了コード: 0 = 通る、1 = 外れた、2 = 記録か設定を読めなかった（JSON・JSONL として読めない、`name` のない件がある、`payload` がオブジェクトでない）
- 標準出力: 外れた回ごとに `<PR の URL> attempt=<回> job=<job_id> <rule>: 理由` と、PR ごとと全体の件数の行。`--json` は `{ "ok", "prs": [{ "pr_url", "attempts", "conflicted_heads", "unknown", "errors" }] }`
- `test_events.py` が、`fixtures/c0005-pr659-events.json`（C-0005 の流れ）で 3 つの rule がすべて落ちること（`over_limit` 17 件・`resumed_after_limit` 19 件・`agent_for_mechanical` 19 件）と、線どおりの積み方が通ることを確かめる
- `fixtures/c0005-pr659-events.json` は、事例記録（`.factory-config/knowledge/cases/C-0005.md`）にある事実だけを Engine のイベントの形に組み直したもの（制御プレーンの events はワーカーから読めない）。id・job_id・head_sha は仮の値。13 回目は衝突したファイルの記録を置かず、照らせない回にした

制御プレーンから出すときの例（その PR の conflict の続きのジョブと、それに結ぶジョブのイベント）:

```sql
select coalesce(json_agg(json_build_object('id', e.id, 'name', e.name, 'job_id', e.job_id, 'payload', e.payload) order by e.id), '[]')
from events e
where (e.name in ('pr.conflicted', 'continue.enqueued') and e.payload->>'pr_url' = :'pr_url')
   or (e.name in ('job.started', 'job.finished', 'conflict.files')
       and e.job_id in (select c.job_id from events c where c.name = 'continue.enqueued' and c.payload->>'reason' = 'conflict'
                                                         and c.payload->>'pr_url' = :'pr_url'));
```

2026-11-07 の Engine（`main` の `03974c0`）の記録との違い:

- agent_resumed は制御プレーンの events に載っていない（ワーカーのログの `[worker] agent resumed=` の行と、チケットのボリュームの `ticket.json` だけ）。載るまでは `resumed_after_limit` は照らせず、件数に出る。`job.finished` に `agent_resumed` を足すのは建設者
- tier は `continuation.enqueue` が常に tier-large で積むので、機械的に解いた回は `cost_usd` が 0 のことでしか `none` と分からない。解き方の tier（`agent_tier`）を記録に足すのも建設者
- この検査を PR ごと・supervisor の周期ごとに走らせる配線（イベントを出す手順と `.github/workflows/`）はまだない

## Engine の本物の処理のテスト（`test_engine.py`）

`test_policy.py` が確かめるのは `policy.py` の線（参照の実装）。`test_engine.py` は `engine_harness.py` で Engine の本物の処理を動かし、その結果を線に照らす。関数と手順の中身には手を入れない。

- supervisor（積む側）: `orchestrator/dispatcher/supervisor.py` の `conflict_step`・`ci_failure_step` を取り出し、`continuation.py`（`enqueue`・`attempts`・`pending`）をそのまま読み込んで、DB（events・jobs・tickets）と停止・GitHub を差し替えて動かす。同じ PR が head ごとに衝突し続ける流れを supervisor の周期（`merge_step` と同じ順）で回す
- worker（解く側）: `orchestrator/worker/entrypoint.sh` の衝突を機械的に解く手順（`SEEN_CONFLICTS=` の行から `[worker] continue` の echo の前まで）を、一時の git リポジトリで本物の衝突を起こしてから bash で動かす。変えるのは一時ファイルの置き場（`/work/cl.*`）だけ
- worker（引き継ぎ）: `entrypoint.sh` の `STATE_ARGS` の行を conflict と ci_failed の環境で動かし、エージェントに `--state-dir`（セッションの引き継ぎ）を渡すかを見る

| テスト | 確かめること |
|---|---|
| (0) | 取り出す関数と手順がすべて Engine にある。名前や置き場が変わったら落ちる |
| (1) | 上限（3 回）を超えた衝突では続きのジョブを積まず、衝突の停止（`ci:`）を開いて人の判断待ちにする。上限までは tier-large で積み、停止は開かない |
| (2) | 2 回目以降の conflict の続きのジョブはセッションを引き継がない |
| (3) | `CHANGELOG.md` だけの衝突、`changelog.d/` を含む CHANGELOG だけの衝突はエージェントを使わずに解く（衝突も衝突の印も残らない）。ほかのファイルも衝突したらエージェントに渡す |

- Engine のチェックアウトは環境変数 `ENGINE_DIR` で渡す（なければリポジトリの隣の `../engine`）。どちらもなければ飛ばす。ただし `ENGINE_DIR` を渡したときと CI（`CI=true`）では飛ばさずに落とす（`tools/sendback-check/engine.test.mjs` と同じ。議決 `D-0011`）
- 偽の cursor が知らない問い合わせが来たとき、取り出した手順が落ちたときも落ちる（黙って通さない）

```sh
ENGINE_DIR=/path/to/factory-engine python3 -B -m unittest discover -s tools/conflict-check -p 'test_*.py' -v
python3 -B tools/conflict-check/engine_harness.py --check /path/to/factory-engine   # 取り出せるかだけを見る
```

### 今の Engine

2026-11-05 の Engine（`main` の `9fee9c2`）で走らせた結果。外れている線は、Engine が今の形のときだけ `todo:` の理由で飛ばす（結果に出る）。Engine の振る舞いが今の形から変わったら、その版からは線に合わないと落ちる。

| 線 | 今の Engine | テスト |
|---|---|---|
| 回数の上限（3 回）と人への上げ | 外れる。conflict は head ごとに 1 回で、回数では切らない（ADR 0016、`continuation.py` の `enqueue`）。6 回の衝突で 6 回積み、停止は開かない | (1) の上限は `todo` |
| 2 回目以降はセッションを引き継がない | 合う。conflict のときは `--state-dir` を渡さない（2026-11-02 から。引き継がない分、1 回目も新しいセッション） | (2) は必須 |
| CHANGELOG.md だけの衝突を機械的に解く | 合う（2026-11-02 から） | (3) は必須 |
| changelog.d/ を含む CHANGELOG だけの衝突を機械的に解く | 外れる。機械的に解くのは衝突が `CHANGELOG.md` だけのときだけ | (3) の断片は `todo` |

建設者が Engine を線に合わせる（`continuation.enqueue` か `conflict_step` に `max_attempts_per_pr` の上限を入れて、超えたら積まずに `ci_failure_step` の停止に任せる。`entrypoint.sh` の機械的な解消を `mechanical.paths` に広げる）。そのあと `test_engine.py` の `legacy_supervisor`・`legacy_worker` と `todo_while_legacy` の呼び出しを消す。Engine がこの設定を読む配線（今は値を Engine 側の定数に写すことになる）も建設者が決める。

### CI への組み込み

ワーカーは `.github/workflows/` を変えられないので、建設者が足す。PR ごとに Engine を固定した版でチェックアウトして `ENGINE_DIR` を向け、上のコマンドを走らせる（`tools/sendback-check/ci/sendback-engine.yml` の Engine のチェックアウトと同じ形でよい）。
