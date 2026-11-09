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
（写しがなくても飛ばさない。議決 D-0053、Issue #1143）。knowledge-check.yml が ENGINE_REF を .factory-config/engine.lock から読む形（Engine ADR 0041）なら、
engine.lock の版と照らし、engine.lock だけを上げた PR も落とす（議決 D-0080（起票時の番号は D-0078）、Issue #1225）。
skipped を見る手順はひな形から走らせたときしか効かないので、test_engine.py と test_engine_patch.py そのものが CI=true か ENGINE_DIR を渡したときに
Engine のチェックアウトや ENGINE_REF の版がなくても飛ばさずに落ちることを、Engine のない一時の置き場で子のプロセスとして走らせて確かめ、
どちらも渡さないときだけ飛ぶことを対照にする（議決 D-0037 の ci_check の案。議決 D-0055、Issue #1152）。
このディレクトリで unittest.TestCase を定義する .py がすべて unittest discover の既定の形（test*.py、下のディレクトリは __init__.py のあるパッケージ）に
合うかを照らし、ひな形の engine-test は飛ばした件数の手順のすぐ後で test_ci.py --log knowledge-enforcement-unittest.log を走らせ、
拾われないファイルか、その出力にテストが 1 件も（飛ばさずに）走った跡のないファイルがあれば落とす（議決 D-0057（起票時の番号は D-0055）、Issue #1154）。
ひな形の enforcement のジョブが check.py の結果で落とす範囲が、check.py が呼ぶことを求める検査とワークフロー・K-*.yaml の wired_by を覆うことと、
配線先のジョブから REQUIRED_RUNS の検査を外した PR でその手順が落ちることを、手順を bash で動かして確かめる（議決 D-0066（起票時の番号は D-0062）、Issue #1173）。
落とす範囲は、check.py の REQUIRED_* の一覧と K-*.yaml の requires_check が指す検査の置き場（ディレクトリ）から導いた道でも照らし、
check.py に新しい置き場の検査を足して範囲に足し忘れたら落とす（議決 D-0090（Issue #1259 の起票時の番号は D-0089））。

  python3 -m unittest discover -s tools/knowledge-enforcement-check
  python3 tools/knowledge-enforcement-check/test_ci.py --log knowledge-enforcement-unittest.log   # 出力に走った跡があるか
"""
from __future__ import annotations

import ast
import importlib.util
import os
import re
import subprocess
import sys
import tempfile
import types
import unittest
from fnmatch import fnmatchcase
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
LOG_STEP = re.compile(rf"^\s*python3\s+(?:-B\s+)?{re.escape(DIR)}/test_ci\.py\s+--log\s+{re.escape(LOG)}\s*$", re.M)
PATTERN = "test*.py"   # unittest discover の既定の -p
LOG_HEAD = re.compile(r"^(test\S*) \(([\w.]+)\)", re.M)   # unittest -v の 1 件の行（3.11 からは mod.Class.test、それより前は mod.Class）

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
    runs = [s for s in steps if DIR in (s.get("run") or "") and not LOG_STEP.search(s.get("run") or "")]
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
    log_steps = [x for x in steps if LOG_STEP.search(x.get("run") or "")]
    if len(log_steps) != 1 or len(after) < 2 or log_steps[0] is not after[1]:
        e.append(f"飛ばした件数の手順のすぐ後に、python3 -B {DIR}/test_ci.py --log {LOG} で拾われないファイル・走らなかったファイルを落とす手順を 1 つ置く")
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
    """ひな形と（あれば）写しのうち、ENGINE_REF が knowledge-check.yml の ENGINE_REF と同じ版でないもの（議決 D-0053、Issue #1143）。
    knowledge-check.yml が .factory-config/engine.lock を読む形なら、その版と照らす（test_engine_patch.py の engine_ref()。議決 D-0080、Issue #1225）"""
    ref = patch_check.engine_ref(repo / patch_check.WORKFLOW.relative_to(REPO))
    e = []
    for path, required in ((repo / TEMPLATE.relative_to(REPO), True), (repo / COPY.relative_to(REPO), False)):
        if not required and not path.is_file():
            continue
        refs = REF_VALUE.findall(path.read_text(encoding="utf-8"))
        if refs != [ref]:
            e.append(f"{path.relative_to(repo)} の ENGINE_REF（{', '.join(refs) or 'なし'}）が knowledge-check.yml の {ref} と同じ版でない。"
                     "ENGINE_REF は knowledge-check.yml（が読む .factory-config/engine.lock）・ひな形・写しの 3 つを同じ PR でそろえて上げる")
    return e


