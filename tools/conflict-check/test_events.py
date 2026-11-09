"""events.py（制御プレーンのイベント列を policy.py の線に照らす検査）のテスト（知見 K-0010 の担保。議決 D-0023・Issue #972）。

EngineFinishedTest のほかは Engine は要らない。EngineFinishedTest は Engine の dispatcher.py の job.finished の欄を engine_harness.py で読み、
agent_resumed と解き方の tier（agent_tier）を出す版の Engine で照らせない件数が 1 以上なら落とす（議決 D-0030・Issue #1035）。
Engine のチェックアウトの渡し方と飛ばし方は test_engine.py と同じ（ENGINE_DIR を渡したときと CI=true では飛ばさずに落とす）。
Engine が今の形（job.finished にどちらも出さない。LEGACY_FINISHED）のときだけ「todo:」の理由で飛ばす（K-0004 の engine.test.mjs の todo と同じ形）。
todo は照らせない件数を理由に添えて test_engine.py の todo で数え、終わりに件数を出す。知見 K-0010 の review_by を過ぎても
今の形なら飛ばさずに落とす（議決 D-0043・Issue #1104。照らせない件数が見えない形で緑にならないように）

MissingConflictFilesTest は、衝突したファイルがその場で記録されなかった回を落とす rule missing_conflict_files（知見 K-0014 の担保。議決 D-0049・Issue #1135）を見る。
continue.enqueued の payload の conflict_files（schema.py の定義の欄）をその場の記録として読むことも見る（議決 D-0101・Issue #1208）。
MechanicalWithoutVerifyTest は、mechanical.verify の検査に落ちる head を機械的に解いた回を落とす rule mechanical_without_verify と、
同じ PR・同じファイルの機械的な解消が 3 回を超えた件数（知見 K-0015 の担保。議決 D-0078（Issue #1220 の起票時の番号は D-0077）・Issue #1220）を見る。
RecordOnlySinceTest は、直近の窓の本物のイベント列を定期に照らすジョブ（ci/conflict-schema.yml の conflict-record）が使う --record-only と
--since を見る（議決 D-0076（Issue #1214 の起票時の番号は D-0074）・Issue #1214）
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stderr
from io import StringIO
from unittest import mock
from collections import Counter
from datetime import date, datetime, timedelta
from pathlib import Path

import yaml

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import events  # noqa: E402
import policy  # noqa: E402
import schema  # noqa: E402
import test_engine  # noqa: E402   名前で取り込むと EngineTest がこのモジュールでも走るので、モジュールで取り込む

C0005 = HERE / "fixtures" / "c0005-pr659-events.json"
C0007 = HERE / "fixtures" / "c0007-pr290-pr295-events.json"
C0008 = HERE / "fixtures" / "c0008-pr678-events.json"
K0010 = HERE.parents[1] / ".factory-config" / "knowledge" / "K-0010.yaml"
URL = "https://github.com/guccyon/rally/pull/1"


def stream(attempts: list[dict], url: str = URL) -> list[dict]:
    """attempts の各項目（resumed・tier・files・cost）から、Engine の形のイベント列を作る。files は積んだ直後（job.started の前）に
    conflict.files として載せる（source は既定で supervisor）。late があれば今の Engine と同じく job.finished のあとに載せ（source は既定で
    worker_log）、stop があれば job.finished を出さない（終わらずに止まった回）。inline があれば continue.enqueued の payload の conflict_files に置く"""
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
        enq = {"pr_url": url, "head_sha": sha, "reason": "conflict", "attempt": n}
        if "inline" in a:
            enq["conflict_files"] = a["inline"]
        add("continue.enqueued", enq, job)
        if "files" in a and not a.get("late"):
            add("conflict.files", {"files": a["files"], "source": a.get("source", "supervisor")}, job)
        add("job.started", {"model_tier": a.get("tier", "tier-large")}, job)
        fin = {"state": "succeeded"}
        if "resumed" in a:
            fin["agent_resumed"] = a["resumed"]
        if "cost" in a:
            fin["cost_usd"] = a["cost"]
        if "agent_tier" in a:
            fin["agent_tier"] = a["agent_tier"]
        if not a.get("stop"):
            add("job.finished", fin, job)
        if "files" in a and a.get("late"):
            add("conflict.files", {"files": a["files"], "source": a.get("source", "worker_log")}, job)
    return out


def rules(evs: list[dict], verify: dict | None = None) -> Counter:
    c = policy.load()
    return Counter(e["rule"] for r in events.records_from_events(evs, verify=verify).values() for e in events.check(c, r))


def recorded(evs: list[dict]) -> Counter:
    """record_check（K-0014）の rule の件数"""
    return Counter(e["rule"] for r in events.records_from_events(evs).values() for e in events.record_check(r))


def run(path: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, "-I", "-B", str(HERE / "events.py"), "--events", str(path), *args], capture_output=True, text=True)


class C0005Test(unittest.TestCase):
    def test_C0005のイベント列は3つの線のどれでも落ちる(self):
        evs = events.parse_events(C0005.read_text(encoding="utf-8"))
        self.assertEqual(rules(evs), Counter({"over_limit": 17, "resumed_after_limit": 19, "agent_for_mechanical": 19}))
        r = events.records_from_events(evs)["https://github.com/guccyon/rally/pull/659"]
        self.assertEqual(len(r["attempts"]), 20)   # ci_failed の続きは数えない
        self.assertEqual(events.unknown(r), {"agent_resumed": 0, "tier": 0, "files": 1})   # 13 回目は衝突したファイルの記録がない
        # K-0014 の rule では 20 回すべて落ちる（13 回目は記録がなく、ほかは今の Engine と同じく job.finished のあとに記録されている）
        self.assertEqual(recorded(evs), Counter({"missing_conflict_files": 20}))

    def test_C0005のイベント列はコマンドでも終了コード1(self):
        p = run(C0005)
        self.assertEqual(p.returncode, 1, p.stderr)
        self.assertIn("attempt=4 job=9004 over_limit", p.stdout)
        self.assertIn("attempt=2 job=9002 resumed_after_limit", p.stdout)
        self.assertIn("attempt=1 job=9001 agent_for_mechanical", p.stdout)
        self.assertNotIn("attempt=1 job=9001 resumed_after_limit", p.stdout)   # 1 回目の引き継ぎは線の中
        self.assertNotIn("attempt=3 job=9003 over_limit", p.stdout)
        self.assertIn("attempt=13 job=9013 missing_conflict_files", p.stdout)
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


class MechanicalWithoutVerifyTest(unittest.TestCase):
    """知見 K-0015: PR が CHANGELOG.md を直接書き換えている（mechanical.verify の検査 tools/changelog-check/check.mjs が落ちる）のに、
    機械的に（tier none で）衝突を解いた回を落とす。同じ PR・同じファイルの機械的な解消が 3 回を超えたら件数を出す（議決 D-0078）"""
    PR678 = "https://github.com/guccyon/rally/pull/678"

    def c0008(self) -> tuple[list[dict], dict]:
        text = C0008.read_text(encoding="utf-8")
        return events.parse_events(text), events.parse_verify(text)

    def test_C0008のイベント列はmechanical_without_verifyで落ちる(self):
        evs, verify = self.c0008()
        c = policy.load()
        r = events.records_from_events(evs, verify=verify)[self.PR678]
        self.assertEqual(len(r["attempts"]), 13)
        # over_limit は機械的に解いた回も数える（README の「線にまだ合わせていない」）。衝突したファイルは今の Engine と同じく事後の記録
        self.assertEqual(rules(evs, verify), Counter({"mechanical_without_verify": 13, "over_limit": 10}))
        self.assertEqual(recorded(evs), Counter({"missing_conflict_files": 13}))
        self.assertEqual(events.mechanical_repeats(c, r), {"CHANGELOG.md": 13})
        self.assertEqual(events.unverified(c, r), 0)
        # 検査の結果を渡さなければ照らさず、件数を出す（同じファイルの繰り返しの件数は出す）
        r = events.records_from_events(evs)[self.PR678]
        self.assertEqual(rules(evs), Counter({"over_limit": 10}))
        self.assertEqual((events.unverified(c, r), events.mechanical_repeats(c, r)), (13, {"CHANGELOG.md": 13}))

    def test_C0008のイベント列はコマンドでも終了コード1(self):
        p = run(C0008, "--verify", str(C0008))
        self.assertEqual(p.returncode, 1, p.stderr)
        self.assertIn("attempt=1 job=3414 mechanical_without_verify", p.stdout)
        self.assertIn("attempt=13 job=3822 mechanical_without_verify", p.stdout)
        self.assertIn("同じファイルの機械的な解消が 3 回を超えた: CHANGELOG.md 13 回", p.stdout)
        d = json.loads(run(C0008, "--verify", str(C0008), "--json").stdout)
        self.assertFalse(d["ok"])
        self.assertEqual(d["prs"][0]["mechanical"], {"repeats": {"CHANGELOG.md": 13}, "unverified": 0})
        self.assertNotIn("mechanical_without_verify", run(C0008).stdout)   # --verify がなければ照らさない
        self.assertIn("機械的に解いた回の検査の結果 13 件", run(C0008).stdout)

    def test_線どおりの積み方は通る(self):
        # C-0008 と同じ CHANGELOG.md だけの衝突が 13 回。検査に落ちるのでエージェント（tier-large）の回になり、3 回で停止（4 回目からは積まない）
        evs = stream([{"resumed": True, "files": ["CHANGELOG.md"], "cost": 1.2}]
                     + [{"resumed": False, "files": ["CHANGELOG.md"], "cost": 1.2}] * 2 + [{"skip": True}] * 10, url=self.PR678)
        verify = {self.PR678: {f"h{n}": 1 for n in range(1, 14)}}
        self.assertEqual(rules(evs, verify), Counter())   # 検査に落ちた head をエージェントで解くのは agent_for_mechanical でもない
        r = events.records_from_events(evs, verify=verify)[self.PR678]
        self.assertEqual((events.mechanical_repeats(policy.load(), r), events.unverified(policy.load(), r)), ({}, 0))
        self.assertEqual(rules(evs), Counter({"agent_for_mechanical": 3}))   # 検査の結果がなければ今までどおり

    def test_検査に通ったheadの機械的な解消と衝突のなかった回は落とさない(self):
        evs = stream([{"files": ["CHANGELOG.md"], "cost": 0}, {"files": [], "cost": 0}, {"files": ["changelog.d/700.md"], "cost": 0}])
        verify = {URL: {"h1": 0, "h2": 1, "h3": 0}}
        self.assertEqual(rules(evs, verify), Counter())
        self.assertEqual(rules(evs, {URL: {"h1": 2}}), Counter({"mechanical_without_verify": 1}))   # 読めなかった（2）も通らない
        self.assertEqual(rules(evs, {URL.replace("/1", "/2"): {"h1": 1}}), Counter())   # ほかの PR の結果は使わない

    def test_同じファイルの機械的な解消は3回までは件数を出さない(self):
        c = policy.load()
        r = events.records_from_events(stream([{"files": ["CHANGELOG.md"], "cost": 0}] * 3 + [{"files": ["changelog.d/1.md"], "cost": 0}]))[URL]
        self.assertEqual(events.mechanical_repeats(c, r), {})
        r = events.records_from_events(stream([{"files": ["CHANGELOG.md"], "cost": 0}] * 4 + [{"files": ["CHANGELOG.md"], "cost": 2.0}]))[URL]
        self.assertEqual(events.mechanical_repeats(c, r), {"CHANGELOG.md": 4})   # エージェントで解いた回は数えない

    def test_検査の結果の形が違えば終了コード2(self):
        for bad in ("[]", "{", '{"u": 1}', '{"u": {"h": true}}', '{"verify": {"u": {"h": "1"}}}'):
            with self.assertRaises(events.EventsError, msg=bad):
                events.parse_verify(bad)
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "verify.json"
            path.write_text('{"u": {"h": true}}', encoding="utf-8")
            self.assertEqual(run(C0008, "--verify", str(path)).returncode, 2)


class MissingConflictFilesTest(unittest.TestCase):
    """知見 K-0014: 衝突を解く続きのジョブに、continue.enqueued の時点で（ジョブが動き出す前に。backfill やワーカーのログからの事後の記録ではなく）
    衝突したファイルが記録されているか（議決 D-0049）"""

    def test_C0007のイベント列はmissing_conflict_filesで落ちる(self):
        evs = events.parse_events(C0007.read_text(encoding="utf-8"))
        prs = events.records_from_events(evs)
        self.assertEqual({url: [e["job_id"] for e in events.record_check(r)] for url, r in prs.items()},
                         {"https://github.com/guccyon/rally/pull/290": [309],               # CHANGELOG.md は 2026-11-03 の backfill でだけ分かる
                          "https://github.com/guccyon/rally/pull/295": [318, 325, 338]})    # samples の detail はすべて null
        self.assertEqual(prs["https://github.com/guccyon/rally/pull/290"]["attempts"][0]["files"], ["CHANGELOG.md"])   # backfill の中身は K-0010 の線には使う
        self.assertEqual(rules(evs), Counter({"agent_for_mechanical": 1}))   # K-0010 の線（check）には入れない

    def test_C0007のイベント列はコマンドでも終了コード1(self):
        p = run(C0007)
        self.assertEqual(p.returncode, 1, p.stderr)
        for job in (309, 318, 325, 338):
            self.assertIn(f"job={job} missing_conflict_files", p.stdout)
        d = json.loads(run(C0007, "--json").stdout)
        self.assertFalse(d["ok"])
        self.assertEqual(Counter(e["rule"] for r in d["prs"] for e in r["errors"]),
                         Counter({"missing_conflict_files": 4, "agent_for_mechanical": 1}))

    def test_conflict_filesをその場で載せた列は通る(self):
        evs = events.parse_events(C0007.read_text(encoding="utf-8"))
        jobs = [e["job_id"] for e in evs if e["name"] == "continue.enqueued"]
        evs = [e for e in evs if e["name"] != "conflict.files"]
        for job in jobs:   # 積んだ continue.enqueued のすぐ後ろに載せる（id は並びどおりに振り直す）
            k = next(i for i, e in enumerate(evs) if e["name"] == "continue.enqueued" and e["job_id"] == job)
            evs.insert(k + 1, {"name": "conflict.files", "job_id": job, "payload": {"files": ["CHANGELOG.md"], "source": "supervisor"}})
        evs = [{**e, "id": i} for i, e in enumerate(evs, 1)]
        self.assertEqual(recorded(evs), Counter())
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "events.json"
            path.write_text(json.dumps(evs), encoding="utf-8")
            p = run(path, "--pr", "https://github.com/guccyon/rally/pull/295")   # PR #290 は CHANGELOG.md だけを tier-large で解いた（K-0010 の agent_for_mechanical）
            self.assertEqual(p.returncode, 0, p.stdout + p.stderr)

    def test_衝突がなかった回も空の列をその場で載せていれば通る(self):
        self.assertEqual(recorded(stream([{"files": []}, {"files": ["CHANGELOG.md"], "cost": 0}])), Counter())

    def test_ジョブが動き出したあとのworker_logの記録だけなら落ちる(self):
        # 今の Engine（main の 57069a8）の形: dispatcher がジョブの終わりにワーカーのログから記録する
        self.assertEqual(recorded(stream([{"files": ["CHANGELOG.md"], "late": True}])), Counter({"missing_conflict_files": 1}))
        self.assertEqual(recorded(stream([{"files": ["CHANGELOG.md"], "late": True, "stop": True}])), Counter({"missing_conflict_files": 1}))

    def test_ないかbackfillだけなら落ちsourceのない記録はその場のものとみなす(self):
        self.assertEqual(recorded(stream([{}, {"files": ["CHANGELOG.md"], "source": "backfill"}, {"files": ["a.ts"]}])),
                         Counter({"missing_conflict_files": 2}))
        evs = stream([{"files": ["a.ts"]}])
        del next(e for e in evs if e["name"] == "conflict.files")["payload"]["source"]
        self.assertEqual(recorded(evs), Counter())

    def test_終わらずに止まった回も照らす(self):
        # 終わるのを待たない。積んだ時点で載っていなければ落とし、載っていれば通す
        self.assertEqual(recorded(stream([{"files": ["a.ts"]}, {"stop": True}])), Counter({"missing_conflict_files": 1}))
        self.assertEqual(recorded(stream([{"files": ["a.ts"], "stop": True}])), Counter())
        evs = [e for e in stream([{}]) if e["name"] not in ("job.started", "job.finished")]   # 積んだだけで動き出していない回
        self.assertEqual(recorded(evs), Counter({"missing_conflict_files": 1}))

    def test_ci_failedの続きは数えない(self):
        evs = [{"name": "continue.enqueued", "job_id": 900, "payload": {"pr_url": URL, "head_sha": "x", "reason": "ci_failed", "attempt": 1}},
               {"name": "job.finished", "job_id": 900, "payload": {"state": "succeeded"}}]
        self.assertEqual(recorded(evs), Counter())

    def test_C0007の流れでcontinue_enqueuedにconflict_filesを載せれば通る(self):
        # 議決 D-0101: schema の案（conflict-files.yaml）の形で Engine が積むときに載せた列。backfill の conflict.files は外す
        evs = [e for e in events.parse_events(C0007.read_text(encoding="utf-8")) if e["name"] != "conflict.files"]
        for e in evs:
            if e["name"] == "continue.enqueued":
                e["payload"]["conflict_files"] = ["CHANGELOG.md"]
        self.assertEqual(recorded(evs), Counter())
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "events.json"
            path.write_text(json.dumps(evs), encoding="utf-8")
            p = run(path, "--pr", "https://github.com/guccyon/rally/pull/295")
            self.assertEqual(p.returncode, 0, p.stdout + p.stderr)

    def test_continue_enqueuedのconflict_filesが形どおりでなければ載っていないとみなす(self):
        # schema.py の bad_field・missing_field と同じ線（空でない文字列の列で 1 件以上）。ほかの記録がなければ落ちる
        for bad in ([], None, [""], ["  "], "CHANGELOG.md", [1]):
            self.assertEqual(recorded(stream([{"inline": bad}])), Counter({"missing_conflict_files": 1}), bad)
            self.assertIsNotNone(schema.field_error(schema.load()["continue.enqueued"]["fields"]["conflict_files"], bad), bad)   # 判定は schema.py と同じ
        # 形どおりでなくても、その場の conflict.files があれば今までどおり通る
        self.assertEqual(recorded(stream([{"inline": [], "files": ["a.ts"]}])), Counter())

    def test_conflict_filesの定義を読めなければDefsErrorでコマンドは終了コード2(self):
        # 項目がない・YAML として読めない定義は schema.DefsError に包む（素の KeyError・YAMLError を漏らさない）
        cases = ({}, {"continue.enqueued": {"required": [], "required_when": [], "fields": {}}}, yaml.YAMLError("壊れた定義"))
        self.addCleanup(events._inline_spec.cache_clear)
        for case in cases:
            events._inline_spec.cache_clear()
            load = mock.Mock(side_effect=case) if isinstance(case, Exception) else mock.Mock(return_value=case)
            with mock.patch.object(schema, "load", load):
                with self.assertRaises(schema.DefsError, msg=case):
                    events.records_from_events(stream([{"inline": ["a.ts"]}]))
                events._inline_spec.cache_clear()
                err = StringIO()
                with redirect_stderr(err):
                    self.assertEqual(events.main(["--events", str(C0007)]), 2, case)
                self.assertIn("conflict-files.yaml", err.getvalue())

    def test_continue_enqueuedのconflict_filesはworker_logやbackfillより先に見る(self):
        self.assertEqual(recorded(stream([{"inline": ["a.ts"], "files": ["a.ts"], "late": True},
                                          {"inline": ["a.ts"], "files": ["a.ts"], "source": "backfill"},
                                          {"inline": ["a.ts"], "stop": True}])), Counter())
        # K-0010 の線にもその列を使う（CHANGELOG.md だけの衝突を tier-large で解いた回）
        evs = stream([{"inline": ["CHANGELOG.md"], "cost": 1.2}])
        self.assertEqual(rules(evs), Counter({"agent_for_mechanical": 1}))
        self.assertEqual(events.records_from_events(evs)[URL]["attempts"][0]["files"], ["CHANGELOG.md"])


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
                     "Engine を直して ENGINE_REF を上げる（K-0010 の review_by を延ばすだけでは tools/knowledge-enforcement-check/check.py が落とす。"
                     "議決 D-0043・D-0067）")
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


def timed(evs: list[dict], start: str = "2026-11-10T00:00:00+00:00") -> list[dict]:
    """イベント列の各件に、start から 1 分ずつ進む occurred_at を付ける"""
    t0 = datetime.fromisoformat(start)
    return [{**e, "occurred_at": (t0 + timedelta(minutes=i)).isoformat()} for i, e in enumerate(evs)]


class RecordOnlySinceTest(unittest.TestCase):
    """--record-only は record_check（missing_conflict_files）だけを出し、--since は差分を当てた時刻より前のイベントを組まない
    （知見 K-0014 の担保。議決 D-0076（Issue #1214 の起票時の番号は D-0074）・Issue #1214）"""

    def write(self, evs: list[dict]) -> Path:
        t = tempfile.TemporaryDirectory()
        self.addCleanup(t.cleanup)
        path = Path(t.name) / "events.json"
        path.write_text(json.dumps(evs), encoding="utf-8")
        return path

    def test_C0007のイベント列はrecord_onlyでもmissing_conflict_filesだけで落ちる(self):
        d = json.loads(run(C0007, "--record-only", "--json").stdout)
        self.assertFalse(d["ok"])
        self.assertEqual(Counter(e["rule"] for r in d["prs"] for e in r["errors"]), Counter({"missing_conflict_files": 4}))   # agent_for_mechanical は出さない
        self.assertEqual(run(C0007, "--record-only").returncode, 1)

    def test_C0007と同じ形の衝突でもcontinue_enqueuedにconflict_filesを載せればrecord_onlyで通る(self):
        # 差分を当てた Engine の形。PR #290 の CHANGELOG.md だけを tier-large で解いた回（K-0010 の agent_for_mechanical）も、record_only では照らさない
        evs = [e for e in events.parse_events(C0007.read_text(encoding="utf-8")) if e["name"] != "conflict.files"]
        for e in evs:
            if e["name"] == "continue.enqueued":
                e["payload"]["conflict_files"] = ["CHANGELOG.md"]
        p = run(self.write(evs), "--record-only")
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        self.assertEqual(run(self.write(evs)).returncode, 1)   # --record-only がなければ K-0010 の線でも照らす

    def test_sinceより前のイベントは組まない(self):
        old = stream([{}], url="https://github.com/guccyon/rally/pull/2")                     # 差分の前の Engine の回（記録なし）
        new = stream([{"inline": ["CHANGELOG.md"]}])
        evs = timed(old) + timed(new, "2026-11-10T01:00:00+00:00")
        evs = [{**e, "id": i} for i, e in enumerate(evs, 1)]
        path = self.write(evs)
        self.assertEqual(run(path, "--record-only").returncode, 1)
        p = run(path, "--record-only", "--since", "2026-11-10T09:30:00+09:00")
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        self.assertIn(f"より前の {len(old)} 件は照らさない", p.stdout)
        late = timed(stream([{}], url="https://github.com/guccyon/rally/pull/3"), "2026-11-10T02:00:00+00:00")
        p = run(self.write(evs + [{**e, "id": len(evs) + i} for i, e in enumerate(late, 1)]), "--record-only", "--since", "2026-11-10T00:30:00Z")
        self.assertEqual(p.returncode, 1, p.stdout + p.stderr)
        self.assertIn("pull/3 attempt=1 job=101 missing_conflict_files", p.stdout)

    def test_sinceで絞れないか時刻が読めなければ終了コード2(self):
        evs = timed(stream([{}]))
        del evs[1]["occurred_at"]
        self.assertEqual(run(self.write(evs), "--since", "2026-11-10T00:00:00Z").returncode, 2)
        good = self.write(timed(stream([{"inline": ["a.ts"]}])))
        for since in ("2026-11-10", "2026-11-10T00:00:00", "main"):
            self.assertEqual(run(good, "--since", since).returncode, 2, since)
        self.assertEqual(run(good, "--since", "2026-11-10T00:00:00Z").returncode, 0)


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
            path.write_text("\n".join(json.dumps(e) for e in stream([{"resumed": True, "files": ["a.ts"]}])), encoding="utf-8")
            p = run(path)
            self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
            self.assertIn("外れ 0 件", p.stdout)


if __name__ == "__main__":
    unittest.main()
