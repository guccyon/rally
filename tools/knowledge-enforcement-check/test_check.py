"""check.py のテスト。python3 -m unittest discover -s tools/knowledge-enforcement-check"""
from __future__ import annotations

import importlib.util
import json
import os
import re
import shutil
import subprocess
import tempfile
import unittest
from unittest import mock
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


def without_gate(errors: list[str]) -> list[str]:
    """main の出力から、どの wired: true の解決策にも出る check.py の PR ごとの配線の行（knowledge_gate_unwired）を除く
    （議決 D-0072（起票時の番号は D-0071）、Issue #1201。その線は KnowledgeGateTest が確かめる）"""
    return [x for x in errors if f": {check.KNOWLEDGE_GATE_UNWIRED}: " not in x]


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
    # issue 1018 は sendback-protection.yml のジョブしか数えない（議決 D-0060、Issue #1160。下の SendbackProtectionWorkflowTest）
    WF = ".github/workflows/sendback-protection.yml"

    def repo(self, workflow: str) -> Path:
        t = tempfile.TemporaryDirectory()
        self.addCleanup(t.cleanup)
        d = Path(t.name)
        for rel, text in {self.PROT: "", self.WF: workflow}.items():
            (d / rel).parent.mkdir(parents=True, exist_ok=True)
            (d / rel).write_text(text, encoding="utf-8")
        return d

    def wf(self, on: str, job_if: str = "", step_if: str = "") -> str:
        return (f"on:\n{on}jobs:\n  protection:\n    runs-on: ubuntu-latest\n{job_if}    steps:\n      - uses: actions/checkout@v4\n"
                f"      - {step_if}run: node {self.PROT}\n")

    def errors(self, d: Path, kid="K-0004", issue=1018, wired=True, runs=None) -> list[str]:
        wb = [{"workflow": self.WF, "job": "protection", "runs": runs or self.PROT}]
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

    def test_if_は決まった形だけを許す(self):
        """議決 D-0061（起票時の番号は D-0058）、Issue #1163。if: は無いか、github.event_name == '…' を || でつないで 'schedule' を含む形だけ"""
        for job_if, step_if in (("    if: github.event_name == 'schedule'\n", ""), ("    if: ${{ github.event_name == 'schedule' }}\n", ""),
                                ("    if: github.event_name=='workflow_dispatch'||github.event_name=='schedule'\n", ""),
                                ("", "if: github.event_name == 'schedule' || github.event_name == 'workflow_dispatch'\n        "),
                                ("    if: github.event_name == 'schedule'\n", "if: ${{ github.event_name == 'schedule' }}\n        ")):
            self.assertEqual(self.errors(self.repo(self.wf(self.SCHEDULE, job_if, step_if))), [], (job_if, step_if))

    def test_決まった形でない_if_は_schedule_を含んでも数えない(self):
        """|| や && でほかの条件をつなぐ・needs や always() を見る・真偽の値の if: は、schedule で飛ばしうるので走らない側に数える（議決 D-0061、Issue #1163）"""
        for cond in ("github.event_name == 'schedule' && needs.gate.outputs.skip != 'true'",
                     "github.event_name == 'schedule' && false",
                     "github.event_name == 'push' || (github.event_name == 'schedule' && false)",
                     "github.event_name == 'schedule' || needs.gate.result == 'success'",
                     "needs.gate.result == 'success' && github.event_name == 'schedule'",
                     "always() && github.event_name == 'schedule'",
                     "github.event_name == 'schedule' || always()",
                     "github.event_name == \"schedule\"",
                     "github.event_name != 'push' || github.event_name == 'schedule'",
                     "contains(github.event_name, 'schedule')",
                     "always()", "success()", "true", "${{ true }}",
                     "${{ github.event_name == 'schedule' }} && false"):
            for job_if, step_if in ((f"    if: {cond}\n", ""), ("", f"if: {json.dumps(cond)}\n        ")):
                e = self.errors(self.repo(self.wf(self.SCHEDULE, job_if, step_if)))
                self.assertEqual(len(e), 1, (job_if, step_if))
                self.assertIn("議決 D-0061", e[0])

    def test_許可リストの読み方(self):
        self.assertFalse(check.off_schedule(None))
        for cond in (True, False, 1, "", "${{ }}", "github.event_name == 'schedule' ||", "|| github.event_name == 'schedule'"):
            self.assertTrue(check.off_schedule(cond), cond)

    def test_ほかの検査を呼ぶ項目だけでは落ちる(self):
        d = self.repo(self.wf(self.SCHEDULE))
        (d / "tools/sendback-check/order.mjs").write_text("", encoding="utf-8")
        self.assertEqual(len(self.errors(d, runs="tools/sendback-check/order.mjs")), 1)

    def test_対象でない解決策と配線していない解決策は見ない(self):
        d = self.repo(self.wf("  workflow_dispatch:\n"))
        self.assertEqual(self.errors(d, issue=1017), [])
        self.assertEqual(self.errors(d, kid="K-0001"), [])
        self.assertEqual(self.errors(d, wired=False), [])

    def test_K_0001_の_issue_1106_は_pr_body_check_の_protection_mjs_を_schedule_のジョブから呼ぶ(self):
        """議決 D-0044、Issue #1106。push（既定のブランチ）と schedule の両方で走るワークフローから呼べば通る"""
        prot = "tools/pr-body-check/protection.mjs"
        on = "  push:\n    branches: [main]\n" + self.SCHEDULE
        d = self.repo(self.wf(on).replace(self.PROT, prot))
        (d / prot).parent.mkdir(parents=True, exist_ok=True)
        (d / prot).write_text("", encoding="utf-8")
        self.assertEqual(self.errors(d, kid="K-0001", issue=1106, runs=prot), [])
        # push だけ・sendback-check の protection.mjs を呼ぶだけでは足りない
        d2 = self.repo(self.wf("  push:\n    branches: [main]\n").replace(self.PROT, prot))
        (d2 / prot).parent.mkdir(parents=True, exist_ok=True)
        (d2 / prot).write_text("", encoding="utf-8")
        e = self.errors(d2, kid="K-0001", issue=1106, runs=prot)
        self.assertEqual(len(e), 1, e)
        self.assertIn(prot, e[0])
        e = self.errors(self.repo(self.wf(self.SCHEDULE)), kid="K-0001", issue=1106)
        self.assertEqual(len(e), 1, e)
        self.assertIn(prot, e[0])

    def test_main_で落とす(self):
        d = self.repo(self.wf("  workflow_dispatch:\n"))
        MainTest.write(None, d, "K-0004.yaml", "id: K-0004\nenforcement:\n  level: deterministic\n  solutions:\n"
                       f"    - {{kind: ci_check, what: x, issue: 1018, wired: true, wired_by: [{{workflow: {self.WF}, job: protection, runs: {self.PROT}}}]}}\n")
        out = d / "out.json"
        self.assertEqual(check.main(["--repo", str(d), "--json", str(out)]), 1)
        errors = without_gate(json.loads(out.read_text(encoding="utf-8"))["errors"])
        self.assertEqual(len(errors), 1, errors)
        self.assertIn("issue 1018）: wired にするには", errors[0])


class SendbackProtectionWorkflowTest(unittest.TestCase):
    """K-0004 の issue 1087 と 1018 の解決策は .github/workflows/sendback-protection.yml の schedule のジョブから protection.mjs を呼ばないと
    wired にできない（議決 D-0060（起票時の番号は D-0056）、Issue #1160）"""

    PROT = ScheduleTest.PROT
    TEMPLATE = REPO / "tools/sendback-check/ci/sendback-protection.yml"
    WF = ".github/workflows/sendback-protection.yml"
    OTHER = ".github/workflows/protection.yml"

    def repo(self, files: dict[str, str]) -> Path:
        t = tempfile.TemporaryDirectory()
        self.addCleanup(t.cleanup)
        d = Path(t.name)
        for rel, text in {self.PROT: "", **files}.items():
            (d / rel).parent.mkdir(parents=True, exist_ok=True)
            (d / rel).write_text(text, encoding="utf-8")
        return d

    def template(self) -> str:
        return self.TEMPLATE.read_text(encoding="utf-8")

    def errors(self, d: Path, issue: int, wf=None, job="protection", kid="K-0004") -> list[str]:
        item = {"id": kid, "_file": f"{kid}.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "ci_check", "what": "x", "issue": issue, "wired": True, "verified": False,
             "wired_by": [{"workflow": wf or self.WF, "job": job, "runs": self.PROT}]}]}}
        return check.check_schedule(item, d)

    def test_対象に挙げてある(self):
        for issue in (1087, 1018):
            self.assertEqual(check.REQUIRED_SCHEDULE[("K-0004", issue)], self.PROT)
            self.assertEqual(check.REQUIRED_SCHEDULE_WORKFLOW[("K-0004", issue)][0], self.WF)
        self.assertNotIn(("K-0004", 1158), check.REQUIRED_SCHEDULE_WORKFLOW)
        self.assertNotIn(("K-0001", 1106), check.REQUIRED_SCHEDULE_WORKFLOW)

    def test_ひな形をそのまま写した_protection_のジョブなら通る(self):
        d = self.repo({self.WF: self.template()})
        for issue in (1087, 1018):
            self.assertEqual(self.errors(d, issue), [], issue)

    def test_ほかのワークフローの_schedule_のジョブでは落ちる(self):
        d = self.repo({self.WF: self.template(), self.OTHER: self.template()})
        for issue in (1087, 1018):
            e = self.errors(d, issue, wf=self.OTHER)
            self.assertEqual(len(e), 1, (issue, e))
            self.assertIn(f"on.schedule（cron）のある {self.WF} で", e[0])
            self.assertIn("議決 D-0039・D-0060", e[0])

    def test_写しが_schedule_で走らなければ落ちる(self):
        no_sched = self.template().replace('  schedule:\n    - cron: "0 20 * * *"\n', "", 1)
        self.assertNotEqual(no_sched, self.template())
        d = self.repo({self.WF: no_sched})
        for issue in (1087, 1018):
            self.assertEqual(len(self.errors(d, issue)), 1, issue)
        # pull_request でしか走らない protection-pr のジョブも数えない
        d = self.repo({self.WF: self.template()})
        self.assertEqual(len(self.errors(d, 1087, job="protection-pr")), 1)

    def test_ほかの解決策はワークフローの名前を問わない(self):
        d = self.repo({self.OTHER: self.template()})
        self.assertEqual(self.errors(d, 1158, wf=self.OTHER), [])
        self.assertEqual(self.errors(d, 1087, wf=self.OTHER, kid="K-0001"), [])

    def test_main_で落とす(self):
        d = self.repo({self.OTHER: self.template()})
        MainTest.write(None, d, "K-0004.yaml", "id: K-0004\nenforcement:\n  level: deterministic\n  solutions:\n"
                       f"    - {{kind: ci_check, what: x, issue: 1087, wired: true, wired_by: [{{workflow: {self.OTHER}, job: protection, runs: {self.PROT}}}]}}\n")
        out = d / "out.json"
        self.assertEqual(check.main(["--repo", str(d), "--json", str(out)]), 1)
        errors = without_gate(json.loads(out.read_text(encoding="utf-8"))["errors"])
        self.assertEqual(len(errors), 1, errors)
        self.assertIn("issue 1087）: wired にするには", errors[0])
        self.assertIn(self.WF, errors[0])