class EngineRefTest(unittest.TestCase):
    """ENGINE_REF を knowledge-check.yml とひな形（写し）の片方だけ上げた PR を止める（知見 K-0004 の担保。議決 D-0053、Issue #1143）"""

    def test_ひな形と写しのENGINE_REFはknowledge_checkと同じ版(self):
        self.assertEqual(ref_mismatches(), [])

    def test_ENGINE_REFを片方だけ上げると落ちる(self):
        ref, new = patch_check.engine_ref(), "a" * 40
        check_text = patch_check.WORKFLOW.read_text(encoding="utf-8")
        # knowledge-check.yml の版は、env の ENGINE_REF の行の形でも、.factory-config/engine.lock を読む形（Engine ADR 0041）でも上げる（Issue #1225）
        forms = {"engine.lock を読む形": (check_text if patch_check.LOCK_REL in check_text and not REF_LINE.search(check_text)
                                          else f"run: |\n  lock={patch_check.LOCK_REL}\n", True),
                 "env の ENGINE_REF の行": (f"env:\n  ENGINE_REF: {ref}\n", False)}
        template = TEMPLATE.read_text(encoding="utf-8")
        cases = [
            # （knowledge-check.yml・ひな形・写しの ENGINE_REF（None は写しなし）、落ちるか）
            ((ref, ref, None), False), ((new, new, None), False), ((new, new, new), False), ((ref, ref, ref), False),
            ((new, ref, None), True), ((ref, new, None), True),
            ((new, new, ref), True), ((ref, ref, new), True), ((new, ref, new), True),
            ((ref, f"'{ref}'", None), False), ((ref, f"{ref}  # 版", None), False),
        ]
        for form, (check_body, by_lock) in forms.items():
            for (check_ref, *refs), fails in cases:
                with self.subTest(form=form, refs=(check_ref, *refs)), tempfile.TemporaryDirectory() as d:
                    files = {patch_check.WORKFLOW: check_body if by_lock else REF_LINE.sub(f"  ENGINE_REF: {check_ref}", check_body)}
                    if by_lock:
                        files[REPO / patch_check.LOCK_REL] = f"# 版\n{check_ref}\n"
                    for src, value in zip((TEMPLATE, COPY), refs):   # 写しはひな形と同じ中身で置く
                        if value is not None:
                            files[src] = REF_LINE.sub(f"  ENGINE_REF: {value}", template)
                    for src, body in files.items():
                        dst = Path(d) / src.relative_to(REPO)
                        dst.parent.mkdir(parents=True, exist_ok=True)
                        dst.write_text(body, encoding="utf-8")
                    self.assertEqual(bool(ref_mismatches(Path(d))), fails, ref_mismatches(Path(d)))

    def test_ENGINE_REFがないか2行あると落ちる(self):
        template = TEMPLATE.read_text(encoding="utf-8")
        for name, text in {"なし": REF_LINE.sub("", template),
                           "2 行": template.replace("        env:\n          CI: \"true\"\n",
                                                   f"        env:\n          ENGINE_REF: {patch_check.engine_ref()}\n          CI: \"true\"\n", 1)}.items():
            with self.subTest(name), tempfile.TemporaryDirectory() as d:
                self.assertNotEqual(text, template)
                for src, body in ((patch_check.WORKFLOW, patch_check.WORKFLOW.read_text(encoding="utf-8")), (TEMPLATE, text),
                                  (REPO / patch_check.LOCK_REL, f"{patch_check.engine_ref()}\n")):
                    dst = Path(d) / src.relative_to(REPO)
                    dst.parent.mkdir(parents=True, exist_ok=True)
                    dst.write_text(body, encoding="utf-8")
                self.assertNotEqual(ref_mismatches(Path(d)), [])


def defines_testcase(path: Path) -> bool:
    """ファイルが unittest.TestCase（を継いだ同じファイルのクラス）を継ぐクラスを定義するか。名前の末尾が TestCase の基底を数える"""
    tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    local: set[str] = set()
    for node in ast.walk(tree):
        if not isinstance(node, ast.ClassDef):
            continue
        for b in node.bases:
            name = b.attr if isinstance(b, ast.Attribute) else b.id if isinstance(b, ast.Name) else ""
            if name.endswith("TestCase") or name in local:
                local.add(node.name)
                break
    return bool(local)


