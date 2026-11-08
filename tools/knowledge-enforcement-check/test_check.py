"""check.py のテスト。python3 -m unittest discover -s tools/knowledge-enforcement-check"""
from __future__ import annotations

import importlib.util
import json
import re
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


class VerificationTest(unittest.TestCase):
    """verified: true の解決策の確認の記録の照合（議決 D-0034、Issue #1059）"""

    def repo(self, *cases: str) -> Path:
        t = tempfile.TemporaryDirectory()
        self.addCleanup(t.cleanup)
        d = Path(t.name)
        (d / check.ROOT / "cases").mkdir(parents=True)
        for c in cases:
            (d / check.ROOT / "cases" / f"{c}.md").write_text(f"# {c}\n", encoding="utf-8")
        return d

    def errors(self, d: Path, verified_by=None, **flags) -> list[str]:
        s = sol(wired=True, verified=True, **flags)
        if verified_by is not None:
            s["verified_by"] = verified_by
        return check.check_verification(k("deterministic", s), d)

    def test_事例記録を指せば通る(self):
        d = self.repo("C-0001")
        self.assertEqual(self.errors(d, [{"case": "C-0001"}]), [])
        self.assertEqual(self.errors(d, [{"case": "C-0001"}, {"pr": 395}, {"event": "case.analyzed:40376"}]), [])

    def test_verified_by_がなければ落ちる(self):
        for vb in (None, [], {"case": "C-0001"}):
            e = self.errors(self.repo("C-0001"), vb)
            self.assertEqual(len(e), 1, vb)
            self.assertIn("verified_by（確かめた本物の記録を指す case・pr・event の列）がない", e[0])

    def test_事例記録がなければ落ちる(self):
        e = self.errors(self.repo("C-0001"), [{"case": "C-0002"}])
        self.assertEqual(len(e), 1)
        self.assertIn("事例記録 C-0002 が .factory-config/knowledge/cases/ にない", e[0])

    def test_pr_と_event_だけでは落ちる(self):
        e = self.errors(self.repo("C-0001"), [{"pr": 395}, {"event": "case.analyzed:40376"}])
        self.assertEqual(len(e), 1)
        self.assertIn("事例記録（case）が 1 件もない", e[0])

    def test_形の崩れた参照は落ちる(self):
        d = self.repo("C-0001")
        for v, msg in (({"case": "C-1"}, "case は C-NNNN"), ({"pr": "395"}, "pr は PR の番号"), ({"pr": True}, "pr は PR の番号"),
                       ({"pr": 0}, "pr は PR の番号"), ({"event": "case.analyzed"}, "event は <種類>:<id>"),
                       ({"issue": 1}, "種類は case・pr・event"), ({"case": "C-0001", "pr": 1}, "1 項目に 1 つの参照"), ("C-0001", "1 項目に 1 つの参照")):
            e = self.errors(d, [{"case": "C-0001"}, v])
            self.assertEqual(len(e), 1, v)
            self.assertIn(msg, e[0])

    def test_確認していない解決策は見ない(self):
        d = self.repo()
        sols = (sol(wired=True, verified=False), sol(wired=True), sol(wired=True, verified="true"), sol(verified=False, verified_by=[{"pr": "x"}]))
        self.assertEqual(check.check_verification(k("deterministic", *sols), d), [])

    def test_配線していなければ_verified_by_があっても落ちる(self):
        d = self.repo("C-0001")
        MainTest.write(None, d, "K-0001.yaml", "id: K-0001\nenforcement:\n  level: deterministic\n  solutions:\n"
                       "    - {kind: ci_check, what: x, issue: 1, wired: false, verified: true, verified_by: [{case: C-0001}]}\n")
        out = d / "out.json"
        self.assertEqual(check.main(["--repo", str(d), "--json", str(out)]), 1)
        errors = json.loads(out.read_text(encoding="utf-8"))["errors"]
        self.assertEqual(len(errors), 1)
        self.assertIn("wired でないのに verified が true", errors[0])

    def test_main_で落とす(self):
        d = self.repo("C-0001")
        MainTest.write(None, d, "K-0001.yaml", "id: K-0001\nenforcement:\n  level: deterministic\n  solutions:\n"
                       "    - {kind: ci_check, what: x, issue: 1, wired: false, verified: false, verified_by: [{case: C-0009}]}\n"
                       "    - {kind: ci_check, what: x, issue: 2, wired: false, verified: true, verified_by: [{case: C-0009}]}\n")
        out = d / "out.json"
        self.assertEqual(check.main(["--repo", str(d), "--json", str(out)]), 1)
        errors = json.loads(out.read_text(encoding="utf-8"))["errors"]
        self.assertTrue(any("issue 2）: verified_by[0]: 事例記録 C-0009" in x for x in errors), errors)
        self.assertFalse(any("issue 1）" in x for x in errors), errors)


