"""watch.py（イベント列で線から外れた PR に衝突の停止を開く見張り）のテスト（知見 K-0010 の担保。議決 D-0030・Issue #1034）。
EngineFinishedTest のほかは Engine は要らない。Engine の本物の stoppage.open_stoppage に渡して動かすのは test_engine.py の (4)。

EngineFinishedTest は Engine の dispatcher.py の job.finished の欄で組んだイベント列で、watch.py が照らせずに停止を開かなかった外れの件数が
1 以上なら落とす（議決 D-0042・Issue #1101）。欄の読み方・Engine のチェックアウトの渡し方と飛ばし方・今の形（LEGACY_FINISHED）のときだけの
「todo:」は test_events.py の EngineFinishedTest と同じ（K-0004 の engine.test.mjs の todo と同じ形）。todo は見逃した件数を理由に添えて
test_engine.py の todo で数え、知見 K-0010 の review_by を過ぎても Engine が今の形なら落とすのも同じ（議決 D-0043・Issue #1104）
todo の前に、watch.py が events.py と別の扱い（開く停止の rule の食い違いか、events.py では照らせる外れの見逃し）に変わっていないかを見て、
変わっていれば Engine が今の形でも落とす（議決 D-0062・Issue #1174。委員会の記録は committee/D-0066.yaml）
"""
from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import types
import unittest
from unittest import mock
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import events  # noqa: E402
import policy  # noqa: E402
import test_engine  # noqa: E402   名前で取り込むと EngineTest がこのモジュールでも走るので、モジュールで取り込む
import test_events  # noqa: E402   同じ理由で、テストの型はモジュールから読む
import watch  # noqa: E402
from test_events import C0005, FINISHED_FIELDS, FULL, URL, engine_shaped, legacy_finished, stream, todo_until_review_by  # noqa: E402

PR659 = "https://github.com/guccyon/rally/pull/659"


def run(path: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, "-I", "-B", str(HERE / "watch.py"), "--events", str(path), *args], capture_output=True, text=True)


class FakeStoppage:
    """Engine の stoppage.open_stoppage と同じ引数で受け、同じ key は開き直さない（Engine は dedupe_key で 1 回だけ開く）"""

    def __init__(self):
        self.calls: list[dict] = []
        self.keys: set[str] = set()

    def open_stoppage(self, cur, key, category, what, *, waiting_for="", cause="", detected_by="watcher", started_at=None, prevention="",
                      correlation_id=None, job_id=None, quiet=False):
        self.calls.append({"key": key, "category": category, "what": what, "waiting_for": waiting_for, "detected_by": detected_by,
                           "correlation_id": correlation_id, "job_id": job_id})
        if key in self.keys:
            return False
        self.keys.add(key)
        return True


class StoppagesTest(unittest.TestCase):
    def setUp(self):
        self.c = policy.load()

    def test_C0005のイベント列ではPR659に衝突の停止を1つ開く(self):
        found = watch.stoppages(self.c, events.parse_events(C0005.read_text(encoding="utf-8")))
        self.assertEqual([s["key"] for s in found], [f"ci:{PR659}"])
        s = found[0]
        self.assertEqual(s["rules"], ["agent_for_mechanical", "over_limit", "resumed_after_limit"])
        self.assertEqual((s["category"], s["detected_by"]), ("ticket", "conflict-check"))
        self.assertEqual(s["job_id"], 9001)   # 線から外れた最初の回
        self.assertIn("最初は 1 回目", s["what"])
        self.assertIn("人の判断待ち", s["waiting_for"])

    def test_3つのruleのどれか1つでも停止を開く(self):
        cases = {
            "over_limit": stream([{"resumed": False}] * 4),
            "resumed_after_limit": stream([{"resumed": True}, {"resumed": True}]),
            "agent_for_mechanical": stream([{"files": ["CHANGELOG.md"], "cost": 1.2}]),
        }
        for rule, evs in cases.items():
            with self.subTest(rule=rule):
                found = watch.stoppages(self.c, evs)
                self.assertEqual([(s["key"], s["rules"]) for s in found], [(f"ci:{URL}", [rule])])

    def test_線どおりの積み方と照らせないだけのPRには開かない(self):
        ok = stream([{"resumed": True, "files": ["CHANGELOG.md", "src/a.rs"]}, {"resumed": False, "files": ["CHANGELOG.md"], "cost": 0},
                     {"resumed": False}, {"skip": True}])
        unknown = [e for e in stream([{"files": ["CHANGELOG.md"]}] * 2, url=URL.replace("/1", "/2")) if e["name"] != "job.started"]
        self.assertEqual(watch.stoppages(self.c, ok + unknown), [])

    def test_外れたPRだけに開きprで絞れる(self):
        other = URL.replace("/1", "/2")
        evs = stream([{"resumed": False}] * 4) + stream([{"resumed": False}] * 2, url=other)
        self.assertEqual([s["pr_url"] for s in watch.stoppages(self.c, evs)], [URL])
        self.assertEqual(watch.stoppages(self.c, evs, other), [])

    def test_correlation_idは外れた回のcontinue_enqueuedから取る(self):
        evs = stream([{"resumed": False}] * 4)
        for e in evs:
            if e["name"] == "continue.enqueued":
                e["correlation_id"] = "github:guccyon/rally#1"
        self.assertEqual(watch.stoppages(self.c, evs)[0]["correlation_id"], "github:guccyon/rally#1")


