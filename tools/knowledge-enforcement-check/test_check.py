"""check.py のテスト。python3 -m unittest discover -s tools/knowledge-enforcement-check"""
from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("knowledge_enforcement_check", HERE / "check.py")
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)  # type: ignore[union-attr]

REPO = HERE.parent.parent


def k(level="deterministic", *sols):
    return {"id": "K-9999", "_file": "K-9999.yaml", "enforcement": {"level": level, "solutions": list(sols)}}


def sol(**flags):
    return {"kind": "test", "what": "x", "issue": 1, **flags}


class LabelTest(unittest.TestCase):
    def test_deterministic_でない強さはそのまま(self):
        self.assertEqual(check.label(k("context")), "context")
        self.assertEqual(check.label(k("checklist", sol(wired=True, verified=True))), "checklist")
        self.assertEqual(check.label({"id": "K-9999"}), "context")

    def test_すべて_wired_false_なら_deterministic_と出さない(self):
        self.assertEqual(check.label(k("deterministic", sol(wired=False, verified=False), sol(wired=False, verified=False))), "未配線")

    def test_未記入は_false_として数える(self):
        self.assertEqual(check.label(k("deterministic", sol())), "未配線")
        self.assertEqual(check.label(k("deterministic", sol(wired=True))), "deterministic（未確認）")

    def test_解決策がない_deterministic_は未配線(self):
        self.assertEqual(check.label(k("deterministic")), "未配線")

    def test_配線済みで未確認(self):
        self.assertEqual(check.label(k("deterministic", sol(wired=True, verified=False), sol(wired=False, verified=False))), "deterministic（未確認）")

    def test_配線済みで確認済みが_1_件あれば_deterministic(self):
        self.assertEqual(check.label(k("deterministic", sol(wired=True, verified=True), sol(wired=False, verified=False))), "deterministic")

    def test_真偽でない値は配線済みと数えない(self):
        self.assertEqual(check.label(k("deterministic", sol(wired="true", verified="true"))), "未配線")


class CheckItemTest(unittest.TestCase):
    def test_真偽の項目は通る(self):
        self.assertEqual(check.check_item(k("deterministic", sol(wired=True, verified=True), sol(wired=False, verified=False), sol())), [])

    def test_真偽でない値は落ちる(self):
        e = check.check_item(k("deterministic", sol(wired="yes", verified=0)))
        self.assertEqual(len(e), 2)
        self.assertIn("wired は true か false", e[0])
        self.assertIn("verified は true か false", e[1])

    def test_配線せずに確認済みにはできない(self):
        for s in (sol(wired=False, verified=True), sol(verified=True)):
            e = check.check_item(k("deterministic", s))
            self.assertEqual(len(e), 1)
            self.assertIn("wired でないのに verified が true", e[0])


class MainTest(unittest.TestCase):
    def write(self, d: Path, name: str, text: str):
        root = d / check.ROOT
        root.mkdir(parents=True, exist_ok=True)
        (root / name).write_text(text, encoding="utf-8")

    def test_落ちる記録で_1_を返し要約に出す(self):
        with tempfile.TemporaryDirectory() as t:
            d = Path(t)
            self.write(d, "K-0001.yaml", "id: K-0001\nenforcement:\n  level: deterministic\n  solutions:\n    - {kind: test, what: x, issue: 1, wired: false, verified: true}\n")
            (d / ".github/workflows").mkdir(parents=True)
            (d / ".github/workflows/t.yml").write_text("jobs:\n  t:\n    steps:\n      - run: python3 t.py\n", encoding="utf-8")
            (d / "t.py").write_text("", encoding="utf-8")
            self.write(d, "K-0002.yaml", "id: K-0002\nenforcement:\n  level: deterministic\n  solutions:\n    - {kind: test, what: x, issue: 2, wired: true, verified: true,"
                       " wired_by: [{workflow: .github/workflows/t.yml, job: t, runs: t.py}]}\n")
            out, summary = d / "out.json", d / "summary.md"
            self.assertEqual(check.main(["--repo", t, "--json", str(out), "--summary", str(summary)]), 1)
            result = json.loads(out.read_text(encoding="utf-8"))
            self.assertFalse(result["ok"])
            self.assertEqual([r["label"] for r in result["items"]], ["未配線", "deterministic"])
            self.assertIn("| K-0001 | deterministic | 未配線 | 1 | 0 | 0 | 0 |", summary.read_text(encoding="utf-8"))

    def test_読めない_YAML_は_2(self):
        with tempfile.TemporaryDirectory() as t:
            self.write(Path(t), "K-0001.yaml", "id: [K-0001\n")
            self.assertEqual(check.main(["--repo", t]), 2)


