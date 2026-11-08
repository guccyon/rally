"""ci/knowledge-enforcement.yml（PR ごとの CI のひな形）の形のテスト（知見 K-0004 の担保。議決 D-0035、Issue #1063）。Engine は要らない。

ひな形の engine-test のジョブが、PR ごとに必ず走り（絞り込みのない pull_request・if: と continue-on-error と needs なし）、
knowledge-check.yml と同じ ENGINE_REF の版の Engine を取り、test_engine.py と test_engine_patch.py を CI=true と ENGINE_DIR を渡し、
pytest を入れて飛ばさずに走らせる形かを見る（差分が ENGINE_REF に当たらないか、差分で足したテストが落ちればジョブが落ちる）。
.github/workflows/knowledge-enforcement.yml に写してあれば、その写しも同じ形かと、ENGINE_REF の行のほかがひな形と同じかも見る。
形の検査（problems）が外れを見逃さないことは、ひな形を書き換えた写しで確かめる。

  python3 -m unittest discover -s tools/knowledge-enforcement-check
"""
from __future__ import annotations

import importlib.util
import re
import subprocess
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
    runs = [s for s in steps if any(t.rsplit("/", 1)[-1] in (s.get("run") or "") for t in TESTS)]
    if len(runs) != 1:
        return e + ["test_engine.py と test_engine_patch.py を 1 つの手順で走らせる"]
    s = runs[0]
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

    def test_外れた写しは落ちる(self):
        t = self.text
        engine_step = "          python3 -B tools/knowledge-enforcement-check/test_engine.py -v\n"
        self.assertIn(engine_step, t)
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
            "test_engine.py を呼ばない": t.replace(engine_step, ""),
            "test_engine_patch.py を呼ばない": t.replace("          python3 -B tools/knowledge-enforcement-check/test_engine_patch.py -v\n", ""),
            "失敗を握りつぶす": t.replace(engine_step, engine_step.rstrip("\n") + " || true\n"),
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