class ApplyTest(unittest.TestCase):
    def test_open_stoppageで開き同じkeyは開き直さない(self):
        found = watch.stoppages(policy.load(), events.parse_events(C0005.read_text(encoding="utf-8")))
        st = FakeStoppage()
        self.assertEqual(watch.apply(object(), st, found), [f"ci:{PR659}"])
        self.assertEqual(watch.apply(object(), st, found), [])   # 次の周期
        self.assertEqual(st.calls[0]["category"], "ticket")
        self.assertEqual(st.calls[0]["job_id"], 9001)

    def test_開く停止がなければ呼ばない(self):
        st = types.SimpleNamespace(open_stoppage=lambda *a, **k: self.fail("開く停止がないのに呼んだ"))
        self.assertEqual(watch.apply(object(), st, []), [])


class CommandTest(unittest.TestCase):
    def test_C0005は終了コード1で開く停止を出す(self):
        p = run(C0005)
        self.assertEqual(p.returncode, 1, p.stderr)
        self.assertIn(f"open ci:{PR659} (agent_for_mechanical, over_limit, resumed_after_limit)", p.stdout)
        d = json.loads(run(C0005, "--json").stdout)
        self.assertEqual([s["key"] for s in d["stoppages"]], [f"ci:{PR659}"])

    def test_開く停止がなければ終了コード0で読めなければ2(self):
        with tempfile.TemporaryDirectory() as tmp:
            ok = Path(tmp) / "ok.jsonl"
            ok.write_text("\n".join(json.dumps(e) for e in stream([{"resumed": True}])), encoding="utf-8")
            p = run(ok)
            self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
            self.assertIn("開く停止 0 件", p.stdout)
            bad = Path(tmp) / "bad.json"
            bad.write_text("{", encoding="utf-8")
            self.assertEqual(run(bad).returncode, 2)
            self.assertEqual(run(Path(tmp) / "none.json").returncode, 2)
            self.assertEqual(run(ok, "--config", str(Path(tmp) / "none.yaml")).returncode, 2)