class PullRequestTest(unittest.TestCase):
    """K-0010 の issue 1035・1103 の解決策は pull_request のジョブから test_events.py を呼ばないと wired にできない（議決 D-0043、Issue #1103）"""

    EVENTS = "tools/conflict-check/test_events.py"
    TEMPLATE = REPO / "tools/conflict-check/ci/conflict-check.yml"

    def repo(self, workflow: str) -> Path:
        t = tempfile.TemporaryDirectory()
        self.addCleanup(t.cleanup)
        d = Path(t.name)
        for rel, text in {self.EVENTS: "", "tools/conflict-check/test_ci.py": "", ".github/workflows/c.yml": workflow}.items():
            (d / rel).parent.mkdir(parents=True, exist_ok=True)
            (d / rel).write_text(text, encoding="utf-8")
        return d

    def wf(self, on: str, job_if: str = "", step_if: str = "", run: str = f"python3 -m unittest {EVENTS}") -> str:
        return (f"on:\n{on}jobs:\n  conflict-check:\n    runs-on: ubuntu-latest\n{job_if}    steps:\n      - uses: actions/checkout@v4\n"
                f"      - {step_if}run: {run}\n")

    def errors(self, d: Path, kid="K-0010", issue=1035, wired=True, runs=None, job="conflict-check") -> list[str]:
        wb = [{"workflow": ".github/workflows/c.yml", "job": job, "runs": runs or self.EVENTS}]
        item = {"id": kid, "_file": f"{kid}.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "test", "what": "x", "issue": issue, "wired": wired, "verified": False, "wired_by": wb}]}}
        return check.check_pull_request(item, d)

    PR = "  pull_request:\n"

    def test_pull_request_のジョブから呼べば通る(self):
        for on in (self.PR, "  pull_request:\n    branches: [main]\n  workflow_dispatch:\n", "  [pull_request, push]\n"):
            self.assertEqual(self.errors(self.repo(self.wf(on))), [], on)
        self.assertEqual(self.errors(self.repo(self.wf(self.PR, "    if: github.event_name == 'pull_request'\n"))), [])
        self.assertEqual(self.errors(self.repo(self.wf(self.PR)), issue=1103), [])

    def test_ひな形をそのまま写せば通る(self):
        """ci/conflict-check.yml の unittest discover で test_events.py を呼ぶ形を、呼んでいる側に数える"""
        d = self.repo(self.TEMPLATE.read_text(encoding="utf-8"))
        self.assertEqual(self.errors(d), [])
        self.assertEqual(check.check_wiring({"id": "K-0010", "_file": "K-0010.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "test", "issue": 1035, "wired": True, "wired_by": [{"workflow": ".github/workflows/c.yml", "job": "conflict-check", "runs": self.EVENTS}]}]}}, d), [])

    def test_定期や手動だけのワークフローでは落ちる(self):
        for on in ("  workflow_dispatch:\n", "  schedule:\n    - cron: '0 0 * * *'\n", "  push:\n"):
            e = self.errors(self.repo(self.wf(on)))
            self.assertEqual(len(e), 1, on)
            self.assertIn("on.pull_request", e[0])
            self.assertIn("議決 D-0043", e[0])

    def test_pull_request_では走らないジョブと手順は数えない(self):
        for job_if, step_if in (("    if: github.event_name == 'workflow_dispatch'\n", ""), ("", "if: github.event_name != 'pull_request'\n        "),
                                ("    if: false\n", ""), ("", "if: ${{ false }}\n        ")):
            self.assertEqual(len(self.errors(self.repo(self.wf(self.PR, job_if, step_if)))), 1, (job_if, step_if))

    def test_test_events_py_を呼ばないジョブでは落ちる(self):
        d = self.repo(self.wf(self.PR, run="python3 -m unittest tools/conflict-check/test_ci.py"))
        self.assertEqual(len(self.errors(d)), 1)
        self.assertEqual(len(self.errors(d, runs="tools/conflict-check/test_ci.py")), 1)
        self.assertEqual(len(self.errors(self.repo(self.wf(self.PR)), job="other")), 1)

    def test_対象でない解決策と配線していない解決策は見ない(self):
        d = self.repo(self.wf("  workflow_dispatch:\n"))
        self.assertEqual(self.errors(d, issue=972), [])
        self.assertEqual(self.errors(d, kid="K-0004"), [])
        self.assertEqual(self.errors(d, wired=False), [])

    def test_main_で落とす(self):
        d = self.repo(self.wf("  workflow_dispatch:\n"))
        MainTest.write(None, d, "K-0010.yaml", "id: K-0010\nenforcement:\n  level: deterministic\n  solutions:\n"
                       f"    - {{kind: test, what: x, issue: 1035, wired: true, wired_by: [{{workflow: .github/workflows/c.yml, job: conflict-check, runs: {self.EVENTS}}}]}}\n")
        out = d / "out.json"
        self.assertEqual(check.main(["--repo", str(d), "--json", str(out)]), 1)
        errors = without_gate(json.loads(out.read_text(encoding="utf-8"))["errors"])
        self.assertEqual(len(errors), 1, errors)
        self.assertIn("issue 1035）: wired にするには", errors[0])


class KnowledgeGateTest(unittest.TestCase):
    """wired: true の解決策は、知見と issue を問わずどれも、.github/workflows/knowledge-enforcement.yml の pull_request のジョブから check.py を
    呼ぶ項目を wired_by に要する（議決 D-0072（起票時の番号は D-0071）の test の案、Issue #1201。前は K-0004 の issue 986・1059・1129 だけを
    REQUIRED_PR に名前で挙げていた（議決 D-0048、Issue #1129。議決 D-0072、Issue #1200））"""

    CHECK = "tools/knowledge-enforcement-check/check.py"
    TEMPLATE = HERE / "ci/knowledge-enforcement.yml"
    WF = ".github/workflows/knowledge-enforcement.yml"
    # K-0004 の前の 3 つに加え、REQUIRED_PR に挙げていない組（K-0004 の issue 1018 は schedule だけを求める組、ほかは別の知見や架空の知見）でも同じ線で落ちること
    SOLUTIONS = (("K-0004", 986), ("K-0004", 1059), ("K-0004", 1129), ("K-0004", 1018), ("K-0001", 1106), ("K-0005", 936),
                 ("K-0010", 1034), ("K-0012", 1197), ("K-9999", 1))

    def repo(self, workflow: str) -> Path:
        t = tempfile.TemporaryDirectory()
        self.addCleanup(t.cleanup)
        d = Path(t.name)
        for rel, text in {self.CHECK: "", "tools/knowledge-enforcement-check/test_check.py": "", self.WF: workflow}.items():
            (d / rel).parent.mkdir(parents=True, exist_ok=True)
            (d / rel).write_text(text, encoding="utf-8")
        return d

    def template(self, on: str | None = None) -> str:
        text = self.TEMPLATE.read_text(encoding="utf-8")
        return text if on is None else text.replace("on:\n  pull_request:\n", f"on:\n{on}", 1)

    def item(self, kid: str, issue: int, *wired_by: dict, wired=True) -> dict:
        wb = list(wired_by) or [{"workflow": self.WF, "job": "enforcement", "runs": self.CHECK}]
        return {"id": kid, "_file": f"{kid}.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "ci_check", "what": "x", "issue": issue, "wired": wired, "verified": False, "wired_by": wb}]}}

    def errors(self, d: Path, kid="K-0004", issue=986, *wired_by: dict, wired=True) -> list[str]:
        item = self.item(kid, issue, *wired_by, wired=wired)
        return check.check_knowledge_gate(item, d) + check.check_wiring(item, d)

    def assert_gate(self, e: list[str], what) -> None:
        self.assertEqual(len(e), 1, (what, e))
        self.assertIn(f": {check.KNOWLEDGE_GATE_UNWIRED}: wired にするには", e[0])
        self.assertIn(f"on.pull_request のある {self.WF} で", e[0])
        self.assertIn(self.CHECK, e[0])
        self.assertIn("議決 D-0072", e[0])

    def test_ひな形をそのまま写した_enforcement_のジョブなら通る(self):
        d = self.repo(self.template())
        for kid, issue in self.SOLUTIONS:
            self.assertEqual(self.errors(d, kid, issue), [], (kid, issue))

    def test_知見と_issue_の列挙に頼らない(self):
        # K-0004 の 3 つも REQUIRED_PR・REQUIRED_PR_WORKFLOW から外し、check.py の組はどこにも名前で挙げない
        self.assertEqual(check.KNOWLEDGE_GATE[:2], (self.CHECK, self.WF))
        self.assertFalse([key for key, (target, _) in check.REQUIRED_PR.items() if target == self.CHECK])
        self.assertFalse([key for key, wf in check.REQUIRED_PR_WORKFLOW.items() if wf == self.WF])
        for kid, issue in self.SOLUTIONS[3:]:
            self.assertNotIn((kid, issue), {key for key, (target, _) in check.REQUIRED_PR.items() if target == self.CHECK})

    def test_check_py_を呼ぶ項目がなければ落ちる(self):
        d = self.repo(self.template())
        (d / ".github/workflows/c.yml").write_text("on: pull_request\njobs:\n  c:\n    runs-on: ubuntu-latest\n    steps:\n"
                                                  "      - run: python3 tools/conflict-check/test_events.py\n", encoding="utf-8")
        (d / "tools/conflict-check").mkdir(parents=True)
        (d / "tools/conflict-check/test_events.py").write_text("", encoding="utf-8")
        for kid, issue in self.SOLUTIONS:
            item = self.item(kid, issue, {"workflow": ".github/workflows/c.yml", "job": "c", "runs": "tools/conflict-check/test_events.py"})
            self.assertEqual(check.check_wiring(item, d), [], (kid, issue))
            self.assert_gate(check.check_knowledge_gate(item, d), (kid, issue))

    def test_ほかのワークフローのジョブでは落ちる(self):
        other = ".github/workflows/knowledge.yml"
        d = self.repo(self.template())
        (d / other).write_text(self.template(), encoding="utf-8")
        for kid, issue in self.SOLUTIONS:
            self.assert_gate(self.errors(d, kid, issue, {"workflow": other, "job": "enforcement", "runs": self.CHECK}), (kid, issue))

    def test_定期や手動だけのワークフローでは落ちる(self):
        for on in ("  workflow_dispatch:\n", "  schedule:\n    - cron: '0 0 * * *'\n", "  push:\n    branches: [main]\n"):
            d = self.repo(self.template(on))
            self.assertNotEqual(self.template(on), self.template(), on)
            for kid, issue in self.SOLUTIONS:
                self.assert_gate(self.errors(d, kid, issue), (on, kid, issue))

    def test_check_py_を呼ばないジョブや検査では落ちる(self):
        d = self.repo(self.template())
        for kid, issue in self.SOLUTIONS:
            self.assertEqual(len(check.check_knowledge_gate(self.item(kid, issue, {"workflow": self.WF, "job": "engine-test", "runs": self.CHECK}), d)), 1, (kid, issue))
            self.assert_gate(check.check_knowledge_gate(self.item(kid, issue, {"workflow": self.WF, "job": "enforcement",
                                                                                "runs": "tools/knowledge-enforcement-check/test_check.py"}), d), (kid, issue))

    def test_pull_request_では走らないジョブは数えない(self):
        text = self.template().replace("  enforcement:\n    runs-on: ubuntu-latest\n",
                                       "  enforcement:\n    if: github.event_name != 'pull_request'\n    runs-on: ubuntu-latest\n", 1)
        d = self.repo(text)
        for kid, issue in self.SOLUTIONS:
            self.assert_gate(check.check_knowledge_gate(self.item(kid, issue), d), (kid, issue))

    def test_配線していない解決策は見ない(self):
        d = self.repo(self.template("  workflow_dispatch:\n"))
        for kid, issue in self.SOLUTIONS:
            self.assertEqual(self.errors(d, kid, issue, wired=False), [], (kid, issue))
        self.assertEqual(check.check_knowledge_gate(k("deterministic", sol(wired="true"), sol()), d), [])

    def test_main_で落とす(self):
        d = self.repo(self.template("  workflow_dispatch:\n"))
        MainTest.write(None, d, "K-0004.yaml", "id: K-0004\nenforcement:\n  level: deterministic\n  solutions:\n"
                       f"    - {{kind: ci_check, what: x, issue: 1059, wired: true, verified: false, wired_by: [{{workflow: {self.WF}, job: enforcement, runs: {self.CHECK}}}]}}\n")
        MainTest.write(None, d, "K-0005.yaml", "id: K-0005\nenforcement:\n  level: deterministic\n  solutions:\n"
                       f"    - {{kind: config, what: x, issue: 936, wired: true, verified: false, wired_by: [{{workflow: {self.WF}, job: enforcement, runs: {self.CHECK}}}]}}\n")
        out = d / "out.json"
        self.assertEqual(check.main(["--repo", str(d), "--json", str(out)]), 1)
        errors = json.loads(out.read_text(encoding="utf-8"))["errors"]
        self.assertEqual(len(errors), 2, errors)
        self.assertIn(f"issue 1059）: {check.KNOWLEDGE_GATE_UNWIRED}: wired にするには", errors[0])
        self.assertIn(f"issue 936）: {check.KNOWLEDGE_GATE_UNWIRED}: wired にするには", errors[1])
        d = self.repo(self.template())
        MainTest.write(None, d, "K-0005.yaml", "id: K-0005\nenforcement:\n  level: deterministic\n  solutions:\n"
                       f"    - {{kind: config, what: x, issue: 936, wired: true, verified: false, wired_by: [{{workflow: {self.WF}, job: enforcement, runs: {self.CHECK}}}]}}\n")
        self.assertEqual(check.main(["--repo", str(d), "--json", str(out)]), 0, json.loads(out.read_text(encoding="utf-8"))["errors"])


class ReadmeListPullRequestTest(unittest.TestCase):
    """K-0004 の issue 1062 の解決策（README の一覧と K-*.yaml を照らす test_check.py のテスト）は pull_request のジョブから test_check.py を呼ばないと
    wired にできない（議決 D-0052、Issue #1139）"""

    TEST = "tools/knowledge-enforcement-check/test_check.py"
    CHECK, TEMPLATE, WF = KnowledgeGateTest.CHECK, KnowledgeGateTest.TEMPLATE, KnowledgeGateTest.WF
    repo, template = KnowledgeGateTest.repo, KnowledgeGateTest.template

    def errors(self, d: Path, issue=1062, kid="K-0004", wired=True, job="enforcement", runs=None) -> list[str]:
        item = {"id": kid, "_file": f"{kid}.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "test", "what": "x", "issue": issue, "wired": wired, "verified": False,
             "wired_by": [{"workflow": self.WF, "job": job, "runs": runs or self.TEST}]}]}}
        return check.check_pull_request(item, d) + check.check_wiring(item, d)

    def test_対象に挙げてある(self):
        self.assertEqual(check.REQUIRED_PR[("K-0004", 1062)], (self.TEST, "D-0052"))

    def test_ひな形をそのまま写した_enforcement_のジョブなら通る(self):
        self.assertEqual(self.errors(self.repo(self.template())), [])

    def test_定期や手動だけのワークフローでは落ちる(self):
        for on in ("  workflow_dispatch:\n", "  schedule:\n    - cron: '0 0 * * *'\n", "  push:\n    branches: [main]\n"):
            e = self.errors(self.repo(self.template(on)))
            self.assertEqual(len(e), 1, (on, e))
            self.assertIn("on.pull_request", e[0])
            self.assertIn(self.TEST, e[0])
            self.assertIn("議決 D-0052", e[0])

    def test_test_check_py_でなく_check_py_を指す項目では落ちる(self):
        e = check.check_pull_request({"id": "K-0004", "_file": "K-0004.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "test", "issue": 1062, "wired": True, "wired_by": [{"workflow": self.WF, "job": "enforcement", "runs": self.CHECK}]}]}},
            self.repo(self.template()))
        self.assertEqual(len(e), 1, e)
        self.assertIn(self.TEST, e[0])

    def test_test_check_py_を呼ばないジョブでは落ちる(self):
        text = self.template().replace("          python3 -B tools/knowledge-enforcement-check/test_check.py -v\n", "", 1)
        self.assertNotEqual(text, self.template())
        self.assertEqual(len(check.check_pull_request({"id": "K-0004", "_file": "K-0004.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "test", "issue": 1062, "wired": True, "wired_by": [{"workflow": self.WF, "job": "enforcement", "runs": self.TEST}]}]}},
            self.repo(text))), 1)

    def test_pull_request_では走らないジョブは数えない(self):
        text = self.template().replace("  enforcement:\n    runs-on: ubuntu-latest\n",
                                       "  enforcement:\n    if: github.event_name != 'pull_request'\n    runs-on: ubuntu-latest\n", 1)
        self.assertNotEqual(text, self.template())
        self.assertEqual(len(check.check_pull_request({"id": "K-0004", "_file": "K-0004.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "test", "issue": 1062, "wired": True, "wired_by": [{"workflow": self.WF, "job": "enforcement", "runs": self.TEST}]}]}},
            self.repo(text))), 1)

    def test_配線していない解決策は見ない(self):
        d = self.repo(self.template("  workflow_dispatch:\n"))
        self.assertEqual(check.check_pull_request({"id": "K-0004", "_file": "K-0004.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "test", "issue": 1062, "wired": False}]}}, d), [])
        self.assertEqual(check.check_pull_request({"id": "K-0010", "_file": "K-0010.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "test", "issue": 1062, "wired": True, "wired_by": [{"workflow": self.WF, "job": "enforcement", "runs": self.TEST}]}]}}, d), [])

    def test_main_で落とす(self):
        d = self.repo(self.template("  workflow_dispatch:\n"))
        MainTest.write(None, d, "K-0004.yaml", "id: K-0004\nenforcement:\n  level: deterministic\n  solutions:\n"
                       f"    - {{kind: test, what: x, issue: 1062, wired: true, wired_by: [{{workflow: {self.WF}, job: enforcement, runs: {self.TEST}}}]}}\n")
        out = d / "out.json"
        self.assertEqual(check.main(["--repo", str(d), "--json", str(out)]), 1)
        errors = without_gate(json.loads(out.read_text(encoding="utf-8"))["errors"])
        self.assertEqual(len(errors), 1, errors)
        self.assertIn("issue 1062）: wired にするには", errors[0])


class EnginePatchPullRequestTest(unittest.TestCase):
    """K-0004 の issue 1063・1144 の解決策（Engine に移す差分を ENGINE_REF の版で確かめる engine-test のジョブ）は pull_request のジョブから
    test_engine_patch.py を呼ばないと wired にできない（議決 D-0053、Issue #1144）"""

    PATCH = "tools/knowledge-enforcement-check/test_engine_patch.py"
    CHECK, TEMPLATE, WF = KnowledgeGateTest.CHECK, KnowledgeGateTest.TEMPLATE, KnowledgeGateTest.WF
    template = KnowledgeGateTest.template

    def repo(self, workflow: str) -> Path:
        d = KnowledgeGateTest.repo(self, workflow)
        (d / self.PATCH).write_text("", encoding="utf-8")
        return d

    def errors(self, d: Path, issue=1144, kid="K-0004", wired=True, job="engine-test", runs=None) -> list[str]:
        item = {"id": kid, "_file": f"{kid}.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "ci_check", "what": "x", "issue": issue, "wired": wired, "verified": False,
             "wired_by": [{"workflow": self.WF, "job": job, "runs": runs or self.PATCH}]}]}}
        return check.check_pull_request(item, d) + check.check_wiring(item, d)

    def test_対象に挙げてある(self):
        for issue in (1063, 1144):
            self.assertEqual(check.REQUIRED_PR[("K-0004", issue)], (self.PATCH, "D-0053"), issue)

    def test_ひな形をそのまま写した_engine_test_のジョブなら通る(self):
        d = self.repo(self.template())
        for issue in (1063, 1144):
            self.assertEqual(self.errors(d, issue=issue), [], issue)

    def test_定期や手動だけのワークフローでは落ちる(self):
        for on in ("  workflow_dispatch:\n", "  schedule:\n    - cron: '0 0 * * *'\n", "  push:\n    branches: [main]\n"):
            for issue in (1063, 1144):
                e = self.errors(self.repo(self.template(on)), issue=issue)
                self.assertEqual(len(e), 1, (on, issue, e))
                self.assertIn("on.pull_request", e[0])
                self.assertIn(self.PATCH, e[0])
                self.assertIn("議決 D-0053", e[0])

    def test_enforcement_のジョブや_check_py_を指す項目では落ちる(self):
        d = self.repo(self.template())
        e = check.check_pull_request({"id": "K-0004", "_file": "K-0004.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "ci_check", "issue": 1144, "wired": True, "wired_by": [{"workflow": self.WF, "job": "enforcement", "runs": self.PATCH}]}]}}, d)
        self.assertEqual(len(e), 1, e)
        e = check.check_pull_request({"id": "K-0004", "_file": "K-0004.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "ci_check", "issue": 1063, "wired": True, "wired_by": [{"workflow": self.WF, "job": "engine-test", "runs": self.CHECK}]}]}}, d)
        self.assertEqual(len(e), 1, e)
        self.assertIn(self.PATCH, e[0])

    def test_テストを走らせない_engine_test_のジョブでは落ちる(self):
        step = "          python3 -B -m unittest discover -s tools/knowledge-enforcement-check -v 2>&1 | tee knowledge-enforcement-unittest.log\n"
        text = self.template().replace(step, "", 1)
        self.assertNotEqual(text, self.template())
        self.assertEqual(len(check.check_pull_request({"id": "K-0004", "_file": "K-0004.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "ci_check", "issue": 1144, "wired": True, "wired_by": [{"workflow": self.WF, "job": "engine-test", "runs": self.PATCH}]}]}},
            self.repo(text))), 1)

    def test_pull_request_では走らないジョブは数えない(self):
        text = self.template().replace("  engine-test:\n    runs-on: ubuntu-latest\n",
                                       "  engine-test:\n    if: github.event_name != 'pull_request'\n    runs-on: ubuntu-latest\n", 1)
        self.assertNotEqual(text, self.template())
        self.assertEqual(len(check.check_pull_request({"id": "K-0004", "_file": "K-0004.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "ci_check", "issue": 1063, "wired": True, "wired_by": [{"workflow": self.WF, "job": "engine-test", "runs": self.PATCH}]}]}},
            self.repo(text))), 1)

    def test_配線していない解決策とほかの知見は見ない(self):
        d = self.repo(self.template("  workflow_dispatch:\n"))
        self.assertEqual(self.errors(d, wired=False), [])
        self.assertEqual(check.check_pull_request({"id": "K-0010", "_file": "K-0010.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "ci_check", "issue": 1144, "wired": True, "wired_by": [{"workflow": self.WF, "job": "engine-test", "runs": self.PATCH}]}]}}, d), [])

    def test_main_で落とす(self):
        d = self.repo(self.template("  workflow_dispatch:\n"))
        MainTest.write(None, d, "K-0004.yaml", "id: K-0004\nenforcement:\n  level: deterministic\n  solutions:\n"
                       f"    - {{kind: ci_check, what: x, issue: 1144, wired: true, wired_by: [{{workflow: {self.WF}, job: engine-test, runs: {self.PATCH}}}]}}\n")
        out = d / "out.json"
        self.assertEqual(check.main(["--repo", str(d), "--json", str(out)]), 1)
        errors = without_gate(json.loads(out.read_text(encoding="utf-8"))["errors"])
        self.assertEqual(len(errors), 1, errors)
        self.assertIn("issue 1144）: wired にするには", errors[0])


class SendbackProtectionScheduleAndPullRequestTest(unittest.TestCase):
    """K-0004 の issue 1158 の解決策は schedule のジョブと pull_request のジョブの両方から protection.mjs を呼ばないと wired にできない
    （議決 D-0059（起票時の番号は D-0056）、Issue #1158）"""

    PROT = ScheduleTest.PROT
    TEMPLATE = REPO / "tools/sendback-check/ci/sendback-protection.yml"
    WF = ".github/workflows/sendback-protection.yml"

    def repo(self, workflow: str) -> Path:
        t = tempfile.TemporaryDirectory()
        self.addCleanup(t.cleanup)
        d = Path(t.name)
        for rel, text in {self.PROT: "", "tools/sendback-check/order.mjs": "", self.WF: workflow}.items():
            (d / rel).parent.mkdir(parents=True, exist_ok=True)
            (d / rel).write_text(text, encoding="utf-8")
        return d

    def template(self) -> str:
        return self.TEMPLATE.read_text(encoding="utf-8")

    def errors(self, d: Path, jobs=("protection", "protection-pr"), issue=1158, kid="K-0004", wired=True, runs=None) -> list[str]:
        item = {"id": kid, "_file": f"{kid}.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "ci_check", "what": "x", "issue": issue, "wired": wired, "verified": False,
             "wired_by": [{"workflow": self.WF, "job": job, "runs": runs or self.PROT} for job in jobs]}]}}
        return check.check_schedule(item, d) + check.check_pull_request(item, d) + check.check_wiring(item, d)

    def test_対象に挙げてある(self):
        self.assertEqual(check.REQUIRED_SCHEDULE[("K-0004", 1158)], self.PROT)
        self.assertEqual(check.REQUIRED_PR[("K-0004", 1158)], (self.PROT, "D-0059"))

    def test_ひな形をそのまま写した_protection_と_protection_pr_なら通る(self):
        self.assertEqual(self.errors(self.repo(self.template())), [])

    def test_片方のジョブだけでは落ちる(self):
        d = self.repo(self.template())
        e = self.errors(d, jobs=("protection",))
        self.assertEqual(len(e), 1, e)
        self.assertIn("on.pull_request", e[0])
        self.assertIn("議決 D-0059", e[0])
        e = self.errors(d, jobs=("protection-pr",))
        self.assertEqual(len(e), 1, e)
        self.assertIn("on.schedule（cron）", e[0])

    def test_pull_request_か_schedule_を外した写しでは落ちる(self):
        no_pr = self.template().replace("  workflow_dispatch:\n  pull_request:\n", "  workflow_dispatch:\n", 1)
        self.assertNotEqual(no_pr, self.template())
        e = self.errors(self.repo(no_pr))
        self.assertEqual(len(e), 1, e)
        self.assertIn("on.pull_request", e[0])
        no_sched = self.template().replace('  schedule:\n    - cron: "0 20 * * *"\n', "", 1)
        self.assertNotEqual(no_sched, self.template())
        e = self.errors(self.repo(no_sched))
        self.assertEqual(len(e), 1, e)
        self.assertIn("on.schedule（cron）", e[0])

    def test_pull_request_では走らない_protection_pr_は数えない(self):
        text = self.template().replace("    if: github.event_name == 'pull_request'\n", "    if: github.event_name != 'pull_request'\n", 1)
        self.assertNotEqual(text, self.template())
        self.assertEqual(len(self.errors(self.repo(text))), 1)

    def test_protection_mjs_を呼ばない項目では落ちる(self):
        e = self.errors(self.repo(self.template()), runs="tools/sendback-check/order.mjs")
        self.assertEqual(len([x for x in e if "wired にするには" in x]), 2, e)

    def test_配線していない解決策とほかの解決策は見ない(self):
        d = self.repo("on:\n  workflow_dispatch:\njobs: {}\n")
        self.assertEqual(self.errors(d, wired=False), [])
        self.assertEqual(check.check_pull_request({"id": "K-0004", "_file": "K-0004.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "ci_check", "issue": 1018, "wired": True, "wired_by": [{"workflow": self.WF, "job": "protection", "runs": self.PROT}]}]}}, d), [])



class SendbackHeartbeatScheduleTest(unittest.TestCase):
    """K-0004 の issue 1162 の解決策は schedule でも走るジョブから heartbeat.mjs を呼ばないと wired にできない
    （議決 D-0061（起票時の番号は D-0058）、Issue #1162）"""

    HB = "tools/sendback-check/heartbeat.mjs"
    TEMPLATE = REPO / "tools/sendback-check/ci/sendback-protection-heartbeat.yml"
    WF = ".github/workflows/sendback-protection-heartbeat.yml"

    def repo(self, workflow: str) -> Path:
        t = tempfile.TemporaryDirectory()
        self.addCleanup(t.cleanup)
        d = Path(t.name)
        for rel, text in {self.HB: "", self.WF: workflow}.items():
            (d / rel).parent.mkdir(parents=True, exist_ok=True)
            (d / rel).write_text(text, encoding="utf-8")
        return d

    def template(self) -> str:
        return self.TEMPLATE.read_text(encoding="utf-8")

    def errors(self, d: Path, runs=None, wired=True) -> list[str]:
        item = {"id": "K-0004", "_file": "K-0004.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "ci_check", "what": "x", "issue": 1162, "wired": wired, "verified": False,
             "wired_by": [{"workflow": self.WF, "job": "heartbeat", "runs": runs or self.HB}]}]}}
        return check.check_schedule(item, d) + check.check_wiring(item, d)

    def test_対象に挙げてある(self):
        self.assertEqual(check.REQUIRED_SCHEDULE[("K-0004", 1162)], self.HB)
        self.assertNotIn(("K-0004", 1162), check.REQUIRED_PR)

    def test_ひな形をそのまま写した_heartbeat_なら通る(self):
        self.assertEqual(self.errors(self.repo(self.template())), [])

    def test_schedule_を外した写しでは落ちる(self):
        text = self.template().replace('  schedule:\n    - cron: "0 8 * * *"\n', "", 1)
        self.assertNotEqual(text, self.template())
        e = self.errors(self.repo(text))
        self.assertEqual(len(e), 1, e)
        self.assertIn("on.schedule（cron）", e[0])

    def test_heartbeat_mjs_を呼ばない項目では落ちる(self):
        e = self.errors(self.repo(self.template()), runs=ScheduleTest.PROT)
        self.assertEqual(len([x for x in e if "wired にするには" in x]), 1, e)

    def test_配線していなければ見ない(self):
        self.assertEqual(self.errors(self.repo("on:\n  workflow_dispatch:\njobs: {}\n"), wired=False), [])

class ConflictSchemaScheduleTest(unittest.TestCase):
    """K-0014 の issue 1136・1207 の解決策は .github/workflows/conflict-schema.yml の schedule でも走るジョブから schema.py を呼ばないと
    wired にできない（議決 D-0101（起票時の番号は D-0073）、Issue #1207）"""

    SCHEMA = "tools/conflict-check/schema.py"
    TEMPLATE = REPO / "tools/conflict-check/ci/conflict-schema.yml"
    WF = ".github/workflows/conflict-schema.yml"

    def repo(self, files: dict[str, str]) -> Path:
        t = tempfile.TemporaryDirectory()
        self.addCleanup(t.cleanup)
        d = Path(t.name)
        for rel, text in {self.SCHEMA: "", **files}.items():
            (d / rel).parent.mkdir(parents=True, exist_ok=True)
            (d / rel).write_text(text, encoding="utf-8")
        return d

    def template(self) -> str:
        return self.TEMPLATE.read_text(encoding="utf-8")

    def errors(self, d: Path, issue=1207, wf=None, runs=None, wired=True) -> list[str]:
        item = {"id": "K-0014", "_file": "K-0014.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "ci_check", "what": "x", "issue": issue, "wired": wired, "verified": False,
             "wired_by": [{"workflow": wf or self.WF, "job": "conflict-schema", "runs": runs or self.SCHEMA}]}]}}
        return check.check_schedule(item, d) + check.check_wiring(item, d)

    def test_対象に挙げてある(self):
        for issue in (1136, 1207):
            self.assertEqual(check.REQUIRED_SCHEDULE[("K-0014", issue)], self.SCHEMA)
            self.assertEqual(check.REQUIRED_SCHEDULE_WORKFLOW[("K-0014", issue)], (self.WF, "D-0101"))
            self.assertNotIn(("K-0014", issue), check.REQUIRED_PR)

    def test_ひな形をそのまま写した_conflict_schema_なら通る(self):
        for issue in (1136, 1207):
            self.assertEqual(self.errors(self.repo({self.WF: self.template()}), issue), [])

    def test_別の名前のワークフローでは落ちる(self):
        other = ".github/workflows/schema.yml"
        e = self.errors(self.repo({other: self.template()}), wf=other)
        self.assertEqual(len(e), 1, e)
        self.assertIn(self.WF, e[0])

    def test_schedule_を外した写しでは落ちる(self):
        text = self.template().replace('  schedule:\n    - cron: "40 20 * * *"\n', "", 1)
        self.assertNotEqual(text, self.template())
        e = self.errors(self.repo({self.WF: text}))
        self.assertEqual(len(e), 1, e)
        self.assertIn("on.schedule（cron）", e[0])

    def test_schema_py_を呼ばない写しでは落ちる(self):
        text = self.template().replace("python3 -B tools/conflict-check/schema.py", "true", 1)
        self.assertNotEqual(text, self.template())
        e = self.errors(self.repo({self.WF: text}))
        self.assertTrue(any("wired にするには" in x for x in e), e)

    def test_ほかの検査を指す項目では落ちる(self):
        e = self.errors(self.repo({self.WF: self.template(), "tools/conflict-check/events.py": ""}), runs="tools/conflict-check/events.py")
        self.assertTrue(any("wired にするには" in x for x in e), e)

    def test_配線していなければ見ない(self):
        self.assertEqual(self.errors(self.repo({}), wired=False), [])


class ConflictRecordScheduleTest(unittest.TestCase):
    """K-0014 の issue 1214 の解決策は .github/workflows/conflict-schema.yml の schedule でも走るジョブから events.py を呼ばないと
    wired にできない（議決 D-0076（起票時の番号は D-0074）、Issue #1214）"""

    EVENTS = "tools/conflict-check/events.py"
    TEMPLATE = REPO / "tools/conflict-check/ci/conflict-schema.yml"
    WF = ".github/workflows/conflict-schema.yml"

    def repo(self, files: dict[str, str]) -> Path:
        t = tempfile.TemporaryDirectory()
        self.addCleanup(t.cleanup)
        d = Path(t.name)
        for rel, text in {self.EVENTS: "", "tools/conflict-check/schema.py": "", **files}.items():
            (d / rel).parent.mkdir(parents=True, exist_ok=True)
            (d / rel).write_text(text, encoding="utf-8")
        return d

    def template(self) -> str:
        return self.TEMPLATE.read_text(encoding="utf-8")

    def errors(self, d: Path, wf=None, job="conflict-record", runs=None, wired=True) -> list[str]:
        item = {"id": "K-0014", "_file": "K-0014.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "ci_check", "what": "x", "issue": 1214, "wired": wired, "verified": False,
             "wired_by": [{"workflow": wf or self.WF, "job": job, "runs": runs or self.EVENTS}]}]}}
        return check.check_schedule(item, d) + check.check_wiring(item, d)

    def test_対象に挙げてある(self):
        self.assertEqual(check.REQUIRED_SCHEDULE[("K-0014", 1214)], self.EVENTS)
        self.assertEqual(check.REQUIRED_SCHEDULE_WORKFLOW[("K-0014", 1214)], (self.WF, "D-0076"))
        self.assertNotIn(("K-0014", 1214), check.REQUIRED_PR)

    def test_ひな形をそのまま写した_conflict_record_なら通る(self):
        self.assertEqual(self.errors(self.repo({self.WF: self.template()})), [])

    def test_別の名前のワークフローでは落ちる(self):
        other = ".github/workflows/record.yml"
        e = self.errors(self.repo({other: self.template()}), wf=other)
        self.assertEqual(len(e), 1, e)
        self.assertIn(self.WF, e[0])

    def test_schedule_を外した写しでは落ちる(self):
        text = self.template().replace('  schedule:\n    - cron: "40 20 * * *"\n', "", 1)
        self.assertNotEqual(text, self.template())
        e = self.errors(self.repo({self.WF: text}))
        self.assertEqual(len(e), 1, e)
        self.assertIn("on.schedule（cron）", e[0])

    def test_events_py_を呼ばない写しや_conflict_schema_のジョブでは落ちる(self):
        text = self.template().replace("python3 -B tools/conflict-check/events.py", "true", 1)
        self.assertNotEqual(text, self.template())
        e = self.errors(self.repo({self.WF: text}))
        self.assertTrue(any("wired にするには" in x for x in e), e)
        e = self.errors(self.repo({self.WF: self.template()}), job="conflict-schema")   # schema.py だけを呼ぶジョブ
        self.assertTrue(any("wired にするには" in x for x in e), e)

    def test_配線していなければ見ない(self):
        self.assertEqual(self.errors(self.repo({}), wired=False), [])


class PrBodyEngineWiringTest(unittest.TestCase):
    """K-0001 の issue 1182 の解決策は、.github/workflows/pr-body-engine.yml の pull_request のジョブから engine.test.mjs を呼び、
    .github/workflows/pr-body-protection.yml の schedule のジョブから protection.mjs を呼ばないと wired にできない（議決 D-0070（起票時の番号は D-0067）、Issue #1182）"""

    ENGINE_TEST = "tools/pr-body-check/engine.test.mjs"
    PROT = "tools/pr-body-check/protection.mjs"
    ENGINE_TEMPLATE = REPO / "tools/pr-body-check/ci/pr-body-engine.yml"
    PROT_TEMPLATE = REPO / "tools/pr-body-check/ci/pr-body-protection.yml"
    ENGINE_WF = ".github/workflows/pr-body-engine.yml"
    PROT_WF = ".github/workflows/pr-body-protection.yml"
    OTHER = ".github/workflows/pr-body.yml"
    WIRED_BY = ((ENGINE_WF, "engine-test", ENGINE_TEST), (PROT_WF, "protection", PROT))

    def repo(self, files: dict[str, str] | None = None) -> Path:
        t = tempfile.TemporaryDirectory()
        self.addCleanup(t.cleanup)
        d = Path(t.name)
        base = {self.ENGINE_TEST: "", self.PROT: "", self.ENGINE_WF: self.engine(), self.PROT_WF: self.protection()}
        for rel, text in {**base, **(files or {})}.items():
            (d / rel).parent.mkdir(parents=True, exist_ok=True)
            (d / rel).write_text(text, encoding="utf-8")
        return d

    def engine(self) -> str:
        return self.ENGINE_TEMPLATE.read_text(encoding="utf-8")

    def protection(self) -> str:
        return self.PROT_TEMPLATE.read_text(encoding="utf-8")

    def errors(self, d: Path, wired_by=WIRED_BY, issue=1182, wired=True) -> list[str]:
        item = {"id": "K-0001", "_file": "K-0001.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "ci_check", "what": "x", "issue": issue, "wired": wired, "verified": False,
             "wired_by": [{"workflow": wf, "job": job, "runs": runs} for wf, job, runs in wired_by]}]}}
        return check.check_schedule(item, d) + check.check_pull_request(item, d) + check.check_wiring(item, d)

    def test_対象に挙げてある(self):
        self.assertEqual(check.REQUIRED_SCHEDULE[("K-0001", 1182)], self.PROT)
        self.assertEqual(check.REQUIRED_SCHEDULE_WORKFLOW[("K-0001", 1182)], (self.PROT_WF, "D-0070"))
        self.assertEqual(check.REQUIRED_PR[("K-0001", 1182)], (self.ENGINE_TEST, "D-0070"))
        self.assertEqual(check.REQUIRED_PR_WORKFLOW[("K-0001", 1182)], self.ENGINE_WF)

    def test_2_つのひな形をそのまま写したジョブなら通る(self):
        self.assertEqual(self.errors(self.repo()), [])

    def test_片方の項目だけでは落ちる(self):
        d = self.repo()
        e = self.errors(d, wired_by=self.WIRED_BY[:1])
        self.assertEqual(len(e), 1, e)
        self.assertIn(f"on.schedule（cron）のある {self.PROT_WF} で", e[0])
        self.assertIn("議決 D-0070", e[0])
        e = self.errors(d, wired_by=self.WIRED_BY[1:])
        self.assertEqual(len(e), 1, e)
        self.assertIn(f"on.pull_request のある {self.ENGINE_WF} で", e[0])
        self.assertIn("議決 D-0070", e[0])

    def test_ほかのワークフローのジョブでは落ちる(self):
        d = self.repo({self.OTHER: self.engine()})
        e = self.errors(d, wired_by=((self.OTHER, "engine-test", self.ENGINE_TEST), self.WIRED_BY[1]))
        self.assertEqual(len(e), 1, e)
        self.assertIn(self.ENGINE_WF, e[0])
        d = self.repo({self.OTHER: self.protection()})
        e = self.errors(d, wired_by=(self.WIRED_BY[0], (self.OTHER, "protection", self.PROT)))
        self.assertEqual(len(e), 1, e)
        self.assertIn(self.PROT_WF, e[0])

    def test_pull_request_か_schedule_を外した写しでは落ちる(self):
        no_pr = self.engine().replace("on:\n  pull_request:\n", "on:\n  workflow_dispatch:\n", 1)
        self.assertNotEqual(no_pr, self.engine())
        e = self.errors(self.repo({self.ENGINE_WF: no_pr}))
        self.assertEqual(len(e), 1, e)
        self.assertIn("on.pull_request", e[0])
        no_sched = self.protection().replace('  schedule:\n    - cron: "30 20 * * *"\n', "", 1)
        self.assertNotEqual(no_sched, self.protection())
        e = self.errors(self.repo({self.PROT_WF: no_sched}))
        self.assertEqual(len(e), 1, e)
        self.assertIn("on.schedule（cron）", e[0])

    def test_テストを走らせない_engine_test_では落ちる(self):
        text = self.engine().replace("node --test --test-reporter=spec", "node --test --test-reporter=spec check.test.mjs", 1)
        self.assertNotEqual(text, self.engine())
        e = self.errors(self.repo({self.ENGINE_WF: text}))
        self.assertEqual(len([x for x in e if "wired にするには" in x]), 1, e)
        self.assertIn("engine.test.mjs", e[0])

    def test_配線していない解決策とほかの解決策は見ない(self):
        d = self.repo({self.ENGINE_WF: "on:\n  workflow_dispatch:\njobs: {}\n", self.PROT_WF: "on:\n  workflow_dispatch:\njobs: {}\n"})
        self.assertEqual(self.errors(d, wired=False), [])
        # issue 1106 の解決策は REQUIRED_PR に挙げておらず、ワークフローの名前も問わない
        d = self.repo({self.OTHER: self.protection()})
        self.assertEqual(self.errors(d, wired_by=((self.OTHER, "protection", self.PROT),), issue=1106), [])

    def test_main_で落とす(self):
        d = self.repo()
        MainTest.write(None, d, "K-0001.yaml", "id: K-0001\nenforcement:\n  level: deterministic\n  solutions:\n"
                       f"    - {{kind: ci_check, what: x, issue: 1182, wired: true, wired_by: [{{workflow: {self.PROT_WF}, job: protection, runs: {self.PROT}}}]}}\n")
        out = d / "out.json"
        self.assertEqual(check.main(["--repo", str(d), "--json", str(out)]), 1)
        errors = without_gate(json.loads(out.read_text(encoding="utf-8"))["errors"])
        self.assertEqual(len(errors), 1, errors)
        self.assertIn("issue 1182）: wired にするには", errors[0])
        self.assertIn(self.ENGINE_WF, errors[0])


class PrBodyRecheckWiringTest(unittest.TestCase):
    """K-0012 の issue 1122・1196 と K-0006 の issue 906 の解決策は、.github/workflows/pr-body-check.yml の pull_request のジョブから check.mjs を
    呼ばないと wired にできない。K-0012 の 2 つは、.github/workflows/pr-body-engine.yml の pull_request のジョブから engine.test.mjs を呼び、
    その ENGINE_REF の Engine の merge_step が body_recheck_pending を呼んでいることも求める（議決 D-0100（起票時の番号は D-0071）、Issue #1196）。
    Engine は一時の git リポジトリに supervisor.py だけを置いて作る"""

    CHECK = "tools/pr-body-check/check.mjs"
    ENGINE_TEST = "tools/pr-body-check/engine.test.mjs"
    CHECK_TEMPLATE = REPO / "tools/pr-body-check/ci/pr-body-check.yml"
    ENGINE_TEMPLATE = REPO / "tools/pr-body-check/ci/pr-body-engine.yml"
    RECHECK_PATCH = REPO / "tools/pr-body-check/engine/pr-body-recheck.patch"
    CHECK_WF = ".github/workflows/pr-body-check.yml"
    ENGINE_WF = ".github/workflows/pr-body-engine.yml"
    OTHER = ".github/workflows/pr-body.yml"
    WIRED_BY = ((CHECK_WF, "pr-body-check", CHECK), (ENGINE_WF, "engine-test", ENGINE_TEST))
    WAITS = ("def merge_step(cur, rules, manifests):\n    for p in prs:\n        if body_fix_pending(cur, p):\n            continue\n"
             "        if body_recheck_pending(cur, p, sha):\n            continue\n        merge(p)\n\n\n"
             "def body_fix_pending(cur, pr_url):\n    return False\n\n\ndef body_recheck_pending(cur, pr_url, sha):\n    return None\n")
    LEGACY = ("def merge_step(cur, rules, manifests):\n    for p in prs:\n        if body_fix_pending(cur, p):\n            continue\n        merge(p)\n\n\n"
              "def body_fix_pending(cur, pr_url):\n    return False\n")

    @staticmethod
    def git(cwd: Path, *args: str) -> str:
        return subprocess.run(["git", "-c", "user.email=t@example.invalid", "-c", "user.name=t", *args], cwd=cwd,
                              check=True, capture_output=True, text=True).stdout.strip()

    def tmp(self) -> Path:
        t = tempfile.TemporaryDirectory()
        self.addCleanup(t.cleanup)
        return Path(t.name)

    def engine(self, text: str) -> tuple[Path, str]:
        """supervisor.py を text にした版の Engine と、その SHA"""
        d = self.tmp()
        self.git(d, "init", "-q")
        f = d / check.ENGINE_SUPERVISOR
        f.parent.mkdir(parents=True)
        f.write_text(text, encoding="utf-8")
        self.git(d, "add", "-A")
        self.git(d, "commit", "-q", "-m", "v")
        return d, self.git(d, "rev-parse", "HEAD")

    def repo(self, ref: str = "0" * 40, files: dict[str, str | None] | None = None) -> Path:
        """2 つのひな形をそのまま写したリポジトリ（pr-body-engine.yml の写しの ENGINE_REF は ref）。files の値が None ならそのファイルを置かない"""
        d = self.tmp()
        engine_wf = check.ENGINE_REF_RE.sub(f"  ENGINE_REF: {ref}", self.ENGINE_TEMPLATE.read_text(encoding="utf-8"))
        base = {self.CHECK: "", self.ENGINE_TEST: "", self.CHECK_WF: self.CHECK_TEMPLATE.read_text(encoding="utf-8"), self.ENGINE_WF: engine_wf}
        for rel, text in {**base, **(files or {})}.items():
            if text is None:
                continue
            (d / rel).parent.mkdir(parents=True, exist_ok=True)
            (d / rel).write_text(text, encoding="utf-8")
        return d

    def item(self, kid="K-0012", issue=1196, wired_by=WIRED_BY, wired=True) -> dict:
        return {"id": kid, "_file": f"{kid}.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "ci_check", "what": "x", "issue": issue, "wired": wired, "verified": False,
             "wired_by": [{"workflow": wf, "job": job, "runs": runs} for wf, job, runs in wired_by]}]}}

    def errors(self, d: Path, engine: Path | None, **kw) -> list[str]:
        item = self.item(**kw)
        return check.check_pull_request(item, d) + check.check_wiring(item, d) + check.check_engine_wait(item, d, engine)

    def test_対象に挙げてある(self):
        for key in (("K-0012", 1122), ("K-0012", 1196), ("K-0006", 906)):
            self.assertEqual(check.REQUIRED_PR[key], (self.CHECK, "D-0100"), key)
            self.assertEqual(check.REQUIRED_PR_WORKFLOW[key], self.CHECK_WF, key)
        for key in (("K-0012", 1122), ("K-0012", 1196)):
            self.assertEqual(check.REQUIRED_PR_ALSO[key], ((self.ENGINE_TEST, self.ENGINE_WF, "D-0100"),), key)
        self.assertEqual(check.REQUIRED_ENGINE_WAIT, {("K-0012", 1122), ("K-0012", 1196)})
        self.assertNotIn(("K-0006", 906), check.REQUIRED_PR_ALSO)

    def test_ひな形の名前は_Engine_の差分が待つものと合う(self):
        patch = self.RECHECK_PATCH.read_text(encoding="utf-8")
        self.assertIn(f'+BODY_CHECK_WORKFLOW = "{self.CHECK_WF}"', patch)
        self.assertIn(f"+def {check.ENGINE_RECHECK_WAIT}(", patch)
        self.assertIn(f"+        if {check.ENGINE_RECHECK_WAIT}(", patch)

    def test_2_つのひな形の写しと待つ_Engine_なら通る(self):
        eng, sha = self.engine(self.WAITS)
        d = self.repo(sha)
        for issue in (1122, 1196):
            self.assertEqual(self.errors(d, eng, issue=issue), [], issue)
        # K-0006 の issue 906 は pr-body-check の写しだけで足り、Engine を見ない
        self.assertEqual(self.errors(d, None, kid="K-0006", issue=906, wired_by=self.WIRED_BY[:1]), [])

    def test_pr_body_check_の項目がないかほかのワークフローでは落ちる(self):
        eng, sha = self.engine(self.WAITS)
        for kid, issue in (("K-0012", 1196), ("K-0006", 906)):
            e = self.errors(self.repo(sha), eng, kid=kid, issue=issue, wired_by=self.WIRED_BY[1:])
            self.assertEqual(len(e), 1, e)
            self.assertIn(f"on.pull_request のある {self.CHECK_WF} で", e[0])
            self.assertIn(self.CHECK, e[0])
            self.assertIn("議決 D-0100", e[0])
        d = self.repo(sha, {self.OTHER: self.CHECK_TEMPLATE.read_text(encoding="utf-8")})
        e = self.errors(d, eng, wired_by=((self.OTHER, "pr-body-check", self.CHECK), self.WIRED_BY[1]))
        self.assertEqual(len(e), 1, e)
        self.assertIn(self.CHECK_WF, e[0])

    def test_pull_request_を外すか_check_mjs_を呼ばない写しでは落ちる(self):
        eng, sha = self.engine(self.WAITS)
        text = self.CHECK_TEMPLATE.read_text(encoding="utf-8")
        no_pr = text.replace("on:\n  pull_request:\n    types: [opened, synchronize, reopened, edited]\n", "on:\n  workflow_dispatch:\n", 1)
        self.assertNotEqual(no_pr, text)
        e = self.errors(self.repo(sha, {self.CHECK_WF: no_pr}), eng)
        self.assertEqual(len(e), 1, e)
        self.assertIn("on.pull_request", e[0])
        no_call = text.replace("node tools/pr-body-check/check.mjs", "node tools/pr-body-check/followers.mjs", 1)
        self.assertNotEqual(no_call, text)
        e = self.errors(self.repo(sha, {self.CHECK_WF: no_call}), eng)
        self.assertEqual(len([x for x in e if "wired にするには" in x]), 1, e)

    def test_K_0012_は_engine_test_の項目がないと落ちる(self):
        eng, sha = self.engine(self.WAITS)
        e = self.errors(self.repo(sha), eng, wired_by=self.WIRED_BY[:1])
        self.assertEqual(len(e), 1, e)
        self.assertIn(f"on.pull_request のある {self.ENGINE_WF} で", e[0])
        self.assertIn(self.ENGINE_TEST, e[0])

    def test_merge_step_が待たない_Engine_では落ちる(self):
        for text in (self.LEGACY,
                     # 関数はあるが merge_step から呼ばない
                     self.LEGACY + "\n\ndef body_recheck_pending(cur, pr_url, sha):\n    return None\n",
                     # 呼ぶが関数を定義していない
                     self.WAITS.split("\n\n\ndef body_fix_pending")[0] + "\n",
                     # 別の関数の中でだけ呼ぶ
                     self.LEGACY + "\n\ndef other(cur):\n    body_recheck_pending(cur, 1, 2)\n\n\ndef body_recheck_pending(cur, pr_url, sha):\n    return None\n"):
            eng, sha = self.engine(text)
            e = self.errors(self.repo(sha), eng)
            self.assertEqual(len(e), 1, e)
            for part in (f"issue 1196）: {check.ENGINE_UNWIRED}:", sha, self.ENGINE_WF, "pr-body-recheck.patch", "議決 D-0100"):
                self.assertIn(part, e[0])

    def test_照らせなければ落とす(self):
        eng, sha = self.engine(self.WAITS)
        cases = {"engine_missing": (self.repo(sha), None),
                 "engine_ref_invalid": (self.repo(sha, {self.ENGINE_WF: None}), eng),
                 "engine_ref_missing": (self.repo("1" * 40), eng)}
        for kind, (d, engine) in cases.items():
            e = [x for x in self.errors(d, engine) if check.ENGINE_UNVERIFIABLE in x]
            self.assertEqual(len(e), 1, (kind, e))
            self.assertIn(f"{check.ENGINE_UNVERIFIABLE}（{kind}）", e[0])
        broken, bsha = self.engine("def merge_step(:\n")
        e = self.errors(self.repo(bsha), broken)
        self.assertEqual(len(e), 1, e)
        self.assertIn(f"{check.ENGINE_UNVERIFIABLE}（supervisor_unreadable）", e[0])

    def test_配線していない解決策とほかの解決策は_Engine_を見ない(self):
        d = self.repo(files={self.CHECK_WF: None, self.ENGINE_WF: None})
        self.assertEqual(self.errors(d, None, wired=False), [])
        self.assertEqual(check.check_engine_wait(self.item(kid="K-0001", issue=1182), d, None), [])

    @unittest.skipUnless(os.environ.get("ENGINE_DIR"), "ENGINE_DIR（Engine のチェックアウト）がない")
    def test_本物の_Engine_は差分を当てると待ち_当てる前は待たない(self):
        src = Path(os.environ["ENGINE_DIR"]) / check.ENGINE_SUPERVISOR
        events = Path(os.environ["ENGINE_DIR"]) / "definitions/events.yaml"
        d = self.tmp()
        self.git(d, "init", "-q")
        for f, rel in ((src, check.ENGINE_SUPERVISOR), (events, "definitions/events.yaml")):
            (d / rel).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy(f, d / rel)
        self.git(d, "add", "-A")
        self.git(d, "commit", "-q", "-m", "before")
        before = self.git(d, "rev-parse", "HEAD")
        if f"def {check.ENGINE_RECHECK_WAIT}(" not in src.read_text(encoding="utf-8"):
            self.assertFalse(check.engine_merge_waits(d, before))
            self.git(d, "apply", str(self.RECHECK_PATCH))
            self.git(d, "commit", "-q", "-am", "after")
        self.assertTrue(check.engine_merge_waits(d, self.git(d, "rev-parse", "HEAD")))

    def test_main_で落とす(self):
        eng, sha = self.engine(self.LEGACY)
        d = self.repo(sha)
        MainTest.write(None, d, "K-0012.yaml", "id: K-0012\nenforcement:\n  level: deterministic\n  solutions:\n"
                       f"    - {{kind: ci_check, what: x, issue: 1196, wired: true, verified: false, wired_by: ["
                       f"{{workflow: {self.CHECK_WF}, job: pr-body-check, runs: {self.CHECK}}}, "
                       f"{{workflow: {self.ENGINE_WF}, job: engine-test, runs: {self.ENGINE_TEST}}}]}}\n")
        out = d / "out.json"
        self.assertEqual(check.main(["--repo", str(d), "--json", str(out), "--engine", str(eng)]), 1)
        errors = without_gate(json.loads(out.read_text(encoding="utf-8"))["errors"])
        self.assertEqual(len(errors), 1, errors)
        self.assertIn(f"issue 1196）: {check.ENGINE_UNWIRED}", errors[0])


class PrBodyEngineTestsWiringTest(unittest.TestCase):
    """K-0012 の issue 1197（test）の解決策は、.github/workflows/pr-body-engine.yml の pull_request のジョブから recheck.test.mjs・engine.test.mjs・
    ci.test.mjs をそれぞれ呼ぶ項目がないと wired にできない（議決 D-0100（起票時の番号は D-0071）、Issue #1197）"""

    TESTS = ("tools/pr-body-check/recheck.test.mjs", "tools/pr-body-check/engine.test.mjs", "tools/pr-body-check/ci.test.mjs")
    TEMPLATE = REPO / "tools/pr-body-check/ci/pr-body-engine.yml"
    WF = ".github/workflows/pr-body-engine.yml"
    OTHER = ".github/workflows/pr-body.yml"

    def repo(self, text: str | None = None, files: dict[str, str] | None = None) -> Path:
        """ひな形 ci/pr-body-engine.yml を text（既定はそのまま）で写したリポジトリ"""
        t = tempfile.TemporaryDirectory()
        self.addCleanup(t.cleanup)
        d = Path(t.name)
        base = {**{rel: "" for rel in self.TESTS}, self.WF: self.TEMPLATE.read_text(encoding="utf-8") if text is None else text}
        for rel, body in {**base, **(files or {})}.items():
            (d / rel).parent.mkdir(parents=True, exist_ok=True)
            (d / rel).write_text(body, encoding="utf-8")
        return d

    def item(self, runs=TESTS, workflow=WF, issue=1197, wired=True) -> dict:
        return {"id": "K-0012", "_file": "K-0012.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "test", "what": "x", "issue": issue, "wired": wired, "verified": False,
             "wired_by": [{"workflow": workflow, "job": "engine-test", "runs": r} for r in runs]}]}}

    def errors(self, d: Path, **kw) -> list[str]:
        item = self.item(**kw)
        return check.check_pull_request(item, d) + check.check_wiring(item, d) + check.check_engine_wait(item, d, None)

    def test_対象に挙げてある(self):
        key = ("K-0012", 1197)
        self.assertEqual(check.REQUIRED_PR[key], (self.TESTS[0], "D-0100"))
        self.assertEqual(check.REQUIRED_PR_WORKFLOW[key], self.WF)
        self.assertEqual(check.REQUIRED_PR_ALSO[key], tuple((t, self.WF, "D-0100") for t in self.TESTS[1:]))
        self.assertNotIn(key, check.REQUIRED_ENGINE_WAIT)

    def test_ひな形をそのまま写したジョブで_3_つとも呼べば通る(self):
        self.assertEqual(self.errors(self.repo()), [])

    def test_3_つのどれかの項目がなければ落ちる(self):
        d = self.repo()
        for missing in self.TESTS:
            e = self.errors(d, runs=tuple(t for t in self.TESTS if t != missing))
            self.assertEqual(len(e), 1, (missing, e))
            for part in (f"on.pull_request のある {self.WF} で", missing, "議決 D-0100", "issue 1197"):
                self.assertIn(part, e[0])

    def test_別のワークフローの写しでは落ちる(self):
        d = self.repo(files={self.OTHER: self.TEMPLATE.read_text(encoding="utf-8")})
        e = self.errors(d, workflow=self.OTHER)
        self.assertEqual(len(e), 3, e)
        self.assertTrue(all(self.WF in x for x in e), e)

    def test_pull_request_を外すか道を渡して絞った写しでは落ちる(self):
        text = self.TEMPLATE.read_text(encoding="utf-8")
        no_pr = text.replace("on:\n  pull_request:\n", "on:\n  workflow_dispatch:\n", 1)
        self.assertNotEqual(no_pr, text)
        self.assertEqual(len(self.errors(self.repo(no_pr))), 3)
        # 道を渡して recheck.test.mjs だけを走らせると、engine.test.mjs と ci.test.mjs は呼んでいない側に数える
        narrowed = text.replace("node --test --test-reporter=spec >", "node --test --test-reporter=spec recheck.test.mjs >", 1)
        self.assertNotEqual(narrowed, text)
        e = [x for x in self.errors(self.repo(narrowed)) if "wired にするには" in x]
        self.assertEqual(len(e), 2, e)
        self.assertFalse(any(self.TESTS[0] + " を呼ぶ" in x for x in e), e)

    def test_配線していなければ見ない(self):
        d = self.repo()
        (d / self.WF).unlink()
        self.assertEqual(self.errors(d, wired=False), [])


