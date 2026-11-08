"""Engine に移す差分が ENGINE_REF の版に当たるかの検査（知見 K-0004 の担保、議決 D-0024、Issue #1014）。

engine/knowledge-enforcement.patch を、.github/workflows/knowledge-check.yml の ENGINE_REF の版の Engine に git apply --check で当て、
当たらず、しかも Engine 側にまだ入っていない（git apply --reverse --check が通らず、check_wired_by もない）ときは patch_stale で落とす。
差分が当たるか、もう入っていれば通る。test_engine.py は ENGINE_DIR のチェックアウトの今の中身に当てるが、こちらは ENGINE_REF の版を
名指しで取り出して当てるので、チェックアウトが別の版でも製品の CI が使う版で確かめられる。
Engine のチェックアウトは環境変数 ENGINE_DIR で渡す（なければリポジトリの隣の ../engine）。チェックアウトがないか、ENGINE_REF の版を
含まないときは飛ばす。ただし ENGINE_DIR を渡したときと CI（CI=true）では飛ばさずに落とす（test_engine.py と同じ）。
環境変数 ENGINE_PATCH_REF に版を渡すと、ENGINE_REF の代わりにその版に当てる（ci/knowledge-enforcement-head.yml が Engine の既定のブランチの
先頭の版を渡し、ENGINE_REF が古いまま先頭だけが進んで差分が当たらなくなったことを定期に見る。議決 D-0036、Issue #1076）。

  python3 -m unittest discover -s tools/knowledge-enforcement-check
"""
from __future__ import annotations

import io
import os
import re
import subprocess
import tarfile
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
PATCH = HERE / "engine" / "knowledge-enforcement.patch"
WORKFLOW = REPO / ".github" / "workflows" / "knowledge-check.yml"
MARKER = ("core-gates/knowledge_check.py", "def check_wired_by(")   # 差分で最後に足したもの（Issue #1060）

ENGINE = Path(os.environ.get("ENGINE_DIR") or REPO.parent / "engine")
REQUIRED = bool(os.environ.get("ENGINE_DIR")) or os.environ.get("CI") == "true"
PATCH_REF = os.environ.get("ENGINE_PATCH_REF", "")

APPLIES, MERGED, STALE = "applies", "merged", "patch_stale"


def engine_ref(workflow: Path = WORKFLOW) -> str:
    """ワークフローの env の ENGINE_REF（40 桁のコミット SHA）。"""
    m = re.search(r"^\s*ENGINE_REF:\s*['\"]?([0-9a-f]{40})['\"]?\s*$", workflow.read_text(encoding="utf-8"), re.M)
    if not m:
        raise ValueError(f"{workflow} に ENGINE_REF（40 桁のコミット SHA）がない")
    return m.group(1)


def target_ref() -> tuple[str, str]:
    """差分を当てる版と、その出どころ（ENGINE_PATCH_REF があればその版、なければ ENGINE_REF）。"""
    return (PATCH_REF, "ENGINE_PATCH_REF") if PATCH_REF else (engine_ref(), "ENGINE_REF")


def has_commit(engine: Path, ref: str) -> bool:
    r = subprocess.run(["git", "-C", str(engine), "cat-file", "-e", f"{ref}^{{commit}}"], capture_output=True)
    return r.returncode == 0


def patch_state(engine: Path, ref: str, patch: Path = PATCH, marker: tuple[str, str] = MARKER) -> tuple[str, str]:
    """ref の版の Engine に対する差分の状態と、git apply の出力。APPLIES = 当たる、MERGED = もう入っている、STALE = どちらでもない。"""
    tar = subprocess.run(["git", "-C", str(engine), "archive", "--format=tar", ref], capture_output=True, check=True).stdout
    with tempfile.TemporaryDirectory() as t:
        with tarfile.open(fileobj=io.BytesIO(tar)) as f:
            f.extractall(t, filter="data") if hasattr(tarfile, "data_filter") else f.extractall(t)
        apply = lambda *a: subprocess.run(["git", "apply", "--check", *a, str(patch)], cwd=t, capture_output=True, text=True)  # noqa: E731
        fwd = apply()
        if fwd.returncode == 0:
            return APPLIES, ""
        if apply("--reverse").returncode == 0:
            return MERGED, ""
        path = Path(t) / marker[0]
        if path.is_file() and marker[1] in path.read_text(encoding="utf-8"):
            return MERGED, ""
        return STALE, fwd.stderr.strip()


