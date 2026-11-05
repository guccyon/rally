"""Engine の本物の衝突の扱いを、DB・GitHub・エージェントを差し替えて動かす（知見 K-0010 の担保。議決 D-0013・Issue #954）。

test_engine.py から呼ぶ。policy.py の線（参照の実装）ではなく、Engine の次の処理をそのまま動かして結果を返す。

- supervisor（積む側）: orchestrator/dispatcher/supervisor.py の conflict_step・ci_failure_step と、continuation.py（enqueue・attempts・pending）。
  同じ PR が head ごとに衝突し続ける流れ（C-0005 の PR #659）を周期ごとに回し、積んだ続きのジョブ（attempt・model_tier）と、開いた停止を返す
- worker（解く側）: orchestrator/worker/entrypoint.sh の、衝突を機械的に解く手順（SEEN_CONFLICTS= から「[worker] continue」の echo の前まで）。
  一時の git リポジトリで本物の衝突を起こしてから動かし、残った衝突と、エージェントを使わなかったか（MECHANICAL）を返す
- worker（引き継ぎ）: entrypoint.sh の STATE_ARGS の行。conflict の続きのジョブでエージェントに --state-dir（セッションの引き継ぎ）を渡すかを返す

supervisor.py は読み込むと DB と GitHub に繋ぐので、使う関数だけを構文木から取り出して、偽の cursor と停止・GitHub の差し替えを渡して動かす。
entrypoint.sh は手順の行をそのまま bash で動かす。変えるのは一時ファイルの置き場（/work/cl.* を一時ディレクトリに）だけ。

  python3 engine_harness.py supervisor <Engine のチェックアウト> [--heads N]
  python3 engine_harness.py worker <Engine のチェックアウト> <衝突させるファイル>...
  python3 engine_harness.py resume <Engine のチェックアウト>
  python3 engine_harness.py --check <Engine のチェックアウト>   # 取り出せるかだけを見る（取り出せなければ終了コード 1）
"""
from __future__ import annotations

import ast
import json
import re
import subprocess
import sys
import tempfile
import types
from pathlib import Path

SUPERVISOR = "orchestrator/dispatcher/supervisor.py"
CONTINUATION = "orchestrator/dispatcher/continuation.py"
ENTRYPOINT = "orchestrator/worker/entrypoint.sh"
SUPERVISOR_FUNCTIONS = ["conflict_step", "ci_failure_step"]
CONTINUATION_FUNCTIONS = ["enqueue", "attempts", "pending"]
MECHANICAL_START = 'SEEN_CONFLICTS="${CONFLICTS:-}"'
MECHANICAL_END = 'echo "[worker] continue ${CONTINUE_REASON}'
PR_URL = "https://github.com/guccyon/rally/pull/659"
REPO = "guccyon/rally"
BRANCH = "factory/issue-648-job-1"


class Jsonb:
    """psycopg.types.json.Jsonb の代わり。中身をそのまま持つ"""

    def __init__(self, obj):
        self.obj = obj


def _unwrap(v):
    return v.obj if isinstance(v, Jsonb) else v


def _insert(sql: str, params: tuple) -> tuple[str, dict]:
    """insert into <表>(<列>) values (<%s か文字列>) を、表の名前と {列: 値} にする"""
    m = re.match(r"insert into (\w+)\s*\(([^)]*)\) values \((.*?)\)( returning \w+)?$", sql)
    if not m:
        raise AssertionError(f"engine_harness が読めない insert: {sql}")
    cols = [c.strip() for c in m.group(2).split(",")]
    vals, it = [], iter(params)
    for tok in [t.strip() for t in m.group(3).split(",")]:
        vals.append(next(it) if tok == "%s" else tok.strip("'"))
    return m.group(1), dict(zip(cols, vals))


