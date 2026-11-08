"""ci/knowledge-enforcement.yml（PR ごとの CI のひな形）の形のテスト（知見 K-0004 の担保。議決 D-0035、Issue #1063）。Engine は要らない。

ひな形の engine-test のジョブが、PR ごとに必ず走り（絞り込みのない pull_request・if: と continue-on-error と needs なし）、
knowledge-check.yml と同じ ENGINE_REF の版の Engine を取り、test_engine.py と test_engine_patch.py を CI=true と ENGINE_DIR を渡し、
pytest を入れて飛ばさずに走らせる形かを見る（差分が ENGINE_REF に当たらないか、差分で足したテストが落ちればジョブが落ちる）。
テストは unittest discover でまとめて走らせて出力を残し、次の手順がその出力の skipped が 1 件以上なら落とす（議決 D-0037、Issue #1080）。
.github/workflows/knowledge-enforcement.yml に写してあれば、その写しも同じ形かと、ENGINE_REF の行のほかがひな形と同じかも見る。
写しがないときは、CI（CI=true）か、K-*.yaml の wired: true の解決策が wired_by に写しを書いているときは飛ばさずに落とす（建設者が写したあとで
写しを消した・名前を変えた PR を止める。議決 D-0053、Issue #1144）。どちらでもない手元では、まだ写していないとして飛ばす。
形の検査（problems）が外れを見逃さないことは、ひな形を書き換えた写しで確かめる。
ひな形と写しの ENGINE_REF は、形の検査とは別に knowledge-check.yml の ENGINE_REF と字面で照らし、片方だけ上げたら落とす
（写しがなくても飛ばさない。議決 D-0053、Issue #1143）。
skipped を見る手順はひな形から走らせたときしか効かないので、test_engine.py と test_engine_patch.py そのものが CI=true か ENGINE_DIR を渡したときに
Engine のチェックアウトや ENGINE_REF の版がなくても飛ばさずに落ちることを、Engine のない一時の置き場で子のプロセスとして走らせて確かめ、
どちらも渡さないときだけ飛ぶことを対照にする（議決 D-0037 の ci_check の案。議決 D-0055、Issue #1152）。

  python3 -m unittest discover -s tools/knowledge-enforcement-check
"""
from __future__ import annotations

import importlib.util
import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import yaml

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
TEMPLATE = HERE / "ci" / "knowledge-enforcement.yml"
COPY_REL = ".github/workflows/knowledge-enforcement.yml"
COPY = REPO / COPY_REL
IN_CI = os.environ.get("CI") == "true"
JOB = "engine-test"
TESTS = ("tools/knowledge-enforcement-check/test_engine.py", "tools/knowledge-enforcement-check/test_engine_patch.py")
REF_LINE = re.compile(r"^\s*ENGINE_REF:\s*\S+\s*$", re.M)
DIR = "tools/knowledge-enforcement-check"
LOG = "knowledge-enforcement-unittest.log"
REF_VALUE = re.compile(r"^\s*ENGINE_REF:\s*['\"]?([^'\"\s#]*)['\"]?\s*(?:#.*)?$", re.M)

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


def copy_claims(items: list[dict]) -> list[str]:
    """wired: true の解決策のうち、wired_by に写し（COPY_REL）を書いたもの（「K-0004 の issue 1063」の形）"""
    out = []
    for k in items:
        for s in check.solutions(k):
            wb = s.get("wired_by") if isinstance(s, dict) and s.get("wired") is True else None
            if isinstance(wb, list) and any(isinstance(w, dict) and isinstance(w.get("workflow"), str) and check.norm("", w["workflow"]) == COPY_REL
                                            for w in wb):
                out.append(f"{k.get('id')} の issue {s.get('issue')}")
    return out


def copy_missing(exists: bool, in_ci: bool, claims: list[str]) -> str | None:
    """写しがないときに落とす理由。None なら（写しがあるか、まだ写していない手元なので）落とさない（議決 D-0053、Issue #1144）"""
    if exists:
        return None
    if in_ci:
        return "CI（CI=true）では写しがないまま飛ばさない"
    if claims:
        return f"{'・'.join(claims)} の解決策が wired_by に写しを書いている"
    return None