TOOL = "tools/knowledge-enforcement-check/check.py"


class WiringTest(unittest.TestCase):
    """wired: true の解決策の配線先の照合（議決 D-0021、Issue #986）"""

    def repo(self, workflows: dict[str, str], files: dict[str, str] | None = None) -> Path:
        t = tempfile.TemporaryDirectory()
        self.addCleanup(t.cleanup)
        d = Path(t.name)
        for rel, text in {TOOL: "", "tools/sendback-check/order.mjs": "", "tools/sendback-check/engine.test.mjs": "",
                          **{f".github/workflows/{n}": w for n, w in workflows.items()}, **(files or {})}.items():
            (d / rel).parent.mkdir(parents=True, exist_ok=True)
            (d / rel).write_text(text, encoding="utf-8")
        return d

    def errors(self, d: Path, *wired_by, runs=TOOL, workflow=".github/workflows/w.yml", job="j") -> list[str]:
        wb = list(wired_by) or [{"workflow": workflow, "job": job, "runs": runs}]
        return check.check_wiring(k("deterministic", sol(wired=True, verified=False, wired_by=wb)), d)

    def job(self, run: str, extra: str = "") -> str:
        body = "\n".join("          " + line for line in run.splitlines())
        return f"on: pull_request\njobs:\n  j:\n    runs-on: ubuntu-latest\n{extra}    steps:\n      - uses: actions/checkout@v4\n      - run: |\n{body}\n"

    def test_wired_by_がなければ落ちる(self):
        for s in (sol(wired=True), sol(wired=True, wired_by=[]), sol(wired=True, wired_by={"workflow": "x"})):
            e = check.check_wiring(k("deterministic", s), Path("."))
            self.assertEqual(len(e), 1)
            self.assertIn("wired_by", e[0])

    def test_配線していない解決策は見ない(self):
        self.assertEqual(check.check_wiring(k("deterministic", sol(wired=False), sol(), sol(wired="true")), Path(".")), [])

    def test_項目が欠けていれば落ちる(self):
        e = self.errors(self.repo({}), {"workflow": ".github/workflows/w.yml", "job": "j"})
        self.assertIn("workflow・job・runs を文字列で書く", e[0])

    def test_直に呼べば通る(self):
        d = self.repo({"w.yml": self.job(f"python3 {TOOL} --repo .")})
        self.assertEqual(self.errors(d), [])

    def test_ファイルがなければ落ちる(self):
        d = self.repo({})
        self.assertIn(".github/workflows/w.yml がない", self.errors(d)[0])

    def test_ひな形は配線先にならない(self):
        d = self.repo({}, {"tools/knowledge-enforcement-check/ci/knowledge-enforcement.yml": self.job(f"python3 {TOOL}")})
        e = self.errors(d, workflow="tools/knowledge-enforcement-check/ci/knowledge-enforcement.yml")
        self.assertIn("ひな形の ci/*.yml は配線ではない", e[0])

    def test_job_がなければ落ちる(self):
        d = self.repo({"w.yml": self.job(f"python3 {TOOL}")})
        self.assertIn("job other がない", self.errors(d, job="other")[0])

    def test_runs_がなければ落ちる(self):
        d = self.repo({"w.yml": self.job("python3 tools/x/check.py")})
        self.assertIn("runs の tools/x/check.py がない", self.errors(d, runs="tools/x/check.py")[0])

    def test_ほかのジョブや検査を呼ぶだけなら落ちる(self):
        other_job = self.job("echo") + f"  k:\n    runs-on: ubuntu-latest\n    steps:\n      - run: python3 {TOOL}\n"
        for w in (self.job("node tools/sendback-check/order.mjs"), self.job(f"echo {TOOL}".replace(".py", ".txt")), other_job):
            d = self.repo({"w.yml": w})
            e = self.errors(d)
            self.assertEqual(len(e), 1, w)
            self.assertIn(f"job j が {TOOL} を呼んでいない", e[0])

    def test_注釈の中の呼び出しは数えない(self):
        d = self.repo({"w.yml": self.job(f"# python3 {TOOL}\necho ok")})
        self.assertEqual(len(self.errors(d)), 1)

    def test_字面で偽の_if_のジョブと手順は数えない(self):
        for w in (self.job(f"python3 {TOOL}", "    if: false\n"), self.job(f"python3 {TOOL}", "    if: ${{ false }}\n"),
                  f"jobs:\n  j:\n    steps:\n      - if: false\n        run: python3 {TOOL}\n"):
            self.assertEqual(len(self.errors(self.repo({"w.yml": w}))), 1, w)

    def test_working_directory_cd_変数_行の継ぎで呼ぶ形も辿る(self):
        runs = [
            "cd tools/sendback-check && node order.mjs --summary x",
            "node \\\n  tools/sendback-check/order.mjs",
            "node $GITHUB_WORKSPACE/tools/sendback-check/order.mjs",
            "node ${{ github.workspace }}/tools/sendback-check/order.mjs",
            "ORDER=tools/sendback-check/order.mjs\nnode \"$ORDER\"",
            "node ${{ env.ORDER }}",
        ]
        for run in runs:
            d = self.repo({"w.yml": "env:\n  ORDER: tools/sendback-check/order.mjs\n" + self.job(run)})
            self.assertEqual(self.errors(d, runs="tools/sendback-check/order.mjs"), [], run)
        w = f"jobs:\n  j:\n    steps:\n      - working-directory: tools/sendback-check\n        run: node order.mjs\n"
        self.assertEqual(self.errors(self.repo({"w.yml": w}), runs="tools/sendback-check/order.mjs"), [])
        w = f"jobs:\n  j:\n    defaults:\n      run:\n        working-directory: tools/sendback-check\n    steps:\n      - run: node ../sendback-check/order.mjs\n"
        self.assertEqual(self.errors(self.repo({"w.yml": w}), runs="tools/sendback-check/order.mjs"), [])

    def test_同じ名前のほかのディレクトリの検査は数えない(self):
        d = self.repo({"w.yml": self.job("node order.mjs")}, {"order.mjs": ""})
        self.assertEqual(len(self.errors(d, runs="tools/sendback-check/order.mjs")), 1)

    def test_npm_scripts_とラッパーのスクリプトを辿る(self):
        pkg = json.dumps({"scripts": {"order": "node order.mjs", "check": "node check.mjs", "test": "node --test"}})
        files = {"tools/sendback-check/package.json": pkg, "tools/ci/run.sh": "#!/bin/sh\nnode tools/sendback-check/order.mjs \"$@\"\n",
                 "tools/ci/outer.sh": "sh tools/ci/run.sh\n"}
        runs = ["npm --prefix tools/sendback-check run order", "cd tools/sendback-check && npm run-script order", "bash tools/ci/run.sh", "sh tools/ci/outer.sh"]
        for run in runs:
            self.assertEqual(self.errors(self.repo({"w.yml": self.job(run)}, files), runs="tools/sendback-check/order.mjs"), [], run)
        for run in ("npm --prefix tools/sendback-check run check", "npm --prefix tools/sendback-check run missing", "bash tools/ci/none.sh"):
            self.assertEqual(len(self.errors(self.repo({"w.yml": self.job(run)}, files), runs="tools/sendback-check/order.mjs")), 1, run)
        self.assertEqual(self.errors(self.repo({"w.yml": self.job("cd tools/sendback-check && npm test")}, files),
                                     runs="tools/sendback-check/engine.test.mjs"), [])

    def test_テストをまとめて走らせる形はテストのファイルだけ数える(self):
        files = {"tools/knowledge-enforcement-check/test_check.py": ""}
        run = "python3 -m unittest discover -s tools/knowledge-enforcement-check"
        self.assertEqual(self.errors(self.repo({"w.yml": self.job(run)}, files), runs="tools/knowledge-enforcement-check/test_check.py"), [])
        self.assertEqual(len(self.errors(self.repo({"w.yml": self.job(run)}, files))), 1)
        self.assertEqual(len(self.errors(self.repo({"w.yml": self.job("python3 -m unittest discover -s tools/other")}, files),
                                         runs="tools/knowledge-enforcement-check/test_check.py")), 1)

    def test_main_で落とす(self):
        d = self.repo({"w.yml": self.job("echo")})
        MainTest.write(None, d, "K-0001.yaml", "id: K-0001\nenforcement:\n  level: deterministic\n  solutions:\n"
                       f"    - {{kind: ci_check, what: x, issue: 1, wired: true, wired_by: [{{workflow: .github/workflows/w.yml, job: j, runs: {TOOL}}}]}}\n")
        self.assertEqual(check.main(["--repo", str(d)]), 1)


class RepoTest(unittest.TestCase):
    """このリポジトリの記録"""

    def test_リポジトリの知見が通る(self):
        self.assertEqual(check.main(["--repo", str(REPO)]), 0)

    def test_K_0004_はすべての解決策に配線と確認を書き_未配線と出る(self):
        k4 = next(x for x in check.load(REPO) if x["id"] == "K-0004")
        r = check.row(k4)
        self.assertEqual(r["unrecorded"], 0)
        self.assertEqual(r["level"], "deterministic")
        self.assertEqual(r["label"], "未配線")


if __name__ == "__main__":
    unittest.main()
