"""Engine の本物の判定を、DB と GitHub を差し替えて動かす（知見 K-0004 の担保。議決 D-0009・Issue #935）。

engine.test.mjs から呼ぶ。check.mjs の線（参照の実装）ではなく、Engine の次の関数をそのまま動かして、
起きた後続（差し戻し・続きのジョブ・本文の直し）と、検査役の承認の basis を返す。

- orchestrator/dispatcher/judgment.py の record_judged（judge の結果から、直すもの・差し替え・人に回すものを集める）
- orchestrator/dispatcher/dispatcher.py の act_on_judgment・file_sendback・enqueue_body_fix・is_body_only（後続を起こす）
- orchestrator/dispatcher/supervisor.py の inspector_approved（承認と basis を書く）

dispatcher.py と supervisor.py は読み込むと DB・オブジェクトストア・GitHub に繋ぐので、使う関数だけを構文木から取り出して、
制御プレーンの記録を持つ偽の cursor と、GitHub・継続ジョブ・停止の差し替えを渡して動かす。関数の中身には手を入れない。

  python3 engine_harness.py <Engine のチェックアウト> < case.json > result.json
  python3 engine_harness.py --check <Engine のチェックアウト>   # 関数を取り出せるかだけを見る（取り出せなければ終了コード 1）

入力: { "pr_url", "merged", "judgments": check.mjs の記録の judgments（subject・verdict・defect・already_applied） }
出力: { "followups": [{ "kind" }], "basis": str | null, "approved": bool, "events": [{ "name", "payload", "judgment" }] }

events は、Engine が偽の cursor に記録したイベントを、README の「制御プレーンから出すときの例」の SQL と同じ形（同じ名前に絞り、
judgment.judged に artifacts の Judgment を添える）で出したもの。check.mjs --events にそのまま渡せる（Issue #945）。
supervisor.inspector_approved と supervisor.merged は supervisor の周期（取り出していない）が記録する形で足す。
"""
from __future__ import annotations

import ast
import json
import re
import sys
import types
from datetime import datetime, timezone
from pathlib import Path

JUDGMENT_FUNCTIONS = ["record_judged"]
ENGINE_FUNCTIONS = {
    "orchestrator/dispatcher/dispatcher.py": ["_quote", "is_body_only", "enqueue_body_fix", "act_on_judgment", "file_sendback"],
    "orchestrator/dispatcher/supervisor.py": ["inspector_approved"],
}


# README の「制御プレーンから出すときの例」の SQL が出すイベントの名前
EXPORTED_EVENTS = ("supervisor.judge_enqueued", "judgment.judged", "sendback.filed", "continue.enqueued", "job.enqueued",
                   "supervisor.inspector_approved", "supervisor.merged")


class Jsonb:
    """psycopg.types.json.Jsonb の代わり。中身をそのまま持つ"""

    def __init__(self, obj):
        self.obj = obj


def _unwrap(v):
    return v.obj if isinstance(v, Jsonb) else v