class FakeCursor:
    """制御プレーンの events・jobs・tickets を、Engine の衝突の扱いが引く問い合わせの分だけ真似る"""

    def __init__(self):
        self.events: list[dict] = []
        self.jobs: list[dict] = []
        self._rows: list[dict] = []

    def add_event(self, name: str, payload: dict, job_id=None, dedupe_key=None):
        self.events.append({"id": len(self.events) + 1, "name": name, "payload": payload, "job_id": job_id, "dedupe_key": dedupe_key})

    def execute(self, sql: str, params=()):
        sql = " ".join(sql.split())
        params = tuple(_unwrap(p) for p in (params or ()))
        self._rows = []
        if sql.startswith("insert into"):
            table, row = _insert(sql, params)
            if table == "jobs":
                self.jobs.append({"id": 100 + len(self.jobs), "state": "queued", **row})
                self._rows = [{"id": self.jobs[-1]["id"]}]
            elif table == "events":
                self.add_event(row["name"], row.get("payload") or {}, row.get("job_id"), row.get("dedupe_key"))
                self._rows = [{"id": self.events[-1]["id"]}]
            else:
                raise AssertionError(f"engine_harness の偽の cursor が知らない表への insert: {sql}")
        elif sql == "select 1 from events where dedupe_key=%s":
            self._rows = [{"1": 1} for e in self.events if e["dedupe_key"] == params[0]][:1]
        elif sql.startswith("select count(*) as n from events where name='continue.enqueued'"):
            url, reason = params
            self._rows = [{"n": sum(1 for e in self.events if e["name"] == "continue.enqueued"
                                    and e["payload"].get("pr_url") == url and e["payload"].get("reason") == reason)}]
        elif sql.startswith("select id, priority from tickets"):
            self._rows = [{"id": 1, "priority": 50}]
        elif "from events e join jobs j on j.id=e.job_id where e.name='job.enqueued'" in sql:
            url, sha, reason = params[0], params[1], params[2]
            jobs = {j["id"]: j for j in self.jobs}
            self._rows = [{"payload": e["payload"], "job_id": e["job_id"], "state": jobs[e["job_id"]]["state"]}
                          for e in reversed(self.events) if e["name"] == "job.enqueued" and "continue" in e["payload"]
                          and e["payload"].get("pr_url") == url and e["payload"].get("head_sha") == sha
                          and (reason is None or e["payload"]["continue"].get("reason") == reason)
                          and jobs.get(e["job_id"], {}).get("state") in ("queued", "running")][:1]
        elif sql.startswith("select name, payload from events where name in ('pr.ci_failed','pr.ci_passed','pr.conflicted')"):
            url, sha = params
            self._rows = [{"name": e["name"], "payload": e["payload"]} for e in self.events
                          if e["name"] in ("pr.ci_failed", "pr.ci_passed", "pr.conflicted")
                          and e["payload"].get("pr_url") == url and e["payload"].get("head_sha") == sha]
        else:
            raise AssertionError(f"engine_harness の偽の cursor が知らない問い合わせ: {sql}")

    def fetchone(self):
        return self._rows[0] if self._rows else None

    def fetchall(self):
        return list(self._rows)


def _need(engine: Path, rel: str) -> Path:
    if not (engine / rel).is_file():
        raise SystemExit(f"Engine のチェックアウトに {rel} がない（ENGINE_DIR={engine}）")
    return engine / rel


def _functions(path: Path, names: list[str], namespace: dict) -> dict:
    """ファイルから、名前で指した関数の定義だけを取り出して namespace で動かす"""
    tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    found = {n.name: n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name in names}
    missing = sorted(set(names) - set(found))
    if missing:
        raise SystemExit(f"Engine の {path.name} に {', '.join(missing)} がない（関数の名前が変わったら engine_harness.py を合わせる）")
    exec(compile(ast.Module(body=[found[n] for n in names], type_ignores=[]), str(path), "exec"), namespace)
    return namespace


def load_continuation(engine: Path):
    """continuation.py は psycopg（Jsonb・UniqueViolation）だけに依存するので、それを差し替えてそのまま読み込む"""
    psycopg = types.ModuleType("psycopg")
    psycopg.types = types.ModuleType("psycopg.types")
    psycopg.types.json = types.ModuleType("psycopg.types.json")
    psycopg.types.json.Jsonb = Jsonb
    psycopg.errors = types.SimpleNamespace(UniqueViolation=type("UniqueViolation", (Exception,), {}))
    sys.modules.update({"psycopg": psycopg, "psycopg.types": psycopg.types, "psycopg.types.json": psycopg.types.json})
    _need(engine, CONTINUATION)
    sys.path.insert(0, str(engine / "orchestrator/dispatcher"))
    import continuation
    missing = [n for n in CONTINUATION_FUNCTIONS if not callable(getattr(continuation, n, None))]
    if missing:
        raise SystemExit(f"Engine の {CONTINUATION} に {', '.join(missing)} がない（関数の名前が変わったら engine_harness.py を合わせる）")
    return continuation


def _block(text: str, start: str, end: str) -> str:
    lines = text.splitlines()
    i = next((k for k, l in enumerate(lines) if l.strip().startswith(start)), None)
    j = next((k for k, l in enumerate(lines) if i is not None and k > i and l.strip().startswith(end)), None)
    if i is None or j is None:
        raise SystemExit(f"Engine の {ENTRYPOINT} に衝突を機械的に解く手順（{start} から {end} の前まで）がない（手順が変わったら engine_harness.py を合わせる）")
    return "\n".join(lines[i:j]) + "\n"


def mechanical_block(engine: Path) -> str:
    return _block(_need(engine, ENTRYPOINT).read_text(encoding="utf-8"), MECHANICAL_START, MECHANICAL_END)