class CopyMissingTest(unittest.TestCase):
    """写しがないときに飛ばすか落とすか（議決 D-0053、Issue #1144）"""

    def item(self, wired=True, workflow=COPY_REL, kid="K-0004", issue=1063) -> dict:
        return {"id": kid, "enforcement": {"level": "deterministic", "solutions": [
            {"kind": "ci_check", "issue": issue, "wired": wired, "wired_by": [{"workflow": workflow, "job": JOB, "runs": TESTS[1]}]}]}}

    def test_写しがあれば落とさない(self):
        self.assertIsNone(copy_missing(True, True, ["K-0004 の issue 1063"]))

    def test_CI_では写しがなければ落とす(self):
        self.assertIn("CI=true", copy_missing(False, True, []))

    def test_記録が写しを配線先に書いていれば落とす(self):
        why = copy_missing(False, False, copy_claims([self.item(), self.item(kid="K-0010", issue=1035)]))
        self.assertIn("K-0004 の issue 1063・K-0010 の issue 1035", why)
        self.assertIsNotNone(copy_missing(False, False, copy_claims([self.item(workflow="./" + COPY_REL)])))

    def test_手元でまだ写していなければ飛ばす(self):
        self.assertIsNone(copy_missing(False, False, []))
        claims = copy_claims([self.item(wired=False), self.item(workflow=".github/workflows/knowledge-check.yml"),
                              {"id": "K-0001", "enforcement": {"level": "context"}}, {"id": "K-0002"}])
        self.assertEqual(claims, [])
        self.assertIsNone(copy_missing(False, False, claims))


def ref_mismatches(repo: Path = REPO) -> list[str]:
    """ひな形と（あれば）写しのうち、ENGINE_REF が knowledge-check.yml の ENGINE_REF と同じ版でないもの（議決 D-0053、Issue #1143）"""
    ref = patch_check.engine_ref(repo / patch_check.WORKFLOW.relative_to(REPO))
    e = []
    for path, required in ((repo / TEMPLATE.relative_to(REPO), True), (repo / COPY.relative_to(REPO), False)):
        if not required and not path.is_file():
            continue
        refs = REF_VALUE.findall(path.read_text(encoding="utf-8"))
        if refs != [ref]:
            e.append(f"{path.relative_to(repo)} の ENGINE_REF（{', '.join(refs) or 'なし'}）が knowledge-check.yml の {ref} と同じ版でない。"
                     "ENGINE_REF は knowledge-check.yml・ひな形・写しの 3 つを同じ PR でそろえて上げる")
    return e


class EngineRefTest(unittest.TestCase):
    """ENGINE_REF を knowledge-check.yml とひな形（写し）の片方だけ上げた PR を止める（知見 K-0004 の担保。議決 D-0053、Issue #1143）"""

    def test_ひな形と写しのENGINE_REFはknowledge_checkと同じ版(self):
        self.assertEqual(ref_mismatches(), [])

    def test_ENGINE_REFを片方だけ上げると落ちる(self):
        ref, new = patch_check.engine_ref(), "a" * 40
        texts = {patch_check.WORKFLOW: patch_check.WORKFLOW.read_text(encoding="utf-8"),
                 TEMPLATE: TEMPLATE.read_text(encoding="utf-8")}
        texts[COPY] = texts[TEMPLATE]   # 写しはひな形と同じ中身で置く
        cases = [
            # （knowledge-check.yml・ひな形・写しの ENGINE_REF（None は写しなし）、落ちるか）
            ((ref, ref, None), False), ((new, new, None), False), ((new, new, new), False), ((ref, ref, ref), False),
            ((new, ref, None), True), ((ref, new, None), True),
            ((new, new, ref), True), ((ref, ref, new), True), ((new, ref, new), True),
            ((ref, f"'{ref}'", None), False), ((ref, f"{ref}  # 版", None), False),
        ]
        for refs, fails in cases:
            with self.subTest(refs), tempfile.TemporaryDirectory() as d:
                for (src, text), value in zip(texts.items(), refs):
                    if value is None:
                        continue
                    dst = Path(d) / src.relative_to(REPO)
                    dst.parent.mkdir(parents=True, exist_ok=True)
                    dst.write_text(REF_LINE.sub(f"  ENGINE_REF: {value}", text), encoding="utf-8")
                self.assertEqual(bool(ref_mismatches(Path(d))), fails, ref_mismatches(Path(d)))

    def test_ENGINE_REFがないか2行あると落ちる(self):
        template = TEMPLATE.read_text(encoding="utf-8")
        for name, text in {"なし": REF_LINE.sub("", template),
                           "2 行": template.replace("        env:\n          CI: \"true\"\n",
                                                   f"        env:\n          ENGINE_REF: {patch_check.engine_ref()}\n          CI: \"true\"\n", 1)}.items():
            with self.subTest(name), tempfile.TemporaryDirectory() as d:
                self.assertNotEqual(text, template)
                for src, body in ((patch_check.WORKFLOW, patch_check.WORKFLOW.read_text(encoding="utf-8")), (TEMPLATE, text)):
                    dst = Path(d) / src.relative_to(REPO)
                    dst.parent.mkdir(parents=True, exist_ok=True)
                    dst.write_text(body, encoding="utf-8")
                self.assertNotEqual(ref_mismatches(Path(d)), [])


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

    def test_外れた写しは落ちる(self):
        t = self.text
        engine_step = f"          python3 -B -m unittest discover -s {DIR} -v 2>&1 | tee {LOG}\n"
        self.assertIn(engine_step, t)
        skip_step = "      - name: 飛ばしたテストがないか（K-0004）\n"
        self.assertIn(skip_step, t)
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
            why = copy_missing(False, IN_CI, copy_claims(check.load(REPO)))
            if why:
                self.fail(f"{COPY_REL} がない（{why}）。写しを消さない（議決 D-0053、Issue #1144）")
            self.skipTest(f"{COPY_REL} にまだ写していない（建設者が写す）")
        text = COPY.read_text(encoding="utf-8")
        self.assertEqual(problems(text, self.ref), [])
        self.assertEqual(REF_LINE.sub("ENGINE_REF:", text), REF_LINE.sub("ENGINE_REF:", self.text))


