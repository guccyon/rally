"""ci/conflict-schema.yml（衝突したファイルの列を直近の窓のイベント列で照らす定期のワークフローのひな形）のテスト
（知見 K-0014 の担保。議決 D-0101（Issue #1207 の起票時の番号は D-0073。PR #1204）・Issue #1207）。Engine は要らない。

ひな形が、毎日と手動で必ず走り（schedule の cron と workflow_dispatch・if: と continue-on-error なし）、CONFLICT_FILES_SINCE（Engine に
engine/conflict-files.patch を当てた時刻）が空か形が違えば最初の手順で落ち、シークレットがなければ読まずに落ち、window-events.sql で読んだ列を
schema.py に --since を渡して照らし、外れの終了コードを握りつぶさない形かを見る。.github/workflows/conflict-schema.yml に写してあれば、
CONFLICT_FILES_SINCE の行のほかがひな形と同じで、その値が時刻であるかも見る（写すときに変えてよいのはその行だけ）。
WindowSqlTest は window-events.sql を DATABASE_URL の Postgres の使い捨ての schema で走らせ、差分を当てる前・窓の外・ほかのイベントを出さず、
出した列が schema.py で読めることを見る。psql か DATABASE_URL がなければ飛ばす。

RecordTemplateTest・RecordSqlTest は、同じひな形の conflict-record のジョブ（record-events.sql で読んだ列を events.py --record-only で照らす。
議決 D-0076（Issue #1214 の起票時の番号は D-0074。PR #1203）・Issue #1214）の形・手順の振る舞いと、SQL が窓に積まれた衝突の続きのジョブと
それに結ぶイベントだけを出すことを同じように見る。

  python3 -B -m unittest discover -s tools/conflict-check -p 'test_schema_ci.py' -v
"""
from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
import uuid
from datetime import datetime, timedelta, timezone
from pathlib import Path

import yaml

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import events  # noqa: E402
import schema  # noqa: E402

REPO = HERE.parents[1]
TEMPLATE = HERE / "ci" / "conflict-schema.yml"
COPY = REPO / ".github" / "workflows" / "conflict-schema.yml"
SQL = HERE / "window-events.sql"
RECORD_SQL = HERE / "record-events.sql"
SINCE_LINE = re.compile(r"^(\s*CONFLICT_FILES_SINCE:).*$", re.M)


def load(path: Path) -> dict:
    d = yaml.safe_load(path.read_text(encoding="utf-8"))
    if True in d:   # YAML 1.1 では on: が真偽になる
        d["on"] = d.pop(True)
    return d


def bash(script: str, **env: str) -> subprocess.CompletedProcess:
    """Actions の shell: bash と同じ bash -e -o pipefail で手順を動かす"""
    return subprocess.run(["bash", "--noprofile", "--norc", "-e", "-o", "pipefail", "-c", script], cwd=REPO,
                          env={"PATH": os.environ.get("PATH", "/usr/bin:/bin"), **env}, capture_output=True, text=True)


class TemplateTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.wf = load(TEMPLATE)
        cls.job = cls.wf["jobs"]["conflict-schema"]
        cls.steps = cls.job["steps"]

    def step(self, needle: str) -> dict:
        found = [s for s in self.steps if needle in (s.get("run") or "")]
        self.assertEqual(len(found), 1, needle)
        return found[0]

    def test_毎日と手動で絞り込みなしで走る(self):
        on = self.wf["on"]
        self.assertEqual(set(on), {"schedule", "workflow_dispatch"})   # PR には紐づかない記録を見るので PR では走らせない
        self.assertTrue(any(isinstance(c, dict) and str(c.get("cron", "")).strip() for c in on["schedule"]))
        self.assertEqual(list(self.wf["jobs"]), ["conflict-schema", "conflict-record"])   # conflict-record は RecordTemplateTest
        self.assertNotIn("if", self.job)
        self.assertNotIn("continue-on-error", self.job)
        for s in self.steps:
            self.assertNotIn("if", s, s)
            self.assertNotIn("continue-on-error", s, s)

    def test_ひな形のCONFLICT_FILES_SINCEは空で最初の手順で落ちる(self):
        self.assertEqual(self.wf["env"]["CONFLICT_FILES_SINCE"], "")   # 差分を当てる前は写しても照らさずに落ちる
        guard = self.steps[0]
        self.assertIn("CONFLICT_FILES_SINCE", guard["run"])
        self.assertEqual(guard.get("shell"), "bash")
        for since, rc in (("", 1), ("2026-11-10", 1), ("2026-11-10T00:00:00", 1), ("main", 1), ("2026-11-10T00:00:00Z; true", 1),
                          ("2026-11-10T00:00:00Z", 0), ("2026-11-10T09:00:00+09:00", 0), ("2026-11-10T00:00:00.5+00:00", 0)):
            p = bash(guard["run"], CONFLICT_FILES_SINCE=since)
            self.assertEqual(p.returncode, rc, since)
            if rc:
                self.assertIn("conflict_files_since_missing", p.stdout)

    def test_最初の手順で通る時刻はschema_pyも読める(self):
        for since in ("2026-11-10T00:00:00Z", "2026-11-10T09:00:00+09:00", "2026-11-10T00:00:00.5+00:00"):
            schema.parse_time(since)

    def test_シークレットがなければ読まずに落ちる(self):
        read = self.step("window-events.sql")
        self.assertEqual(read["env"]["DATABASE_URL"], "${{ secrets.CONFLICT_EVENTS_DATABASE_URL }}")
        self.assertIn("ON_ERROR_STOP=1", read["run"])
        self.assertIn('since="$CONFLICT_FILES_SINCE"', read["run"])
        self.assertIn('window_days="$WINDOW_DAYS"', read["run"])
        self.assertIn("-f tools/conflict-check/window-events.sql > conflict-events.json", read["run"])
        self.assertTrue(SQL.is_file())
        p = bash(read["run"], DATABASE_URL="", CONFLICT_FILES_SINCE="2026-11-10T00:00:00Z", WINDOW_DAYS="8")
        self.assertEqual(p.returncode, 1)
        self.assertIn("events_unreadable", p.stdout)
        self.assertRegex(self.wf["env"]["WINDOW_DAYS"], r"^[1-9][0-9]*$")

    def test_schema_pyにsinceを渡して外れで落ちる(self):
        run = self.step("tools/conflict-check/schema.py")
        self.assertIn('python3 -B tools/conflict-check/schema.py --events conflict-events.json --since "$CONFLICT_FILES_SINCE"', run["run"])
        self.assertNotIn("--defs", run["run"])   # 定義は conflict-files.yaml（EngineDefsTest が Engine の定義とそろうことを見る）
        old = {"name": "pr.conflicted", "occurred_at": "2026-11-09T00:00:00+00:00", "payload": {"pr_url": "u"}}
        new = {"name": "pr.conflicted", "occurred_at": "2026-11-10T00:00:01+00:00", "payload": {"pr_url": "u", "conflict_files": ["CHANGELOG.md"]}}
        bad = {"name": "continue.enqueued", "occurred_at": "2026-11-10T00:00:02+00:00", "payload": {"pr_url": "u", "reason": "conflict"}}
        with tempfile.TemporaryDirectory() as t:
            script = run["run"].replace("tools/conflict-check/schema.py", str(HERE / "schema.py")) \
                               .replace("conflict-events.json", f"{t}/events.json").replace("conflict-schema.txt", f"{t}/out.txt")
            for events, rc in (([old, new], 0), ([old, new, bad], 1), ([{"name": "pr.conflicted", "payload": {}}], 2)):
                Path(t, "events.json").write_text(json.dumps(events), encoding="utf-8")
                summary = Path(t, "summary.md")
                summary.write_text("", encoding="utf-8")
                p = bash(script, CONFLICT_FILES_SINCE="2026-11-10T00:00:00Z", GITHUB_STEP_SUMMARY=str(summary))
                self.assertEqual(p.returncode, rc, p.stdout + p.stderr)
                if rc != 2:
                    self.assertIn("conflict-check schema:", summary.read_text(encoding="utf-8"))

    def test_写しがあればCONFLICT_FILES_SINCEの行のほかはひな形と同じ(self):
        if not COPY.is_file():
            self.skipTest(".github/workflows/conflict-schema.yml にまだ写していない（建設者が Engine に差分を当ててから写す）")
        strip = lambda p: SINCE_LINE.sub(r"\1", p.read_text(encoding="utf-8"))
        self.assertEqual(strip(COPY), strip(TEMPLATE))
        schema.parse_time(load(COPY)["env"]["CONFLICT_FILES_SINCE"])