def state_args_lines(engine: Path) -> str:
    lines = [l for l in _need(engine, ENTRYPOINT).read_text(encoding="utf-8").splitlines() if re.search(r"\bSTATE_ARGS=\(", l)]
    if not lines:
        raise SystemExit(f"Engine の {ENTRYPOINT} に STATE_ARGS の行がない（引き継ぎの渡し方が変わったら engine_harness.py を合わせる）")
    return "\n".join(lines) + "\n"


# ------------------------------------------------------------------ supervisor
def supervisor(engine: Path, heads: int) -> dict:
    """同じ PR が head ごとに衝突し続ける流れを、supervisor の周期（merge_step の conflict_step → ci_failure_step）で回す。
    積んだ続きのジョブは終わって新しい head を push し、その head もまた衝突する（C-0005 の PR #659 と同じ流れ）。
    積まなかった周期で止める（新しい head が来ない）"""
    continuation = load_continuation(engine)
    stoppages: list[dict] = []
    gh_calls: list[dict] = []
    ns = {
        "__name__": "engine_harness_scope", "re": re, "continuation": continuation, "CONFLICT_FIX": True,
        "over_budget": lambda cur: False, "awaiting_decision": lambda cur, url, pr: False, "log": lambda *a, **k: None,
        "stoppage": types.SimpleNamespace(open_stoppage=lambda cur, key, *a, **k: stoppages.append({"key": key, "open": True, **k}),
                                          close_stoppage=lambda cur, key, *a, **k: stoppages.append({"key": key, "open": False, **k}),
                                          open_keys=lambda *a, **k: []),
        "gh": lambda method, path, body=None: gh_calls.append({"method": method, "path": path}) or {},
    }
    _functions(_need(engine, SUPERVISOR), SUPERVISOR_FUNCTIONS, ns)
    cur = FakeCursor()
    p = {"repo": REPO, "number": 659, "url": PR_URL, "corr": f"github:{REPO}#648"}
    cycles = []
    for i in range(1, heads + 1):
        sha = f"head{i}"
        cur.add_event("pr.conflicted", {"pr_url": PR_URL, "head_sha": sha}, dedupe_key=f"pr.conflicted:{PR_URL}@{sha}")
        pr = {"head": {"sha": sha, "ref": BRANCH, "repo": {"full_name": REPO}}, "mergeable_state": "dirty", "labels": []}
        before_jobs, before_stop = len(cur.jobs), len(stoppages)
        ns["conflict_step"](cur, p, pr)
        ns["ci_failure_step"](cur, p, sha, "dirty", pr)
        new_jobs = cur.jobs[before_jobs:]
        cont = {e["job_id"]: e["payload"] for e in cur.events if e["name"] == "continue.enqueued"}
        cycles.append({"head_sha": sha, "enqueued": [{"attempt": cont.get(j["id"], {}).get("attempt"), "reason": cont.get(j["id"], {}).get("reason"),
                                                      "model_tier": j.get("model_tier")} for j in new_jobs],
                       "stoppages": stoppages[before_stop:]})
        for j in cur.jobs:
            j["state"] = "done"   # 続きのジョブが終わって push した（次の head）
        if not new_jobs:
            break
    return {"cycles": cycles, "gh_calls": gh_calls}


# ------------------------------------------------------------------ worker
def _git(cwd: Path, *args: str, check=True) -> str:
    return subprocess.run(["git", *args], cwd=cwd, check=check, capture_output=True, text=True).stdout


def conflicted_repo(root: Path, files: list[str]) -> Path:
    """既定ブランチ（main）と PR のブランチ（pr）が files の同じ場所を変えた一時のリポジトリを作り、pr に main を取り込んで衝突させる"""
    repo = root / "repo"
    repo.mkdir()
    _git(repo, "init", "-q", "-b", "main")
    _git(repo, "config", "user.email", "harness@example.invalid")
    _git(repo, "config", "user.name", "harness")
    (repo / "CHANGELOG.md").write_text("# 変更履歴\n\n## [Unreleased]\n\n### 追加\n- 前からある行（#1）\n\n## [0.1.0]\n", encoding="utf-8")
    (repo / "changelog.d").mkdir()
    (repo / "changelog.d/README.md").write_text("# 断片\n", encoding="utf-8")
    (repo / "src").mkdir()
    (repo / "src/app.txt").write_text("value = 1\n", encoding="utf-8")
    _git(repo, "add", "-A")
    _git(repo, "commit", "-q", "-m", "base")
    _git(repo, "checkout", "-q", "-b", "pr")

    def change(side: str):
        for f in files:
            path = repo / f
            path.parent.mkdir(parents=True, exist_ok=True)
            if f == "CHANGELOG.md":
                path.write_text(path.read_text(encoding="utf-8").replace("（#1）\n", f"（#1）\n- {side} の行（#{side}）\n"), encoding="utf-8")
            elif f.startswith("changelog.d/"):
                path.write_text(f"### 追加\n- {side} の断片（#{side}）\n", encoding="utf-8")   # 同じ名前の断片を両方が足す（add/add）
            else:
                path.write_text(f"value = {side}\n", encoding="utf-8")
        _git(repo, "add", "-A")
        _git(repo, "commit", "-q", "-m", side)

    change("pr")
    _git(repo, "checkout", "-q", "main")
    change("main")
    _git(repo, "checkout", "-q", "pr")
    _git(repo, "merge", "--no-edit", "-q", "main", check=False)
    return repo