class PrBodyEngineCopyWiringTest(PrBodyEngineTestsWiringTest):
    """K-0012 の issue 1131（recheck.test.mjs）と issue 1205（写しとひな形のずれを落とす ci.test.mjs）の ci_check の解決策は、
    .github/workflows/pr-body-engine.yml の pull_request のジョブからその検査を呼ぶ項目がないと wired にできない（議決 D-0074（起票時の番号は D-0072）、Issue #1205）"""

    CASES = {1131: "tools/pr-body-check/recheck.test.mjs", 1205: "tools/pr-body-check/ci.test.mjs"}

    def item(self, runs=None, workflow=PrBodyEngineTestsWiringTest.WF, issue=1205, wired=True) -> dict:
        runs = (self.CASES[issue],) if runs is None else runs
        return {"id": "K-0012", "_file": "K-0012.yaml", "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "ci_check", "what": "x", "issue": issue, "wired": wired, "verified": False,
             "wired_by": [{"workflow": workflow, "job": "engine-test", "runs": r} for r in runs]}]}}

    def test_対象に挙げてある(self):
        for issue, target in self.CASES.items():
            key = ("K-0012", issue)
            self.assertEqual(check.REQUIRED_PR[key], (target, "D-0074"))
            self.assertEqual(check.REQUIRED_PR_WORKFLOW[key], self.WF)
            self.assertNotIn(key, check.REQUIRED_PR_ALSO)
            self.assertNotIn(key, check.REQUIRED_ENGINE_WAIT)

    def test_ひな形をそのまま写したジョブで_3_つとも呼べば通る(self):
        d = self.repo()
        for issue in self.CASES:
            self.assertEqual(self.errors(d, issue=issue), [], issue)

    def test_3_つのどれかの項目がなければ落ちる(self):
        d = self.repo()
        for issue, target in self.CASES.items():
            other = next(t for t in self.TESTS if t != target)
            e = self.errors(d, issue=issue, runs=(other,))
            self.assertEqual(len(e), 1, (issue, e))
            for part in (f"on.pull_request のある {self.WF} で", target, "議決 D-0074", f"issue {issue}"):
                self.assertIn(part, e[0])

    def test_別のワークフローの写しでは落ちる(self):
        d = self.repo(files={self.OTHER: self.TEMPLATE.read_text(encoding="utf-8")})
        for issue in self.CASES:
            e = self.errors(d, issue=issue, workflow=self.OTHER)
            self.assertEqual(len(e), 1, e)
            self.assertIn(self.WF, e[0])

    def test_pull_request_を外すか道を渡して絞った写しでは落ちる(self):
        text = self.TEMPLATE.read_text(encoding="utf-8")
        no_pr = text.replace("on:\n  pull_request:\n", "on:\n  workflow_dispatch:\n", 1)
        self.assertNotEqual(no_pr, text)
        for issue in self.CASES:
            self.assertEqual(len(self.errors(self.repo(no_pr), issue=issue)), 1, issue)
        # 道を渡して recheck.test.mjs だけを走らせると、写しのずれの見張り（ci.test.mjs）は呼んでいない側に数える
        narrowed = text.replace("node --test --test-reporter=spec >", "node --test --test-reporter=spec recheck.test.mjs >", 1)
        self.assertNotEqual(narrowed, text)
        d = self.repo(narrowed)
        self.assertEqual([x for x in self.errors(d, issue=1131) if "wired にするには" in x], [])
        e = [x for x in self.errors(d, issue=1205) if "wired にするには" in x]
        self.assertEqual(len(e), 1, e)
        self.assertIn(self.CASES[1205], e[0])