def discovered_module(root: Path, path: Path) -> str | None:
    """unittest discover -s root（-p は既定の test*.py）が path を拾うときのモジュール名。拾わなければ None"""
    rel = path.relative_to(root)
    if not fnmatchcase(rel.name, PATTERN) or not unittest.loader.VALID_MODULE_NAME.match(rel.name):
        return None
    d = root
    for part in rel.parts[:-1]:   # 下のディレクトリは __init__.py のあるパッケージのときだけ辿る
        d = d / part
        if not part.isidentifier() or not (d / "__init__.py").is_file():
            return None
    return ".".join(rel.parts[:-1] + (rel.stem,))


def testcase_files(root: Path = HERE) -> list[Path]:
    return sorted(p for p in root.rglob("*.py") if "__pycache__" not in p.parts and defines_testcase(p))


def discovery_problems(root: Path = HERE) -> list[str]:
    """TestCase を定義するのに unittest discover に拾われないファイル（議決 D-0057、Issue #1154）"""
    return [f"{p.relative_to(root)} は unittest.TestCase を定義するが、unittest discover の既定の形（{PATTERN}・"
            "__init__.py のあるパッケージの下）に合わず拾われない" for p in testcase_files(root) if discovered_module(root, p) is None]


def ran_modules(log: str) -> set[str]:
    """unittest -v の出力で、飛ばさずに走ったテストが 1 件以上あるモジュール（の名前を含む点区切りの名前）"""
    heads, out = list(LOG_HEAD.finditer(log)), set()
    for i, m in enumerate(heads):
        body = log[m.end():heads[i + 1].start() if i + 1 < len(heads) else len(log)]
        result = re.search(r" \.\.\. (\S+)", body)   # docstring のあるテストは結果が次の行に出る
        if result and result.group(1) != "skipped":
            out.add(m.group(2))
    return out


def log_problems(log: str, root: Path = HERE) -> list[str]:
    """拾われないファイルと、unittest discover の出力（LOG）にテストが 1 件も走った跡のないファイル（議決 D-0057、Issue #1154）"""
    e, ran = discovery_problems(root), ran_modules(log)
    for p in testcase_files(root):
        mod = discovered_module(root, p)
        # 3.11 からは mod.Class.test、それより前は mod.Class。setUpClass などの失敗の行は test で始まらないので数えない
        if mod and not any(r.startswith(mod + ".") and r[len(mod) + 1:].count(".") <= 1 for r in ran):
            e.append(f"{p.relative_to(root)} のテストが {LOG} に 1 件も走った跡がない（読み込めなかった・すべて飛ばした・走らせていない）")
    return e