class RecordTemplateTest(unittest.TestCase):
    """conflict-record のジョブ（議決 D-0076（Issue #1214 の起票時の番号は D-0074）・Issue #1214）"""

    @classmethod
    def setUpClass(cls):
        cls.job = load(TEMPLATE)["jobs"]["conflict-record"]
        cls.steps = cls.job["steps"]

    def step(self, needle: str) -> dict:
        found = [s for s in self.steps if needle in (s.get("run") or "")]
        self.assertEqual(len(found), 1, needle)
        return found[0]

    def test_絞り込みなしで走り最初の手順で時刻を確かめる(self):
        self.assertNotIn("if", self.job)
        self.assertNotIn("needs", self.job)   # conflict-schema が落ちても照らす
        self.assertNotIn("continue-on-error", self.job)
        for s in self.steps:
            self.assertNotIn("if", s, s)
            self.assertNotIn("continue-on-error", s, s)
        guard = self.steps[0]
        self.assertEqual(guard.get("shell"), "bash")
        for since, rc in (("", 1), ("2026-11-10", 1), ("2026-11-10T00:00:00", 1), ("2026-11-10T00:00:00Z; true", 1),
                          ("2026-11-10T00:00:00Z", 0), ("2026-11-10T09:00:00+09:00", 0)):
            p = bash(guard["run"], CONFLICT_FILES_SINCE=since)
            self.assertEqual(p.returncode, rc, since)
            if rc:
                self.assertIn("conflict_files_since_missing", p.stdout)

    def test_シークレットがなければ読まずに落ちる(self):
        read = self.step("record-events.sql")
        self.assertEqual(read["env"]["DATABASE_URL"], "${{ secrets.CONFLICT_EVENTS_DATABASE_URL }}")
        self.assertIn("ON_ERROR_STOP=1", read["run"])
        self.assertIn('since="$CONFLICT_FILES_SINCE"', read["run"])
        self.assertIn('window_days="$WINDOW_DAYS"', read["run"])
        self.assertIn("-f tools/conflict-check/record-events.sql > conflict-record-events.json", read["run"])
        self.assertTrue(RECORD_SQL.is_file())
        p = bash(read["run"], DATABASE_URL="", CONFLICT_FILES_SINCE="2026-11-10T00:00:00Z", WINDOW_DAYS="8")
        self.assertEqual(p.returncode, 1)
        self.assertIn("events_unreadable", p.stdout)

    def test_events_pyにrecord_onlyとsinceを渡して外れで落ちる(self):
        run = self.step("tools/conflict-check/events.py")
        self.assertIn('python3 -B tools/conflict-check/events.py --events conflict-record-events.json --since "$CONFLICT_FILES_SINCE" --record-only',
                      run["run"])
        url = "https://github.com/guccyon/rally/pull/1"
        enq = lambda i, at, p: {"id": i, "name": "continue.enqueued", "job_id": 100 + i, "occurred_at": at,
                                "payload": {"pr_url": url, "head_sha": f"h{i}", "reason": "conflict", "attempt": i, **p}}
        old = enq(1, "2026-11-09T00:00:00+00:00", {})                                          # 差分の前（照らさない）
        new = enq(2, "2026-11-10T00:00:01+00:00", {"conflict_files": ["CHANGELOG.md"]})       # 積んだその場で載せた
        bad = enq(3, "2026-11-10T00:00:02+00:00", {})                                          # 差分のあとなのに載せていない
        with tempfile.TemporaryDirectory() as t:
            script = run["run"].replace("tools/conflict-check/events.py", str(HERE / "events.py")) \
                               .replace("conflict-record-events.json", f"{t}/events.json").replace("conflict-record.txt", f"{t}/out.txt")
            for evs, rc in (([old, new], 0), ([old, new, bad], 1), ([{**new, "occurred_at": None}], 2)):
                Path(t, "events.json").write_text(json.dumps(evs), encoding="utf-8")
                summary = Path(t, "summary.md")
                summary.write_text("", encoding="utf-8")
                p = bash(script, CONFLICT_FILES_SINCE="2026-11-10T00:00:00Z", GITHUB_STEP_SUMMARY=str(summary))
                self.assertEqual(p.returncode, rc, p.stdout + p.stderr)
                if rc != 2:
                    self.assertIn("conflict-check: PR", summary.read_text(encoding="utf-8"))
                if rc == 1:
                    self.assertIn("job=103 missing_conflict_files", p.stdout)


