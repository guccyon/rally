"""policy.py の線（.factory-config/continuation.yaml の conflict）のテスト（知見 K-0010 の担保。議決 D-0013・Issue #954）。Engine は要らない"""
from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import policy  # noqa: E402


class ConfigTest(unittest.TestCase):
    def test_リポジトリの設定を読める(self):
        c = policy.load()
        self.assertEqual(c["max_attempts_per_pr"], 3)
        self.assertEqual(c["on_limit"], "stoppage")
        self.assertEqual(c["resume_session_until_attempt"], 1)
        self.assertIn("CHANGELOG.md", c["mechanical"]["paths"])
        self.assertIn("changelog.d/", c["mechanical"]["paths"])

    def test_形が正しくない設定は読めないとして終了コード2(self):
        bad = {
            "上限がない": "conflict: {on_limit: stoppage}",
            "上限が0": "conflict: {max_attempts_per_pr: 0, on_limit: stoppage, resume_session_until_attempt: 1, mechanical: {paths: [CHANGELOG.md], agent_tier: none}, agent_tier: tier-large}",
            "機械的に解くのにエージェントを使う": "conflict: {max_attempts_per_pr: 3, on_limit: stoppage, resume_session_until_attempt: 1, mechanical: {paths: [CHANGELOG.md], agent_tier: tier-small}, agent_tier: tier-large}",
            "上限を超えたときの扱いが知らない値": "conflict: {max_attempts_per_pr: 3, on_limit: retry, resume_session_until_attempt: 1, mechanical: {paths: [CHANGELOG.md], agent_tier: none}, agent_tier: tier-large}",
        }
        with tempfile.TemporaryDirectory() as tmp:
            for name, text in bad.items():
                path = Path(tmp) / "continuation.yaml"
                path.write_text(text, encoding="utf-8")
                with self.assertRaises(policy.PolicyError, msg=name):
                    policy.load(path)
                p = subprocess.run([sys.executable, "-I", str(HERE / "policy.py"), "--config", str(path)], capture_output=True, text=True)
                self.assertEqual(p.returncode, 2, name)


class LineTest(unittest.TestCase):
    def setUp(self):
        self.c = policy.load()

    def test_C0005と同じ20回の衝突では3回だけ積み4回目からは停止を開いて人へ(self):
        attempts, decisions = 0, []
        for _ in range(20):
            d = policy.enqueue_decision(self.c, attempts)
            decisions.append(d)
            if d == "enqueue":
                attempts += 1
            else:
                break   # 停止を開いたら新しい head は来ない
        self.assertEqual(decisions, ["enqueue"] * 3 + ["stoppage"])

    def test_2回目以降はセッションを引き継がない(self):
        self.assertTrue(policy.may_resume(self.c, 1))
        for attempt in (2, 3, 20):
            self.assertFalse(policy.may_resume(self.c, attempt), attempt)

    def test_CHANGELOGと断片だけの衝突はエージェントを使わずに機械的に解く(self):
        for files in (["CHANGELOG.md"], ["changelog.d/700.md"], ["CHANGELOG.md", "changelog.d/700.md", "changelog.d/701.md"]):
            r = policy.resolution(self.c, files)
            self.assertEqual((r["mode"], r["tier"], r["agent"]), ("mechanical", "none", []), files)

    def test_ほかのファイルも衝突したらエージェントで解く(self):
        r = policy.resolution(self.c, ["CHANGELOG.md", "apps/backend/src/x.rs"])
        self.assertEqual((r["mode"], r["tier"]), ("agent", "tier-large"))
        self.assertEqual((r["mechanical"], r["agent"]), (["CHANGELOG.md"], ["apps/backend/src/x.rs"]))

    def test_名前が似ているだけのファイルは機械的に解かない(self):
        for f in ("docs/CHANGELOG.md", "CHANGELOG.md.bak", "changelog.dx/1.md", "apps/changelog.d/1.md"):
            self.assertEqual(policy.resolution(self.c, [f])["mode"], "agent", f)

    def test_衝突がなければ何もしない(self):
        self.assertEqual(policy.resolution(self.c, [])["mode"], "none")


if __name__ == "__main__":
    unittest.main()