class DiscoveryTest(unittest.TestCase):
    """TestCase を定義するファイルがすべて unittest discover に拾われ、CI の出力に走った跡があるか（知見 K-0004 の担保。議決 D-0057、Issue #1154）"""

    def tree(self, d: str, files: dict[str, str]) -> Path:
        root = Path(d)
        for rel, body in files.items():
            (root / rel).parent.mkdir(parents=True, exist_ok=True)
            (root / rel).write_text(body, encoding="utf-8")
        return root

    CASE = "import unittest\n\n\nclass T(unittest.TestCase):\n    def test_a(self):\n        pass\n"

    def test_このディレクトリのTestCaseはすべてdiscoverに拾われる(self):
        self.assertEqual(discovery_problems(), [])
        found = {p.name for p in testcase_files()}
        self.assertTrue({"test_check.py", "test_ci.py"} <= found, found)
        self.assertNotIn("check.py", found)

    def test_拾われないファイルは落ちる(self):
        cases = {
            "check_x.py": {"check_x.py": self.CASE},
            "x_test.py": {"x_test.py": self.CASE},
            "test-x.py": {"test-x.py": self.CASE},
            "TestCase を直に継ぐ": {"helper.py": "from unittest import TestCase\n\n\nclass T(TestCase):\n    pass\n"},
            "継いだクラスをさらに継ぐ": {"helper.py": self.CASE + "\n\nclass U(T):\n    pass\n"},
            "パッケージでない下のディレクトリ": {"sub/test_x.py": self.CASE},
        }
        for name, files in cases.items():
            with self.subTest(name), tempfile.TemporaryDirectory() as d:
                self.assertEqual(len(discovery_problems(self.tree(d, files))), 1, name)

    def test_拾われるファイルとTestCaseのないファイルは通る(self):
        with tempfile.TemporaryDirectory() as d:
            root = self.tree(d, {"test_x.py": self.CASE, "check.py": "import unittest\n\n\nclass X:\n    pass\n",
                                 "sub/__init__.py": "", "sub/test_y.py": self.CASE, "data/helper.py": "x = 1\n"})
            self.assertEqual(discovery_problems(root), [])
            self.assertEqual([discovered_module(root, p) for p in testcase_files(root)], ["sub.test_y", "test_x"])

    def test_ログに走った跡がないファイルは落ちる(self):
        ran = "Ran 3 tests in 0.010s\n\nOK\n"
        cases = [
            # （ログ、落ちるファイル）
            ("test_a (test_x.T.test_a) ... ok\ntest_a (sub.test_y.T.test_a) ... ok\n" + ran, []),
            ("test_a (test_x.T) ... ok\ntest_a (sub.test_y.T) ... FAIL\n" + ran, []),   # 3.10 までの形・落ちたテストも走った跡に数える
            ("test_a (test_x.T.test_a)\ndocstring の 1 行目 ... ok\ntest_a (sub.test_y.T.test_a) ... ERROR\n" + ran, []),
            ("test_a (test_x.T.test_a) ... ok\n" + ran, ["sub/test_y.py"]),
            ("test_a (test_x.T.test_a) ... ok\ntest_a (sub.test_y.T.test_a) ... skipped 'Engine がない'\n" + ran, ["sub/test_y.py"]),
            ("test_a (test_x.T.test_a) ... ok\nsetUpClass (sub.test_y.T) ... ERROR\n" + ran, ["sub/test_y.py"]),
            ("test_a (test_x.T.test_a) ... ok\nsub.test_y (unittest.loader._FailedTest.sub.test_y) ... ERROR\n" + ran, ["sub/test_y.py"]),
            ("test_a (test_x_more.T.test_a) ... ok\ntest_a (sub.test_y.T.test_a) ... ok\n" + ran, ["test_x.py"]),
            ("", ["sub/test_y.py", "test_x.py"]),
        ]
        with tempfile.TemporaryDirectory() as d:
            root = self.tree(d, {"test_x.py": self.CASE, "sub/__init__.py": "", "sub/test_y.py": self.CASE})
            for log, files in cases:
                with self.subTest(log):
                    e = log_problems(log, root)
                    self.assertEqual(sorted(f.split(" ")[0] for f in e), files, e)
            self.tree(d, {"helper.py": self.CASE})
            self.assertEqual(len(log_problems("test_a (test_x.T.test_a) ... ok\ntest_a (sub.test_y.T.test_a) ... ok\n" + ran, root)), 1)

    def test_ログの手順はこのファイルの_log_で落ちるか通る(self):
        ok = "".join(f"test_a ({discovered_module(HERE, p)}.T.test_a) ... ok\n" for p in testcase_files())
        for log, rc in ((ok + "Ran 5 tests in 0.1s\n\nOK\n", 0), (ok.split("\n", 1)[1], 1), (None, 1)):
            with self.subTest(log), tempfile.TemporaryDirectory() as d:
                path = Path(d) / LOG
                if log is not None:
                    path.write_text(log, encoding="utf-8")
                p = subprocess.run([sys.executable, "-B", str(HERE / "test_ci.py"), "--log", str(path)], capture_output=True, text=True)
                self.assertEqual(p.returncode, rc, p.stdout + p.stderr)
                if rc:
                    self.assertIn("::error title=knowledge-enforcement tests_not_discovered::", p.stdout)


