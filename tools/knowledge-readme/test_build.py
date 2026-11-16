import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
BUILD = HERE / "build.py"


def run(repo: Path, *extra: str):
    return subprocess.run([sys.executable, "-I", str(BUILD), "--repo", str(repo), *extra], capture_output=True, text=True)


class BuildTest(unittest.TestCase):
    def test_リポジトリの_README_は断片から作り直して差分が出ない(self):
        r = run(REPO, "--check")
        self.assertEqual(r.returncode, 0, r.stderr)

    def test_手で書き換えた_README_は検査で落ちる(self):
        with tempfile.TemporaryDirectory() as d:
            k = Path(d) / ".factory-config/knowledge"
            (k / "readme.d").mkdir(parents=True)
            (k / "readme.d/01.md").write_text("# a\n", encoding="utf-8")
            (k / "README.md").write_text("# a\n手書き\n", encoding="utf-8")
            self.assertEqual(run(Path(d), "--check").returncode, 1)
            self.assertEqual(run(Path(d)).returncode, 0)
            self.assertEqual(run(Path(d), "--check").returncode, 0)


if __name__ == "__main__":
    unittest.main()
