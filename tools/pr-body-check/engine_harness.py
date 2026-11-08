"""関所（gate.mjs）が返す body_fix の入力を、Engine の本物の本文の直しの積み方に渡す（知見 K-0001 の担保。議決 D-0013、Issue #956）。

engine.test.mjs から呼ぶ。Engine の次の関数をそのまま動かして、関所の指摘が本文だけの指摘と読まれ、body_fix のジョブが積まれるかを返す。

- orchestrator/dispatcher/dispatcher.py の is_body_only（箇所が PR 本文だけの指摘か）・enqueue_body_fix（body_fix のジョブを積む）
- 関所を配線した Engine（engine/pr-body-gate.patch を当てたもの）では act_on_body_gate（ワーカーの FACTORY_RESULT の body_gate を body_fix か人に回す。
  議決 D-0019、Issue #975）も

dispatcher.py は読み込むと DB・オブジェクトストア・GitHub に繋ぐので、使う関数だけを構文木から取り出し、偽の cursor（DB）を渡して動かす。
取り出し方と偽の cursor は tools/sendback-check/engine_harness.py のものを使う。関数の中身には手を入れない。

  python3 engine_harness.py <Engine のチェックアウト> < body_fix.json > result.json
  python3 engine_harness.py --check <Engine のチェックアウト>   # 関数を取り出せるかだけを見る（取り出せなければ終了コード 1）
  python3 engine_harness.py --body-gate <Engine のチェックアウト> < gate.json > result.json   # act_on_body_gate を動かす

入力: gate.mjs の body_fix（{ "pr_url", "defects": [{ rubric_id, severity, location, evidence, remediation_hint }] }）
出力: { "body_only": [bool]（指摘ごとの is_body_only）, "jobs": [{ "kind" }], "input": body_fix の job.enqueued の input | null }

--body-gate の入力: { "pr_url", "body_gate": ワーカーの FACTORY_RESULT の body_gate（{ rc, body_fix, pending_entry }） }
--body-gate の出力: { "jobs": [{ "kind" }], "input": 上と同じ, "events": [イベントの名前], "stoppages": [{ "key", "category" }] }

本文の直しのあとの再検査を待つ supervisor（engine/pr-body-recheck.patch を当てた Engine。知見 K-0012 の担保、議決 D-0050、Issue #1122）では
orchestrator/dispatcher/supervisor.py の body_fix_pending・body_recheck_pending（と BODY_CHECK・BODY_CHECK_WORKFLOW）を取り出し、merge_step と同じ順
（body_fix_pending のあと body_recheck_pending）で呼んで、マージを待つかを返す。GitHub（ワークフローの有無と BODY_CHECK のチェックの実行）は入力で真似る。

  python3 engine_harness.py --body-recheck <Engine のチェックアウト> < recheck.json > result.json

--body-recheck の入力: { "pr_url", "head_sha", "jobs": [{ "id", "kind", "state", "enqueued_at", "started_at" }]（時刻は ISO 8601）,
  "events": [{ "name", "payload", "dedupe_key" }]（もう記録されたイベント）, "workflow": BODY_CHECK_WORKFLOW があるか（既定 true）,
  "workflow_error": 読むと返る HTTP の状態（404 以外。省けば読める）, "check_runs": GitHub の check-runs の check_runs }
--body-recheck の出力: { "fix_pending": bool, "reason": 待つ理由 | null, "events": [記録したイベント { name, payload, dedupe_key }],
  "stoppages": [{ "key", "category" }], "gh": [GitHub に問い合わせた道], "order": merge_step で待つ確かめがマージより前にあるか（下の merge_order） }
"""
from __future__ import annotations

import ast
import importlib.util
import io
import json
import re
import sys
import urllib.error
from datetime import datetime
from pathlib import Path

sys.dont_write_bytecode = True   # sendback-check の engine_harness.py を読んでも __pycache__ を書き換えない
_SENDBACK = Path(__file__).resolve().parent.parent / "sendback-check" / "engine_harness.py"
_spec = importlib.util.spec_from_file_location("sendback_engine_harness", _SENDBACK)
sendback = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(sendback)

