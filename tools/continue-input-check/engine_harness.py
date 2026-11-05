"""Engine の本物の続きのジョブの積み方を、DB と GitHub を差し替えて動かす（知見 K-0009 の担保。議決 D-0013・Issue #955）。

engine.test.mjs から呼ぶ。line.mjs の線（参照の実装）ではなく、Engine の次の関数をそのまま動かして、
continue（conflict・ci_failed）の implement ジョブがワーカーに渡す入力を返す。

- orchestrator/dispatcher/judgment.py の record_judged（judge の結果を Judgment と judgment.judged に残す）
- orchestrator/dispatcher/dispatcher.py の act_on_judgment・file_sendback・enqueue_body_fix・is_body_only（採用した本文の指摘を body_fix に回す）
- orchestrator/dispatcher/supervisor.py の conflict_step・ci_fix_step（衝突・CI 落ちで続きのジョブを積む）
- orchestrator/dispatcher/continuation.py の enqueue（続きのジョブと job.enqueued の input を記録する）と SIGNALED

偽の cursor と judgment.py の読み込み・裁きの組み立ては tools/sendback-check/engine_harness.py（K-0004 の担保）のものを使う。
関数の中身には手を入れない。ワーカーに渡す入力は、dispatcher.run_job と同じく、reason が continuation.SIGNALED にあるときだけ
job.enqueued の input（なければ {}）を標準入力で渡し、ほかは渡さない（null）として組む。

  python3 engine_harness.py <Engine のチェックアウト> < case.json > result.json
  python3 engine_harness.py --check <Engine のチェックアウト>   # 関数を取り出せるかだけを見る（取り出せなければ終了コード 1）

入力: { "pr_url", "reason": "conflict" | "ci_failed", "judgments": sendback-check の記録の judgments, "body_rules": [str] }
  body_rules は body_fix が本文を直したときに残す決まり。Engine の body_fix が終わったときの pr.body_amended に rules として足す
出力: { "enqueued": bool, "reason", "input": job.enqueued の input | null, "piped": bool, "delivered": ワーカーが受け取る入力 | null,
        "events": その PR のイベント（古い順。judgment.judged に Judgment を添える。続きのジョブの job.enqueued より前まで） }
"""
from __future__ import annotations

import importlib.util
import json
import re
import sys
import types
from pathlib import Path

sys.dont_write_bytecode = True   # sendback-check の engine_harness.py を読んでも __pycache__ を書き換えない
_SENDBACK = Path(__file__).resolve().parent.parent / "sendback-check" / "engine_harness.py"
_spec = importlib.util.spec_from_file_location("sendback_engine_harness", _SENDBACK)
sendback = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(sendback)

ENGINE_FUNCTIONS = {
    "orchestrator/dispatcher/dispatcher.py": ["_quote", "is_body_only", "enqueue_body_fix", "act_on_judgment", "file_sendback"],
    "orchestrator/dispatcher/supervisor.py": ["conflict_step", "ci_fix_step"],
}
CONTINUATION_NAMES = ["enqueue", "SIGNALED", "REASONS"]


class FakeCursor(sendback.FakeCursor):
    """sendback-check の偽の cursor に、continuation.enqueue と supervisor の続きのジョブの手順が引く問い合わせを足す"""

    def execute(self, sql: str, params=()):
        flat = " ".join(sql.split())
        p = tuple(sendback._unwrap(x) for x in (params or ()))
        if flat.startswith("insert into events"):
            # (correlation_id, job_id, payload[, dedupe_key])。continuation.enqueue は dedupe_key も渡す
            self.add_event(re.search(r"values \('([^']+)'", flat)[1], p[2], p[3] if len(p) > 3 else None)
            self._rows = []
        elif flat.startswith("insert into jobs"):
            kind = re.findall(r"'([a-z_-]+)'", flat.split(" values ", 1)[1])[-1]
            self.jobs.append({"id": 100 + len(self.jobs), "kind": kind, "state": "done"})
            self._rows = [{"id": self.jobs[-1]["id"]}]
        elif "count(*)" in flat and "name='continue.enqueued'" in flat:
            self._rows = [{"n": sum(1 for e in self.events if e["name"] == "continue.enqueued" and e["payload"].get("pr_url") == p[0]
                                    and e["payload"].get("reason") == p[1])}]
        elif flat.startswith("select id, priority from tickets"):
            self._rows = []   # チケットの記録はない（優先度は既定）
        elif "name='pr.ci_failed'" in flat:
            self._rows = [{"payload": e["payload"]} for e in self.events if e["name"] == "pr.ci_failed"
                          and e["payload"].get("pr_url") == p[0] and e["payload"].get("head_sha") == p[1]][:1]
        elif m := re.fullmatch(r"select (?:id, )?payload from events where name='([a-z_.]+)' and payload->>'pr_url'=%s(?: order by id)?", flat):
            # Engine が続きのジョブの入力を組むときに引きそうな形（その PR の裁き・本文の直しの記録を古い順に）
            self._rows = [{"id": e["id"], "payload": e["payload"]} for e in self.events if e["name"] == m[1] and e["payload"].get("pr_url") == p[0]]
        elif re.fullmatch(r"select (?:id, )?payload from artifacts where id=%s", flat):
            self._rows = [{"id": a["id"], "payload": a["payload"]} for a in self.artifacts if a["id"] == p[0]]
        else:
            super().execute(sql, params)


