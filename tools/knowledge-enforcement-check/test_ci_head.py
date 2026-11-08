"""ci/knowledge-enforcement-head.yml のテスト（知見 K-0004 の担保、議決 D-0036、Issue #1076）。

knowledge-enforcement-head は Engine を既定のブランチの先頭で取り、test_engine_patch.py を ENGINE_PATCH_REF で先頭の版へ向けて走らせ、
patch_stale なら ::error を出して Issue #968 に記録する。ワークフローの形と、手順の run を作った Engine と偽の gh で動かした振る舞いを見る。

  python3 -m unittest discover -s tools/knowledge-enforcement-check
"""
from __future__ import annotations

import importlib.util
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
spec = importlib.util.spec_from_file_location("knowledge_enforcement_check", HERE / "check.py")
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)  # type: ignore[union-attr]
patch_spec = importlib.util.spec_from_file_location("knowledge_enforcement_patch", HERE / "test_engine_patch.py")
patch_check = importlib.util.module_from_spec(patch_spec)
patch_spec.loader.exec_module(patch_check)  # type: ignore[union-attr]

TEMPLATE = HERE / "ci" / "knowledge-enforcement-head.yml"
COPY = REPO / ".github" / "workflows" / "knowledge-enforcement-head.yml"
KNOWLEDGE_CHECK = REPO / ".github" / "workflows" / "knowledge-check.yml"

WF = check.load_yaml(TEMPLATE)
STEPS = WF["jobs"]["patch-head"]["steps"]
TEST_STEP = next(s for s in STEPS if s.get("id") == "head-patch")
NOTIFY_STEP = next(s for s in STEPS if "patch_stale なら知らせて Issue に記録する" in s.get("name", ""))
ENGINE_REF = patch_check.engine_ref()   # ENGINE_REF の読み方は test_engine_patch.py の engine_ref() の 1 か所


def git(cwd: Path, *args: str) -> str:
    env = {**os.environ, "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@example.invalid",
           "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@example.invalid"}
    return subprocess.run(["git", *args], cwd=cwd, env=env, capture_output=True, text=True, check=True).stdout.strip()


