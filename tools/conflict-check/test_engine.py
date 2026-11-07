"""Engine の本物の衝突の扱いを policy.py の線に照らすテスト（知見 K-0010 の担保。議決 D-0013・Issue #954）。

Engine のチェックアウトは環境変数 ENGINE_DIR で渡す（なければリポジトリの隣の ../engine）。どちらもなければ飛ばす。
ただし ENGINE_DIR を渡したときと CI（CI=true）では飛ばさずに落とす（K-0004 の engine.test.mjs と同じ。議決 D-0011）。

今の Engine が線から外れている場合は、Engine が今の形（LEGACY_*）のときだけ「todo:」の理由で飛ばす（結果には出る）。
Engine の振る舞いが今の形から変わったら、その版からは線に合わないと落ちる。建設者が Engine を線に合わせて ENGINE_REF を上げたら、
LEGACY_* と todo_while_legacy の呼び出しを消してよい（README の「今の Engine」）。
Engine が設定 continuation.yaml を読む版（engine_harness.py --check の reads_config が空でない）なのに今の形のままなら、
todo にせずに落とす（設定を読んでも線に合っていない。議決 D-0018・Issue #971。K-0004 の order.mjs と同じく、todo のまま次へ進ませない）。
CI では tools/conflict-check/ci/conflict-engine.yml が Engine を固定の版（ENGINE_REF）で取って ENGINE_DIR を向け、CI=true で走らせる。
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
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
    p = subprocess.run([sys.executable, "-I", str(HARNESS), *args], capture_output=True, text=True)
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


def todo_or_fail(is_legacy: bool, reads_config: list[str], what: str) -> tuple[str, str] | None:
    """今の形なら todo（飛ばす）。ただし Engine が continuation.yaml を読む版なら落とす。今の形でなければ None（必須のまま）"""
    if not is_legacy:
        return None
    if reads_config:
        return ("fail", f"Engine は continuation.yaml を読む（{'・'.join(reads_config)}）のに、今の形のまま {what}。"
                        "設定の線（max_attempts_per_pr・mechanical.paths）に合わせてから ENGINE_REF を上げる（README の「今の Engine」）")
    return ("todo", f"todo: 今の Engine は {what}（README の「今の Engine」。Engine を直して ENGINE_REF を上げたら消す）")


@unittest.skipIf(SKIP, SKIP)
class EngineTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.c = policy.load()
        cls.engine = str(ENGINE_DIR)
        cls.reads_config = harness("--check", cls.engine)["reads_config"]

    def todo_while_legacy(self, is_legacy: bool, what: str):
        verdict = todo_or_fail(is_legacy, self.reads_config, what)
        if verdict and verdict[0] == "fail":
            self.fail(verdict[1])
        if verdict:
            self.skipTest(verdict[1])

    def test_0_取り出す関数と手順がすべてEngineにある(self):
        out = harness("--check", self.engine)
        for name in ("conflict_step", "ci_failure_step", "enqueue", "attempts", "pending"):
            self.assertIn(name, out["functions"])
        self.assertEqual(out["entrypoint"], ["mechanical", "state_args"])
        self.assertIsInstance(out["reads_config"], list)

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

    def test_3_CHANGELOGだけの衝突はエージェントを使わずに機械的に解く(self):
        out = harness("worker", self.engine, "CHANGELOG.md")
        self.assertEqual(out["before"], ["CHANGELOG.md"], "衝突を起こせている確かめ")
        self.assertEqual(policy.resolution(self.c, out["before"])["tier"], "none")
        self.assertFalse(out["agent"], f"エージェントを使った: {out}")
        self.assertEqual((out["conflicts"], out["unmerged"], out["markers"]), ([], [], []))

    def test_3_断片を含むCHANGELOGだけの衝突もエージェントを使わずに機械的に解く(self):
        for files in (["changelog.d/700.md"], ["CHANGELOG.md", "changelog.d/700.md"]):
            with self.subTest(files=files):
                out = harness("worker", self.engine, *files)
                self.assertEqual(out["before"], sorted(files), "衝突を起こせている確かめ")
                self.assertEqual(policy.resolution(self.c, out["before"])["mode"], "mechanical")
                self.todo_while_legacy(legacy_worker(out), "changelog.d/ の衝突を機械的に解かない")
                self.assertFalse(out["agent"], f"エージェントを使った: {out}")
                self.assertEqual((out["conflicts"], out["unmerged"], out["markers"]), ([], [], []))

    def test_3_ほかのファイルも衝突したらエージェントに渡す(self):
        out = harness("worker", self.engine, "CHANGELOG.md", "src/app.txt")
        self.assertEqual(policy.resolution(self.c, out["before"])["mode"], "agent")
        self.assertTrue(out["agent"], f"機械的に解いてしまった: {out}")
        self.assertIn("src/app.txt", out["conflicts"])


class TodoScopeTest(unittest.TestCase):
    """todo が Engine の今の形にだけ効き、continuation.yaml を読む版では落ちること、CI では飛ばさないこと（Engine は要らない）"""

    @staticmethod
    def cycles(enqueued_per_cycle: list[list[dict]], stoppages=None) -> dict:
        return {"cycles": [{"head_sha": f"head{i}", "enqueued": e, "stoppages": (stoppages or {}).get(i, [])}
                           for i, e in enumerate(enqueued_per_cycle, 1)]}

    def test_todoは今の形のsupervisorにだけ効く(self):
        legacy = self.cycles([[{"attempt": i, "reason": "conflict", "model_tier": "tier-large"}] for i in range(1, HEADS + 1)])
        self.assertTrue(legacy_supervisor(legacy))
        fixed = self.cycles([[{"attempt": i, "reason": "conflict", "model_tier": "tier-large"}] for i in range(1, 4)] + [[]],
                            {4: [{"key": "ci:https://github.com/guccyon/rally/pull/659", "open": True}]})
        self.assertFalse(legacy_supervisor(fixed))
        # 回数で切らないまま停止だけ開く・tier を変えるなど、今の形から変わったら todo にしない（必須で落ちる）
        self.assertFalse(legacy_supervisor(self.cycles([[{"attempt": i, "reason": "conflict", "model_tier": "tier-small"}] for i in range(1, HEADS + 1)])))
        self.assertFalse(legacy_supervisor(self.cycles([[{"attempt": i, "reason": "conflict", "model_tier": "tier-large"}] for i in range(1, HEADS + 1)],
                                                       {2: [{"key": "ci:x", "open": True}]})))

    def test_todoは今の形のworkerにだけ効く(self):
        files = ["changelog.d/700.md"]
        self.assertTrue(legacy_worker({"before": files, "conflicts": files, "agent": True}))
        self.assertFalse(legacy_worker({"before": files, "conflicts": [], "agent": False}))
        self.assertFalse(legacy_worker({"before": ["CHANGELOG.md"], "conflicts": ["CHANGELOG.md"], "agent": True}))
        self.assertFalse(legacy_worker({"before": files, "conflicts": [], "agent": True}))

    def test_continuation_yamlを読む版で今の形のままならtodoにせず落とす(self):
        self.assertIsNone(todo_or_fail(False, [], "x"))
        self.assertIsNone(todo_or_fail(False, ["orchestrator/dispatcher/continuation.py"], "x"))
        self.assertEqual(todo_or_fail(True, [], "x")[0], "todo")
        kind, message = todo_or_fail(True, ["orchestrator/dispatcher/continuation.py"], "x")
        self.assertEqual(kind, "fail")
        self.assertIn("orchestrator/dispatcher/continuation.py", message)

    def test_continuation_yamlを読むかをEngineのorchestratorから見つける(self):
        sys.path.insert(0, str(HERE))
        import engine_harness
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "orchestrator/dispatcher").mkdir(parents=True)
            (root / "orchestrator/worker").mkdir(parents=True)
            (root / "orchestrator/dispatcher/continuation.py").write_text("LIMIT = 3\n", encoding="utf-8")
            (root / "docs").mkdir()
            (root / "docs/x.md").write_text(".factory-config/continuation.yaml\n", encoding="utf-8")   # orchestrator の外は数えない
            self.assertEqual(engine_harness.reads_config(root), [])
            (root / "orchestrator/worker/entrypoint.sh").write_text("cat .factory-config/continuation.yaml\n", encoding="utf-8")
            self.assertEqual(engine_harness.reads_config(root), ["orchestrator/worker/entrypoint.sh"])

    def test_CIではEngineのチェックアウトがなくても飛ばさずに落とす(self):
        with tempfile.TemporaryDirectory() as tmp:
            for env in ({"CI": "true"}, {"ENGINE_DIR": str(Path(tmp) / "engine")}):
                with self.subTest(env=env):
                    base = {k: v for k, v in os.environ.items() if k not in ("CI", "ENGINE_DIR")}
                    if "CI" in env:
                        # CI=true で ENGINE_DIR がないときは隣の ../engine を見る。ない場所を指すように、写しから走らせる
                        work = Path(tmp) / "repo/tools/conflict-check"
                        work.mkdir(parents=True, exist_ok=True)
                        for f in ("test_engine.py", "engine_harness.py", "policy.py"):
                            (work / f).write_bytes((HERE / f).read_bytes())
                        cfg = Path(tmp) / "repo/.factory-config"
                        cfg.mkdir(exist_ok=True)
                        (cfg / "continuation.yaml").write_bytes((HERE.parents[1] / ".factory-config/continuation.yaml").read_bytes())
                        cwd = work
                    else:
                        cwd = HERE
                    p = subprocess.run([sys.executable, "-B", "-m", "unittest", "test_engine.EngineTest"], cwd=cwd,
                                       env={**base, **env}, capture_output=True, text=True)
                    self.assertNotEqual(p.returncode, 0, p.stderr)
                    self.assertNotIn("skipped", p.stderr.splitlines()[-1], p.stderr)


if __name__ == "__main__":
    unittest.main()
