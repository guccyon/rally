# 衝突の続きのジョブの検査

既定ブランチとの衝突で supervisor が積む続きのジョブ（continue の reason=conflict）の積み方を、設定 `.factory-config/continuation.yaml` の `conflict` に照らして決定論で（LLM を使わずに）見る。知見 `K-0010`（`.factory-config/knowledge/K-0010.yaml`）の担保で、委員会の議決 `D-0013` と Issue #954 で作った。

PR #659（事例 `C-0005`）では、`CHANGELOG.md` の同じ場所の衝突のたびに、上限なしで tier-large の続きのジョブが 21 回積まれ、セッションを引き継いだ 1 回ごとの費用が $1.95 から $10.36 に増えた。この検査は、その積み方を設定の線にし、Engine の本物の処理がそれに合っているかを見る。

```sh
python3 -B -m unittest discover -s tools/conflict-check -p 'test_*.py' -v   # 線のテストと、Engine の本物の処理のテスト
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