# 「照らせないだけの PR には開かない」は、Engine が今の形（job.finished に agent_resumed と解き方の tier を出さない版）のときだけの扱い。
# それらを出す版の Engine（ENGINE_REF で固定）では、照らせずに開かなかった外れが 1 件以上なら落とす（議決 D-0042・Issue #1101）。
# 外れは、照らすのに agent_resumed が要る回（2 回目以降の引き継ぎ）と tier が要る回（CHANGELOG だけの衝突をエージェントで解いた）を別の PR に置く
PLANTED = {
    "agent_resumed": ("resumed_after_limit", [{"resumed": True, "agent_tier": "tier-large", "files": ["apps/backend/src/x.rs"], "cost": 1.95},
                                              {"resumed": True, "agent_tier": "tier-large", "files": ["apps/admin-web/src/a.ts"], "cost": 2.4}]),
    "tier": ("agent_for_mechanical", [{"resumed": False, "agent_tier": "tier-large", "files": ["CHANGELOG.md"], "cost": 1.2}]),
}
PLANTED_URLS = {k: URL.replace("/1", f"/{n}") for n, k in enumerate(PLANTED, 2)}
# job.finished の欄によらず events.py で照らせる外れの PR。agent_resumed と agent_tier を continue.enqueued に載せ（events.py はそこも読む）、
# 4 回積む。Engine が今の形でも 3 つの rule が出るので、watch.py が events.py と別の扱いに変わったらここで食い違う（議決 D-0062）
CHECKABLE_URL = URL.replace("/1", f"/{len(PLANTED) + 2}")
CHECKABLE = [({"agent_resumed": False, "agent_tier": "tier-large"}, {"files": ["CHANGELOG.md"], "cost": 1.2}),
             ({"agent_resumed": True, "agent_tier": "tier-large"}, {"files": ["apps/backend/src/x.rs"], "cost": 1.95}),
             ({}, {"files": ["apps/admin-web/src/a.ts"], "cost": 2.4}),
             ({}, {"files": ["apps/admin-web/src/b.ts"], "cost": 2.4})]


def planted(fields: list[str]) -> list[dict]:
    """線どおりの PR（test_events.py の FULL）と PLANTED・CHECKABLE の外れの PR を 1 本にし、job.finished を Engine が出す欄だけにしたイベント列"""
    evs = stream(FULL)
    prs = [(PLANTED_URLS[k], attempts) for k, (_, attempts) in PLANTED.items()] + [(CHECKABLE_URL, [a for _, a in CHECKABLE])]
    for m, (url, attempts) in enumerate(prs, 1):
        for e in stream(attempts, url=url):
            # stream は PR ごとに job_id を 101 から振るので、job_id で結ぶ job.finished が PR をまたがないようにずらす
            e = {**e, "job_id": e["job_id"] + 100 * m if e["job_id"] is not None else None}
            if url == CHECKABLE_URL and e["name"] == "continue.enqueued":
                e["payload"] = {**e["payload"], **CHECKABLE[e["payload"]["attempt"] - 1][0]}
            evs.append(e)
    for n, e in enumerate(evs, 1):
        e["id"] = n   # PR ごとに作った列を 1 本にしたので id を振り直す
    return engine_shaped(evs, fields)


def finished_missed(fields: list[str]) -> dict:
    """Engine が出す欄の job.finished で組んだイベント列で、watch.py が照らせずに停止を開かなかった外れの件数（照らすのに要る項目ごと）。
    線どおりの PR（test_events.py の FULL）も混ぜ、そこに開いたら落とす"""
    found = {s["pr_url"]: s["rules"] for s in watch.stoppages(policy.load(), planted(fields))}
    if URL in found:
        raise AssertionError(f"線どおりの PR に停止を開いた: {found[URL]}")
    return {k: 0 if rule in found.get(PLANTED_URLS[k], []) else 1 for k, (rule, _) in PLANTED.items()}


def finished_diverged(fields: list[str]) -> list[str]:
    """Engine が出す欄の job.finished で組んだイベント列で、watch.py が events.py と別の扱いをした PR ごとの説明（議決 D-0062）。
    watch.py が開く停止の rule が events.py の check の rule と違う PR と、watch.py が見逃した外れのうち events.py ではその項目を照らせる
    （照らせない件数が 0 の）もの。Engine が今の形で todo のあいだも、watch.py の見逃しが events.py の照らせない件数だけで説明できるかを見る"""
    c, evs = policy.load(), planted(fields)
    records = events.records_from_events(evs)
    opened = {s["pr_url"]: s["rules"] for s in watch.stoppages(c, evs)}
    out = [f"{url}: watch.py は {opened.get(url, [])}、events.py は {want}" for url, r in records.items()
           if opened.get(url, []) != (want := sorted({e["rule"] for e in events.check(c, r)}))]
    for k, n in finished_missed(fields).items():
        if n and not events.unknown(records[PLANTED_URLS[k]])[k]:
            out.append(f"{PLANTED_URLS[k]}: events.py では {k} を照らせるのに watch.py が {PLANTED[k][0]} の停止を開かない")
    return out