ENGINE_FUNCTIONS = {"orchestrator/dispatcher/dispatcher.py": ["is_body_only", "enqueue_body_fix"]}
BODY_GATE_FUNCTIONS = {"orchestrator/dispatcher/dispatcher.py": ["is_body_only", "enqueue_body_fix", "act_on_body_gate"]}


RECHECK_FILE = "orchestrator/dispatcher/supervisor.py"
RECHECK_FUNCTIONS = ["body_fix_pending", "body_recheck_pending"]
RECHECK_CONSTANTS = ["BODY_CHECK", "BODY_CHECK_WORKFLOW"]


class FakeStoppage:
    """stoppage.open_stoppage の差し替え（開いた停止を覚えるだけ）"""

    def __init__(self):
        self.opened: list[dict] = []

    def open_stoppage(self, cur, key, category, what, **kw):
        self.opened.append({"key": key, "category": category})
        return True


def load(engine: Path, functions: dict = ENGINE_FUNCTIONS, **extra) -> dict:
    """sendback-check の取り出し方で、この担保が使う関数を取り出す"""
    _, psycopg = sendback.load_judgment(engine)   # psycopg の差し替え（enqueue_body_fix が UniqueViolation を引く）
    ns = {"__name__": "engine_harness_scope", "re": re, "psycopg": psycopg, "log": lambda *a, **k: None, **extra}
    saved = sendback.ENGINE_FUNCTIONS
    sendback.ENGINE_FUNCTIONS = functions
    try:
        return sendback.load_engine_functions(engine, ns)
    finally:
        sendback.ENGINE_FUNCTIONS = saved


def run(engine: Path, body_fix: dict) -> dict:
    ns = load(engine)
    pr_url, defects = body_fix["pr_url"], body_fix["defects"]
    repo, pr_number = re.match(r"https://github\.com/([^/]+/[^/]+)/pull/(\d+)$", pr_url).groups()
    cur = sendback.FakeCursor()
    ns["record_event"] = lambda cur_, name, correlation_id, job_id, payload, dedupe_key=None: cur_.add_event(name, payload, dedupe_key)
    body_only = [bool(ns["is_body_only"](d)) for d in defects]
    if all(body_only):   # dispatcher.act_on_judgment と同じく、本文だけの指摘を body_fix に回す
        ns["enqueue_body_fix"](cur, repo, f"github:{repo}#1", pr_url, int(pr_number), defects)
    enq = next((e for e in cur.events if e["name"] == "job.enqueued" and e["payload"].get("kind") == "body_fix"), None)
    return {"body_only": body_only, "jobs": [{"kind": j["kind"]} for j in cur.jobs], "input": enq["payload"].get("input") if enq else None}


def run_body_gate(engine: Path, inp: dict) -> dict:
    """ワーカーが関所で本文を書かなかった結果（body_gate）を、Engine の act_on_body_gate に渡す"""
    stop = FakeStoppage()
    ns = load(engine, BODY_GATE_FUNCTIONS, stoppage=stop)
    pr_url = inp["pr_url"]
    repo, pr_number = re.match(r"https://github\.com/([^/]+/[^/]+)/pull/(\d+)$", pr_url).groups()
    cur = sendback.FakeCursor()
    ns["record_event"] = lambda cur_, name, correlation_id, job_id, payload, dedupe_key=None: cur_.add_event(name, payload, dedupe_key)
    ns["act_on_body_gate"](cur, repo, f"github:{repo}#1", 1, pr_url, int(pr_number), inp["body_gate"])
    enq = next((e for e in cur.events if e["name"] == "job.enqueued" and e["payload"].get("kind") == "body_fix"), None)
    return {"jobs": [{"kind": j["kind"]} for j in cur.jobs], "input": enq["payload"].get("input") if enq else None,
            "events": [e["name"] for e in cur.events], "stoppages": stop.opened}