class ScheduleTest(unittest.TestCase):
    """K-0004 の issue 1018 の解決策は schedule のジョブから protection.mjs を呼ばないと wired にできない（議決 D-0039、Issue #1088）"""

    PROT = "tools/sendback-check/protection.mjs"

    def repo(self, workflow: str) -> Path:
        t = tempfile.TemporaryDirectory()
        self.addCleanup(t.cleanup)
        d = Path(t.name)
        for rel, text in {self.PROT: "", ".github/workflows/p.yml": workflow}.items():
            (d / rel).parent.mkdir(parents=True, exist_ok=True)
            (d / rel).write_text(text, encoding="utf-8")
        return d

    def wf(self, on: str, job_if: str = "", step_if: str = "") -> str:
        return (f"on:\n{on}jobs:\n  protection:\n    runs-on: ubuntu-latest\n{job_if}    steps:\n      - uses: actions/checkout@v4\n"
                f"      - {step_if}run: node {self.PROT}\n")

    def errors(self, d: Path, kid="K-0004", issue=1018, wired=True, runs=None) -> list[str]:
        wb = [{"workflow": ".github/workflows/p.yml", "job": "protection", "runs": runs or self.PROT}]
        item = {"id": kid, "_file": f"{kid}.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "ci_check", "what": "x", "issue": issue, "wired": wired, "verified": False, "wired_by": wb}]}}
        return check.check_schedule(item, d)

    SCHEDULE = "  schedule:\n    - cron: '0 0 * * *'\n  workflow_dispatch:\n"

    def test_schedule_のジョブから呼べば通る(self):
        d = self.repo(self.wf(self.SCHEDULE))
        self.assertEqual(self.errors(d), [])
        self.assertEqual(self.errors(self.repo(self.wf(self.SCHEDULE, "    if: github.event_name == 'schedule' || github.event_name == 'workflow_dispatch'\n"))), [])

    def test_手で走らせるだけのワークフローでは落ちる(self):
        for on in ("  workflow_dispatch:\n", "  pull_request:\n  workflow_dispatch:\n", "  schedule: []\n  workflow_dispatch:\n", "  schedule:\n    - cron: ''\n"):
            e = self.errors(self.repo(self.wf(on)))
            self.assertEqual(len(e), 1, on)
            self.assertIn("on.schedule（cron）", e[0])
            self.assertIn("議決 D-0039", e[0])

    def test_schedule_では走らないジョブと手順は数えない(self):
        for job_if, step_if in (("    if: github.event_name == 'workflow_dispatch'\n", ""), ("", "if: github.event_name != 'schedule'\n        "),
                                ("    if: github.event_name == 'schedule' && github.event_name != 'schedule'\n", ""),
                                ("", "if: ${{ github.event_name == 'workflow_dispatch' }}\n        "), ("    if: false\n", "")):
            self.assertEqual(len(self.errors(self.repo(self.wf(self.SCHEDULE, job_if, step_if)))), 1, (job_if, step_if))

    def test_ほかの検査を呼ぶ項目だけでは落ちる(self):
        d = self.repo(self.wf(self.SCHEDULE))
        (d / "tools/sendback-check/order.mjs").write_text("", encoding="utf-8")
        self.assertEqual(len(self.errors(d, runs="tools/sendback-check/order.mjs")), 1)

    def test_対象でない解決策と配線していない解決策は見ない(self):
        d = self.repo(self.wf("  workflow_dispatch:\n"))
        self.assertEqual(self.errors(d, issue=1017), [])
        self.assertEqual(self.errors(d, kid="K-0001"), [])
        self.assertEqual(self.errors(d, wired=False), [])

    def test_main_で落とす(self):
        d = self.repo(self.wf("  workflow_dispatch:\n"))
        MainTest.write(None, d, "K-0004.yaml", "id: K-0004\nenforcement:\n  level: deterministic\n  solutions:\n"
                       f"    - {{kind: ci_check, what: x, issue: 1018, wired: true, wired_by: [{{workflow: .github/workflows/p.yml, job: protection, runs: {self.PROT}}}]}}\n")
        out = d / "out.json"
        self.assertEqual(check.main(["--repo", str(d), "--json", str(out)]), 1)
        errors = json.loads(out.read_text(encoding="utf-8"))["errors"]
        self.assertEqual(len(errors), 1, errors)
        self.assertIn("issue 1018）: wired にするには", errors[0])


class RequiredRunsTest(unittest.TestCase):
    """K-0010 の issue 1034・1100 の解決策は 1 つのジョブから test_watch.py・test_engine.py を呼ばないと wired にできない（議決 D-0042、Issue #1100）"""

    WATCH, ENGINE, POLICY = "tools/conflict-check/test_watch.py", "tools/conflict-check/test_engine.py", "tools/conflict-check/test_policy.py"
    TEMPLATE = REPO / "tools/conflict-check/ci/conflict-check.yml"

    def repo(self, workflows: dict[str, str]) -> Path:
        t = tempfile.TemporaryDirectory()
        self.addCleanup(t.cleanup)
        d = Path(t.name)
        for rel, text in {self.WATCH: "", self.ENGINE: "", self.POLICY: "", **{f".github/workflows/{n}": w for n, w in workflows.items()}}.items():
            (d / rel).parent.mkdir(parents=True, exist_ok=True)
            (d / rel).write_text(text, encoding="utf-8")
        return d

    def wf(self, *runs: str, job="c") -> str:
        steps = "".join(f"      - run: {r}\n" for r in runs)
        return f"on: pull_request\njobs:\n  {job}:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n{steps}"

    def items(self, *pairs) -> list[dict]:
        return [{"workflow": f".github/workflows/{w}", "job": j, "runs": r} for w, j, r in pairs]

    def errors(self, d: Path, wired_by=None, kid="K-0010", issue=1034, wired=True) -> list[str]:
        wb = self.items(("c.yml", "c", self.WATCH), ("c.yml", "c", self.ENGINE)) if wired_by is None else wired_by
        item = {"id": kid, "_file": f"{kid}.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "ci_check", "what": "x", "issue": issue, "wired": wired, "verified": False, "wired_by": wb}]}}
        return check.check_required_runs(item, d)

    DISCOVER = "python3 -m unittest discover -s tools/conflict-check -p 'test_*.py'"

    def test_1_つのジョブから両方を呼べば通る(self):
        self.assertEqual(self.errors(self.repo({"c.yml": self.wf(self.DISCOVER)})), [])
        self.assertEqual(self.errors(self.repo({"c.yml": self.wf(f"python3 {self.WATCH}", f"python3 {self.ENGINE}")})), [])
        self.assertEqual(self.errors(self.repo({"c.yml": self.wf(self.DISCOVER)}), issue=1100), [])

    def test_ひな形をそのまま写せば通る(self):
        d = self.repo({"conflict-check.yml": self.TEMPLATE.read_text(encoding="utf-8")})
        wb = self.items(("conflict-check.yml", "conflict-check", self.WATCH), ("conflict-check.yml", "conflict-check", self.ENGINE))
        self.assertEqual(self.errors(d, wb), [])

    def test_片方しか呼ばないジョブでは落ちる(self):
        for runs in ((f"python3 {self.WATCH}",), (f"python3 {self.ENGINE}",), (f"python3 {self.POLICY}",)):
            e = self.errors(self.repo({"c.yml": self.wf(*runs)}))
            self.assertEqual(len(e), 1, runs)
            self.assertIn("test_watch.py・tools/conflict-check/test_engine.py をすべて呼ぶ", e[0])
            self.assertIn("議決 D-0042", e[0])

    def test_wired_by_に片方しか書かなければ落ちる(self):
        d = self.repo({"c.yml": self.wf(self.DISCOVER)})
        for wb in (self.items(("c.yml", "c", self.WATCH)), self.items(("c.yml", "c", self.ENGINE)), self.items(("c.yml", "c", self.POLICY)), []):
            self.assertEqual(len(self.errors(d, wb)), 1, wb)

    def test_別のジョブに分けて呼ぶだけでは落ちる(self):
        d = self.repo({"a.yml": self.wf(f"python3 {self.WATCH}", job="a"), "b.yml": self.wf(f"python3 {self.ENGINE}", job="b")})
        self.assertEqual(len(self.errors(d, self.items(("a.yml", "a", self.WATCH), ("b.yml", "b", self.ENGINE)))), 1)

    def test_ひな形は配線先にならない(self):
        d = self.repo({})
        wb = [{"workflow": "tools/conflict-check/ci/conflict-check.yml", "job": "conflict-check", "runs": r} for r in (self.WATCH, self.ENGINE)]
        self.assertEqual(len(self.errors(d, wb)), 1)

    def test_対象でない解決策と配線していない解決策は見ない(self):
        d = self.repo({"c.yml": self.wf(f"python3 {self.POLICY}")})
        wb = self.items(("c.yml", "c", self.POLICY))
        self.assertEqual(self.errors(d, wb, issue=972), [])
        self.assertEqual(self.errors(d, wb, kid="K-0004"), [])
        self.assertEqual(self.errors(d, wb, wired=False), [])

    def test_main_で落とす(self):
        d = self.repo({"c.yml": self.wf(f"python3 {self.WATCH}")})
        MainTest.write(None, d, "K-0010.yaml", "id: K-0010\nenforcement:\n  level: deterministic\n  solutions:\n"
                       f"    - {{kind: ci_check, what: x, issue: 1100, wired: true, wired_by: [{{workflow: .github/workflows/c.yml, job: c, runs: {self.WATCH}}}]}}\n")
        out = d / "out.json"
        self.assertEqual(check.main(["--repo", str(d), "--json", str(out)]), 1)
        errors = json.loads(out.read_text(encoding="utf-8"))["errors"]
        self.assertEqual(len(errors), 1, errors)
        self.assertIn("issue 1100）: wired にするには", errors[0])


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

    def test_README_の記入済みと未記入の知見の一覧が記録と一致する(self):
        """README の「すべての解決策に書いた知見」「未記入の解決策が残る知見」の一覧を、check.py が K-*.yaml から数えた結果と照らす（議決 D-0035、Issue #1062）"""
        text = (HERE / "README.md").read_text(encoding="utf-8")
        ids = r"((?:`K-\d{4,}`・?)+)"
        m = re.search(rf"すべての解決策に書いたのは {ids}。{ids} の未記入の解決策は", text)
        self.assertIsNotNone(m, "README.md に「すべての解決策に書いたのは `K-…`。`K-…` の未記入の解決策は」の一覧がない")
        listed_done, listed_open = (re.findall(r"K-\d{4,}", g) for g in m.groups())
        rows = [check.row(x) for x in check.load(REPO)]
        done = [r["id"] for r in rows if r["solutions"] and not r["unrecorded"]]
        still_open = [r["id"] for r in rows if r["unrecorded"]]
        self.assertEqual(listed_done, done, "README.md の「すべての解決策に書いた」知見の一覧を K-*.yaml に合わせて直す")
        self.assertEqual(listed_open, still_open, "README.md の「未記入の解決策が残る」知見の一覧を K-*.yaml に合わせて直す")


if __name__ == "__main__":
    unittest.main()