class Sandbox:
    """一時のディレクトリにリポジトリの形（検査・差分・knowledge-check.yml）と、呼び出しを log に書く偽のコマンドを置く。"""

    def __init__(self, fakes: dict[str, str]):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.dir = self.root / "tools" / "knowledge-enforcement-check"
        (self.dir / "engine").mkdir(parents=True)
        shutil.copy(HERE / "test_engine_patch.py", self.dir)
        shutil.copy(HERE / "engine" / "knowledge-enforcement.patch", self.dir / "engine")
        (self.root / ".github" / "workflows").mkdir(parents=True)
        shutil.copy(KNOWLEDGE_CHECK, self.root / ".github" / "workflows")
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.log = self.root / "calls.log"
        for name, body in fakes.items():
            f = self.bin / name
            f.write_text(f'#!/usr/bin/env bash\necho "{name} $*" >> "{self.log}"\n{body}\n', encoding="utf-8")
            f.chmod(0o755)

    def calls(self) -> str:
        return self.log.read_text(encoding="utf-8") if self.log.exists() else ""

    def engine(self, content: str) -> tuple[Path, str]:
        """core-gates/knowledge_check.py が content の版 1 つの Engine を作り、その版を返す。"""
        d = self.root / ".engine"
        (d / "core-gates").mkdir(parents=True)
        (d / "core-gates" / "knowledge_check.py").write_text(content, encoding="utf-8")
        git(d, "init", "-q")
        git(d, "add", ".")
        git(d, "commit", "-qm", "head")
        return d, git(d, "rev-parse", "HEAD")

    def run(self, step: dict, **env: str) -> subprocess.CompletedProcess:
        base = {k: v for k, v in os.environ.items() if k not in ("ENGINE_PATCH_REF", "ENGINE_DIR", "GITHUB_ACTIONS", "CI")}
        return subprocess.run(["bash", "-e", "-o", "pipefail", "-c", step["run"]], cwd=self.dir, capture_output=True, text=True,
                              env={**base, "PATH": f"{self.bin}:{os.environ['PATH']}", "ISSUE": "968", "GITHUB_REPOSITORY": "guccyon/rally",
                                   "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "123", **env})

    def close(self):
        self.tmp.cleanup()


HEAD = "a" * 40
STALE_OUT = ("::error file=x::patch_stale: x\n"
             "AssertionError: patch_stale: tools/knowledge-enforcement-check/engine/knowledge-enforcement.patch が ENGINE_PATCH_REF "
             f"{HEAD} の Engine に当たらず、Engine 側にもまだ入っていない。今の Engine に合わせて作り直す: error: patch failed: core-gates/knowledge_check.py:1\n"
             "error: core-gates/knowledge_check.py: patch does not apply\n\n")


class ShapeTest(unittest.TestCase):
    def test_定期と手動でだけ走り_PR_では走らない(self):
        on = WF.get("on", WF.get(True))   # PyYAML は on を真偽として読む
        self.assertEqual(set(on), {"schedule", "workflow_dispatch"})
        self.assertTrue(on["schedule"][0]["cron"])
        self.assertEqual(WF["permissions"], {"contents": "read", "issues": "write"})
        self.assertEqual(WF["env"]["ISSUE"], "968")
        self.assertNotIn("continue-on-error", TEMPLATE.read_text(encoding="utf-8").split("\njobs:")[1], "落ちても気づかなくなる")

    def test_Engine_を_ref_なしで先頭から取り_先頭の版へ向けて_test_engine_patch_を走らせる(self):
        checkout = next(s for s in STEPS if s.get("with", {}).get("repository") == "guccyon/factory-engine")
        self.assertNotIn("ref", checkout["with"], "ref を付けると先頭ではなくなる")
        self.assertEqual(checkout["with"]["path"], ".engine")
        self.assertLess(STEPS.index(checkout), STEPS.index(TEST_STEP))
        self.assertEqual(TEST_STEP["env"]["ENGINE_DIR"], "${{ github.workspace }}/.engine")
        self.assertEqual(TEST_STEP["working-directory"], "tools/knowledge-enforcement-check")
        self.assertIn('ENGINE_PATCH_REF="$head_sha" python3 -m unittest', TEST_STEP["run"])
        self.assertIn("test_engine_patch.py", TEST_STEP["run"])
        self.assertNotIn("if", TEST_STEP)
        self.assertEqual(NOTIFY_STEP["if"], "always()")
        self.assertEqual(NOTIFY_STEP["env"]["HEAD_OUTCOME"], "${{ steps.head-patch.outcome }}")
        self.assertLess(STEPS.index(TEST_STEP), STEPS.index(NOTIFY_STEP))

    @unittest.skipUnless(COPY.exists(), "まだ .github/workflows/ に写していない")
    def test_写しがあればひな形と同じ(self):
        self.assertEqual(COPY.read_text(encoding="utf-8"), TEMPLATE.read_text(encoding="utf-8"))


class StepTest(unittest.TestCase):
    def setUp(self):
        self.box = Sandbox({"gh": '[ "$1" = api ] && printf "%s\\n" "ほかの記録"; exit 0'})
        self.addCleanup(self.box.close)

    def test_先頭に当たらなければ_patch_stale_で落ち_ENGINE_HEAD_を残し_先頭の版を名指しする(self):
        engine, sha = self.box.engine("x\n")
        ghenv, summary = self.box.root / "github_env", self.box.root / "summary"
        r = self.box.run(TEST_STEP, ENGINE_DIR=str(engine), GITHUB_ENV=str(ghenv), GITHUB_STEP_SUMMARY=str(summary))
        self.assertNotEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertEqual(ghenv.read_text(encoding="utf-8"), f"ENGINE_HEAD={sha}\n")
        out = (self.box.dir / "patch-head.txt").read_text(encoding="utf-8")
        self.assertIn(f"patch_stale: tools/knowledge-enforcement-check/engine/knowledge-enforcement.patch が ENGINE_PATCH_REF {sha} の Engine に", out)
        self.assertIn(f"Engine の先頭 {sha}", summary.read_text(encoding="utf-8"))

        n = self.box.run(NOTIFY_STEP, ENGINE_HEAD=sha, HEAD_OUTCOME="failure")
        self.assertEqual(n.returncode, 0, n.stdout + n.stderr)
        self.assertRegex(n.stdout, rf"(?m)^::error title=knowledge-enforcement-head patch_stale::.*{sha}.*ENGINE_REF（{ENGINE_REF}）")
        self.assertRegex(self.box.calls(), r"(?m)^gh api repos/guccyon/rally/issues/968/comments ")
        self.assertRegex(self.box.calls(), r"(?m)^gh issue comment 968 --repo guccyon/rally --body-file patch-head-comment\.md$")
        body = (self.box.dir / "patch-head-comment.md").read_text(encoding="utf-8")
        self.assertTrue(body.startswith(f"<!-- knowledge-enforcement-head: {sha} patch_stale -->\n"), body)
        self.assertIn(f"`{sha}`", body)
        self.assertIn(f"`{ENGINE_REF}`", body)
        self.assertIn("https://github.com/guccyon/rally/actions/runs/123", body)
        self.assertIn("patch does not apply", body)

    def test_先頭にもう入っていれば通り_記録しない(self):
        engine, _ = self.box.engine("a\n")
        f = engine / "core-gates" / "knowledge_check.py"
        f.write_text("a\ndef check_wired_by(x):\n", encoding="utf-8")
        (self.box.dir / "engine" / "knowledge-enforcement.patch").write_text(git(engine, "diff") + "\n", encoding="utf-8")
        git(engine, "commit", "-qam", "merged")   # 先頭は差分を逆に当てられる（もう全部入っている）版
        sha = git(engine, "rev-parse", "HEAD")
        r = self.box.run(TEST_STEP, ENGINE_DIR=str(engine), GITHUB_ENV=str(self.box.root / "e"), GITHUB_STEP_SUMMARY=str(self.box.root / "s"))
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        n = self.box.run(NOTIFY_STEP, ENGINE_HEAD=sha, HEAD_OUTCOME="success")
        self.assertEqual(n.returncode, 0, n.stdout + n.stderr)
        self.assertNotIn("::error", n.stdout)
        self.assertEqual(self.box.calls(), "")

    def test_ENGINE_PATCH_REF_がなければ_ENGINE_REF_の版に当てる(self):
        engine, _ = self.box.engine("def check_wired_by(x):\n")
        r = subprocess.run(["python3", "-m", "unittest", "discover", "-s", ".", "-p", "test_engine_patch.py"], cwd=self.box.dir,
                           capture_output=True, text=True, env={**{k: v for k, v in os.environ.items() if k != "ENGINE_PATCH_REF"}, "ENGINE_DIR": str(engine)})
        self.assertNotEqual(r.returncode, 0)
        self.assertIn(f"ENGINE_REF {ENGINE_REF} の版がない", r.stderr)

    def test_記録の_ENGINE_REF_は_engine_ref_と同じ読み方で_クオートで囲んでも読み_読めなければそう書く(self):
        (self.box.dir / "patch-head.txt").write_text(STALE_OUT, encoding="utf-8")
        wf = self.box.root / ".github" / "workflows" / "knowledge-check.yml"
        bumped = "b" * 40
        wf.write_text(KNOWLEDGE_CHECK.read_text(encoding="utf-8").replace(f"ENGINE_REF: {ENGINE_REF}", f'ENGINE_REF: "{bumped}"'), encoding="utf-8")
        self.assertEqual(patch_check.engine_ref(wf), bumped)
        n = self.box.run(NOTIFY_STEP, ENGINE_HEAD=HEAD, HEAD_OUTCOME="failure")
        self.assertEqual(n.returncode, 0, n.stdout + n.stderr)
        self.assertIn(f"ENGINE_REF（{bumped}）", n.stdout)
        wf.write_text("env:\n  ENGINE_REF: main\n", encoding="utf-8")
        n = self.box.run(NOTIFY_STEP, ENGINE_HEAD=HEAD, HEAD_OUTCOME="failure")
        self.assertEqual(n.returncode, 0, n.stdout + n.stderr)
        self.assertIn("ENGINE_REF（読めなかった）", n.stdout)

    def test_同じ先頭の記録があれば足さない(self):
        box = Sandbox({"gh": f'[ "$1" = api ] && printf "%s\\n" "<!-- knowledge-enforcement-head: {HEAD} patch_stale -->"; exit 0'})
        self.addCleanup(box.close)
        (box.dir / "patch-head.txt").write_text(STALE_OUT, encoding="utf-8")
        n = box.run(NOTIFY_STEP, ENGINE_HEAD=HEAD, HEAD_OUTCOME="failure")
        self.assertEqual(n.returncode, 0, n.stdout + n.stderr)
        self.assertRegex(n.stdout, r"(?m)^::error title=knowledge-enforcement-head patch_stale::")
        self.assertNotRegex(box.calls(), r"(?m)^gh issue comment")

    def test_patch_stale_でない落ち方は_error_を出すが記録しない(self):
        (self.box.dir / "patch-head.txt").write_text("ImportError: x\n", encoding="utf-8")
        for env in ({"ENGINE_HEAD": HEAD, "HEAD_OUTCOME": "failure"}, {"ENGINE_HEAD": "", "HEAD_OUTCOME": "failure"}, {"ENGINE_HEAD": HEAD}):
            n = self.box.run(NOTIFY_STEP, **env)
            self.assertEqual(n.returncode, 0, n.stdout + n.stderr)
            self.assertRegex(n.stdout, r"(?m)^::error title=knowledge-enforcement-head head_check_failed::")
        self.assertEqual(self.box.calls(), "")

    def test_Issue_への記録に失敗したら落ちる_これまでの記録が読めなければ足す側に倒す(self):
        fail_comment = Sandbox({"gh": '[ "$1" = api ] && exit 0; exit 1'})
        fail_api = Sandbox({"gh": '[ "$1" = api ] && exit 1; exit 0'})
        for b in (fail_comment, fail_api):
            self.addCleanup(b.close)
            (b.dir / "patch-head.txt").write_text(STALE_OUT, encoding="utf-8")
        self.assertNotEqual(fail_comment.run(NOTIFY_STEP, ENGINE_HEAD=HEAD, HEAD_OUTCOME="failure").returncode, 0)
        self.assertEqual(fail_api.run(NOTIFY_STEP, ENGINE_HEAD=HEAD, HEAD_OUTCOME="failure").returncode, 0)
        self.assertRegex(fail_api.calls(), r"(?m)^gh issue comment 968 ")


if __name__ == "__main__":
    unittest.main()