class DecisionTest(unittest.TestCase):
    """decided_by の議決との照合（議決 D-0041、Issue #1094）"""

    def repo(self, **decisions: str) -> Path:
        t = tempfile.TemporaryDirectory()
        self.addCleanup(t.cleanup)
        d = Path(t.name)
        (d / check.ROOT / "committee").mkdir(parents=True)
        for did, text in decisions.items():
            (d / check.ROOT / "committee" / f"{did.replace('_', '-')}.yaml").write_text(text, encoding="utf-8")
        return d

    PLAN = "id: D-0001\nenforcement:\n- {knowledge: K-9999, level: deterministic, solution_kind: test, proposal: x}\n"

    def test_議決の案に当たれば通る(self):
        d = self.repo(D_0001=self.PLAN)
        self.assertEqual(check.check_decision(k("deterministic", sol(decided_by="committee:D-0001")), d), [])

    def test_decided_by_を書いていない解決策は見ない(self):
        self.assertEqual(check.check_decision(k("deterministic", sol(), "x"), self.repo()), [])

    def test_議決がなければ落ちる(self):
        e = check.check_decision(k("deterministic", sol(decided_by="committee:D-0002")), self.repo(D_0001=self.PLAN))
        self.assertEqual(len(e), 1)
        self.assertIn("decision_mismatch: 議決 D-0002 が .factory-config/knowledge/committee/ にない", e[0])

    def test_知見の_id_か種類が違えば落ちる(self):
        d = self.repo(D_0001=self.PLAN.replace("K-9999", "K-0001"), D_0002=self.PLAN.replace("solution_kind: test", "solution_kind: lint"),
                      D_0003="id: D-0003\noutcome: adopt\nknowledge: [K-9999]\n", D_0004="id: D-0004\nenforcement: {knowledge: K-9999, solution_kind: test}\n")
        for did in ("D-0001", "D-0002", "D-0003", "D-0004"):
            e = check.check_decision(k("deterministic", sol(decided_by=f"committee:{did}")), d)
            self.assertEqual(len(e), 1, did)
            self.assertIn(f"decision_mismatch: 議決 {did} の enforcement に K-9999 の test の案がない", e[0])

    def test_形の崩れた_decided_by_は落ちる(self):
        d = self.repo(D_0001=self.PLAN)
        for v in ("D-0001", "committee:D-1", "committee:D-0001 ", None, 1):
            e = check.check_decision(k("deterministic", sol(decided_by=v)), d)
            self.assertEqual(len(e), 1, v)
            self.assertIn("decision_mismatch: decided_by は committee:D-NNNN", e[0])

    def test_読めない議決は落ちる(self):
        e = check.check_decision(k("deterministic", sol(decided_by="committee:D-0001")), self.repo(D_0001="id: [D-0001\n"))
        self.assertEqual(len(e), 1)
        self.assertIn("decision_mismatch: 議決 D-0001 が読めない", e[0])

    def test_main_で落とす(self):
        d = self.repo(D_0001=self.PLAN.replace("K-9999", "K-0001"))
        MainTest.write(None, d, "K-0001.yaml", "id: K-0001\nenforcement:\n  level: deterministic\n  solutions:\n"
                       "    - {kind: test, what: x, issue: 1, wired: false, verified: false, decided_by: 'committee:D-0001'}\n"
                       "    - {kind: lint, what: x, issue: 2, wired: false, verified: false, decided_by: 'committee:D-0001'}\n")
        out = d / "out.json"
        self.assertEqual(check.main(["--repo", str(d), "--json", str(out)]), 1)
        errors = json.loads(out.read_text(encoding="utf-8"))["errors"]
        self.assertEqual(len(errors), 1, errors)
        self.assertIn("issue 2）: decision_mismatch: 議決 D-0001 の enforcement に K-0001 の lint の案がない", errors[0])


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
        errors = without_gate(json.loads(out.read_text(encoding="utf-8"))["errors"])
        self.assertEqual(len(errors), 1, errors)
        self.assertIn("issue 1100）: wired にするには", errors[0])