class NoSkipTest(unittest.TestCase):
    """test_engine.py と test_engine_patch.py が、CI=true か ENGINE_DIR を渡したときは Engine の写しや ENGINE_REF の版がなくても
    飛ばさずに落ちることを、Engine のない一時の置き場で子のプロセスとして走らせて確かめる（知見 K-0004 の担保。議決 D-0037・D-0055、Issue #1152）。
    飛ばす条件がゆるめられると、ひな形の skipped を見る手順に頼るだけになり、写しのない手元や別の呼び方では黙って緑になる"""

    MODULES = ("test_engine", "test_engine_patch")
    FILES = (f"{DIR}/check.py", f"{DIR}/test_engine.py", f"{DIR}/test_engine_patch.py",
             f"{DIR}/engine/knowledge-enforcement.patch", ".github/workflows/knowledge-check.yml")
    ENV_KEYS = ("CI", "ENGINE_DIR", "ENGINE_PATCH_REF", "GITHUB_ACTIONS")

    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        root = Path(cls.tmp.name)
        cls.repo = root / "repo"   # 隣の root/engine は置かないので、ENGINE_DIR を渡さなければ Engine はない
        for rel in cls.FILES:
            (cls.repo / rel).parent.mkdir(parents=True, exist_ok=True)
            (cls.repo / rel).write_bytes((REPO / rel).read_bytes())
        cls.empty = root / "empty"
        cls.empty.mkdir()
        # Engine のファイルはあるが ENGINE_REF の版がないチェックアウト
        cls.other = root / "other"
        for f in ("core-gates/knowledge_check.py", "definitions/knowledge.yaml", "core-gates/tests/test_knowledge_check.py"):
            (cls.other / f).parent.mkdir(parents=True, exist_ok=True)
            (cls.other / f).write_text("# x\n", encoding="utf-8")
        git = {**os.environ, "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@example.invalid",
               "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@example.invalid"}
        for args in (["init", "-q"], ["add", "-A"], ["commit", "-q", "-m", "x"]):
            subprocess.run(["git", "-C", str(cls.other), *args], env=git, capture_output=True, check=True)

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def run_module(self, module: str, **env: str) -> subprocess.CompletedProcess:
        base = {k: v for k, v in os.environ.items() if k not in self.ENV_KEYS}
        return subprocess.run([sys.executable, "-B", "-m", "unittest", "-v", module], cwd=self.repo / DIR,
                              env={**base, **env}, capture_output=True, text=True, timeout=120)

    def test_CIかENGINE_DIRを渡すとEngineがなくても飛ばさずに落ちる(self):
        cases = {
            "CI=true（隣の ../engine もない）": {"CI": "true"},
            "ENGINE_DIR が空のディレクトリ": {"ENGINE_DIR": str(self.empty)},
            "ENGINE_DIR がないディレクトリ": {"ENGINE_DIR": str(self.repo / "nothing")},
            "ENGINE_DIR に ENGINE_REF の版がない": {"ENGINE_DIR": str(self.other)},
            "CI=true と ENGINE_DIR に ENGINE_REF の版がない": {"CI": "true", "ENGINE_DIR": str(self.other)},
        }
        for name, env in cases.items():
            for module in self.MODULES:
                with self.subTest(name, module=module):
                    p = self.run_module(module, **env)
                    out = p.stdout + p.stderr
                    self.assertNotEqual(p.returncode, 0, out)
                    self.assertRegex(out, r"(?m)^FAILED \((failures|errors)=[1-9]", out)
                    self.assertNotIn("skipped", out, out)
                    self.assertIn("Engine のチェックアウト", out, out)   # 読み込みの誤りなどほかの理由で落ちたのではない

    def test_どちらも渡さずEngineがなければ飛ばす(self):
        # 上のテストが「飛ばした」を見分けられることと、一時の置き場に本当に Engine がないことを確かめる対照
        for module in self.MODULES:
            with self.subTest(module=module):
                p = self.run_module(module)
                out = p.stdout + p.stderr
                self.assertEqual(p.returncode, 0, out)
                self.assertRegex(out, r"(?m)^OK \(skipped=[1-9][0-9]*\)$", out)


if __name__ == "__main__":
    unittest.main()