def main_log(path: str) -> int:
    """ひな形の engine-test の手順から呼ぶ。拾われないファイルか、LOG に走った跡のないファイルがあれば 1"""
    p = Path(path)
    e = log_problems(p.read_text(encoding="utf-8")) if p.is_file() else [f"{path} がない（unittest discover の出力を残していない）"]
    for x in e:
        print(f"::error title=knowledge-enforcement tests_not_discovered::{x}")
    if not e:
        print(f"{len(testcase_files())} 件のテストのファイルがすべて拾われ、{path} に走った跡がある")
    return 1 if e else 0


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
        log_step = ("      - name: テストのファイルがすべて拾われて走ったか（K-0004）\n        shell: bash\n        run: |\n"
                    f"          python3 -B {DIR}/test_ci.py --log {LOG}\n")
        self.assertIn(log_step, t)
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
            "走った跡を照らさない": t.replace(log_step, ""),
            "走った跡を照らす手順が飛ばした件数の前": t.replace(log_step, "").replace(skip_step, log_step + skip_step, 1),
            "走った跡を別のログで照らす": t.replace(log_step, log_step.replace(f"--log {LOG}", "--log other.log")),
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


SCOPE_JOB = "enforcement"
CONFLICT_TEMPLATE = REPO / "tools/conflict-check/ci/conflict-check.yml"


# check.py が名前で並べた、呼ぶことを求める検査とワークフローの一覧の名前の頭。REQUIRED_SCHEDULE・REQUIRED_SCHEDULE_WORKFLOW・REQUIRED_RUNS と、
# いまは解決策の requires_check に移った REQUIRED_PR（議決 D-0077）を、一覧を足しても手で書き足さずに拾う（議決 D-0090（Issue #1259 の起票時の番号は D-0089））
REQUIRED_PREFIX = "REQUIRED_"
NAMED_REQUIRED = ("REQUIRED_SCHEDULE", "REQUIRED_SCHEDULE_WORKFLOW", "REQUIRED_RUNS")


def path_values(v) -> list[str]:
    """一覧の値（道の文字列か、その組・列・辞書の入れ子）のうち、リポジトリの根からの道の形の文字列"""
    if isinstance(v, str):
        return [v] if "/" in v and not re.search(r"\s", v) and check.norm("", v) == v else []
    if isinstance(v, dict):
        v = list(v.values())
    if isinstance(v, (list, tuple, set, frozenset)):
        return [p for x in v for p in path_values(x)]
    return []


def required_paths(mod=check) -> dict[str, list[str]]:
    """check.py の REQUIRED_* の一覧の名前 → その値が指す道（鍵の (知見の id, 解決策の issue) は見ない）"""
    out = {}
    for name, v in vars(mod).items():
        if name.startswith(REQUIRED_PREFIX) and isinstance(v, (dict, list, tuple, set, frozenset)):
            out[name] = list(dict.fromkeys(path_values(list(v.values()) if isinstance(v, dict) else v)))
    return out


def scope_places(items: list[dict], mod=check) -> list[str]:
    """check.py の REQUIRED_* と、K-*.yaml の解決策の requires_check（前の REQUIRED_PR）が指す検査とワークフローの置き場（ディレクトリ）"""
    paths = [p for ps in required_paths(mod).values() for p in ps]
    for k in items:
        for s in check.solutions(k):
            paths += [x for t, wf in (check.required_checks(s)[0] if isinstance(s, dict) else []) for x in (t, wf) if x]
    return sorted({p.rsplit("/", 1)[0] for p in paths})


def scope_targets(items: list[dict]) -> list[str]:
    """enforcement のジョブが落とす範囲に入れる道。check.py が呼ぶことを求める検査とそのワークフロー、K-*.yaml の wired_by の workflow・runs"""
    out = [f"{check.ROOT}/K-0010.yaml", check.KNOWLEDGE_CHECK, ".github/workflows/conflict-check.yml", COPY_REL]
    out += [p for ps in required_paths().values() for p in ps]
    for k in items:
        for s in check.solutions(k):
            # 解決策の requires_check に書いた検査とワークフロー（議決 D-0077（起票時の番号は D-0075）、Issue #1216）
            out += [x for t, wf in (check.required_checks(s)[0] if isinstance(s, dict) else []) for x in (t, wf) if x]
            for w in (s.get("wired_by") if isinstance(s, dict) and isinstance(s.get("wired_by"), list) else []):
                out += [check.norm("", w[key]) or w[key] for key in ("workflow", "runs") if isinstance(w, dict) and isinstance(w.get(key), str)]
    return list(dict.fromkeys(out))


