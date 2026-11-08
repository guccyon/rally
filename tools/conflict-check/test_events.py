"""events.py（制御プレーンのイベント列を policy.py の線に照らす検査）のテスト（知見 K-0010 の担保。議決 D-0023・Issue #972）。

EngineFinishedTest のほかは Engine は要らない。EngineFinishedTest は Engine の dispatcher.py の job.finished の欄を engine_harness.py で読み、
agent_resumed と解き方の tier（agent_tier）を出す版の Engine で照らせない件数が 1 以上なら落とす（議決 D-0030・Issue #1035）。
Engine のチェックアウトの渡し方と飛ばし方は test_engine.py と同じ（ENGINE_DIR を渡したときと CI=true では飛ばさずに落とす）。
Engine が今の形（job.finished にどちらも出さない。LEGACY_FINISHED）のときだけ「todo:」の理由で飛ばす（K-0004 の engine.test.mjs の todo と同じ形）。
todo は照らせない件数を理由に添えて test_engine.py の todo で数え、終わりに件数を出す。知見 K-0010 の review_by を過ぎても
今の形なら飛ばさずに落とす（議決 D-0043・Issue #1104。照らせない件数が見えない形で緑にならないように）
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from collections import Counter
from datetime import date
from pathlib import Path

import yaml

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import events  # noqa: E402
import policy  # noqa: E402
import test_engine  # noqa: E402   名前で取り込むと EngineTest がこのモジュールでも走るので、モジュールで取り込む

C0005 = HERE / "fixtures" / "c0005-pr659-events.json"
K0010 = HERE.parents[1] / ".factory-config" / "knowledge" / "K-0010.yaml"
URL = "https://github.com/guccyon/rally/pull/1"


def stream(attempts: list[dict], url: str = URL) -> list[dict]:
    """attempts の各項目（resumed・tier・files・cost）から、Engine の形のイベント列を作る"""
    out, i = [], 0

    def add(name, payload, job_id=None):
        nonlocal i
        i += 1
        out.append({"id": i, "name": name, "job_id": job_id, "payload": payload})

    for n, a in enumerate(attempts, 1):
        sha, job = f"h{n}", 100 + n
        add("pr.conflicted", {"pr_url": url, "head_sha": sha})
        if a.get("skip"):
            continue   # 上限を超えたので積まなかった（停止を開いた）
        add("continue.enqueued", {"pr_url": url, "head_sha": sha, "reason": "conflict", "attempt": n}, job)
        add("job.started", {"model_tier": a.get("tier", "tier-large")}, job)
        fin = {"state": "succeeded"}
        if "resumed" in a:
            fin["agent_resumed"] = a["resumed"]
        if "cost" in a:
            fin["cost_usd"] = a["cost"]
        if "agent_tier" in a:
            fin["agent_tier"] = a["agent_tier"]
        add("job.finished", fin, job)
        if "files" in a:
            add("conflict.files", {"files": a["files"]}, job)
    return out


def rules(evs: list[dict]) -> Counter:
    c = policy.load()
    return Counter(e["rule"] for r in events.records_from_events(evs).values() for e in events.check(c, r))


def run(path: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, "-I", "-B", str(HERE / "events.py"), "--events", str(path), *args], capture_output=True, text=True)


class C0005Test(unittest.TestCase):
    def test_C0005のイベント列は3つの線のどれでも落ちる(self):
        evs = events.parse_events(C0005.read_text(encoding="utf-8"))
        self.assertEqual(rules(evs), Counter({"over_limit": 17, "resumed_after_limit": 19, "agent_for_mechanical": 19}))
        r = events.records_from_events(evs)["https://github.com/guccyon/rally/pull/659"]
        self.assertEqual(len(r["attempts"]), 20)   # ci_failed の続きは数えない
        self.assertEqual(events.unknown(r), {"agent_resumed": 0, "tier": 0, "files": 1})   # 13 回目は衝突したファイルの記録がない

    def test_C0005のイベント列はコマンドでも終了コード1(self):
        p = run(C0005)
        self.assertEqual(p.returncode, 1, p.stderr)
        self.assertIn("attempt=4 job=9004 over_limit", p.stdout)
        self.assertIn("attempt=2 job=9002 resumed_after_limit", p.stdout)
        self.assertIn("attempt=1 job=9001 agent_for_mechanical", p.stdout)
        self.assertNotIn("attempt=1 job=9001 resumed_after_limit", p.stdout)   # 1 回目の引き継ぎは線の中
        self.assertNotIn("attempt=3 job=9003 over_limit", p.stdout)
        d = json.loads(run(C0005, "--json").stdout)
        self.assertFalse(d["ok"])
        self.assertEqual(d["prs"][0]["attempts"], 20)


class LineTest(unittest.TestCase):
    def test_線どおりの積み方は通る(self):
        evs = stream([
            {"resumed": True, "files": ["CHANGELOG.md", "apps/backend/src/x.rs"]},         # 1 回目は引き継いでよい・ほかも衝突したら tier-large
            {"resumed": False, "files": ["CHANGELOG.md", "changelog.d/700.md"], "cost": 0},  # 機械的に解いた回（費用 0）
            {"resumed": False, "files": ["apps/admin-web/src/a.ts"]},
            {"skip": True}, {"skip": True},                                               # 4 回目からは積まない
        ])
        self.assertEqual(rules(evs), Counter())

    def test_4回目が積まれたら落ちる(self):
        self.assertEqual(rules(stream([{"resumed": False}] * 4)), Counter({"over_limit": 1}))

    def test_attemptが数えた回より大きければそれで数える(self):
        evs = [{"name": "continue.enqueued", "job_id": 1, "payload": {"pr_url": URL, "head_sha": "h", "reason": "conflict", "attempt": 4}}]
        self.assertEqual(rules(evs), Counter({"over_limit": 1}))

    def test_2回目以降で引き継いだら落ちる(self):
        self.assertEqual(rules(stream([{"resumed": True}, {"resumed": True}, {"resumed": False}])), Counter({"resumed_after_limit": 1}))

    def test_CHANGELOGと断片だけの衝突でtier_largeなら落ちる(self):
        for files in (["CHANGELOG.md"], ["changelog.d/700.md"], ["CHANGELOG.md", "changelog.d/700.md"]):
            self.assertEqual(rules(stream([{"files": files, "cost": 1.2}])), Counter({"agent_for_mechanical": 1}), files)
            self.assertEqual(rules(stream([{"files": files, "cost": 0}])), Counter(), files)   # エージェントを使わなかった

    def test_記録されたagent_tierを先に見る(self):
        evs = stream([{"files": ["CHANGELOG.md"], "cost": 0}])
        evs[1]["payload"]["agent_tier"] = "tier-small"
        self.assertEqual(rules(evs), Counter({"agent_for_mechanical": 1}))
        evs = stream([{"files": ["CHANGELOG.md"], "cost": 3.0}])
        evs[1]["payload"]["agent_tier"] = "none"
        self.assertEqual(rules(evs), Counter())

    def test_衝突がないかほかのファイルも衝突したならtier_largeでも落ちない(self):
        self.assertEqual(rules(stream([{"files": [], "cost": 1.0}, {"files": ["CHANGELOG.md", "docs/a.md"], "cost": 1.0}])), Counter())

    def test_記録がない項目は照らさず件数を返す(self):
        evs = [e for e in stream([{"files": ["CHANGELOG.md"]}] * 2) if e["name"] != "job.started"]
        r = events.records_from_events(evs)[URL]
        self.assertEqual(rules(evs), Counter())
        self.assertEqual(events.unknown(r), {"agent_resumed": 2, "tier": 2, "files": 0})

    def test_ci_failedの続きとほかのPRは数えない(self):
        evs = stream([{}, {}, {}], url=URL) + stream([{}, {}], url=URL.replace("/1", "/2"))
        evs += [{"name": "continue.enqueued", "job_id": 900, "payload": {"pr_url": URL, "head_sha": "x", "reason": "ci_failed", "attempt": 1}}]
        self.assertEqual(rules(evs), Counter())
        self.assertEqual(set(events.records_from_events(evs, URL)), {URL})

    def test_idの順に並べて数える(self):
        evs = list(reversed(stream([{"resumed": True}, {"resumed": False}])))
        r = events.records_from_events(events.parse_events(json.dumps(evs)))[URL]
        self.assertEqual([(a["attempt"], a["agent_resumed"]) for a in r["attempts"]], [(1, True), (2, False)])


# 「記録がない項目は照らさずに件数を出す」は、Engine が今の形（job.finished に agent_resumed と解き方の tier を出さない版）のときだけの扱い。
# それらを出す版の Engine（ENGINE_REF で固定）では、照らせない件数が 1 以上なら落とす（議決 D-0030・Issue #1035）
FINISHED_FIELDS = ("agent_resumed", "agent_tier")
LEGACY_TODO = "job.finished に agent_resumed と解き方の tier（agent_tier）を出さない（README の「今の Engine との違い」。Engine を直して ENGINE_REF を上げたら消す）"


def legacy_finished(fields: list[str]) -> bool:
    """今の Engine の形: ワーカーを動かし終えたジョブの job.finished に agent_resumed も agent_tier もない"""
    return not set(FINISHED_FIELDS) & set(fields)


def engine_shaped(evs: list[dict], fields: list[str]) -> list[dict]:
    """イベント列の job.finished を Engine が出す欄だけにする。解き方の tier を積んだ段（job.started の model_tier。continuation.enqueue は常に
    tier-large で積む）から推さないよう、model_tier も外す"""
    out = []
    for e in evs:
        p = e.get("payload") or {}
        if e["name"] == "job.finished":
            p = {k: v for k, v in p.items() if k in fields}
        elif e["name"] == "job.started":
            p = {k: v for k, v in p.items() if k != "model_tier"}
        out.append({**e, "payload": p})
    return out


# エージェントで解いた回・機械的に解いた回（tier は none でも費用は 0 と限らない）・2 回目以降の引き継がない回。どれも記録はそろっている
FULL = [{"resumed": True, "agent_tier": "tier-large", "files": ["CHANGELOG.md", "apps/backend/src/x.rs"], "cost": 1.95},
        {"resumed": False, "agent_tier": "none", "files": ["CHANGELOG.md", "changelog.d/700.md"], "cost": 0.01},
        {"resumed": False, "agent_tier": "tier-large", "files": ["apps/admin-web/src/a.ts"], "cost": 2.4}]


def review_by(path: Path = K0010) -> date:
    """知見 K-0010 の review_by（再確認の期限）。読めなければ落とす（期限の検査を黙って外さない）"""
    v = (yaml.safe_load(path.read_text(encoding="utf-8")) or {}).get("review_by")
    return v if isinstance(v, date) else date.fromisoformat(str(v))


def today() -> date:
    """今日の日付。CONFLICT_CHECK_TODAY（YYYY-MM-DD）で差し替えられる（core-gates/knowledge_check.py の --today と同じ使い方）"""
    v = os.environ.get("CONFLICT_CHECK_TODAY")
    return date.fromisoformat(v) if v else date.today()


def overdue(now: date, deadline: date) -> bool:
    """review_by を過ぎたか。knowledge_check.py と同じく、期限の日の翌日から過ぎたとみなす"""
    return deadline < now


def todo_until_review_by(case: unittest.TestCase, detail: str):
    """Engine が今の形のときの扱い。K-0010 の review_by を過ぎていれば落とし、それまでは detail（照らせない件数など）を添えて
    test_engine.todo で飛ばす（議決 D-0043・Issue #1104）。test_events.py と test_watch.py の EngineFinishedTest が使う"""
    deadline = review_by()
    case.assertFalse(overdue(today(), deadline),
                     f"知見 K-0010 の review_by（{deadline}）を過ぎても、ENGINE_REF の Engine は {LEGACY_TODO}（{detail}）。"
                     "Engine を直して ENGINE_REF を上げるか、委員会で K-0010 を再確認して review_by を動かす（議決 D-0043）")
    test_engine.todo(case, f"今の Engine は {LEGACY_TODO}（{detail}。K-0010 の review_by {deadline} を過ぎたら落とす）")


