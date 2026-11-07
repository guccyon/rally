"""events.py（制御プレーンのイベント列を policy.py の線に照らす検査）のテスト（知見 K-0010 の担保。議決 D-0023・Issue #972）。Engine は要らない"""
from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import events  # noqa: E402
import policy  # noqa: E402

C0005 = HERE / "fixtures" / "c0005-pr659-events.json"
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