def load_engine_functions(engine: Path, namespace: dict) -> dict:
    """sendback-check の取り出し方（構文木から名前で指した関数の定義だけ）で、この担保が使う関数を取り出す"""
    saved = sendback.ENGINE_FUNCTIONS
    sendback.ENGINE_FUNCTIONS = ENGINE_FUNCTIONS
    try:
        return sendback.load_engine_functions(engine, namespace)
    finally:
        sendback.ENGINE_FUNCTIONS = saved


def load_continuation(engine: Path):
    """continuation.py は psycopg と subprocess だけに依存するので、judgment.py と同じく psycopg を差し替えてそのまま読み込む"""
    judgment, psycopg = sendback.load_judgment(engine)   # psycopg の差し替えと、orchestrator/dispatcher を sys.path に足す
    if not (engine / "orchestrator/dispatcher/continuation.py").is_file():
        raise SystemExit(f"Engine のチェックアウトに orchestrator/dispatcher/continuation.py がない（ENGINE_DIR={engine}）")
    import continuation
    missing = [n for n in CONTINUATION_NAMES if not hasattr(continuation, n)]
    if missing:
        raise SystemExit(f"Engine の orchestrator/dispatcher/continuation.py に {', '.join(missing)} がない（名前が変わったら engine_harness.py を合わせる）")
    return judgment, psycopg, continuation


def export_events(cur: FakeCursor, pr_url: str) -> list[dict]:
    """その PR のイベントを古い順に出す（judgment.judged には artifacts の Judgment を添え、ほかは null）"""
    artifacts = {a["id"]: a["payload"] for a in cur.artifacts}
    return [{"name": e["name"], "payload": e["payload"],
             "judgment": artifacts.get(e["payload"].get("artifact_id")) if e["name"] == "judgment.judged" else None}
            for e in cur.events if (e["payload"].get("pr_url") or e["payload"].get("origin_pr")) == pr_url]