class RecheckCursor:
    """body_fix_pending・body_recheck_pending が引く jobs と events の問い合わせだけを真似る。知らない問い合わせは落とす"""

    def __init__(self, jobs: list[dict], events: list[dict]):
        self.jobs, self.events, self.recorded = jobs, list(events), []
        self._rows: list[dict] = []

    def _job_rows(self, repo: str, number: int) -> list[dict]:
        return sorted((j for j in self.jobs if j.get("repo", repo) == repo and j.get("issue_number", number) == number and j["kind"] == "body_fix"),
                      key=lambda j: j["id"], reverse=True)

    def execute(self, sql: str, params=()):
        sql = " ".join(sql.split())
        if sql.startswith("select 1 from jobs where") and "kind='body_fix' and state in ('queued','running')" in sql:
            self._rows = [{"1": 1} for j in self._job_rows(params[0], params[1]) if j["state"] in ("queued", "running")][:1]
        elif sql.startswith("select id, state, enqueued_at, started_at from jobs where") and "kind='body_fix' order by id desc limit 1" in sql:
            self._rows = [{k: j.get(k) for k in ("id", "state", "enqueued_at", "started_at")} for j in self._job_rows(params[0], params[1])][:1]
        elif sql == "select 1 from events where dedupe_key=%s":
            self._rows = [{"1": 1} for e in self.events if e.get("dedupe_key") == params[0]][:1]
        else:
            raise sendback.HarnessUnknownQuery(f"engine_harness の偽の cursor が知らない問い合わせ: {sql}")

    def fetchone(self):
        return self._rows[0] if self._rows else None


def load_recheck(engine: Path, **extra) -> dict:
    """supervisor.py から本文の再検査の関数と定数だけを取り出す（supervisor.py は読み込むと DB と GitHub に繋ぐ）"""
    path = engine / RECHECK_FILE
    if not path.is_file():
        raise SystemExit(f"Engine のチェックアウトに {RECHECK_FILE} がない（ENGINE_DIR={engine}）")
    tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    funcs = {n.name: n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name in RECHECK_FUNCTIONS}
    consts = {t.id: n for n in tree.body if isinstance(n, ast.Assign) for t in n.targets if isinstance(t, ast.Name) and t.id in RECHECK_CONSTANTS}
    missing = sorted(set(RECHECK_FUNCTIONS) - set(funcs)) + sorted(set(RECHECK_CONSTANTS) - set(consts))
    if missing:
        raise SystemExit(f"Engine の {RECHECK_FILE} に {', '.join(missing)} がない（engine/pr-body-recheck.patch を当てていないか、名前が変わった）")
    ns = {"__name__": "engine_harness_recheck", "re": re, "urllib": urllib, "datetime": datetime, **extra}
    body = [consts[n] for n in RECHECK_CONSTANTS] + [funcs[n] for n in RECHECK_FUNCTIONS]
    exec(compile(ast.Module(body=body, type_ignores=[]), str(path), "exec"), ns)
    return ns


def merge_order(engine: Path) -> dict:
    """merge_step で、body_fix_pending と body_recheck_pending を呼んで待つ（continue する）のが、マージ（gh の PUT と supervisor.merged の記録）より前か"""
    tree = ast.parse((engine / RECHECK_FILE).read_text(encoding="utf-8"))
    step = next((n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == "merge_step"), None)
    if step is None:
        return {"merge_step": False}
    def name_of(c: ast.Call) -> str | None:
        return c.func.id if isinstance(c.func, ast.Name) else None
    calls = [c for c in ast.walk(step) if isinstance(c, ast.Call)]
    merge_put = min((c.lineno for c in calls if name_of(c) == "gh" and c.args and isinstance(c.args[0], ast.Constant) and c.args[0].value == "PUT"),
                    default=None)
    merged = min((c.lineno for c in calls if name_of(c) == "record" and len(c.args) > 1 and isinstance(c.args[1], ast.Constant)
                  and c.args[1].value == "supervisor.merged"), default=None)
    waits = {}
    for n in ast.walk(step):   # if <待つ関数(...)>: continue の形だけを数える
        if isinstance(n, ast.If) and isinstance(n.test, ast.Call) and name_of(n.test) in RECHECK_FUNCTIONS \
                and len(n.body) == 1 and isinstance(n.body[0], ast.Continue):
            waits.setdefault(name_of(n.test), n.lineno)
    first = min(x for x in (merge_put, merged) if x is not None) if merge_put or merged else None
    return {"merge_step": True, "merge_line": first, "waits": waits,
            "before_merge": first is not None and all(f in waits and waits[f] < first for f in RECHECK_FUNCTIONS)}