class FakeCursor:
    """制御プレーンの events・artifacts・jobs を、Engine が引く問い合わせの分だけ真似る"""

    def __init__(self):
        self.events: list[dict] = []
        self.artifacts: list[dict] = []
        self.jobs: list[dict] = []
        self._rows: list[dict] = []
        self._clock = 0

    def now(self) -> datetime:
        self._clock += 1
        return datetime.fromtimestamp(1_700_000_000 + self._clock, tz=timezone.utc)

    def add_event(self, name: str, payload: dict, dedupe_key: str | None = None):
        self.events.append({"id": len(self.events) + 1, "name": name, "payload": payload, "dedupe_key": dedupe_key, "occurred_at": self.now()})

    def execute(self, sql: str, params=()):
        sql = " ".join(sql.split())
        params = tuple(_unwrap(p) for p in (params or ()))
        self._rows = []
        if sql.startswith("insert into artifacts"):
            corr, job_id, payload = params
            self.artifacts.append({"id": len(self.artifacts) + 1, "job_id": job_id, "payload": payload})
            self._rows = [{"id": self.artifacts[-1]["id"]}]
        elif sql.startswith("insert into events"):
            name = re.search(r"values \('([^']+)'", sql)[1]
            self.add_event(name, params[-1])
        elif sql.startswith("insert into jobs"):
            kind = re.search(r"'([a-z_]+)'\) returning id", sql)[1]
            self.jobs.append({"id": 100 + len(self.jobs), "kind": kind, "state": "done"})
            self._rows = [{"id": self.jobs[-1]["id"]}]
        elif "from artifacts where type='Judgment' and payload->>'stage'='judged'" in sql and "human_grade" in sql:
            self._rows = []   # 同じ PR で人が判断した項目はない
        elif "from events where dedupe_key=%s" in sql:
            self._rows = [{"occurred_at": e["occurred_at"], "payload": e["payload"]} for e in self.events if e["dedupe_key"] == params[0]][:1]
        elif "name='gate.evaluated'" in sql and "occurred_at >" in sql:
            self._rows = [{"1": 1} for e in self.events if e["name"] == "gate.evaluated" and e["payload"].get("pr_url") == params[0]
                          and e["occurred_at"] > params[1]][:1]
        elif "name='judgment.judged'" in sql:
            self._rows = [{"verdict": e["payload"].get("verdict")} for e in self.events if e["name"] == "judgment.judged"
                          and e["payload"].get("pr_url") == params[0] and e["occurred_at"] > params[1]]
        elif "name='sendback.filed' and payload->>'fix_corr'" in sql:
            self._rows = []   # この PR は差し戻しの直しではない
        elif "count(*)" in sql and "sendback.filed" in sql:
            self._rows = [{"n": 0}]   # 差し戻しの上限には当たっていない
        elif "from jobs where" in sql and "body_fix" in sql:
            self._rows = []   # 本文の直しは終わっている
        else:
            raise AssertionError(f"engine_harness の偽の cursor が知らない問い合わせ: {sql}")

    def fetchone(self):
        return self._rows[0] if self._rows else None

    def fetchall(self):
        return list(self._rows)


def export_events(cur: FakeCursor, pr_url: str) -> list[dict]:
    """README の SQL と同じ形で、その PR のイベントを古い順に出す（judgment.judged には Judgment を添え、ほかは null）"""
    artifacts = {a["id"]: a["payload"] for a in cur.artifacts}
    return [{"name": e["name"], "payload": e["payload"],
             "judgment": artifacts.get(e["payload"].get("artifact_id")) if e["name"] == "judgment.judged" else None}
            for e in cur.events
            if e["name"] in EXPORTED_EVENTS and (e["payload"].get("pr_url") or e["payload"].get("origin_pr")) == pr_url]


def load_engine_functions(engine: Path, namespace: dict) -> dict:
    """Engine のファイルから、名前で指した関数の定義だけを取り出して namespace で動かす"""
    for rel, names in ENGINE_FUNCTIONS.items():
        if not (engine / rel).is_file():
            raise SystemExit(f"Engine のチェックアウトに {rel} がない（ENGINE_DIR={engine}）")
        tree = ast.parse((engine / rel).read_text(encoding="utf-8"), filename=str(engine / rel))
        found = {n.name: n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name in names}
        missing = sorted(set(names) - set(found))
        if missing:
            raise SystemExit(f"Engine の {rel} に {', '.join(missing)} がない（関数の名前が変わったら engine_harness.py を合わせる）")
        exec(compile(ast.Module(body=[found[n] for n in names], type_ignores=[]), str(engine / rel), "exec"), namespace)
    return namespace