def finished_unknown(fields: list[str]) -> dict:
    """Engine が出す欄の job.finished で組んだイベント列の、agent_resumed と tier の照らせない件数"""
    r = events.records_from_events(engine_shaped(stream(FULL), fields))[URL]
    u = events.unknown(r)
    return {k: u[k] for k in ("agent_resumed", "tier")}


class FinishedFieldsTest(unittest.TestCase):
    """下の EngineFinishedTest の線そのものを、Engine なしで確かめる"""
    NOW = ["billable_tokens", "blob_key", "claude_exit", "cost_usd", "error", "exit", "incomplete", "over_budget", "state",
           "total_tokens", "usage_limited"]   # 2026-11-07 の Engine（main の 2490db6）の job.finished の欄

    def test_todoはEngineの今の形にだけ効きどちらかを出す版は必須にする(self):
        self.assertTrue(legacy_finished(self.NOW))
        self.assertFalse(legacy_finished(self.NOW + ["agent_resumed"]))
        self.assertFalse(legacy_finished(self.NOW + ["agent_tier"]))
        self.assertFalse(legacy_finished(self.NOW + list(FINISHED_FIELDS)))

    def test_今の形では照らせない件数が出る(self):
        self.assertEqual(finished_unknown(self.NOW), {"agent_resumed": 3, "tier": 3})

    def test_両方を出す版では照らせない件数が0で外れも照らせる(self):
        fields = self.NOW + list(FINISHED_FIELDS)
        self.assertEqual(finished_unknown(fields), {"agent_resumed": 0, "tier": 0})
        evs = engine_shaped(stream([{**FULL[0], "agent_tier": "tier-large", "files": ["CHANGELOG.md"]}, {**FULL[1], "resumed": True}]), fields)
        self.assertEqual(rules(evs), Counter({"agent_for_mechanical": 1, "resumed_after_limit": 1}))

    def test_片方だけを出す版ではもう片方が照らせず落ちる側になる(self):
        self.assertEqual(finished_unknown(self.NOW + ["agent_resumed"]), {"agent_resumed": 0, "tier": 3})
        self.assertEqual(finished_unknown(self.NOW + ["agent_tier"]), {"agent_resumed": 3, "tier": 0})

    def test_todoはK0010のreview_byの日までで翌日からは落とす側になる(self):
        deadline = review_by()   # 2026-12-05（議決 D-0043 の案の期限）。知見を再確認して review_by を動かせばそれに付いていく
        self.assertFalse(overdue(date.fromordinal(deadline.toordinal() - 1), deadline))
        self.assertFalse(overdue(deadline, deadline))
        self.assertTrue(overdue(date.fromordinal(deadline.toordinal() + 1), deadline))

    def test_review_byを読めなければ落ちる(self):
        with tempfile.TemporaryDirectory() as tmp:
            for text in ("id: K-0010\n", "review_by: いつか\n"):
                path = Path(tmp) / "K.yaml"
                path.write_text(text, encoding="utf-8")
                with self.assertRaises(ValueError, msg=text):
                    review_by(path)

    def test_todoの件数と中身を出しGitHub_Actionsでは注記にする(self):
        todos = ["test_events.EngineFinishedTest.x: 今の Engine は …"]
        out = test_engine.todo_summary(todos)
        self.assertIn("todo で飛ばしたテスト 1 件", out[0])
        self.assertEqual(out[1:], [f"  - {todos[0]}"])
        self.assertEqual(test_engine.todo_summary(todos, github=True)[-1], f"::warning title=conflict-check todo::{todos[0]}")
        self.assertIn("0 件", test_engine.todo_summary([])[0])


