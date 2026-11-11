"""Engine の新しい PR の道が、PR を開く前に既定ブランチを取り込むかのテスト（知見 K-0023 の担保。議決 D-0148・Issue #1383）。

open_pr_harness.py で、orchestrator/worker/entrypoint.sh の新しい PR の道を本物の git（一時の origin）の上で、gh とエージェントを差し替えて動かし、次の 2 つを見る。

- 新しい PR の道で、既定ブランチの取り込み（git merge … origin/${DEFAULT_BRANCH}）が push と gh pr create より前にある。
  行の順と、実装の間に main が進んだときに gh pr create の時点で PR のブランチが main の最新を含むことの両方で見る
- 取り込みが衝突したまま PR を開かない。エージェントが解けない（何もしない・取り込みを中止する）ときは gh pr create を呼ばず、push もせず、
  失敗（終了コード 0 以外、FACTORY_RESULT の reason が conflict_unresolved）で終える。解けたときは同じセッション（STATE_ARGS の --state-dir）で解いてから開く

PatchedEngineTest は engine/open-pr-merge.patch を Engine の entrypoint.sh の写しに当てて（もう入っていればそのまま）動かす。当たらなければ落ちる。
EngineTest は Engine のチェックアウトの entrypoint.sh をそのまま動かす。今の Engine（新しい PR の道に fetch も取り込みもない形）のときだけ
test_engine.py の todo で飛ばし、知見 K-0023 の review_by を過ぎたら落とす（test_events.py の EngineFinishedTest と同じ。議決 D-0043）。
Engine のチェックアウトの渡し方と飛ばし方は test_engine.py と同じ（ENGINE_DIR を渡したときと CI=true では飛ばさずに落とす）。

  python3 -B -m unittest discover -s tools/conflict-check -p 'test_open_pr.py' -v
"""
from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import test_engine  # noqa: E402   名前で取り込むと EngineTest がこのモジュールでも走るので、モジュールで取り込む
import test_events  # noqa: E402

HARNESS = HERE / "open_pr_harness.py"
PATCH = HERE / "engine" / "open-pr-merge.patch"
ENTRYPOINT = "orchestrator/worker/entrypoint.sh"
K0023 = HERE.parents[1] / ".factory-config" / "knowledge" / "K-0023.yaml"
ENGINE_DIR = test_engine.ENGINE_DIR
HAVE_ENTRYPOINT = (ENGINE_DIR / ENTRYPOINT).is_file()
SKIP = None if test_engine.REQUIRED or HAVE_ENTRYPOINT else f"Engine のチェックアウトがない（ENGINE_DIR={ENGINE_DIR}）"
LEGACY_TODO = "新しい PR の道で既定ブランチを取り込まずに push して PR を開く（README の「PR を開く前の既定ブランチの取り込み」。Engine に engine/open-pr-merge.patch を当てて ENGINE_REF を上げたら消える）"


def harness(*args: str) -> dict:
    p = subprocess.run([sys.executable, "-I", "-B", str(HARNESS), *args], capture_output=True, text=True)
    if p.returncode != 0:
        raise AssertionError(f"open_pr_harness.py {' '.join(args[:1])} が落ちた: {p.stderr or p.stdout}")
    return json.loads(p.stdout)


def legacy(order: dict) -> bool:
    """今の Engine の形: 新しい PR の道に、既定ブランチの fetch も取り込みもない"""
    return order["merge"] is None and order["fetch"] is None


