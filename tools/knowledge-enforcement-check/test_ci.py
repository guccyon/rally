"""ci/knowledge-enforcement.yml（PR ごとの CI のひな形）の形のテスト（知見 K-0004 の担保。議決 D-0035、Issue #1063）。Engine は要らない。

ひな形の engine-test のジョブが、PR ごとに必ず走り（絞り込みのない pull_request・if: と continue-on-error と needs なし）、
knowledge-check.yml と同じ ENGINE_REF の版の Engine を取り、test_engine.py と test_engine_patch.py を CI=true と ENGINE_DIR を渡し、
pytest を入れて飛ばさずに走らせる形かを見る（差分が ENGINE_REF に当たらないか、差分で足したテストが落ちればジョブが落ちる）。
テストは unittest discover でまとめて走らせて出力を残し、次の手順がその出力の skipped が 1 件以上なら落とす（議決 D-0037、Issue #1080）。
その次の手順が、出力に ENGINE_REF の版の差分の状態の記録（engine_patch_state: ENGINE_REF <版> applies か merged）がなければ落とす（議決 D-0053、Issue #1150）。
.github/workflows/knowledge-enforcement.yml に写してあれば、その写しも同じ形かと、ENGINE_REF の行のほかがひな形と同じかも見る。
形の検査（problems）が外れを見逃さないことは、ひな形を書き換えた写しで確かめる。

  python3 -m unittest discover -s tools/knowledge-enforcement-check
"""
from __future__ import annotations

import importlib.util
import re
import subprocess
import tempfile
import unittest
from pathlib import Path

import yaml

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
TEMPLATE = HERE / "ci" / "knowledge-enforcement.yml"
COPY = REPO / ".github" / "workflows" / "knowledge-enforcement.yml"
JOB = "engine-test"
TESTS = ("tools/knowledge-enforcement-check/test_engine.py", "tools/knowledge-enforcement-check/test_engine_patch.py")
REF_LINE = re.compile(r"^\s*ENGINE_REF:\s*\S+\s*$", re.M)
DIR = "tools/knowledge-enforcement-check"
LOG = "knowledge-enforcement-unittest.log"
RECORD = "engine_patch_state"

spec = importlib.util.spec_from_file_location("knowledge_enforcement_check_c", HERE / "check.py")
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)  # type: ignore[union-attr]
spec = importlib.util.spec_from_file_location("knowledge_enforcement_check_c_patch", HERE / "test_engine_patch.py")
patch_check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(patch_check)  # type: ignore[union-attr]


def load(text: str) -> dict:
    d = yaml.safe_load(text)
    if True in d:   # YAML 1.1 では on: が真偽になる
        d["on"] = d.pop(True)
    return d


def read(rel: str) -> str | None:
    p = REPO / rel
    return p.read_text(encoding="utf-8") if rel and p.is_file() else None