@unittest.skipIf(test_engine.SKIP, test_engine.SKIP)
class EngineFinishedTest(unittest.TestCase):
    def test_job_finishedにagent_resumedと解き方のtierを出す版のEngineでは照らせない件数が0(self):
        fields = test_engine.harness("finished", str(test_engine.ENGINE_DIR))["fields"]
        self.assertIn("cost_usd", fields, "job.finished の欄を読めている確かめ")
        if legacy_finished(fields):
            u = finished_unknown(fields)
            todo_until_review_by(self, f"照らせない件数 agent_resumed={u['agent_resumed']}・tier={u['tier']}")
        self.assertEqual(finished_unknown(fields), {"agent_resumed": 0, "tier": 0},
                         f"Engine の job.finished（{', '.join(fields)}）で組んだイベント列に照らせない回がある。"
                         f"{' と '.join(FINISHED_FIELDS)} の両方を出す")


class InputTest(unittest.TestCase):
    def test_JSONとJSONLとeventsの包みを読める(self):
        evs = stream([{}])
        for text in (json.dumps(evs), json.dumps({"events": evs}), "\n".join(json.dumps(e) for e in evs)):
            self.assertEqual(len(events.parse_events(text)), len(evs))

    def test_読めない記録は終了コード2(self):
        bad = {"壊れた JSON": "{", "name がない": '[{"payload": {}}]', "events が配列でない": '{"events": 1}', "payload が配列": '[{"name": "x", "payload": []}]'}
        with tempfile.TemporaryDirectory() as tmp:
            for name, text in bad.items():
                path = Path(tmp) / "events.json"
                path.write_text(text, encoding="utf-8")
                with self.assertRaises(events.EventsError, msg=name):
                    events.parse_events(text)
                self.assertEqual(run(path).returncode, 2, name)
            self.assertEqual(run(Path(tmp) / "none.json").returncode, 2)

    def test_通る記録は終了コード0(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "events.jsonl"
            path.write_text("\n".join(json.dumps(e) for e in stream([{"resumed": True}])), encoding="utf-8")
            p = run(path)
            self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
            self.assertIn("外れ 0 件", p.stdout)


if __name__ == "__main__":
    unittest.main()
