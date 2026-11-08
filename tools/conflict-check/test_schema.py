"""衝突したファイルの列（conflict_files）を必須にした定義のテスト（知見 K-0014 の担保。議決 D-0049・Issue #1136）。

schema.py が conflict-files.yaml の定義で、pr.conflicted・continue.enqueued（reason=conflict）・handoff.saved（continue_reason=conflict）の
conflict_files がない・null・空の列・文字列の列でないイベントを落とし、載せたイベントを通すことを見る。C-0005 の流れ（fixtures/c0005-pr659-events.json。
今の Engine の形でイベントに衝突したファイルが載らず、conflict.files を job_id で結ぶだけ）は落ちる。
SinceTest は --since（Engine に差分を当てた時刻）より前のイベントを照らさず、occurred_at がないか読めなければ読めなかったとすることを見る
（議決 D-0101・Issue #1207。定期のワークフローのひな形 ci/conflict-schema.yml が直近の窓のイベント列に使う）。

EngineDefsTest は engine/conflict-files.patch を Engine の definitions/events.yaml の写しに当て、Engine の定義が conflict-files.yaml とそろうかを見る。
Engine のチェックアウトは環境変数 ENGINE_DIR で渡す（なければリポジトリの隣の ../engine）。どちらもなければ飛ばす。
ただし ENGINE_DIR を渡したときと CI（CI=true）では飛ばさずに落とす（test_engine.py と同じ）。

  python3 -B -m unittest discover -s tools/conflict-check -p 'test_schema.py' -v
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import events as ev  # noqa: E402
import schema  # noqa: E402

FIXTURE = HERE / "fixtures" / "c0005-pr659-events.json"
PATCH = HERE / "engine" / "conflict-files.patch"
ENGINE_DIR = Path(os.environ.get("ENGINE_DIR") or HERE.parents[2] / "engine")
REQUIRED = bool(os.environ.get("ENGINE_DIR")) or os.environ.get("CI") == "true"
HAVE_ENGINE = (ENGINE_DIR / "definitions" / "events.yaml").is_file()
SKIP = None if REQUIRED or HAVE_ENGINE else f"Engine のチェックアウトがない（ENGINE_DIR={ENGINE_DIR}）"
PR = "https://github.com/guccyon/rally/pull/290"


def conflicted(**extra) -> dict:
    return {"name": "pr.conflicted", "payload": {"pr_url": PR, "pr_id": "guccyon/rally#290", "head_sha": "h1", "source": "poll", **extra}}


def enqueued(reason: str = "conflict", **extra) -> dict:
    return {"name": "continue.enqueued", "job_id": 1, "payload": {"pr_url": PR, "head_sha": "h1", "reason": reason, "attempt": 1,
                                                                  "continue": {"reason": reason}, "by": "supervisor", **extra}}


def saved(continue_reason: str | None = "conflict", **extra) -> dict:
    return {"name": "handoff.saved", "job_id": 1, "payload": {"volume": "v", "pr_url": PR, "state": "succeeded", "agent_session": True,
                                                              "continue_reason": continue_reason, **extra}}


class SchemaTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.defs = schema.load()

    def rules(self, events: list[dict]) -> list[tuple[str, str]]:
        return [(x["name"], x["rule"]) for x in schema.check(self.defs, events)]

    def test_定義は3つのイベントにconflict_filesを要る(self):
        self.assertEqual(set(self.defs), {"pr.conflicted", "continue.enqueued", "handoff.saved"})
        self.assertEqual(schema.required_fields(self.defs["pr.conflicted"], {}), ["conflict_files"])
        self.assertEqual(schema.required_fields(self.defs["continue.enqueued"], {"reason": "conflict"}), ["conflict_files"])
        self.assertEqual(schema.required_fields(self.defs["handoff.saved"], {"continue_reason": "conflict"}), ["conflict_files"])

    def test_衝突したファイルを載せたイベントは通る(self):
        files = ["CHANGELOG.md", "apps/backend/src/lib.rs"]
        self.assertEqual(self.rules([conflicted(conflict_files=files), enqueued(conflict_files=files), saved(conflict_files=files)]), [])

    def test_欄がないかnullなら落ちる(self):
        self.assertEqual(self.rules([conflicted(), enqueued(conflict_files=None), saved()]),
                         [("pr.conflicted", "missing_field"), ("continue.enqueued", "missing_field"), ("handoff.saved", "missing_field")])

    def test_空の列や形の違う列は落ちる(self):
        for bad in ([], [""], ["  "], "CHANGELOG.md", [1], {"files": ["CHANGELOG.md"]}):
            with self.subTest(bad=bad):
                self.assertEqual(self.rules([conflicted(conflict_files=bad), enqueued(conflict_files=bad), saved(conflict_files=bad)]),
                                 [("pr.conflicted", "bad_field"), ("continue.enqueued", "bad_field"), ("handoff.saved", "bad_field")])

    def test_衝突でない続きのジョブには要らない(self):
        self.assertEqual(self.rules([enqueued("ci_failed"), enqueued("review_fix"), saved(None), saved("ci_failed")]), [])
        # 要らないときも、載せたなら形は見る
        self.assertEqual(self.rules([enqueued("ci_failed", conflict_files=[])]), [("continue.enqueued", "bad_field")])

    def test_ほかのイベントは照らさない(self):
        self.assertEqual(self.rules([{"name": "conflict.files", "job_id": 1, "payload": {"files": [], "source": "backfill"}},
                                     {"name": "job.finished", "job_id": 1, "payload": {}}]), [])

    def test_C0005の流れは衝突したファイルがイベントに載らず落ちる(self):
        events = ev.parse_events(FIXTURE.read_text(encoding="utf-8"))
        errors = schema.check(self.defs, events)
        n = sum(1 for e in events if e["name"] == "pr.conflicted"
                or (e["name"] == "continue.enqueued" and (e.get("payload") or {}).get("reason") == "conflict"))
        self.assertGreater(n, 0)
        self.assertEqual(len(errors), n)
        self.assertEqual({x["rule"] for x in errors}, {"missing_field"})

    def test_定義の形が違えば読めない(self):
        with tempfile.TemporaryDirectory() as t:
            for body in ("{}", "events: {pr.conflicted: {required: conflict_files}}",
                         "events: {continue.enqueued: {required_when: [{when: {}, fields: [conflict_files]}]}}",
                         "events: {pr.conflicted: {fields: {conflict_files: {type: list}}}}",
                         "events: {pr.conflicted: {fields: {conflict_files: {type: paths, min_items: -1}}}}"):
                with self.subTest(body=body):
                    p = Path(t) / "defs.yaml"
                    p.write_text(body, encoding="utf-8")
                    with self.assertRaises(schema.DefsError):
                        schema.load(p)


def at(e: dict, occurred_at) -> dict:
    return {**e, "occurred_at": occurred_at}


APPLIED = "2026-11-10T00:00:00+00:00"


class SinceTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.defs = schema.load()
        cls.since = schema.parse_time(APPLIED)

    def test_差分を当てる前のイベントは照らさない(self):
        events = [at(conflicted(), "2026-11-09T23:59:59+00:00"), at(enqueued(), "2026-11-09T12:00:00Z"),
                  at(conflicted(conflict_files=["CHANGELOG.md"]), APPLIED)]
        self.assertEqual(schema.check(self.defs, events, self.since), [])
        self.assertEqual(schema.before(events, self.since), {0, 1})
        self.assertEqual(len(schema.check(self.defs, events)), 2)   # 渡さなければ前のイベントも照らす

    def test_当てた後のイベントは照らして落とす(self):
        events = [at(conflicted(), "2026-11-09T00:00:00+00:00"), at(enqueued(), "2026-11-10T09:00:00+09:00"),
                  at(saved(conflict_files=[]), "2026-11-11T00:00:00.123456+00:00")]
        errors = schema.check(self.defs, events, self.since)
        self.assertEqual([(x["index"], x["name"], x["rule"]) for x in errors],
                         [(1, "continue.enqueued", "missing_field"), (2, "handoff.saved", "bad_field")])   # 添え字は渡した列のまま

    def test_occurred_atがないか読めないイベントは絞れない(self):
        events = [[conflicted(conflict_files=["a"])]] + [[at(conflicted(conflict_files=["a"]), v)]
                                                        for v in (None, "", "2026-11-10", "2026-11-10T00:00:00", "きのう", 1762732800)]
        for e in events:
            with self.assertRaises(ev.EventsError, msg=e):
                schema.check(self.defs, e, self.since)
        self.assertEqual(schema.check(self.defs, [conflicted(conflict_files=["a"])]), [])   # 渡さなければ見ない

    def test_時差のない時刻は読まない(self):
        with self.assertRaises(ValueError):
            schema.parse_time("2026-11-10T00:00:00")
        self.assertEqual(schema.parse_time("2026-11-10T09:00:00+09:00"), self.since)


class CliTest(unittest.TestCase):
    def run_cli(self, events: list[dict], *args: str) -> subprocess.CompletedProcess:
        with tempfile.TemporaryDirectory() as t:
            p = Path(t) / "events.json"
            p.write_text(json.dumps(events), encoding="utf-8")
            return subprocess.run([sys.executable, "-I", "-B", str(HERE / "schema.py"), "--events", str(p), *args],
                                  capture_output=True, text=True)

    def test_終了コード(self):
        self.assertEqual(self.run_cli([conflicted(conflict_files=["CHANGELOG.md"])]).returncode, 0)
        r = self.run_cli([conflicted()], "--json")
        self.assertEqual(r.returncode, 1)
        self.assertEqual(json.loads(r.stdout)["errors"][0]["rule"], "missing_field")
        self.assertEqual(self.run_cli([conflicted()], "--defs", str(HERE / "no-such.yaml")).returncode, 2)
        self.assertEqual(subprocess.run([sys.executable, "-I", "-B", str(HERE / "schema.py")], capture_output=True).returncode, 2)

    def test_sinceの終了コード(self):
        old, new = at(conflicted(), "2026-11-09T00:00:00Z"), at(conflicted(conflict_files=["CHANGELOG.md"]), "2026-11-10T00:00:01Z")
        r = self.run_cli([old, new], "--since", APPLIED, "--json")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(json.loads(r.stdout), {"ok": True, "errors": [], "skipped": 1})
        self.assertEqual(self.run_cli([old, new]).returncode, 1)
        self.assertEqual(self.run_cli([old, at(conflicted(), "2026-11-10T00:00:01Z")], "--since", APPLIED).returncode, 1)
        self.assertEqual(self.run_cli([conflicted(conflict_files=["a"])], "--since", APPLIED).returncode, 2)   # occurred_at がない
        for bad in ("", "2026-11-10T00:00:00", "main"):
            self.assertEqual(self.run_cli([new], "--since", bad).returncode, 2, bad)
        self.assertEqual(self.run_cli([new], "--since").returncode, 2)


@unittest.skipIf(SKIP, SKIP)
class EngineDefsTest(unittest.TestCase):
    """差分を Engine の definitions/events.yaml の写しに当て、Engine の定義が conflict-files.yaml とそろうか"""

    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        root = Path(cls.tmp.name)
        (root / "definitions").mkdir()
        shutil.copy(ENGINE_DIR / "definitions" / "events.yaml", root / "definitions" / "events.yaml")
        apply = lambda *a: subprocess.run(["git", "apply", *a, str(PATCH)], cwd=root, capture_output=True, text=True)  # noqa: E731
        if apply("--check").returncode == 0:
            r = apply()
            assert r.returncode == 0, r.stderr
        elif apply("--reverse", "--check").returncode != 0:   # もう入っていれば（--reverse --check が通れば）そのまま
            raise AssertionError(f"engine/conflict-files.patch が Engine（ENGINE_DIR={ENGINE_DIR}）に当たらない: {apply('--check').stderr}")
        cls.engine = schema.load(root / "definitions" / "events.yaml")
        cls.ours = schema.load()

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def test_Engineの定義がそろう(self):
        for name, d in self.ours.items():
            with self.subTest(name=name):
                e = self.engine[name]
                self.assertLessEqual(set(d["required"]), set(e["required"]))
                self.assertEqual(d["required_when"], e["required_when"])
                self.assertEqual(d["fields"], e["fields"])

    def test_Engineの定義でも衝突したファイルのないイベントが落ちる(self):
        files = ["CHANGELOG.md"]
        self.assertEqual(schema.check(self.engine, [conflicted(conflict_files=files), enqueued(conflict_files=files), saved(conflict_files=files),
                                                    enqueued("ci_failed"), saved(None)]), [])
        self.assertEqual([(x["name"], x["rule"]) for x in schema.check(self.engine, [conflicted(conflict_files=[]), enqueued(), saved()])],
                         [("pr.conflicted", "bad_field"), ("continue.enqueued", "missing_field"), ("handoff.saved", "missing_field")])


if __name__ == "__main__":
    unittest.main()
