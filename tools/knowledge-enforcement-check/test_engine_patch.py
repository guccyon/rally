"""engine/knowledge_check.patch のテスト（知見 K-0004 の担保、議決 D-0016、Issue #965）。

ENGINE_DIR に factory-engine のチェックアウト（ENGINE_REF の版）を渡したときだけ走る。渡さなければ飛ばす。
写しに差分を当て、当てた Engine の core-gates/knowledge_check.py で次を確かめる。
  - 差分がそのまま当たり（Engine にもう入っていればそのまま）、Engine の core-gates/tests/test_knowledge_check.py に足したテストが通る
  - このリポジトリの知見で INDEX.md を作り直すと、担保の列が check.py の label() と知見ごとに同じになり、K-0004 は「未配線」
  - wired・verified が真偽でない、wired でないのに verified が true の知見で落ちる
python3 -m unittest discover -s tools/knowledge-enforcement-check
"""
from __future__ import annotations

import importlib.util
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("knowledge_enforcement_check", HERE / "check.py")
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)  # type: ignore[union-attr]

REPO = HERE.parent.parent
PATCH = HERE / "engine" / "knowledge_check.patch"
FILES = ("definitions/knowledge.yaml", "core-gates/knowledge_check.py", "core-gates/tests/test_knowledge_check.py")
ENGINE_DIR = os.environ.get("ENGINE_DIR")


def patched_engine(dst: Path) -> Path:
    """ENGINE_DIR の 3 つのファイルを写して差分を当てる。Engine にもう入っている（逆向きに当たる）なら当てずにそのまま使う"""
    for f in FILES:
        (dst / f).parent.mkdir(parents=True, exist_ok=True)
        shutil.copy(Path(ENGINE_DIR) / f, dst / f)  # type: ignore[arg-type]
    git = lambda *a: subprocess.run(["git", "apply", *a, str(PATCH)], cwd=dst, capture_output=True, text=True)  # noqa: E731
    if git("--reverse", "--check").returncode != 0:
        r = git()
        assert r.returncode == 0, f"engine/knowledge_check.patch が Engine に当たらない: {r.stderr}"
    return dst / "core-gates" / "knowledge_check.py"


def index_labels(index: Path) -> dict[str, str]:
    """INDEX.md の id と担保の列"""
    out = {}
    for line in index.read_text(encoding="utf-8").splitlines():
        cells = [c.strip() for c in line.strip("|").split("|")]
        if len(cells) == 9 and cells[0].startswith("K-"):
            out[cells[0]] = cells[5]
    return out


@unittest.skipUnless(ENGINE_DIR, "ENGINE_DIR（factory-engine のチェックアウト）がない")
class EnginePatchTest(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)
        self.kc = patched_engine(self.tmp / "engine")
        self.repo = self.tmp / "repo"
        shutil.copytree(REPO / check.ROOT, self.repo / check.ROOT)

    def run_kc(self, *args: str) -> subprocess.CompletedProcess:
        return subprocess.run([sys.executable, str(self.kc), "--repo", str(self.repo), *args], capture_output=True, text=True)

    def test_Engine_のテストが通る(self):
        if importlib.util.find_spec("pytest") is None:
            self.skipTest("pytest がない")
        r = subprocess.run([sys.executable, "-m", "pytest", "-q", "core-gates/tests/test_knowledge_check.py"], cwd=self.tmp / "engine",
                           capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)

    def test_INDEX_の担保の列が_label_と同じで_K_0004_は未配線(self):
        self.assertEqual(self.run_kc("--index").returncode, 0)
        got = index_labels(self.repo / check.ROOT / "INDEX.md")
        want = {str(k["id"]): check.label(k) for k in check.load(self.repo)}
        self.assertEqual(got, want)
        self.assertEqual(got["K-0004"], check.UNWIRED)
        r = self.run_kc()
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)

    def test_真偽でない値と配線せずに確認済みは落ちる(self):
        p = self.repo / check.ROOT / "K-0004.yaml"
        text = p.read_text(encoding="utf-8")
        for bad, msg in [(text.replace("wired: false", 'wired: "false"', 1), "wired は true か false"),
                         (text.replace("verified: false", "verified: true", 1), "wired でないのに verified が true")]:
            p.write_text(bad, encoding="utf-8")
            self.run_kc("--index")
            r = self.run_kc()
            self.assertEqual(r.returncode, 1, r.stdout)
            self.assertIn(msg, r.stdout)


if __name__ == "__main__":
    unittest.main()
