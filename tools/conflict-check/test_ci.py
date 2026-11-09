"""ci/conflict-check.yml（PR ごとの CI のひな形）の形のテスト（知見 K-0010 の担保。議決 D-0030・Issue #1034）。Engine は要らない。

ひな形が、PR ごとに必ず走り（絞り込みのない pull_request・if: と continue-on-error なし）、ENGINE_REF の Engine で
test_events.py・test_engine.py を含むテストを CI=true と ENGINE_DIR を渡して走らせる形かを見る。
.github/workflows/conflict-check.yml に写してあれば、ENGINE_REF の行のほかがひな形と同じかも見る（写すときに変えてよいのはその行だけ）。

SqlJobTest は、ジョブ conflict-sql が PR ごとに Postgres のサービスコンテナと psql を用意し、DATABASE_URL と CONFLICT_SQL_REQUIRED=true を渡して
test_schema_ci.py の WindowSqlTest・RecordSqlTest を飛ばさずに走らせる形かを見る（知見 K-0014 の担保。議決 D-0111・Issue #1295）。
"""
from __future__ import annotations

import os
import re
import subprocess
import sys
import unittest
from pathlib import Path

import yaml

HERE = Path(__file__).resolve().parent
TEMPLATE = HERE / "ci" / "conflict-check.yml"
COPY = HERE.parents[1] / ".github" / "workflows" / "conflict-check.yml"
REF_LINE = re.compile(r"^\s*ENGINE_REF:\s*\S+\s*$", re.M)


def load(path: Path) -> dict:
    d = yaml.safe_load(path.read_text(encoding="utf-8"))
    if True in d:   # YAML 1.1 では on: が真偽になる
        d["on"] = d.pop(True)
    return d


class TemplateTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.wf = load(TEMPLATE)
        cls.job = cls.wf["jobs"]["conflict-check"]
        cls.steps = cls.job["steps"]

    def test_PRごとに絞り込みなしで走る(self):
        self.assertEqual(self.wf["on"], {"pull_request": None})
        self.assertNotIn("if", self.job)
        self.assertNotIn("continue-on-error", self.job)
        for s in self.steps:
            self.assertNotIn("if", s, s)
            self.assertNotIn("continue-on-error", s, s)

    def test_ENGINE_REFは40桁のSHAで最初の手順で確かめる(self):
        self.assertRegex(self.wf["env"]["ENGINE_REF"], r"^[0-9a-f]{40}$")
        guard = self.steps[0]
        self.assertIn("ENGINE_REF", guard["run"])
        self.assertEqual(guard.get("shell"), "bash")
        for ref, rc in (("main", 1), (self.wf["env"]["ENGINE_REF"], 0)):
            p = subprocess.run(["bash", "-e", "-c", guard["run"]], env={"PATH": "/usr/bin:/bin", "ENGINE_REF": ref}, capture_output=True, text=True)
            self.assertEqual(p.returncode, rc, ref)

    def test_ENGINE_REFのEngineを取る(self):
        engine = [s for s in self.steps if (s.get("with") or {}).get("repository") == "guccyon/factory-engine"]
        self.assertEqual(len(engine), 1)
        self.assertEqual(engine[0]["with"]["ref"], "${{ env.ENGINE_REF }}")
        self.assertEqual(engine[0]["with"]["path"], ".engine")

    def test_テストをCIとENGINE_DIRを渡して飛ばさずに走らせる(self):
        runs = [s for s in self.steps if "unittest discover" in (s.get("run") or "")]
        self.assertEqual(len(runs), 1)
        s = runs[0]
        self.assertEqual(s["env"]["CI"], "true")   # fixtures・node・git・Engine がなければ飛ばさずに落とす
        self.assertEqual(s["env"]["ENGINE_DIR"], "${{ github.workspace }}/.engine")
        self.assertRegex(s["run"], r"unittest discover -s tools/conflict-check -p 'test_\*\.py'")
        self.assertNotRegex(s["run"], r"\|\|\s*true|; *true\b|set \+e")
        for name in ("test_events.py", "test_engine.py", "test_watch.py", "test_policy.py"):
            self.assertTrue((HERE / name).is_file(), name)

    def test_ジョブはconflict_checkとconflict_sql(self):
        self.assertEqual(list(self.wf["jobs"]), ["conflict-check", "conflict-sql"])   # conflict-sql は SqlJobTest

    def test_写しがあればENGINE_REFの行のほかはひな形と同じ(self):
        if not COPY.is_file():
            self.skipTest(".github/workflows/conflict-check.yml にまだ写していない（建設者が写す）")
        strip = lambda p: REF_LINE.sub("ENGINE_REF:", p.read_text(encoding="utf-8"))
        self.assertEqual(strip(COPY), strip(TEMPLATE))


