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
"""
from __future__ import annotations

import importlib.util
import json
import re
import sys
from pathlib import Path

sys.dont_write_bytecode = True   # sendback-check の engine_harness.py を読んでも __pycache__ を書き換えない
_SENDBACK = Path(__file__).resolve().parent.parent / "sendback-check" / "engine_harness.py"
_spec = importlib.util.spec_from_file_location("sendback_engine_harness", _SENDBACK)
sendback = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(sendback)

ENGINE_FUNCTIONS = {"orchestrator/dispatcher/dispatcher.py": ["is_body_only", "enqueue_body_fix"]}
BODY_GATE_FUNCTIONS = {"orchestrator/dispatcher/dispatcher.py": ["is_body_only", "enqueue_body_fix", "act_on_body_gate"]}


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
    if len(sys.argv) != 2:
        raise SystemExit("使い方: python3 engine_harness.py [--check|--body-gate] <Engine のチェックアウト> < body_fix.json")
    json.dump(run(Path(sys.argv[1]), json.load(sys.stdin)), sys.stdout, ensure_ascii=False)


if __name__ == "__main__":
    main()
