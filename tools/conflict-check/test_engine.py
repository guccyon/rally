"""Engine の本物の衝突の扱いを policy.py の線に照らすテスト（知見 K-0010 の担保。議決 D-0013・Issue #954）。

Engine のチェックアウトは環境変数 ENGINE_DIR で渡す（なければリポジトリの隣の ../engine）。どちらもなければ飛ばす。
ただし ENGINE_DIR を渡したときと CI（CI=true）では飛ばさずに落とす（K-0004 の engine.test.mjs と同じ。議決 D-0011）。

今の Engine が線から外れている場合は、Engine が今の形（LEGACY_*）のときだけ「todo:」の理由で飛ばす（結果には出る）。
Engine の振る舞いが今の形から変わったら、その版からは線に合わないと落ちる。建設者が Engine を線に合わせて ENGINE_REF を上げたら、
LEGACY_* と todo_while_legacy の呼び出しを消してよい（README の「今の Engine」）。
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import policy  # noqa: E402

HARNESS = HERE / "engine_harness.py"
ENGINE_DIR = Path(os.environ.get("ENGINE_DIR") or HERE.parents[2] / "engine")
REQUIRED = bool(os.environ.get("ENGINE_DIR")) or os.environ.get("CI") == "true"
HAVE_ENGINE = (ENGINE_DIR / "orchestrator/dispatcher/supervisor.py").is_file()
SKIP = None if REQUIRED or HAVE_ENGINE else f"Engine のチェックアウトがない（ENGINE_DIR={ENGINE_DIR}）"
HEADS = 6   # 上限（3）を超えて衝突し続ける head の数


def harness(*args: str) -> dict:
    p = subprocess.run([sys.executable, "-I", "-B", str(HARNESS), *args], capture_output=True, text=True)
    if p.returncode != 0:
        raise AssertionError(f"engine_harness.py {' '.join(args[:1])} が落ちた（ENGINE_DIR={ENGINE_DIR}）: {p.stderr or p.stdout}")
    return json.loads(p.stdout)


def legacy_supervisor(out: dict) -> bool:
    """今の Engine の形: 回数の上限がなく、衝突した head ごとに tier-large の続きのジョブを積み、停止は開かない（ADR 0016「回数では切らない」）"""
    return (len(out["cycles"]) == HEADS
            and all(c["enqueued"] == [{"attempt": i, "reason": "conflict", "model_tier": "tier-large"}] and not c["stoppages"]
                    for i, c in enumerate(out["cycles"], 1)))


def legacy_worker(out: dict) -> bool:
    """今の Engine の形: 機械的に解くのは衝突が CHANGELOG.md だけのときだけで、changelog.d/ を含む衝突はそのままエージェントに渡す"""
    return out["before"] != ["CHANGELOG.md"] and any(f.startswith("changelog.d/") for f in out["before"]) \
        and out["agent"] and out["conflicts"] == out["before"]


def legacy_unverified(out: dict) -> bool:
    """今の Engine の形: CHANGELOG.md だけの衝突を両方の行を残して解いたら、K-0002 の検査にかけずにそのまま機械的な解消にする"""
    return out["before"] == ["CHANGELOG.md"] and not out["agent"] and out["verified"] is False


@unittest.skipIf(SKIP, SKIP)
class EngineTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.c = policy.load()
        cls.engine = str(ENGINE_DIR)

    def todo_while_legacy(self, is_legacy: bool, what: str):
        if is_legacy:
            self.skipTest(f"todo: 今の Engine は {what}（README の「今の Engine」。Engine を直して ENGINE_REF を上げたら消す）")

    def test_0_取り出す関数と手順がすべてEngineにある(self):
        out = harness("--check", self.engine)
        for name in ("conflict_step", "ci_failure_step", "enqueue", "attempts", "pending"):
            self.assertIn(name, out["functions"])
        self.assertEqual(out["entrypoint"], ["mechanical", "state_args"])

    def test_1_上限を超えた衝突では続きのジョブを積まず停止を開いて人の判断待ちにする(self):
        out = harness("supervisor", self.engine, "--heads", str(HEADS))
        self.todo_while_legacy(legacy_supervisor(out), "conflict の続きのジョブを回数で切らない")
        n = self.c["max_attempts_per_pr"]
        enqueued = [e for c in out["cycles"] for e in c["enqueued"]]
        self.assertEqual(len(enqueued), n, f"同じ PR で積んだ conflict の続きのジョブは {n} 回まで: {out['cycles']}")
        self.assertEqual([e["attempt"] for e in enqueued], list(range(1, n + 1)))
        over = out["cycles"][n]
        self.assertEqual(over["enqueued"], [], f"{n + 1} 回目の衝突では積まない")
        opened = [s["key"] for s in over["stoppages"] if s["open"]]
        self.assertIn("ci:https://github.com/guccyon/rally/pull/659", opened,
                      f"{n + 1} 回目の衝突で衝突の停止（ci:）を開いて人の判断待ちにする")

    def test_1_機械的に解いた続きのジョブは上限に数えず人の判断待ちに上げない(self):
        out = harness("supervisor", self.engine, "--heads", str(HEADS), "--resolved", "mechanical")
        self.assertEqual(len(out["cycles"]), HEADS, f"機械的に解いた回は上限に数えないので、{HEADS} 回の衝突でも毎回積む: {out['cycles']}")
        for c in out["cycles"]:
            self.assertEqual(len(c["enqueued"]), 1, c)
            self.assertFalse([s for s in c["stoppages"] if s["open"]], f"機械的に解いた回だけで停止を開いた: {c}")

    def test_1_上限までの続きのジョブは停止を開かずエージェントの段で積む(self):
        out = harness("supervisor", self.engine, "--heads", str(self.c["max_attempts_per_pr"]))
        for c in out["cycles"]:
            self.assertEqual(len(c["enqueued"]), 1, c)
            self.assertEqual(c["enqueued"][0]["model_tier"], self.c["agent_tier"], c)
            self.assertFalse([s for s in c["stoppages"] if s["open"]], c)

    def test_2_2回目以降の衝突の解消はセッションを引き継がない(self):
        out = harness("resume", self.engine)
        # Engine の引き継ぎは回数によらない。conflict で引き継ぐなら 2 回目以降も引き継ぐので外れる
        if not policy.may_resume(self.c, 2):
            self.assertFalse(out["conflict"]["resumes"], f"conflict の続きのジョブに --state-dir を渡している: {out['conflict']}")
        self.assertTrue(out["ci_failed"]["resumes"], "ci_failed は引き継ぐ（STATE_ARGS の行を正しく動かせている確かめ）")

    def test_3_CHANGELOGだけの衝突はエージェントを使わずに機械的に解きK0002の検査に通る(self):
        out = harness("worker", self.engine, "--changelog", "compiled", "CHANGELOG.md")   # 取りまとめどうしの衝突
        self.assertEqual(out["before"], ["CHANGELOG.md"], "衝突を起こせている確かめ")
        self.assertEqual(policy.resolution(self.c, out["before"])["tier"], "none")
        self.assertFalse(out["agent"], f"エージェントを使った: {out}")
        self.assertEqual((out["conflicts"], out["unmerged"], out["markers"]), ([], [], []))
        self.assertTrue(out["verified"], f"両方の行を残した結果が K-0002 の検査に通らない: {out}")
        self.assertEqual(policy.resolution(self.c, out["before"], out["verified"])["counts"], False)

    def test_3_機械的に解いた結果がK0002の検査に通らなければエージェントに渡す(self):
        out = harness("worker", self.engine, "--changelog", "direct", "CHANGELOG.md")   # PR が [Unreleased] を直接書き換えていた
        self.assertEqual(out["before"], ["CHANGELOG.md"], "衝突を起こせている確かめ")
        self.assertEqual(policy.resolution(self.c, out["before"], False)["mode"], "agent")
        self.todo_while_legacy(legacy_unverified(out), "機械的に解いた結果を K-0002 の検査にかけない")
        self.assertTrue(out["agent"], f"K-0002 の検査に通らない解き方を機械的な解消にした: {out}")

    def test_3_断片を含むCHANGELOGだけの衝突もエージェントを使わずに機械的に解く(self):
        for files in (["changelog.d/700.md"], ["CHANGELOG.md", "changelog.d/700.md"]):
            with self.subTest(files=files):
                out = harness("worker", self.engine, "--changelog", "compiled", *files)
                self.assertEqual(out["before"], sorted(files), "衝突を起こせている確かめ")
                self.assertEqual(policy.resolution(self.c, out["before"])["mode"], "mechanical")
                self.todo_while_legacy(legacy_worker(out), "changelog.d/ の衝突を機械的に解かない")
                self.assertFalse(out["agent"], f"エージェントを使った: {out}")
                self.assertEqual((out["conflicts"], out["unmerged"], out["markers"]), ([], [], []))
                self.assertTrue(out["verified"], f"両方の行を残した結果が K-0002 の検査に通らない: {out}")

    def test_3_ほかのファイルも衝突したらエージェントに渡す(self):
        out = harness("worker", self.engine, "CHANGELOG.md", "src/app.txt")
        self.assertEqual(policy.resolution(self.c, out["before"])["mode"], "agent")
        self.assertTrue(out["agent"], f"機械的に解いてしまった: {out}")
        self.assertIn("src/app.txt", out["conflicts"])


if __name__ == "__main__":
    unittest.main()