class RecordSqlNamesTest(unittest.TestCase):
    def test_出すイベントはevents_pyが組むものだけ(self):
        """psql がなくても、record-events.sql が出すイベントの名前が events.py の組むイベントに入っているかを見る"""
        names = set(re.findall(r"'((?:pr|continue|job|conflict|handoff)\.\w+)'", RECORD_SQL.read_text(encoding="utf-8")))
        self.assertEqual(names, {"continue.enqueued", "job.started", "job.finished", "conflict.files"})
        self.assertLessEqual(names, set(events.NAMES))


PSQL = shutil.which("psql")
DB = os.environ.get("DATABASE_URL")


class Psql:
    def psql(self, *args: str, sql: str | None = None, search_path: str | None = None) -> subprocess.CompletedProcess:
        env = {**os.environ, **({"PGOPTIONS": f"-c search_path={search_path}"} if search_path else {})}
        return subprocess.run([PSQL, DB, "-X", "-q", "-A", "-t", "-v", "ON_ERROR_STOP=1", *args], input=sql, env=env,
                              capture_output=True, text=True)


@unittest.skipUnless(PSQL and DB, "psql か DATABASE_URL がない")
class WindowSqlTest(Psql, unittest.TestCase):
    """window-events.sql を、Engine の events と同じ列の表を置いた使い捨ての schema で走らせる"""

    def setUp(self):
        self.ns = f"conflict_schema_{uuid.uuid4().hex[:12]}"
        self.addCleanup(lambda: self.psql(sql=f"drop schema if exists {self.ns} cascade;"))
        # 時刻はどれも now() から決める（走らせる日で結果が変わらないように。since もテストの中で同じく決める）
        rows = [
            ("pr.conflicted", "now() - interval '4 days'", {"pr_url": "u"}),                                    # 窓の中だが since（3 日前）より前
            ("pr.conflicted", "now() - interval '1 hour'", {"pr_url": "u", "conflict_files": ["CHANGELOG.md"]}),
            ("continue.enqueued", "now() - interval '2 hours'", {"pr_url": "u", "reason": "conflict"}),            # 窓の中で欄がない
            ("handoff.saved", "now() - interval '30 days'", {"continue_reason": "conflict"}),                    # 窓の外
            ("job.started", "now() - interval '1 hour'", {}),                                                     # 照らさないイベント
        ]
        values = ",\n".join(f"('{n}', {t}, '{json.dumps(p)}'::jsonb)" for n, t, p in rows)
        p = self.psql(sql=f"""create schema {self.ns};
create table {self.ns}.events (id bigserial primary key, name text not null, occurred_at timestamptz not null default now(),
  correlation_id text, job_id bigint, payload jsonb not null default '{{}}'::jsonb, dedupe_key text unique);
insert into {self.ns}.events (name, occurred_at, payload) values {values};""")
        self.assertEqual(p.returncode, 0, p.stderr)

    def window(self, days_ago: int) -> tuple[str, list[dict]]:
        """since を今から days_ago 日前にして window-events.sql を走らせ、(since, 出た列) を返す"""
        since = (datetime.now(timezone.utc) - timedelta(days=days_ago)).isoformat()
        p = self.psql("-v", f"since={since}", "-v", "window_days=8", "-f", str(SQL), search_path=self.ns)
        self.assertEqual(p.returncode, 0, p.stderr)
        return since, json.loads(p.stdout)

    def test_窓と差分を当てた時刻より前とほかのイベントを出さない(self):
        since, events = self.window(3)
        self.assertEqual([e["name"] for e in events], ["pr.conflicted", "continue.enqueued"])
        errors = schema.check(schema.load(), events, schema.parse_time(since))   # occurred_at を schema.py が読める
        self.assertEqual([(x["name"], x["rule"]) for x in errors], [("continue.enqueued", "missing_field")])

    def test_差分を当てた時刻が窓より前なら窓で絞る(self):
        _, events = self.window(60)
        self.assertEqual([e["name"] for e in events], ["pr.conflicted", "pr.conflicted", "continue.enqueued"])   # 30 日前の行は出ない