class HeadRecordTest(unittest.TestCase):
    """K-0004 の engine-head に関わる解決策は Issue #948 に engine-head の記録が残るまで verified にできない（議決 D-0047、Issue #1114）"""

    HEAD = "a" * 40
    RUN = "https://github.com/guccyon/rally/actions/runs/123"

    def comments(self, *bodies: str, login: str = "github-actions[bot]", text: str | None = None) -> str:
        t = tempfile.TemporaryDirectory()
        self.addCleanup(t.cleanup)
        p = Path(t.name) / "comments.jsonl"
        p.write_text(text if text is not None else "".join(json.dumps({"user": {"login": login}, "body": b}) + "\n" for b in bodies), encoding="utf-8")
        return str(p)

    def record(self, kind="engine_head_passed") -> str:
        return f"<!-- sendback-engine-head: {self.HEAD} {kind} -->\n先頭で走らせた。\n\n- 実行: {self.RUN}\n"

    @staticmethod
    def k4(*sols):
        return {"id": "K-0004", "_file": "K-0004.yaml", "enforcement": {"level": "deterministic", "solutions": list(sols)}}

    @staticmethod
    def verified(issue: int):
        return {"kind": "ci_check", "what": "x", "issue": issue, "wired": True, "verified": True}

    def test_記録があれば通る(self):
        for kind in ("engine_head_passed", "engine_head_failed"):
            self.assertEqual(check.check_head_record(self.k4(self.verified(977)), self.comments("ほか", self.record(kind))), [], kind)

    def test_記録がなければ_head_record_missing_で落ちる(self):
        for name, comments in (("コメントがない", self.comments()), ("ほかの記録だけ", self.comments("PR #395 と同じ入力で通った")),
                               ("人が写した印", self.comments(self.record(), login="tessan"))):
            for issue in (977, 1046, 1047):
                e = check.check_head_record(self.k4(self.verified(issue)), comments)
                self.assertEqual(len(e), 1, (name, issue))
                self.assertIn(f"issue {issue}）: head_record_missing: Issue #948 に engine-head", e[0])

    def test_コメントを渡していない_読めないときは_head_record_unchecked_で落ちる(self):
        for comments in (None, "/nonexistent/comments.jsonl", self.comments(text="{}\nnot json\n")):
            e = check.check_head_record(self.k4(self.verified(977)), comments)
            self.assertEqual(len(e), 1, comments)
            self.assertIn("head_record_unchecked", e[0])

    def test_head_record_mjs_が結果を出さずに抜けたら通さない(self):
        t = tempfile.TemporaryDirectory()
        self.addCleanup(t.cleanup)
        fake = Path(t.name) / "head-record.mjs"
        fake.write_text("process.exit(0);\n", encoding="utf-8")
        e = check.check_head_record(self.k4(self.verified(977)), self.comments(self.record()), script=fake)
        self.assertEqual(len(e), 1)
        self.assertIn("head_record_unchecked", e[0])

    def test_対象でない解決策と確認していない解決策は見ない(self):
        sols = (dict(self.verified(977), verified=False), dict(self.verified(977), verified="true"), self.verified(1018), self.verified(1076))
        self.assertEqual(check.check_head_record(self.k4(*sols), None, script=Path("/nonexistent.mjs")), [])
        other = {"id": "K-0001", "_file": "K-0001.yaml", "enforcement": {"level": "deterministic", "solutions": [self.verified(977)]}}
        self.assertEqual(check.check_head_record(other, None, script=Path("/nonexistent.mjs")), [])

    def test_head_record_mjs_は_1_回だけ走らせる(self):
        cache: dict = {}
        comments = self.comments()
        self.assertEqual(len(check.check_head_record(self.k4(self.verified(977), self.verified(1047)), comments, cache)), 2)
        cache["result"] = None  # 走らせ直さず覚えた結果を使う
        self.assertEqual(check.check_head_record(self.k4(self.verified(977)), comments, cache), [])

    def test_main_で落とす(self):
        t = tempfile.TemporaryDirectory()
        self.addCleanup(t.cleanup)
        d = Path(t.name)
        MainTest.write(None, d, "K-0004.yaml", "id: K-0004\nenforcement:\n  level: deterministic\n  solutions:\n"
                       "    - {kind: ci_check, what: x, issue: 977, wired: false, verified: true}\n")
        out = d / "out.json"
        for comments, rule in ((None, "head_record_unchecked"), (self.comments(), "head_record_missing")):
            argv = ["--repo", str(d), "--json", str(out)] + (["--head-comments", comments] if comments else [])
            self.assertEqual(check.main(argv), 1)
            errors = json.loads(out.read_text(encoding="utf-8"))["errors"]
            self.assertTrue(any(f"issue 977）: {rule}" in x for x in errors), errors)
        self.assertEqual(check.main(["--repo", str(d), "--json", str(out), "--head-comments", self.comments(self.record())]), 1)
        errors = json.loads(out.read_text(encoding="utf-8"))["errors"]
        self.assertFalse(any("head_record" in x for x in errors), errors)