class OpenPrCases:
    """entrypoint は下のクラスが setUpClass で決める。todo_while_legacy は Engine をそのまま動かすときだけ todo にする"""
    entrypoint: str

    def todo_while_legacy(self):
        pass

    def test_0_新しいPRの道とstage_allを取り出せる(self):
        out = harness("--check", self.entrypoint)
        self.assertGreater(out["block_lines"], 0)
        self.assertIsNotNone(out["order"]["create"], "新しい PR の道に gh pr create がない")

    def test_1_新しいPRの道で既定ブランチの取り込みがgh_pr_createより前にある(self):
        o = harness("order", self.entrypoint)
        self.todo_while_legacy()
        self.assertIsNotNone(o["merge"], f"新しい PR の道に既定ブランチの取り込み（git merge … origin/${{DEFAULT_BRANCH}}）がない: {o}")
        self.assertIsNotNone(o["fetch"], f"新しい PR の道に既定ブランチの fetch がない: {o}")
        self.assertLess(o["fetch"], o["merge"], o)
        self.assertLess(o["merge"], o["push"], f"取り込みが push より後: {o}")
        self.assertLess(o["merge"], o["create"], f"取り込みが gh pr create より後: {o}")

    def test_1_実装の間に既定ブランチが進んでもPRを開いた時点で最新を含む(self):
        out = harness("run", self.entrypoint, "--main", "advance")
        self.assertEqual(len(out["created"]), 1, f"PR を 1 本開く: {out}")
        self.todo_while_legacy()
        self.assertEqual(out["exit"], 0, out)
        self.assertEqual(out["created"][0], {"pushed": True, "contains_main": True, "markers": False},
                         f"gh pr create の時点で PR のブランチが既定ブランチの最新を含まない: {out}")
        self.assertEqual(out["agent_calls"], [], "衝突しない取り込みでエージェントを呼んだ")

    def test_2_取り込みが衝突したままPRを開かない(self):
        for agent in ("leave", "abort"):   # エージェントが解けない・取り込みを中止した
            with self.subTest(agent=agent):
                out = harness("run", self.entrypoint, "--main", "conflict", "--agent", agent)
                self.todo_while_legacy()
                self.assertEqual(out["created"], [], f"衝突を解けないまま gh pr create を呼んだ: {out}")
                self.assertNotIn("pr create", " ".join(out["gh"]))
                self.assertFalse(out["pushed"], f"衝突を解けないまま push した: {out}")
                self.assertNotEqual(out["exit"], 0, f"失敗として返さない: {out}")
                self.assertIsNotNone(out["result"], f"FACTORY_RESULT がない: {out}")
                self.assertIsNone(out["result"]["pr_url"])
                self.assertEqual(out["result"]["reason"], "conflict_unresolved")

    def test_3_衝突は同じセッションで解いてからPRを開く(self):
        out = harness("run", self.entrypoint, "--main", "conflict", "--agent", "resolve")
        self.todo_while_legacy()
        self.assertEqual(len(out["agent_calls"]), 1, f"衝突を解くエージェントを 1 回呼ぶ: {out}")
        self.assertTrue(out["agent_calls"][0]["resumes"], f"同じセッション（STATE_ARGS の --state-dir）で解かない: {out['agent_calls']}")
        self.assertEqual(out["exit"], 0, out)
        self.assertEqual(out["created"], [{"pushed": True, "contains_main": True, "markers": False}], out)

    def test_4_既定ブランチが進んでいなければそのまま開く(self):
        out = harness("run", self.entrypoint, "--main", "none")
        self.assertEqual(out["exit"], 0, out)
        self.assertEqual(out["created"], [{"pushed": True, "contains_main": True, "markers": False}], out)
        self.assertEqual(out["agent_calls"], [])


@unittest.skipIf(SKIP, SKIP)
class PatchedEngineTest(OpenPrCases, unittest.TestCase):
    """engine/open-pr-merge.patch を当てた Engine の entrypoint.sh の写し。todo にしない"""

    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        root = Path(cls.tmp.name)
        (root / ENTRYPOINT).parent.mkdir(parents=True)
        shutil.copy(ENGINE_DIR / ENTRYPOINT, root / ENTRYPOINT)
        apply = lambda *a: subprocess.run(["git", "apply", *a, str(PATCH)], cwd=root, capture_output=True, text=True)  # noqa: E731
        if apply("--check").returncode == 0:
            r = apply()
            assert r.returncode == 0, r.stderr
        elif apply("--reverse", "--check").returncode != 0:   # もう入っていれば（--reverse --check が通れば）そのまま
            raise AssertionError(f"engine/open-pr-merge.patch が Engine（ENGINE_DIR={ENGINE_DIR}）に当たらない: {apply('--check').stderr}")
        cls.entrypoint = str(root / ENTRYPOINT)

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()


@unittest.skipIf(SKIP, SKIP)
class EngineTest(OpenPrCases, unittest.TestCase):
    """Engine のチェックアウト（CI では ENGINE_REF の版）の entrypoint.sh をそのまま動かす"""

    @classmethod
    def setUpClass(cls):
        cls.entrypoint = str(ENGINE_DIR / ENTRYPOINT)
        cls.legacy = legacy(harness("order", cls.entrypoint))

    def todo_while_legacy(self):
        if not self.legacy:
            return
        deadline = test_events.review_by(K0023)
        self.assertFalse(test_events.overdue(test_events.today(), deadline),
                         f"知見 K-0023 の review_by（{deadline}）を過ぎても、ENGINE_REF の Engine は {LEGACY_TODO}。Engine を直して ENGINE_REF を上げる")
        test_engine.todo(self, f"今の Engine は {LEGACY_TODO}（K-0023 の review_by {deadline} を過ぎたら落とす）")


class LegacyTest(unittest.TestCase):
    """todo の線そのものを Engine なしで確かめる"""

    def test_todoは取り込みもfetchもない形にだけ効く(self):
        self.assertTrue(legacy({"merge": None, "fetch": None, "push": 2, "create": 17}))
        self.assertFalse(legacy({"merge": None, "fetch": 3, "push": 2, "create": 17}))
        self.assertFalse(legacy({"merge": 5, "fetch": None, "push": 2, "create": 17}))

    def test_K0023のreview_byを読める(self):
        self.assertIsNotNone(test_events.review_by(K0023))


if __name__ == "__main__":
    unittest.main()