@unittest.skipUnless(PSQL and DB, "psql か DATABASE_URL がない")
class RecordSqlTest(Psql, unittest.TestCase):
    """record-events.sql を、Engine の events と同じ列の表を置いた使い捨ての schema で走らせる（議決 D-0076・Issue #1214）"""

    def setUp(self):
        self.ns = f"conflict_record_{uuid.uuid4().hex[:12]}"
        self.addCleanup(lambda: self.psql(sql=f"drop schema if exists {self.ns} cascade;"))
        url = "https://github.com/guccyon/rally/pull/1"
        enq = lambda reason, **p: {"pr_url": url, "head_sha": "h", "reason": reason, **p}
        # (name, occurred_at, job_id, payload)。時刻はどれも now() から決める
        rows = [
            ("continue.enqueued", "now() - interval '4 days'", 1, enq("conflict")),                       # 窓の中だが since（3 日前）より前
            ("job.started", "now() - interval '4 days'", 1, {}),
            ("continue.enqueued", "now() - interval '2 hours'", 2, enq("conflict")),                      # 照らす回（記録なし）
            ("job.started", "now() - interval '1 hour'", 2, {}),
            ("job.finished", "now() - interval '30 minutes'", 2, {"state": "succeeded"}),
            ("conflict.files", "now() - interval '20 minutes'", 2, {"files": ["CHANGELOG.md"], "source": "worker_log"}),
            ("continue.enqueued", "now() - interval '1 hour'", 3, enq("conflict", conflict_files=["a.ts"])),   # 照らす回（その場で載せた）
            ("continue.enqueued", "now() - interval '1 hour'", 4, enq("ci_failed")),                      # 衝突でない続き
            ("job.started", "now() - interval '50 minutes'", 4, {}),
            ("continue.enqueued", "now() - interval '30 days'", 5, enq("conflict")),                      # 窓の外
            ("pr.conflicted", "now() - interval '1 hour'", None, {"pr_url": url, "head_sha": "h"}),       # 照らさないイベント
        ]
        values = ",\n".join(f"('{n}', {t}, {'null' if j is None else j}, '{json.dumps(p)}'::jsonb)" for n, t, j, p in rows)
        p = self.psql(sql=f"""create schema {self.ns};
create table {self.ns}.events (id bigserial primary key, name text not null, occurred_at timestamptz not null default now(),
  correlation_id text, job_id bigint, payload jsonb not null default '{{}}'::jsonb, dedupe_key text unique);
insert into {self.ns}.events (name, occurred_at, job_id, payload) values {values};""")
        self.assertEqual(p.returncode, 0, p.stderr)

    def window(self, days_ago: int) -> tuple[str, list[dict]]:
        since = (datetime.now(timezone.utc) - timedelta(days=days_ago)).isoformat()
        p = self.psql("-v", f"since={since}", "-v", "window_days=8", "-f", str(RECORD_SQL), search_path=self.ns)
        self.assertEqual(p.returncode, 0, p.stderr)
        return since, json.loads(p.stdout)

    def test_窓と差分を当てた時刻より前とほかのイベントを出さない(self):
        since, evs = self.window(3)
        self.assertEqual([(e["name"], e["job_id"]) for e in evs],
                         [("continue.enqueued", 2), ("job.started", 2), ("job.finished", 2), ("conflict.files", 2), ("continue.enqueued", 3)])
        skip = schema.before(evs, schema.parse_time(since))   # occurred_at を events.py の --since が読める
        self.assertEqual(skip, set())
        errors = [e for r in events.records_from_events(evs).values() for e in events.record_check(r)]
        self.assertEqual([(e["job_id"], e["rule"]) for e in errors], [(2, "missing_conflict_files")])   # worker_log はジョブの終わりのあと

    def test_差分を当てた時刻が窓より前なら窓で絞る(self):
        _, evs = self.window(60)
        self.assertEqual([(e["name"], e["job_id"]) for e in evs if e["name"] == "continue.enqueued"],
                         [("continue.enqueued", 1), ("continue.enqueued", 2), ("continue.enqueued", 3)])   # 30 日前の回は出ない
        self.assertIn(("job.started", 1), [(e["name"], e["job_id"]) for e in evs])


class SqlNamesTest(unittest.TestCase):
    def test_照らすイベントは定義と同じ(self):
        """psql がなくても、window-events.sql が出すイベントの名前が conflict-files.yaml の events と同じかを見る"""
        names = set(re.findall(r"'([\w.]+)'", re.search(r"e\.name in \(([^)]*)\)", SQL.read_text(encoding="utf-8")).group(1)))
        self.assertEqual(names, set(schema.load()))


if __name__ == "__main__":
    unittest.main()