class ReviewByHoldTest(unittest.TestCase):
    """K-0010 の review_by を前の版より後ろへ動かす PR では、ENGINE_REF の Engine で EngineFinishedTest が todo のままなら落とす
    （議決 D-0067（起票時の番号は D-0066）、Issue #1178）。Engine は一時の git リポジトリに dispatcher.py の job.finished だけを置いて作る"""

    LEGACY = ["state", "cost_usd", "exit"]
    BEFORE = "2026-12-05"

    @staticmethod
    def git(cwd: Path, *args: str) -> str:
        return subprocess.run(["git", "-c", "user.email=t@example.invalid", "-c", "user.name=t", *args], cwd=cwd,
                              check=True, capture_output=True, text=True).stdout.strip()

    def tmp(self) -> Path:
        t = tempfile.TemporaryDirectory()
        self.addCleanup(t.cleanup)
        return Path(t.name)

    def engine(self, *versions: list[str]) -> tuple[Path, list[str]]:
        """versions の欄ごとに job.finished を出す dispatcher.py の版を積んだ Engine と、各版の SHA"""
        d, shas = self.tmp(), []
        self.git(d, "init", "-q")
        f = d / check.ENGINE_DISPATCHER
        f.parent.mkdir(parents=True)
        for fields in versions:
            payload = ", ".join(f'"{x}": None' for x in fields)
            f.write_text(f'def run(cur, corr, job_id):\n    record_event(cur, "job.finished", corr, job_id, {{{payload}}})\n', encoding="utf-8")
            self.git(d, "add", "-A")
            self.git(d, "commit", "-q", "-m", "v")
            shas.append(self.git(d, "rev-parse", "HEAD"))
        return d, shas

    def repo(self, ref: str, after: str | None, before: str | None = BEFORE, copy_ref: str | None = None) -> Path:
        """base の版の K-0010 の review_by が before、作業ツリーで after。conflict-check のひな形の ENGINE_REF は ref（copy_ref があれば写しも置く）"""
        d = self.tmp()
        shutil.copytree(REPO / "tools/conflict-check", d / "tools/conflict-check", ignore=shutil.ignore_patterns("__pycache__", "fixtures"))
        tpl = d / check.CONFLICT_CHECK_WORKFLOWS[1]
        tpl.write_text(check.ENGINE_REF_RE.sub(f"  ENGINE_REF: {ref}", tpl.read_text(encoding="utf-8")), encoding="utf-8")
        if copy_ref is not None:
            (d / check.WORKFLOWS).mkdir(parents=True)
            (d / check.CONFLICT_CHECK_WORKFLOWS[0]).write_text(f"env:\n  ENGINE_REF: {copy_ref}\n", encoding="utf-8")
        self.git(d, "init", "-q")
        if before is not None:
            self.knowledge(d, before)
        self.git(d, "add", "-A")
        self.git(d, "commit", "-q", "-m", "base")
        self.knowledge(d, after)
        return d

    def knowledge(self, d: Path, review_by: str | None):
        MainTest.write(None, d, "K-0010.yaml", "id: K-0010\n" + (f"review_by: {review_by}\n" if review_by is not None else ""))

    def errors(self, d: Path, engine: Path | None, base="HEAD") -> list[str]:
        return [e for x in check.load(d) for e in check.check_review_by_hold(x, d, base, engine)]

    def test_延ばさなければ_Engine_なしでも見ない(self):
        _, (sha,) = self.engine(self.LEGACY)
        for after in (self.BEFORE, "2026-12-01"):
            self.assertEqual(self.errors(self.repo(sha, after), None), [], after)

    def test_延ばして_Engine_が今の形なら落とす(self):
        eng, (sha,) = self.engine(self.LEGACY)
        e = self.errors(self.repo(sha, "2027-01-05"), eng)
        self.assertEqual(len(e), 1, e)
        for s in ("K-0010.yaml: review_by_hold: review_by を 2026-12-05 から 2027-01-05 へ動かす", sha, "EngineFinishedTest が todo のまま", "議決 D-0067"):
            self.assertIn(s, e[0])

    def test_review_by_を読めなくしても延ばしたとみなす(self):
        eng, (sha,) = self.engine(self.LEGACY)
        for after in (None, "いつか"):
            self.assertEqual(len(self.errors(self.repo(sha, after), eng)), 1, after)

    def test_延ばしても_Engine_が欄を出す版なら通る(self):
        eng, shas = self.engine(self.LEGACY, self.LEGACY + ["agent_resumed"], self.LEGACY + ["agent_tier"], self.LEGACY + ["agent_resumed", "agent_tier"])
        self.assertEqual(len(self.errors(self.repo(shas[0], "2027-01-05"), eng)), 1)
        for sha in shas[1:]:   # 片方だけの版は todo でなく EngineFinishedTest そのものが落ちるので、ここでは落とさない
            self.assertEqual(self.errors(self.repo(sha, "2027-01-05"), eng), [], sha)

    def test_写しがあれば写しの_ENGINE_REF_を見る(self):
        eng, (old, new) = self.engine(self.LEGACY, self.LEGACY + ["agent_resumed", "agent_tier"])
        self.assertEqual(self.errors(self.repo(old, "2027-01-05", copy_ref=new), eng), [])
        e = self.errors(self.repo(new, "2027-01-05", copy_ref=old), eng)
        self.assertEqual(len(e), 1, e)
        self.assertIn(".github/workflows/conflict-check.yml", e[0])

    def test_照らせなければ落とす(self):
        eng, (sha,) = self.engine(self.LEGACY)
        other, (missing,) = self.engine(["state", "exit"])   # cost_usd を持つ job.finished がない（engine_harness.py が読めない）
        no_fields = self.repo(sha, "2027-01-05")
        (no_fields / check.CONFLICT_EVENTS_TEST).write_text("X = 1\n", encoding="utf-8")
        cases = {"Engine を渡さない": (self.repo(sha, "2027-01-05"), None, "engine_missing", "--engine か ENGINE_DIR"),
                 "Engine に版がない": (self.repo(missing, "2027-01-05"), eng, "engine_ref_missing", f"ENGINE_REF {missing} の版がない"),
                 "欄を読めない": (self.repo(missing, "2027-01-05"), other, "finished_unreadable", "finished が落ちた"),
                 "ENGINE_REF が SHA でない": (self.repo("main", "2027-01-05"), eng, "engine_ref_invalid", "40 桁のコミット SHA"),
                 "FINISHED_FIELDS がない": (no_fields, eng, "fields_unreadable", "FINISHED_FIELDS がない")}
        for name, (d, engine, kind, msg) in cases.items():
            e = self.errors(d, engine)
            self.assertEqual(len(e), 1, name)
            self.assertIn(f"K-0010.yaml: review_by_unverifiable（{kind}）: ", e[0], name)
            self.assertIn("照らせない", e[0], name)
            self.assertIn(msg, e[0], name)

    def test_照らせないこと以外の例外は照らせないに混ぜない(self):
        """実装の不具合（KeyError など）は review_by_unverifiable の行にせず、そのまま上げる（指摘 ARCH-007）"""
        eng, (sha,) = self.engine(self.LEGACY)
        d = self.repo(sha, "2027-01-05")
        with mock.patch.object(check, "engine_finished_legacy", side_effect=KeyError("fields")), self.assertRaises(KeyError):
            self.errors(d, eng)

    def test_前の版を渡さない_前の版にない_ほかの知見は見ない(self):
        _, (sha,) = self.engine(self.LEGACY)
        self.assertEqual(self.errors(self.repo(sha, "2027-01-05"), None, base=None), [])
        self.assertEqual(self.errors(self.repo(sha, "2027-01-05", before=None), None), [])
        self.assertEqual(self.errors(self.repo(sha, "2027-01-05", before="いつか"), None), [])
        d = self.repo(sha, "2027-01-05")
        self.assertEqual(check.check_review_by_hold({"id": "K-0004", "_file": "K-0010.yaml"}, d, "HEAD", None), [])

    def test_前の版が読めなければ落とす(self):
        _, (sha,) = self.engine(self.LEGACY)
        e = self.errors(self.repo(sha, self.BEFORE), None, base="origin/none")
        self.assertEqual(len(e), 1, e)
        self.assertIn("K-0010.yaml: review_by_unverifiable（base_unreadable）: ", e[0])
        self.assertIn("前の版 origin/none がリポジトリにない", e[0])

    def test_main_で落とす(self):
        eng, (sha,) = self.engine(self.LEGACY)
        d = self.repo(sha, "2027-01-05")
        self.assertEqual(check.main(["--repo", str(d), "--base", "HEAD", "--engine", str(eng)]), 1)
        self.assertEqual(check.main(["--repo", str(d)]), 0)

    def test_求める欄は_test_events_py_の_FINISHED_FIELDS(self):
        self.assertEqual(check.finished_fields_required(REPO), ("agent_resumed", "agent_tier"))