class SqlJobTest(unittest.TestCase):
    """ジョブ conflict-sql（知見 K-0014 の担保。議決 D-0111（PR #1294）・Issue #1295）"""

    @classmethod
    def setUpClass(cls):
        cls.job = load(TEMPLATE)["jobs"]["conflict-sql"]
        cls.steps = cls.job["steps"]
        runs = [s for s in cls.steps if "unittest" in (s.get("run") or "")]
        assert len(runs) == 1, runs
        cls.test_step = runs[0]

    def test_絞り込みなしで走る(self):
        self.assertNotIn("if", self.job)
        self.assertNotIn("needs", self.job)   # conflict-check が落ちても走る
        self.assertNotIn("continue-on-error", self.job)
        for s in self.steps:
            self.assertNotIn("if", s, s)
            self.assertNotIn("continue-on-error", s, s)

    def test_Postgresのサービスコンテナとpsqlを用意する(self):
        pg = self.job["services"]["postgres"]
        self.assertRegex(pg["image"], r"^postgres:\d+$")
        self.assertIn("5432:5432", pg["ports"])
        self.assertIn("pg_isready", pg["options"])   # 立ち上がるまで手順を始めない
        installs = [s for s in self.steps if "postgresql-client" in (s.get("run") or "")]
        self.assertEqual(len(installs), 1)
        self.assertLess(self.steps.index(installs[0]), self.steps.index(self.test_step))
        url = self.test_step["env"]["DATABASE_URL"]
        self.assertRegex(url, r"^postgresql://postgres:%s@localhost:5432/" % re.escape(pg["env"]["POSTGRES_PASSWORD"]))

    def test_SQLのテストを求めて飛ばさずに走らせる(self):
        self.assertEqual(self.test_step["env"]["CONFLICT_SQL_REQUIRED"], "true")   # psql か DATABASE_URL がなければ落ちる（test_schema_ci.py の SqlRequiredTest）
        self.assertNotRegex(self.test_step["run"], r"\|\|\s*true|; *true\b|set \+e")
        self.assertRegex(self.test_step["run"], r"unittest discover -s tools/conflict-check -p 'test_schema_ci\.py'")
        names = re.findall(r"-k (\w+)", self.test_step["run"])
        self.assertEqual(names, ["WindowSqlTest", "RecordSqlTest"])
        import test_schema_ci   # -k の名前が今もテストの組の名前であること（名前が変わると 1 件も走らずに通る）
        for n in names:
            self.assertTrue(issubclass(getattr(test_schema_ci, n), unittest.TestCase), n)

    def test_手順はDATABASE_URLがなければ落ちる(self):
        env = {k: v for k, v in os.environ.items() if k not in ("DATABASE_URL", "CONFLICT_SQL_REQUIRED")}
        env = {**env, **{k: v for k, v in self.test_step["env"].items() if k != "DATABASE_URL"}, "DATABASE_URL": ""}
        p = subprocess.run(["bash", "-e", "-o", "pipefail", "-c", self.test_step["run"].replace("python3", sys.executable)],
                           cwd=HERE.parents[1], env=env, capture_output=True, text=True)
        self.assertNotEqual(p.returncode, 0, p.stderr)
        self.assertIn("conflict_sql_unavailable", p.stderr)


if __name__ == "__main__":
    unittest.main()