class EnforcementScopeTest(unittest.TestCase):
    """ひな形の enforcement のジョブが、配線先や check.py が呼ぶことを求める検査を変えた PR でも check.py の結果で落ちるか
    （議決 D-0066（起票時の番号は D-0062）、Issue #1173）。範囲の手順と落とす手順をひな形から取り出して bash で動かす"""

    @classmethod
    def setUpClass(cls):
        steps = load(TEMPLATE.read_text(encoding="utf-8"))["jobs"][SCOPE_JOB]["steps"]
        cls.scope = next(s for s in steps if s.get("id") == "scope")
        cls.gate = next(s for s in steps if check.KNOWLEDGE_CHECK in (s.get("run") or "") and "CHANGED" in (s.get("env") or {}))

    def changed(self, files: list[str]) -> str:
        with tempfile.TemporaryDirectory() as d:
            bin_ = Path(d) / "bin"
            bin_.mkdir()
            (Path(d) / "diff.txt").write_text("".join(f"{f}\n" for f in files), encoding="utf-8")
            git = bin_ / "git"
            git.write_text(f"#!/bin/sh\ncat '{d}/diff.txt'\n", encoding="utf-8")
            git.chmod(0o755)
            out = Path(d) / "out"
            p = subprocess.run(["bash", "--noprofile", "--norc", "-e", "-c", self.scope["run"]], cwd=d, capture_output=True, text=True,
                               env={"PATH": f"{bin_}:/usr/bin:/bin", "PR_BASE": "main", "GITHUB_OUTPUT": str(out)})
            self.assertEqual(p.returncode, 0, p.stderr)
            return out.read_text(encoding="utf-8").strip()

    def test_check_pyが呼ぶことを求める検査とwired_byの道を変えたPRは落とす範囲に入る(self):
        for f in scope_targets(check.load(REPO)):
            with self.subTest(f):
                self.assertEqual(self.changed([f]), "changed=true")

    def test_ほかの道だけを変えたPRは落とす範囲に入らない(self):
        for f in ("apps/backend/src/main.rs", "CHANGELOG.md", "specs/glossary.yaml", "tools/conflict-check-x/a.py", "docs/.github/workflows/a.yml"):
            with self.subTest(f):
                self.assertEqual(self.changed([f]), "changed=false")
        self.assertEqual(self.changed(["CHANGELOG.md", "tools/conflict-check/test_watch.py"]), "changed=true")

    def test_check_pyのREQUIRED_の検査の置き場はどれも落とす範囲に入る(self):
        # 挙げた道そのものだけでなく、同じ置き場に足した検査や下のディレクトリを変えた PR も範囲に入る（議決 D-0090（Issue #1259 の起票時の番号は D-0089））。
        # check.py に新しい置き場の検査を足して、ひな形の範囲に足し忘れるとここで落ちる
        for place in scope_places(check.load(REPO)):
            for f in (f"{place}/new_check.py", f"{place}/sub/new_check.mjs"):
                with self.subTest(place=place, path=f):
                    self.assertEqual(self.changed([f]), "changed=true")

    def test_REQUIRED_の一覧から置き場を導ける(self):
        # 名前が変わったり値の形が変わったりして、導いた置き場が黙って空にならないことの対照
        got = required_paths()
        for name in NAMED_REQUIRED:
            with self.subTest(name):
                self.assertTrue(got.get(name), f"check.py の {name} から道を 1 つも読めない")
        places = scope_places(check.load(REPO))
        for place in (".github/workflows", "tools/conflict-check", "tools/sendback-check", "tools/pr-body-check"):
            self.assertIn(place, places)

    def test_check_pyに足した新しい置き場を範囲に足し忘れると見分ける(self):
        # 上のテストが足し忘れを落とせることの対照。REQUIRED_SCHEDULE と、もう一度名前で並べた REQUIRED_PR に別の置き場の検査を足した check.py の形
        fake = types.SimpleNamespace(**{k: v for k, v in vars(check).items() if k.startswith(REQUIRED_PREFIX)})
        fake.REQUIRED_SCHEDULE = {**check.REQUIRED_SCHEDULE, ("K-0002", 1): "tools/changelog-check/protection.mjs"}
        fake.REQUIRED_PR = {("K-0009", 1): "tools/continue-input-check/test_check.py"}
        places = scope_places([], fake)
        for place in ("tools/changelog-check", "tools/continue-input-check"):
            with self.subTest(place):
                self.assertIn(place, places)
                self.assertEqual(self.changed([f"{place}/new_check.py"]), "changed=false")

    def gate_rc(self, workflow: str, changed: str) -> int:
        """K-0010 の issue 1034 を wired: true にした一時のリポジトリで、落とす手順を動かした終了コード"""
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            for rel, body in {
                check.KNOWLEDGE_CHECK: (HERE / "check.py").read_text(encoding="utf-8"),
                ".github/workflows/conflict-check.yml": workflow,
                # wired: true の解決策はどれも enforcement のジョブの check.py の配線を求める（議決 D-0072、Issue #1201）ので、ひな形を写して置く
                check.KNOWLEDGE_ENFORCEMENT_WORKFLOW: TEMPLATE.read_text(encoding="utf-8"),
                "tools/conflict-check/test_watch.py": "", "tools/conflict-check/test_engine.py": "",
                f"{check.ROOT}/K-0010.yaml": yaml.safe_dump({"id": "K-0010", "enforcement": {"level": "deterministic", "solutions": [{
                    "kind": "ci_check", "issue": 1034, "wired": True, "verified": False,
                    "wired_by": [{"workflow": ".github/workflows/conflict-check.yml", "job": "conflict-check", "runs": r} for r in check.CONFLICT_TESTS]
                    + [{"workflow": check.KNOWLEDGE_ENFORCEMENT_WORKFLOW, "job": SCOPE_JOB, "runs": check.KNOWLEDGE_CHECK}]}]}},
                    allow_unicode=True),
            }.items():
                (root / rel).parent.mkdir(parents=True, exist_ok=True)
                (root / rel).write_text(body, encoding="utf-8")
            # 手順は check.py に --base "origin/$PR_BASE" を渡すので、CI（fetch-depth: 0）と同じく PR の base の版を置く（議決 D-0067、Issue #1178）
            for cmd in (["init", "-q"], ["add", "-A"], ["-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "-q", "-m", "base"],
                        ["update-ref", "refs/remotes/origin/main", "HEAD"]):
                subprocess.run(["git", *cmd], cwd=d, check=True, capture_output=True)
            p = subprocess.run(["bash", "--noprofile", "--norc", "-e", "-c", self.gate["run"]], cwd=d, capture_output=True, text=True,
                               env={"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "CHANGED": changed, "PR_BASE": "main",
                                    "GITHUB_STEP_SUMMARY": str(root / "summary.md")})
            return p.returncode

    def test_配線先のジョブからREQUIRED_RUNSの検査を外したPRは落ちる(self):
        t = CONFLICT_TEMPLATE.read_text(encoding="utf-8")
        line = "python3 -B -m unittest discover -s tools/conflict-check -p 'test_*.py' -v"
        self.assertIn(line, t)
        dropped = t.replace(line, "python3 -B tools/conflict-check/test_policy.py -v")
        self.assertEqual(self.gate_rc(t, "true"), 0)
        self.assertNotEqual(self.gate_rc(dropped, "true"), 0)
        self.assertEqual(self.gate_rc(dropped, "false"), 0)   # 範囲の外の PR では結果を残すだけ（対照）
        self.assertEqual(self.changed([".github/workflows/conflict-check.yml"]), "changed=true")


class NoSkipTest(unittest.TestCase):
    """test_engine.py と test_engine_patch.py が、CI=true か ENGINE_DIR を渡したときは Engine の写しや ENGINE_REF の版がなくても
    飛ばさずに落ちることを、Engine のない一時の置き場で子のプロセスとして走らせて確かめる（知見 K-0004 の担保。議決 D-0037・D-0055、Issue #1152）。
    飛ばす条件がゆるめられると、ひな形の skipped を見る手順に頼るだけになり、写しのない手元や別の呼び方では黙って緑になる"""

    MODULES = ("test_engine", "test_engine_patch")
    FILES = (f"{DIR}/check.py", f"{DIR}/test_engine.py", f"{DIR}/test_engine_patch.py",
             f"{DIR}/engine/knowledge-enforcement.patch", ".github/workflows/knowledge-check.yml",
             patch_check.LOCK_REL)   # knowledge-check.yml が ENGINE_REF を読む（Issue #1225）
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
    if sys.argv[1:2] == ["--log"] and len(sys.argv) == 3:
        sys.exit(main_log(sys.argv[2]))
    unittest.main()