def worker(engine: Path, files: list[str]) -> dict:
    """entrypoint.sh の衝突を機械的に解く手順を、本物の衝突の上で動かす"""
    block = mechanical_block(engine)
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        repo = conflicted_repo(root, files)
        script = ("set -euo pipefail\n"
                  'CONFLICTS=$(git diff --name-only --diff-filter=U | tr \'\\n\' \' \')\n'
                  'BEFORE="${CONFLICTS}"\n'
                  + block.replace("/work/cl.", f"{root}/cl.") +
                  'jq -nc --arg before "${BEFORE}" --arg conflicts "${CONFLICTS}" --arg mechanical "${MECHANICAL:-}" '
                  "'{before: $before, conflicts: $conflicts, mechanical: $mechanical}'\n")
        env = {"PATH": "/usr/local/bin:/usr/bin:/bin", "HOME": str(root), "CONTINUE_REASON": "conflict", "RENUMBERED": "", "MECHANICAL": ""}
        p = subprocess.run(["bash", "-c", script], cwd=repo, env=env, capture_output=True, text=True)
        if p.returncode != 0:
            raise SystemExit(f"entrypoint.sh の衝突を解く手順が落ちた（{p.returncode}）: {p.stderr[-500:]}")
        out = json.loads(p.stdout.strip().splitlines()[-1])
        markers = sorted(f for f in files if (repo / f).is_file() and "<<<<<<<" in (repo / f).read_text(encoding="utf-8"))
        return {"before": sorted(out["before"].split()), "conflicts": sorted(out["conflicts"].split()),
                "agent": not out["mechanical"], "mechanical": out["mechanical"], "markers": markers,
                "unmerged": sorted(_git(repo, "diff", "--name-only", "--diff-filter=U").split())}


def resume(engine: Path) -> dict:
    """entrypoint.sh の STATE_ARGS の行を、conflict の続きのジョブ（ボリュームあり）の環境で動かし、--state-dir を渡すかを返す"""
    script = "set -euo pipefail\n" + state_args_lines(engine) + 'printf "%s\\n" "${STATE_ARGS[@]+"${STATE_ARGS[@]}"}"\n'
    out = {}
    for reason in ("conflict", "ci_failed"):
        p = subprocess.run(["bash", "-c", script], env={"PATH": "/usr/bin:/bin", "HANDOFF_DIR": "/handoff", "CONTINUE_REASON": reason},
                           capture_output=True, text=True)
        if p.returncode != 0:
            raise SystemExit(f"entrypoint.sh の STATE_ARGS の行が落ちた（{p.returncode}）: {p.stderr[-500:]}")
        out[reason] = {"state_args": p.stdout.split(), "resumes": "--state-dir" in p.stdout.split()}
    return out


def check(engine: Path) -> dict:
    """動かさず、取り出す関数と手順がすべて Engine のチェックアウトにあるかだけを見る（CI で飛ばさずに落とすため）"""
    load_continuation(engine)
    _functions(_need(engine, SUPERVISOR), SUPERVISOR_FUNCTIONS, {"__name__": "engine_harness_scope"})
    mechanical_block(engine)
    state_args_lines(engine)
    return {"functions": SUPERVISOR_FUNCTIONS + CONTINUATION_FUNCTIONS, "entrypoint": ["mechanical", "state_args"]}


def main(argv: list[str]):
    if len(argv) == 2 and argv[0] == "--check":
        out = check(Path(argv[1]))
    elif len(argv) >= 2 and argv[0] == "supervisor":
        out = supervisor(Path(argv[1]), int(argv[argv.index("--heads") + 1]) if "--heads" in argv else 6)
    elif len(argv) >= 3 and argv[0] == "worker":
        out = worker(Path(argv[1]), argv[2:])
    elif len(argv) == 2 and argv[0] == "resume":
        out = resume(Path(argv[1]))
    else:
        raise SystemExit(__doc__)
    json.dump(out, sys.stdout, ensure_ascii=False)


if __name__ == "__main__":
    main(sys.argv[1:])