def load_judgment(engine: Path):
    """judgment.py は psycopg の Jsonb だけに依存するので、それを差し替えてそのまま読み込む"""
    psycopg = types.ModuleType("psycopg")
    psycopg.types = types.ModuleType("psycopg.types")
    psycopg.types.json = types.ModuleType("psycopg.types.json")
    psycopg.types.json.Jsonb = Jsonb
    psycopg.errors = types.SimpleNamespace(UniqueViolation=type("UniqueViolation", (Exception,), {}))
    sys.modules.update({"psycopg": psycopg, "psycopg.types": psycopg.types, "psycopg.types.json": psycopg.types.json})
    if not (engine / "orchestrator/dispatcher/judgment.py").is_file():
        raise SystemExit(f"Engine のチェックアウトに orchestrator/dispatcher/judgment.py がない（ENGINE_DIR={engine}）")
    sys.path.insert(0, str(engine / "orchestrator/dispatcher"))
    import judgment
    missing = [n for n in JUDGMENT_FUNCTIONS if not callable(getattr(judgment, n, None))]
    if missing:
        raise SystemExit(f"Engine の orchestrator/dispatcher/judgment.py に {', '.join(missing)} がない（関数の名前が変わったら engine_harness.py を合わせる）")
    return judgment, psycopg


def judge_io(judgments: list[dict]) -> tuple[dict, dict]:
    """check.mjs の記録の judgments を、judge の入力（仮決め・検査の指摘）と judge の出力（verdict）に組み直す"""
    inp = {"provisional": [], "defects": [], "track_record": {}}
    verdicts = {"provisional": [], "defects": []}
    for i, j in enumerate(judgments):
        if j.get("subject") == "defect" or j.get("defect") is not None:
            verdicts["defects"].append({"index": len(inp["defects"]), "verdict": j["verdict"], "confidence": 0.9})
            inp["defects"].append({"severity": "major", "evidence": "", "remediation_hint": "", **(j.get("defect") or {})})
        else:
            aid = 1000 + i
            inp["provisional"].append({"artifact_id": aid, "category": "ui", "question": f"仮決め {i}", "decision": "案"})
            verdicts["provisional"].append({"artifact_id": aid, "verdict": j["verdict"], "confidence": 0.9,
                                            "replacement": "差し替え後の案" if j["verdict"] == "replace" else None,
                                            "already_applied": bool(j.get("already_applied"))})
    return inp, verdicts