def problems(text: str, ref: str) -> list[str]:
    """ワークフローの文が engine-test の形から外れているところ。ref は knowledge-check.yml の ENGINE_REF"""
    wf, e = load(text), []
    if wf.get("on") != {"pull_request": None}:
        e.append(f"on は絞り込みのない pull_request だけにする（{wf.get('on')!r}）")
    if (wf.get("env") or {}).get("ENGINE_REF") != ref:
        e.append(f"ワークフローの env の ENGINE_REF が knowledge-check.yml の {ref} でない")
    if len(REF_LINE.findall(text)) != 1:
        e.append("ENGINE_REF はワークフローの env に 1 行だけ書く（ジョブや手順で上書きしない）")
    if "GITHUB_ENV" in text:
        e.append("GITHUB_ENV で環境を書き換えない")
    job = (wf.get("jobs") or {}).get(JOB)
    if not isinstance(job, dict):
        return e + [f"{JOB} のジョブがない"]
    for key in ("if", "continue-on-error", "needs", "env", "defaults"):
        if key in job:
            e.append(f"{JOB} のジョブに {key}: を書かない")
    steps = job.get("steps") or []
    for s in steps:
        for key in ("if", "continue-on-error"):
            if key in s:
                e.append(f"手順 {s.get('name') or s.get('uses')} に {key}: を書かない")
    guard = steps[0] if steps else {}
    if "ENGINE_REF" not in (guard.get("run") or "") or guard.get("shell") != "bash":
        e.append("最初の手順で ENGINE_REF が 40 桁の SHA かを bash で確かめる")
    engine = [s for s in steps if (s.get("with") or {}).get("repository") == "guccyon/factory-engine"]
    if len(engine) != 1 or engine[0]["with"].get("ref") != "${{ env.ENGINE_REF }}" or engine[0]["with"].get("path") != ".engine":
        e.append("Engine を ref: ${{ env.ENGINE_REF }}・path: .engine で 1 回だけ取る")
    runs = [s for s in steps if DIR in (s.get("run") or "")]
    if len(runs) != 1:
        return e + [f"{DIR} のテストを 1 つの手順で走らせる"]
    s = runs[0]
    if not re.search(rf"\bunittest\s+discover\s+-s\s+{re.escape(DIR)}\b", s["run"]):
        e.append(f"テストは python3 -m unittest discover -s {DIR} でまとめて走らせる（飛ばした件数を出力で数える）")
    if re.search(r"\s(-p|--pattern|-k)\b", s["run"]):
        e.append("unittest discover に -p・-k を付けない（走らせるテストを絞らない）")
    if not re.search(rf"\|\s*tee\s+{re.escape(LOG)}\s*$", s["run"], re.M):
        e.append(f"unittest discover の出力を tee {LOG} で残す")
    if s.get("shell") != "bash" or "set -o pipefail" not in s["run"]:
        e.append("テストの手順は shell: bash と set -o pipefail で走らせる（tee で失敗を握りつぶさない）")
    after = steps[steps.index(s) + 1:]
    guard_skip = [x for x in after if LOG in (x.get("run") or "") and "skipped=" in (x.get("run") or "")]
    if len(guard_skip) != 1 or guard_skip[0] is not (after[0] if after else None) or guard_skip[0].get("shell") != "bash":
        e.append(f"テストの手順のすぐ後に、{LOG} の skipped が 1 件以上なら落とす手順を bash で 1 つ置く")
    guard_ref = [x for x in after if LOG in (x.get("run") or "") and RECORD in (x.get("run") or "")]
    if (len(guard_ref) != 1 or len(after) < 2 or guard_ref[0] is not after[1] or guard_ref[0].get("shell") != "bash"
            or f"^{RECORD}: ENGINE_REF ${{ENGINE_REF}} " not in guard_ref[0]["run"]):
        e.append(f"飛ばした件数の手順のすぐ後に、{LOG} に ENGINE_REF の版の {RECORD} の記録（applies か merged）がなければ落とす手順を bash で 1 つ置く")
    env = s.get("env") or {}
    if env.get("CI") != "true":
        e.append("テストの手順に CI: \"true\" を渡す（Engine・ENGINE_REF の版・pytest がなければ飛ばさずに落とす）")
    if env.get("ENGINE_DIR") != "${{ github.workspace }}/.engine":
        e.append("テストの手順に ENGINE_DIR: ${{ github.workspace }}/.engine を渡す")
    if not re.search(r"pip install\b.*\bpytest\b", s["run"]):
        e.append("pytest を入れる（差分で Engine に足したテストを飛ばさない）")
    if re.search(r"\|\|\s*true|\|\|\s*:|;\s*true\b|set \+e|\|\|\s*rc=|\|\|\s*exit 0", s["run"]):
        e.append("テストの失敗を握りつぶさない")
    for t in TESTS:
        if not check.job_calls(wf, JOB, t, read):
            e.append(f"{JOB} のジョブが {t} を呼んでいない")
    return e


class TemplateTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.text = TEMPLATE.read_text(encoding="utf-8")
        cls.ref = patch_check.engine_ref()

    def test_ひな形は形どおり(self):
        self.assertEqual(problems(self.text, self.ref), [])

    def test_最初の手順は40桁のSHAのときだけ通る(self):
        guard = load(self.text)["jobs"][JOB]["steps"][0]
        for ref, rc in (("main", 1), (self.ref[:7], 1), (self.ref, 0)):
            p = subprocess.run(["bash", "-e", "-c", guard["run"]], env={"PATH": "/usr/bin:/bin", "ENGINE_REF": ref}, capture_output=True, text=True)
            self.assertEqual(p.returncode, rc, ref)

    def test_飛ばした件数の手順はskippedが1件以上かテストが走っていないときに落ちる(self):
        steps = load(self.text)["jobs"][JOB]["steps"]
        guard = [s for s in steps if LOG in (s.get("run") or "") and "skipped=" in s["run"]][0]
        ran = "Ran 12 tests in 0.345s\n\n"
        cases = [
            (None, 1), ("", 1), ("NO TESTS RAN\n", 1), ("Ran 0 tests in 0.000s\n\nOK\n", 1),
            (ran + "OK\n", 0), (ran + "OK (expected failures=1)\n", 0),
            ("test_x (m.T.test_x) ... skipped 'Engine のチェックアウトがない'\n" + ran + "OK (skipped=1)\n", 1),
            (ran + "OK (skipped=3, expected failures=1)\n", 1), (ran + "FAILED (failures=1, skipped=2)\n", 1),
        ]
        for log, rc in cases:
            with self.subTest(log), tempfile.TemporaryDirectory() as d:
                if log is not None:
                    (Path(d) / LOG).write_text(log, encoding="utf-8")
                p = subprocess.run(["bash", "--noprofile", "--norc", "-eo", "pipefail", "-c", guard["run"]], cwd=d,
                                   env={"PATH": "/usr/bin:/bin"}, capture_output=True, text=True)
                self.assertEqual(p.returncode, rc, p.stdout + p.stderr)
                if rc and log and "skipped=" in log:
                    self.assertIn("tests_skipped", p.stdout)

    def test_差分の状態の手順はENGINE_REFの版がappliesかmergedの記録のときだけ通る(self):
        steps = load(self.text)["jobs"][JOB]["steps"]
        guard = [s for s in steps if LOG in (s.get("run") or "") and RECORD in s["run"]][0]
        ref, other = self.ref, "0" * 40
        test = "test_x (m.T.test_x) ... "
        cases = [
            (None, 1), ("", 1), ("Ran 12 tests in 0.345s\n\nOK\n", 1),
            (f"{test}\n{RECORD}: ENGINE_REF {ref} merged\nok\n", 0), (f"{test}\n{RECORD}: ENGINE_REF {ref} applies\nok\n", 0),
            (f"{test}\n{RECORD}: ENGINE_REF {ref} engine_ref_unavailable\nFAIL\n", 1),
            (f"{test}\n{RECORD}: ENGINE_REF {ref} patch_stale\nFAIL\n", 1),
            (f"{test}\n{RECORD}: ENGINE_REF {ref} patch_partially_applied\nFAIL\n", 1),
            (f"{test}\n{RECORD}: ENGINE_REF {other} merged\nok\n", 1),
            (f"{test}\n{RECORD}: ENGINE_PATCH_REF {ref} merged\nok\n", 1),
            (f"{test}{RECORD}: ENGINE_REF {ref} merged\nok\n", 1),   # 行頭でない
            (f"{test}\n{RECORD}: ENGINE_REF {ref} merged_x\nok\n", 1),
        ]
        for log, rc in cases:
            with self.subTest(log), tempfile.TemporaryDirectory() as d:
                if log is not None:
                    (Path(d) / LOG).write_text(log, encoding="utf-8")
                p = subprocess.run(["bash", "--noprofile", "--norc", "-eo", "pipefail", "-c", guard["run"]], cwd=d,
                                   env={"PATH": "/usr/bin:/bin", "ENGINE_REF": ref}, capture_output=True, text=True)
                self.assertEqual(p.returncode, rc, p.stdout + p.stderr)
                if rc:
                    self.assertIn("engine_ref_unavailable", p.stdout)

    def test_外れた写しは落ちる(self):
        t = self.text
        engine_step = f"          python3 -B -m unittest discover -s {DIR} -v 2>&1 | tee {LOG}\n"
        self.assertIn(engine_step, t)
        skip_step = "      - name: 飛ばしたテストがないか（K-0004）\n"
        self.assertIn(skip_step, t)
        ref_step = "      - name: ENGINE_REF の版の差分の状態を記録したか（K-0004）\n"
        self.assertIn(ref_step, t)
        cases = {
            "paths の絞り込み": t.replace("  pull_request:\n", "  pull_request:\n    paths: ['.factory-config/**']\n", 1),
            "ENGINE_REF のずれ": t.replace(self.ref, "0" * 40),
            "ENGINE_REF の上書き": t.replace("        env:\n          CI: \"true\"\n", f"        env:\n          ENGINE_REF: {'1' * 40}\n          CI: \"true\"\n", 1),
            "ジョブの if:": t.replace(f"  {JOB}:\n    runs-on", f"  {JOB}:\n    if: false\n    runs-on", 1),
            "ジョブの needs:": t.replace(f"  {JOB}:\n    runs-on", f"  {JOB}:\n    needs: enforcement\n    runs-on", 1),
            "ジョブの continue-on-error:": t.replace(f"  {JOB}:\n    runs-on", f"  {JOB}:\n    continue-on-error: true\n    runs-on", 1),
            "手順の continue-on-error:": t.replace("        env:\n          CI: \"true\"\n", "        continue-on-error: true\n        env:\n          CI: \"true\"\n", 1),
            "CI を渡さない": t.replace("          CI: \"true\"\n", "", 1),
            "ENGINE_DIR を渡さない": t.replace("          ENGINE_DIR: ${{ github.workspace }}/.engine\n", "", 1),
            "Engine の版を固定しない": t.replace("ref: ${{ env.ENGINE_REF }}", "ref: main", 1),
            "pytest を入れない": t.replace("pyyaml pytest", "pyyaml", 1),
            "テストを走らせない": t.replace(engine_step, ""),
            "別のディレクトリを走らせる": t.replace(engine_step, engine_step.replace(f"-s {DIR}", f"-s {DIR}/engine")),
            "テストを絞る": t.replace(engine_step, engine_step.replace(" -v ", " -p 'test_check.py' -v ")),
            "test_engine.py だけを直に走らせる": t.replace(engine_step, f"          python3 -B {TESTS[0]} -v 2>&1 | tee {LOG}\n"),
            "失敗を握りつぶす": t.replace(engine_step, engine_step.rstrip("\n") + " || true\n"),
            "出力を残さない": t.replace(engine_step, engine_step.replace(f" 2>&1 | tee {LOG}", "")),
            "pipefail なし": t.replace("          set -o pipefail\n", "", 1),
            "飛ばした件数を見ない": t[:t.index(skip_step)] + t[t.index("  enforcement:\n"):],
            "飛ばした件数の手順がすぐ後でない": t.replace(skip_step, "      - name: x\n        run: echo ok\n" + skip_step, 1),
            "差分の状態の記録を見ない": t[:t.index(ref_step)] + t[t.index("  enforcement:\n"):],
            "差分の状態の手順がすぐ後でない": t.replace(ref_step, "      - name: x\n        run: echo ok\n" + ref_step, 1),
            "差分の状態の手順が ENGINE_REF の版を見ない": t.replace('ENGINE_REF ${ENGINE_REF} " "$log"', 'ENGINE_REF " "$log"', 1),
            "差分の状態の手順の continue-on-error:": t.replace(ref_step + "        shell: bash\n", ref_step + "        continue-on-error: true\n        shell: bash\n", 1),
            "飛ばした件数の手順の continue-on-error:": t.replace(skip_step + "        shell: bash\n", skip_step + "        continue-on-error: true\n        shell: bash\n", 1),
            "最初の手順がない": t.replace("      - name: ENGINE_REF が 40 桁のコミット SHA か（K-0004）\n", "      - name: x\n", 1).replace(
                "        shell: bash\n        run: |\n          if [[", "        run: |\n          if [[", 1),
        }
        for name, text in cases.items():
            with self.subTest(name):
                self.assertNotEqual(text, t)
                self.assertNotEqual(problems(text, self.ref), [], name)

    def test_写しがあれば形どおりでENGINE_REFの行のほかはひな形と同じ(self):
        if not COPY.is_file():
            self.skipTest(".github/workflows/knowledge-enforcement.yml にまだ写していない（建設者が写す）")
        text = COPY.read_text(encoding="utf-8")
        self.assertEqual(problems(text, self.ref), [])
        self.assertEqual(REF_LINE.sub("ENGINE_REF:", text), REF_LINE.sub("ENGINE_REF:", self.text))


if __name__ == "__main__":
    unittest.main()