def run_body_recheck(engine: Path, inp: dict) -> dict:
    """merge_step と同じ順で、本文の直しの終わりと直した本文の BODY_CHECK を待つかを返す"""
    stop = FakeStoppage()
    asked: list[str] = []
    workflow, workflow_error, runs = inp.get("workflow", True), inp.get("workflow_error"), inp.get("check_runs") or []

    def gh(method, path, body=None):
        asked.append(path)
        if method != "GET":
            raise AssertionError(f"本文の再検査で GitHub に書いた: {method} {path}")
        if "/contents/" in path:
            if workflow_error or not workflow:
                raise urllib.error.HTTPError(path, workflow_error or 404, "fake", {}, io.BytesIO(b""))
            return {"path": path}
        if "/check-runs" in path:
            return {"total_count": len(runs), "check_runs": runs}
        raise AssertionError(f"知らない GitHub の問い合わせ: {path}")

    cur = RecheckCursor([{**j, **{k: datetime.fromisoformat(j[k]) for k in ("enqueued_at", "started_at") if j.get(k)}} for j in inp.get("jobs") or []],
                        inp.get("events") or [])

    def record(cur_, name, corr, payload, dedupe_key=None):
        if any(e.get("dedupe_key") == dedupe_key for e in cur_.events if dedupe_key):
            return False
        e = {"name": name, "payload": payload, "dedupe_key": dedupe_key}
        cur_.events.append(e)
        cur_.recorded.append(e)
        return True

    ns = load_recheck(engine, gh=gh, record=record, stoppage=stop)
    pr_url, sha = inp["pr_url"], inp["head_sha"]
    fix_pending = bool(ns["body_fix_pending"](cur, pr_url))
    reason = None if fix_pending else ns["body_recheck_pending"](cur, pr_url, sha)
    return {"fix_pending": fix_pending, "reason": reason, "events": cur.recorded, "stoppages": stop.opened, "gh": asked,
            "order": merge_order(engine)}


def check(engine: Path) -> dict:
    """ジョブは積まず、取り出す関数がすべて Engine のチェックアウトにあるかだけを見る（CI で飛ばさずに落とすため）"""
    load(engine)
    return {"functions": [n for names in ENGINE_FUNCTIONS.values() for n in names]}


def main():
    if len(sys.argv) == 3 and sys.argv[1] == "--check":
        json.dump(check(Path(sys.argv[2])), sys.stdout, ensure_ascii=False)
        return
    if len(sys.argv) == 3 and sys.argv[1] == "--body-gate":
        json.dump(run_body_gate(Path(sys.argv[2]), json.load(sys.stdin)), sys.stdout, ensure_ascii=False)
        return
    if len(sys.argv) == 3 and sys.argv[1] == "--body-recheck":
        json.dump(run_body_recheck(Path(sys.argv[2]), json.load(sys.stdin)), sys.stdout, ensure_ascii=False)
        return
    if len(sys.argv) != 2:
        raise SystemExit("使い方: python3 engine_harness.py [--check|--body-gate|--body-recheck] <Engine のチェックアウト> < body_fix.json")
    json.dump(run(Path(sys.argv[1]), json.load(sys.stdin)), sys.stdout, ensure_ascii=False)


if __name__ == "__main__":
    main()