def run(engine: Path, case: dict) -> dict:
    judgment, psycopg = load_judgment(engine)
    pr_url = case["pr_url"]
    repo, pr_number = re.match(r"https://github\.com/([^/]+/[^/]+)/pull/(\d+)$", pr_url).groups()
    sha, corr, mode = "head1", f"github:{repo}#1", "unattended"
    cur = FakeCursor()
    followups: list[dict] = []

    def gh(method, path, body=None):
        if method == "GET" and path.endswith(f"/pulls/{pr_number}"):
            return {"state": "closed" if case.get("merged") else "open", "title": "PR", "head": {"sha": sha, "ref": "factory/issue-1-job-1", "repo": {"full_name": repo}}}
        if method == "POST" and path.endswith("/issues"):
            return {"html_url": f"https://github.com/{repo}/issues/9999", "number": 9999}
        raise AssertionError(f"engine_harness が知らない GitHub の呼び出し: {method} {path}")

    def record_event(cur_, name, correlation_id, job_id, payload, dedupe_key=None):
        cur_.add_event(name, payload, dedupe_key)
        if name == "sendback.filed":
            followups.append({"kind": "sendback"})
        elif name == "job.enqueued" and payload.get("kind") == "body_fix":
            followups.append({"kind": "body_fix"})

    def continuation_enqueue(cur_, *, reason, **kw):
        # continuation.enqueue と同じく、continue.enqueued と続きのジョブの job.enqueued（kind: implement）を記録する
        cont = {"branch": kw.get("branch"), "pr_number": int(kw["pr_number"]), "reason": reason}
        cur_.add_event("continue.enqueued", {"pr_url": kw["pr_url"], "head_sha": kw.get("head_sha"), "reason": reason, "attempt": 1,
                                             "continue": cont, "by": kw.get("by")}, f"continue:{kw['pr_url']}@{kw.get('head_sha')}:{reason}")
        cur_.add_event("job.enqueued", {"by": kw.get("by"), "kind": "implement", "continue": cont, "pr_url": kw["pr_url"], "head_sha": kw.get("head_sha")})
        followups.append({"kind": reason})
        return 200

    ns = {
        "__name__": "engine_harness_scope", "re": re, "psycopg": psycopg, "judgment": judgment,
        "gh": gh, "record_event": record_event, "comment": lambda *a, **k: None, "swap_label": lambda *a, **k: None,
        "log": lambda *a, **k: None,
        "stoppage": types.SimpleNamespace(open_stoppage=lambda *a, **k: None, close_stoppage=lambda *a, **k: None, open_keys=lambda *a, **k: []),
        "continuation": types.SimpleNamespace(enqueue=continuation_enqueue, pending=lambda *a, **k: None, CONTINUE_MAX=3),
        "REVIEW_FIX": True, "LOW_CONFIDENCE": 0.6, "SENDBACK_PER_PR": 1, "SENDBACK_DAILY_MAX": 5,
        "INSPECT_ENABLED": True, "JUDGE_ENABLED": True,
    }
    load_engine_functions(engine, ns)
    ns["body_fix_pending"] = lambda cur_, url: False   # supervisor.py の body_fix_pending は jobs を引くだけなので、終わった扱いにする

    # supervisor が積む検査と judge の記録（inspector_approved が引く順に）
    cur.add_event("supervisor.inspect", {"pr_url": pr_url, "head_sha": sha}, f"supervisor.inspect:{pr_url}@{sha}")
    cur.add_event("gate.evaluated", {"pr_url": pr_url, "defects": []})
    cur.add_event("supervisor.judge_enqueued", {"pr_url": pr_url, "head_sha": sha, "skipped": False}, f"supervisor.judge:{pr_url}@{sha}")
    inp, verdicts = judge_io(case["judgments"])
    summary = judgment.record_judged(cur, corr, 1, pr_url, inp, verdicts, mode, "テスト")
    ns["act_on_judgment"](cur, repo, corr, 1, pr_url, summary, mode)
    approved, basis = ns["inspector_approved"](cur, pr_url, sha)
    if approved:   # supervisor の周期が承認とマージを記録する形（supervisor.py の record）
        cur.add_event("supervisor.inspector_approved", {"pr_url": pr_url, "head_sha": sha, "basis": basis}, f"inspector.approved:{pr_url}@{sha}")
        if case.get("merged"):
            cur.add_event("supervisor.merged", {"pr_url": pr_url, "head_sha": sha, "basis": basis}, f"supervisor.merged:{pr_url}")
    return {"followups": followups, "basis": basis if approved else None, "approved": approved, "events": export_events(cur, pr_url)}


def check(engine: Path) -> dict:
    """判定は動かさず、取り出す関数がすべて Engine のチェックアウトにあるかだけを見る（CI で飛ばさずに落とすため）"""
    load_judgment(engine)
    load_engine_functions(engine, {"__name__": "engine_harness_scope"})
    return {"functions": JUDGMENT_FUNCTIONS + [n for names in ENGINE_FUNCTIONS.values() for n in names]}


def main():
    if len(sys.argv) == 3 and sys.argv[1] == "--check":
        json.dump(check(Path(sys.argv[2])), sys.stdout, ensure_ascii=False)
        return
    if len(sys.argv) != 2:
        raise SystemExit("使い方: python3 engine_harness.py [--check] <Engine のチェックアウト> < case.json")
    json.dump(run(Path(sys.argv[1]), json.load(sys.stdin)), sys.stdout, ensure_ascii=False)


if __name__ == "__main__":
    main()