def git(cwd: Path, *args: str) -> str:
    env = {**os.environ, "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@example.invalid",
           "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@example.invalid"}
    return subprocess.run(["git", *args], cwd=cwd, env=env, capture_output=True, text=True, check=True).stdout.strip()


class PatchStateTest(unittest.TestCase):
    """作った Engine の写し（git の版を 3 つ）で、当たる・入っている・どちらでもないの判定を確かめる。Engine は要らない。"""

    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        d = cls.dir = Path(cls.tmp.name) / "engine"
        (d / "core-gates").mkdir(parents=True)
        git(d, "init", "-q")
        f = d / "core-gates" / "knowledge_check.py"
        f.write_text("a\nb\nc\n", encoding="utf-8")
        git(d, "add", ".")
        git(d, "commit", "-qm", "base")
        cls.base = git(d, "rev-parse", "HEAD")
        f.write_text("a\nb\ndef check_wired_by(x):\nc\n", encoding="utf-8")
        cls.patch = Path(cls.tmp.name) / "p.patch"
        cls.patch.write_text(git(d, "diff") + "\n", encoding="utf-8")
        git(d, "commit", "-qam", "merged")
        cls.merged = git(d, "rev-parse", "HEAD")
        f.write_text("a\nB\ndef check_wired_by(x, y):\nc\n", encoding="utf-8")
        git(d, "commit", "-qam", "merged-then-changed")
        cls.changed = git(d, "rev-parse", "HEAD")
        f.write_text("x\ny\nz\n", encoding="utf-8")
        git(d, "commit", "-qam", "moved")
        cls.moved = git(d, "rev-parse", "HEAD")

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def state(self, ref):
        return patch_state(self.dir, ref, self.patch)[0]

    def test_当たる版は通る(self):
        self.assertEqual(self.state(self.base), APPLIES)

    def test_もう入っている版は通る(self):
        self.assertEqual(self.state(self.merged), MERGED)

    def test_入れたあと変わった版も目印があれば通る(self):
        self.assertEqual(self.state(self.changed), MERGED)

    def test_当たらずまだ入っていない版は_patch_stale(self):
        state, err = patch_state(self.dir, self.moved, self.patch)
        self.assertEqual(state, STALE)
        self.assertIn("core-gates/knowledge_check.py", err)

    def test_ENGINE_REF_を読む(self):
        self.assertRegex(engine_ref(), r"^[0-9a-f]{40}$")
        w = Path(self.tmp.name) / "w.yml"
        w.write_text("env:\n  ENGINE_REF: main\n", encoding="utf-8")
        with self.assertRaises(ValueError):
            engine_ref(w)


REF, REF_FROM = target_ref()
HAS_ENGINE = (ENGINE / ".git").exists() and has_commit(ENGINE, REF)


@unittest.skipUnless(HAS_ENGINE or REQUIRED, f"{REF_FROM} の版を含む Engine のチェックアウトがない（ENGINE_DIR={ENGINE}）")
class EngineRefPatchTest(unittest.TestCase):
    def test_差分が_ENGINE_REF_の版に当たるかもう入っている(self):
        ref = REF
        if not HAS_ENGINE:
            if REF_FROM == "ENGINE_PATCH_REF":
                raise AssertionError(f"Engine のチェックアウト（ENGINE_DIR={ENGINE}）に ENGINE_PATCH_REF {ref} の版がない")
            raise AssertionError(f"Engine のチェックアウト（ENGINE_DIR={ENGINE}）に knowledge-check.yml の ENGINE_REF {ref} の版がない。"
                                 "ci/knowledge-enforcement.yml の ENGINE_REF を knowledge-check.yml と同じ版にそろえる")
        state, err = patch_state(ENGINE, ref)
        if state == STALE:
            msg = (f"patch_stale: {PATCH.relative_to(REPO)} が {REF_FROM} {ref} の Engine に当たらず、Engine 側にもまだ入っていない。"
                   f"今の Engine に合わせて作り直す: {err}")
            if os.environ.get("GITHUB_ACTIONS") == "true":
                print(f"::error file={PATCH.relative_to(REPO)}::{msg.splitlines()[0]}")
            self.fail(msg)
        self.assertIn(state, (APPLIES, MERGED))


if __name__ == "__main__":
    unittest.main()
