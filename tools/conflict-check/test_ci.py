"""ci/conflict-engine.yml（test_engine.py を PR ごとに走らせるワークフローのひな形）のテスト（知見 K-0010 の担保。議決 D-0018・Issue #971）。Engine は要らない"""
from __future__ import annotations

import re
import subprocess
import unittest
from pathlib import Path

import yaml

HERE = Path(__file__).resolve().parent
TEMPLATE = HERE / "ci/conflict-engine.yml"
WIRED = HERE.parents[1] / ".github/workflows/conflict-engine.yml"   # 建設者が写した先


class WorkflowTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.text = TEMPLATE.read_text(encoding="utf-8")
        cls.wf = yaml.safe_load(cls.text)
        cls.steps = cls.wf["jobs"]["engine-test"]["steps"]

    def ref_check(self, ref: str) -> subprocess.CompletedProcess:
        return subprocess.run(["bash", "-c", self.steps[0]["run"]], env={"PATH": "/usr/bin:/bin", "ENGINE_REF": ref},
                              capture_output=True, text=True)

    def test_PRごとに走りENGINE_REFは40桁のコミットSHA(self):
        self.assertIn("pull_request", self.wf[True])   # PyYAML は on: を真偽の True として読む
        self.assertRegex(self.wf["env"]["ENGINE_REF"], r"^[0-9a-f]{40}$")

    def test_最初の手順でENGINE_REFがSHAでなければEngineを取らずに落とす(self):
        self.assertNotIn("uses", self.steps[0], "SHA の確かめはどのチェックアウトよりも前")
        self.assertEqual(self.ref_check(self.wf["env"]["ENGINE_REF"]).returncode, 0)
        for ref in ("main", "v1.2.0", "9fee9c2", "9FEE9C2B3D8D2DD22DB63E8989D839CD982FD046", ""):
            with self.subTest(ref=ref):
                p = self.ref_check(ref)
                self.assertNotEqual(p.returncode, 0)
                self.assertIn("engine_ref_not_sha", p.stdout)

    def test_Engineを固定した版で取りENGINE_DIRを向けてCIとして走らせる(self):
        engine = [s for s in self.steps if s.get("with", {}).get("repository") == "guccyon/factory-engine"]
        self.assertEqual(len(engine), 1)
        self.assertEqual(engine[0]["with"]["ref"], "${{ env.ENGINE_REF }}")
        test = [s for s in self.steps if "tools/conflict-check" in s.get("run", "")]
        self.assertEqual(len(test), 1)
        self.assertGreater(self.steps.index(test[0]), self.steps.index(engine[0]))
        self.assertEqual(test[0]["env"]["CI"], "true")
        self.assertEqual(test[0]["env"]["ENGINE_DIR"], "${{ github.workspace }}/" + engine[0]["with"]["path"])
        self.assertIn("-p 'test_*.py'", test[0]["run"])
        self.assertIn("exit $rc", test[0]["run"], "テストが落ちたらジョブを落とす")

    @unittest.skipUnless(WIRED.is_file(), "まだ .github/workflows/ に写していない（写すのは建設者）")
    def test_写しはENGINE_REFの行のほかでひな形とずれない(self):
        strip = lambda t: re.sub(r"(?m)^  ENGINE_REF: .*$", "  ENGINE_REF: <ref>", t)
        self.assertEqual(strip(WIRED.read_text(encoding="utf-8")), strip(self.text))


if __name__ == "__main__":
    unittest.main()