def run(engine: Path, case: dict) -> dict:
    judgment, psycopg, continuation = load_continuation(engine)
    pr_url, reason = case["pr_url"], case["reason"]
    repo, pr_number = re.match(r"https://github\.com/([^/]+/[^/]+)/pull/(\d+)$", pr_url).groups()
    sha, branch, corr, mode = "head1", "factory/issue-1-job-1", f"github:{repo}#1", "unattended"
    pr = {"state": "open", "title": "PR", "head": {"sha": sha, "ref": branch, "repo": {"full_name": repo}}}
    cur = FakeCursor()

    def gh(method, path, body=None):
        if method == "GET" and path.endswith(f"/pulls/{pr_number}"):
            return pr
        if method == "POST" and path.endswith("/issues"):
            return {"html_url": f"https://github.com/{repo}/issues/9999", "number": 9999}
        raise AssertionError(f"engine_harness が知らない GitHub の呼び出し: {method} {path}")

    def record_event(cur_, name, correlation_id, job_id, payload, dedupe_key=None):
        cur_.add_event(name, payload, dedupe_key)

    ns = {
        "__name__": "engine_harness_scope", "re": re, "psycopg": psycopg, "judgment": judgment, "continuation": continuation,
        "gh": gh, "record_event": record_event, "comment": lambda *a, **k: None, "swap_label": lambda *a, **k: None,
        "log": lambda *a, **k: None,
        "stoppage": types.SimpleNamespace(open_stoppage=lambda *a, **k: None, close_stoppage=lambda *a, **k: None, open_keys=lambda *a, **k: []),
        "REVIEW_FIX": True, "LOW_CONFIDENCE": 0.6, "SENDBACK_PER_PR": 1, "SENDBACK_DAILY_MAX": 5,
        "CONFLICT_FIX": True, "CI_FIX": True,
        "over_budget": lambda cur_: False, "awaiting_decision": lambda cur_, url, pr_: False,   # 予算の枠の内で、人の判断を待っていない
    }
    load_engine_functions(engine, ns)

    # judge が裁き、採用した本文の指摘を body_fix に回す（supervisor の judge の記録と、dispatcher の後続）
    cur.add_event("supervisor.judge_enqueued", {"pr_url": pr_url, "head_sha": sha, "skipped": False}, f"supervisor.judge:{pr_url}@{sha}")
    inp, verdicts = sendback.judge_io(case.get("judgments") or [])
    summary = judgment.record_judged(cur, corr, 1, pr_url, inp, verdicts, mode, "テスト")
    ns["act_on_judgment"](cur, repo, corr, 1, pr_url, summary, mode)
    if any(e["name"] == "job.enqueued" and e["payload"].get("kind") == "body_fix" for e in cur.events):
        # body_fix が本文を直し終えた（dispatcher が記録する pr.body_amended）。本文の決まりは rules として残す
        cur.add_event("pr.body_amended", {"pr_url": pr_url, "defects": len([d for d in summary["adopted_defects"] if ns["is_body_only"](d)]),
                                          "chars": 100, "rules": list(case.get("body_rules") or [])})

    # 既定ブランチと衝突した・CI が落ちたので、supervisor が続きのジョブを積む
    before = len(cur.events)
    p = {"repo": repo, "number": int(pr_number), "url": pr_url}
    if reason == "conflict":
        ns["conflict_step"](cur, p, pr)
    elif reason == "ci_failed":
        cur.add_event("pr.ci_failed", {"pr_url": pr_url, "head_sha": sha, "failed": ["backend"]})
        before = len(cur.events)
        ns["ci_fix_step"](cur, p, pr)
    else:
        raise SystemExit(f"reason は conflict か ci_failed: {reason}")
    enq = next((e for e in cur.events[before:] if e["name"] == "job.enqueued" and (e["payload"].get("continue") or {}).get("reason") == reason), None)
    events = export_events(cur, pr_url)
    if enq is None:
        return {"enqueued": False, "reason": reason, "input": None, "piped": False, "delivered": None, "events": events}
    # dispatcher.run_job: reason が SIGNALED のときだけ job.enqueued の input（なければ {}）を標準入力で渡す
    piped = reason in continuation.SIGNALED
    given = enq["payload"].get("input")
    upto = next(i for i, e in enumerate(events) if e["payload"] is enq["payload"])
    return {"enqueued": True, "reason": reason, "input": given, "piped": piped, "delivered": (given or {}) if piped else None,
            "events": events[:upto]}


def check(engine: Path) -> dict:
    """続きのジョブは積まず、取り出す関数がすべて Engine のチェックアウトにあるかだけを見る（CI で飛ばさずに落とすため）"""
    load_continuation(engine)
    load_engine_functions(engine, {"__name__": "engine_harness_scope"})
    return {"functions": sendback.JUDGMENT_FUNCTIONS + [n for names in ENGINE_FUNCTIONS.values() for n in names]
            + [f"continuation.{n}" for n in CONTINUATION_NAMES]}


def main():
    if len(sys.argv) == 3 and sys.argv[1] == "--check":
        json.dump(check(Path(sys.argv[2])), sys.stdout, ensure_ascii=False)
        return
    if len(sys.argv) != 2:
        raise SystemExit("使い方: python3 engine_harness.py [--check] <Engine のチェックアウト> < case.json")
    json.dump(run(Path(sys.argv[1]), json.load(sys.stdin)), sys.stdout, ensure_ascii=False)


if __name__ == "__main__":
    main()