class SolutionFieldsTest(unittest.TestCase):
    """PR の差分で変わった K-*.yaml の解決策に wired と verified の両方がなければ solution_fields_missing で落とす（議決 D-0071、Issue #1193）。
    base の版は一時の git リポジトリに積んで作る"""

    OPEN = "id: K-0006\nenforcement:\n  level: deterministic\n  solutions:\n    - {kind: ci_check, what: x, issue: 906}\n"
    DONE = "id: K-0006\nenforcement:\n  level: deterministic\n  solutions:\n    - {kind: ci_check, what: x, issue: 906, wired: false, verified: false}\n"

    def repo(self, before: str | None, after: str | None) -> Path:
        """base の版の K-0006.yaml が before（None ならなし）、作業ツリーで after（None なら base のまま）"""
        d = ReviewByHoldTest.tmp(self)
        ReviewByHoldTest.git(d, "init", "-q")
        if before is not None:
            MainTest.write(None, d, "K-0006.yaml", before)
        (d / "README.md").write_text("x\n", encoding="utf-8")
        ReviewByHoldTest.git(d, "add", "-A")
        ReviewByHoldTest.git(d, "commit", "-q", "-m", "base")
        if after is not None:
            MainTest.write(None, d, "K-0006.yaml", after)
        return d

    def errors(self, d: Path, base: str | None = "HEAD") -> list[str]:
        return [e for x in check.load(d) for e in check.check_solution_fields(x, d, base)]

    def test_未記入のまま直すと落とす(self):
        e = self.errors(self.repo(self.OPEN, self.OPEN.replace("what: x", "what: y")))
        self.assertEqual(len(e), 1)
        self.assertIn(check.SOLUTION_FIELDS_MISSING, e[0])
        self.assertIn("enforcement.solutions[0]（ci_check、issue 906）に wired・verified がない", e[0])

    def test_片方だけの記入も落とす(self):
        e = self.errors(self.repo(self.OPEN, self.OPEN.replace("issue: 906}", "issue: 906, wired: false}")))
        self.assertEqual(len(e), 1)
        self.assertIn("に verified がない", e[0])

    def test_新しく足した知見の未記入も落とす(self):
        e = self.errors(self.repo(None, self.OPEN))
        self.assertEqual(len(e), 1)
        self.assertIn(check.SOLUTION_FIELDS_MISSING, e[0])

    def test_未記入の解決策ごとに_1_行(self):
        two = self.OPEN + "    - {kind: test, what: z, issue: 1193}\n"
        e = self.errors(self.repo(self.OPEN, two))
        self.assertEqual([("solutions[0]" in x, "solutions[1]" in x) for x in e], [(True, False), (False, True)])

    def test_書き足して直せば通る(self):
        self.assertEqual(self.errors(self.repo(self.OPEN, self.DONE)), [])

    def test_変えていない知見の未記入は見ない(self):
        self.assertEqual(self.errors(self.repo(self.OPEN, None)), [])

    def test_base_を渡さなければ見ない(self):
        self.assertEqual(self.errors(self.repo(self.OPEN, self.OPEN.replace("what: x", "what: y")), None), [])

    def test_分かれ目より後に_base_で変わった知見は差分に数えない(self):
        # PR の分岐のあとで base 側だけが知見を直しても、この PR の差分ではない（merge-base と比べる）
        d = self.repo(self.OPEN, None)
        ReviewByHoldTest.git(d, "checkout", "-q", "-b", "base")
        MainTest.write(None, d, "K-0006.yaml", self.OPEN.replace("what: x", "what: base"))
        ReviewByHoldTest.git(d, "commit", "-q", "-am", "base が進む")
        ReviewByHoldTest.git(d, "checkout", "-q", "-")
        self.assertEqual(self.errors(d, "base"), [])
        MainTest.write(None, d, "K-0006.yaml", self.OPEN.replace("what: x", "what: pr"))
        self.assertIn(check.SOLUTION_FIELDS_MISSING, self.errors(d, "base")[0])

    def test_分かれ目を求められなければ照らせないとして落とす(self):
        d = self.repo(self.OPEN, self.OPEN.replace("what: x", "what: y"))
        e = self.errors(d, "no-such-ref")
        self.assertEqual(len(e), 1)
        self.assertIn(check.SOLUTION_FIELDS_UNVERIFIABLE, e[0])
        # 未記入の解決策がなければ、base を読めなくても見ない
        MainTest.write(None, d, "K-0006.yaml", self.DONE)
        self.assertEqual(self.errors(d, "no-such-ref"), [])

    def test_main_が_base_を渡すと照らして_1_を返す(self):
        d = self.repo(self.OPEN, self.OPEN.replace("what: x", "what: y"))
        self.assertEqual(check.main(["--repo", str(d), "--base", "HEAD"]), 1)
        self.assertEqual(check.main(["--repo", str(d)]), 0)


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

    def listed(self, path, tail):
        """README の「すべての解決策に書いたのは `K-…`。`K-…`{tail}」の 2 つの一覧を取り出す"""
        text = path.read_text(encoding="utf-8")
        ids = r"((?:`K-\d{4,}`・?)+)"
        m = re.search(rf"すべての解決策に書いたのは {ids}。{ids}{tail}", text)
        name = path.relative_to(REPO)
        self.assertIsNotNone(m, f"{name} に「すべての解決策に書いたのは `K-…`。`K-…`{tail}」の一覧がない")
        return (re.findall(r"K-\d{4,}", g) for g in m.groups())

    def assert_listed(self, path, tail):
        listed_done, listed_open = self.listed(path, tail)
        rows = [check.row(x) for x in check.load(REPO)]
        done = [r["id"] for r in rows if r["solutions"] and not r["unrecorded"]]
        still_open = [r["id"] for r in rows if r["unrecorded"]]
        name = path.relative_to(REPO)
        self.assertEqual(listed_done, done, f"{name} の「すべての解決策に書いた」知見の一覧を K-*.yaml に合わせて直す")
        self.assertEqual(listed_open, still_open, f"{name} の「未記入の解決策が残る」知見の一覧を K-*.yaml に合わせて直す")

    def test_README_の記入済みと未記入の知見の一覧が記録と一致する(self):
        """README の「すべての解決策に書いた知見」「未記入の解決策が残る知見」の一覧を、check.py が K-*.yaml から数えた結果と照らす（議決 D-0035、Issue #1062）"""
        self.assert_listed(HERE / "README.md", " の未記入の解決策は")

    def test_knowledge_README_の記入済みと未記入の知見の一覧が記録と一致する(self):
        """.factory-config/knowledge/README.md の同じ 2 つの一覧も、順まで照らす（議決 D-0052、Issue #1138）"""
        self.assert_listed(REPO / ".factory-config" / "knowledge" / "README.md", " には未記入の解決策が残っていて")


if __name__ == "__main__":
    unittest.main()