class FinishedFieldsTest(unittest.TestCase):
    """下の EngineFinishedTest の線そのものを、Engine なしで確かめる"""
    NOW = test_events.FinishedFieldsTest.NOW   # 2026-11-07 の Engine（main の 2490db6）の job.finished の欄
    VARIANTS = (NOW, NOW + ["agent_resumed"], NOW + ["agent_tier"], NOW + list(FINISHED_FIELDS))

    def test_今の形では照らせずに外れを見逃す(self):
        self.assertTrue(legacy_finished(self.NOW))
        self.assertEqual(finished_missed(self.NOW), {"agent_resumed": 1, "tier": 1})

    def test_両方を出す版では外れのすべてに停止を開く(self):
        self.assertEqual(finished_missed(self.NOW + list(FINISHED_FIELDS)), {"agent_resumed": 0, "tier": 0})

    def test_片方だけを出す版ではもう片方の外れを見逃し落ちる側になる(self):
        self.assertEqual(finished_missed(self.NOW + ["agent_resumed"]), {"agent_resumed": 0, "tier": 1})
        self.assertEqual(finished_missed(self.NOW + ["agent_tier"]), {"agent_resumed": 1, "tier": 0})

    def test_どの欄でもwatchはeventsと同じ扱いで見逃しはeventsの照らせない件数だけ(self):
        for fields in self.VARIANTS:
            with self.subTest(fields=fields[len(self.NOW):]):
                self.assertEqual(finished_diverged(fields), [])
        evs = planted(self.NOW)
        self.assertEqual(watch.stoppages(policy.load(), evs, CHECKABLE_URL)[0]["rules"],
                         ["agent_for_mechanical", "over_limit", "resumed_after_limit"])   # 今の形でも照らせる外れの PR

    def test_watchがeventsと別の扱いに変わったらどの欄でも食い違う(self):
        def drop(rule):   # watch.py が rule を 1 つ照らさなくなった形
            return lambda c, r: [e for e in events.check(c, r) if e["rule"] != rule]

        def unknown_as_error(c, r):   # watch.py が照らせない回も外れにするようになった形
            return events.check(c, r) + [{"rule": "unknown", "attempt": a["attempt"], "job_id": a["job_id"], "message": "照らせない"}
                                         for a in r["attempts"] if a["agent_resumed"] is None or a["tier"] is None]

        changed = {f"{rule} を照らさない": drop(rule) for rule in ("over_limit", "resumed_after_limit", "agent_for_mechanical")}
        changed["照らせない回を外れにする"] = unknown_as_error
        for what, check in changed.items():
            fake = types.SimpleNamespace(records_from_events=events.records_from_events, check=check)
            for fields in self.VARIANTS:
                with self.subTest(what=what, fields=fields[len(self.NOW):]), mock.patch.object(watch, "events", fake):
                    try:
                        diverged = finished_diverged(fields)
                    except AssertionError:   # 線どおりの PR に停止を開いた。EngineFinishedTest もここで落ちるので、食い違いを見つけた側に数える
                        continue
                    self.assertNotEqual(diverged, [])


@unittest.skipIf(test_engine.SKIP, test_engine.SKIP)
class EngineFinishedTest(unittest.TestCase):
    def test_job_finishedにagent_resumedと解き方のtierを出す版のEngineでは外れのすべてに停止を開く(self):
        fields = test_engine.harness("finished", str(test_engine.ENGINE_DIR))["fields"]
        self.assertIn("cost_usd", fields, "job.finished の欄を読めている確かめ")
        # todo の前に見る。Engine が今の形のあいだも、watch.py が events.py と別の扱いに変わったら飛ばさずに落とす（議決 D-0062）
        self.assertEqual(finished_diverged(fields), [],
                         f"Engine の job.finished（{', '.join(fields)}）で組んだイベント列で、watch.py が events.py と別の扱いをした")
        if legacy_finished(fields):
            m = finished_missed(fields)
            todo_until_review_by(self, f"見逃した外れ agent_resumed={m['agent_resumed']}・tier={m['tier']}")
        self.assertEqual(finished_missed(fields), {"agent_resumed": 0, "tier": 0},
                         f"Engine の job.finished（{', '.join(fields)}）で組んだイベント列で、照らせずに停止を開かなかった外れがある。"
                         f"{' と '.join(FINISHED_FIELDS)} の両方を出す")


if __name__ == "__main__":
    unittest.main()
