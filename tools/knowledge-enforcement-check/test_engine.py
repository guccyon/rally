"""Engine の knowledge_check.py に担保の表示を移す差分のテスト（議決 D-0016・D-0017、Issue #965・#968）。

engine/knowledge-enforcement.patch を Engine のチェックアウトの写しに当て、INDEX.md の担保の列と solutions の真偽の検査が
check.py と同じ線になること、差分で Engine の core-gates/tests/test_knowledge_check.py に足したテストが通ること（pytest があるとき）を
確かめる。Engine がもう差分を取り込んでいれば（enforcement_label がある）当てずにそのまま確かめる。
Engine のチェックアウトは環境変数 ENGINE_DIR で渡す（なければリポジトリの隣の ../engine）。どちらもなければ飛ばす。
ただし ENGINE_DIR を渡したときと CI（CI=true）では飛ばさずに落とす（sendback-check/engine.test.mjs と同じ）。

  python3 -m unittest discover -s tools/knowledge-enforcement-check
"""
from __future__ import annotations

import importlib.util
import itertools
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from datetime import date
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
PATCH = HERE / "engine" / "knowledge-enforcement.patch"
FILES = ("core-gates/knowledge_check.py", "definitions/knowledge.yaml", "core-gates/tests/test_knowledge_check.py")

spec = importlib.util.spec_from_file_location("knowledge_enforcement_check_e", HERE / "check.py")
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)  # type: ignore[union-attr]

ENGINE = Path(os.environ.get("ENGINE_DIR") or REPO.parent / "engine")
REQUIRED = bool(os.environ.get("ENGINE_DIR")) or os.environ.get("CI") == "true"
HAS_ENGINE = all((ENGINE / f).is_file() for f in FILES)
MISSING = object()  # 総当たりで「項目を書いていない」を表す


def sol(**flags):
    return {"kind": "test", "what": "x", "issue": 1, **flags}


def k(level="deterministic", *sols):
    return {"id": "K-9999", "_file": "K-9999.yaml", "enforcement": {"level": level, "solutions": list(sols)}}


@unittest.skipUnless(HAS_ENGINE or REQUIRED, f"Engine のチェックアウトがない（ENGINE_DIR={ENGINE}）")
class EnginePatchTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not HAS_ENGINE:
            raise AssertionError(f"Engine のチェックアウトがない（ENGINE_DIR={ENGINE}）")
        cls.tmp = tempfile.TemporaryDirectory()
        cls.dir = Path(cls.tmp.name)
        for f in FILES:
            (cls.dir / f).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy(ENGINE / f, cls.dir / f)
        src = (cls.dir / FILES[0]).read_text(encoding="utf-8")
        cls.merged = "def enforcement_label(" in src
        if not cls.merged:
            r = subprocess.run(["git", "apply", str(PATCH)], cwd=cls.dir, capture_output=True, text=True)
            if r.returncode != 0:
                raise AssertionError(f"差分が Engine（{ENGINE}）に当たらない。今の Engine に合わせて作り直す: {r.stderr}")
        s = importlib.util.spec_from_file_location("engine_knowledge_check", cls.dir / FILES[0])
        cls.kc = importlib.util.module_from_spec(s)
        s.loader.exec_module(cls.kc)  # type: ignore[union-attr]

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def test_担保の表示が_check_py_の_label_と同じ(self):
        cases = [
            k("context"), k("checklist", sol(wired=True, verified=True)), {"id": "K-9999"},
            k("deterministic"), k("deterministic", sol()), k("deterministic", sol(wired=False, verified=False)),
            k("deterministic", sol(wired=True)), k("deterministic", sol(wired=True, verified=False), sol(wired=False)),
            k("deterministic", sol(wired=False), sol(wired=True, verified=True)), k("deterministic", sol(wired="true", verified=True)),
        ]
        for x in cases + check.load(REPO):
            with self.subTest(x.get("id"), enforcement=x.get("enforcement")):
                self.assertEqual(self.kc.enforcement_label(x), check.label(x))

    def test_担保の表示が_check_py_の_label_と総当たりで同じ(self):
        # 議決 D-0024、Issue #1015。解決策 1 件の状態は wired・verified の値の直積（未記入・true・false・真偽でない値）。
        # その状態を 0〜2 件並べた組（順序も変える）と、dict でない項目を混ぜた組を、強さと enforcement の形ごとに渡す
        values = (MISSING, True, False, "true", 1, None)
        states = [{f: v for f, v in (("wired", w), ("verified", v)) if v is not MISSING} for w, v in itertools.product(values, repeat=2)]
        groups = [[]] + [[s] for s in states] + [list(p) for p in itertools.product(states, repeat=2)]
        groups += [[s, "x"] for s in states] + [["x"], [None]]
        cases = [{"id": "K-9999"}, {"id": "K-9999", "enforcement": None}, {"id": "K-9999", "enforcement": {"level": "deterministic"}},
                 {"id": "K-9999", "enforcement": {"level": "deterministic", "solutions": None}}]
        cases += [k(level, *(sol(**s) if isinstance(s, dict) else s for s in g)) for level in ("deterministic", "context", "checklist") for g in groups]
        got = {check.UNWIRED: 0, check.UNVERIFIED: 0, "deterministic": 0}
        for x in cases:
            want = check.label(x)
            got[want] = got.get(want, 0) + 1
            if self.kc.enforcement_label(x) != want:
                self.fail(f"{x.get('enforcement')!r}: Engine は {self.kc.enforcement_label(x)!r}、check.py は {want!r}")
        # 総当たりが 3 つの表示をどれも通っていること（組の作り方を壊して 1 つの表示に偏ったら気づく）
        self.assertTrue(all(got[x] > 0 for x in (check.UNWIRED, check.UNVERIFIED, "deterministic", "context", "checklist")), got)

    def test_INDEX_md_の担保の列でK_0004_は未配線(self):
        items = [x for x in check.load(REPO)]
        rows = {line.split(" | ")[0].lstrip("| "): line.split(" | ")[5] for line in self.kc.index_md(items, date(2026, 11, 7)).splitlines()
                if line.startswith("| K-")}
        self.assertEqual(rows["K-0004"], "未配線")
        self.assertNotIn("deterministic", [rows[x["id"]] for x in items if check.label(x) != "deterministic"])

    def test_真偽でない値と配線せずに確認済みは落ちる(self):
        f = lambda *s: self.kc.check_item({**k("deterministic", *s), "_file": "K-9999.yaml"}, set(), set())  # noqa: E731
        self.assertTrue(any("wired は true か false" in e for e in f(sol(wired="true"))))
        self.assertTrue(any("verified は true か false" in e for e in f(sol(wired=True, verified="yes"))))
        self.assertTrue(any("wired でないのに verified" in e for e in f(sol(wired=False, verified=True))))
        self.assertTrue(any("wired でないのに verified" in e for e in f(sol(verified=True))))
        self.assertFalse(any("enforcement.solutions" in e for e in f(sol(), sol(wired=True), sol(wired=True, verified=True))))

    def test_作り直した_INDEX_md_でこのリポジトリの知見が通る(self):
        with tempfile.TemporaryDirectory() as t:
            shutil.copytree(REPO / check.ROOT, Path(t) / check.ROOT)
            self.assertEqual(self.kc.main(["--repo", t, "--index"]), 0)
            self.assertEqual(self.kc.main(["--repo", t, "--today", "2026-11-07"]), 0)

    def test_定義に真偽の項目と表示がある(self):
        enf = check.load_yaml(self.dir / FILES[1])["knowledge"]["enforcement"]
        self.assertEqual(sorted(enf["solution_flags"]), ["verified", "wired"])
        self.assertEqual(enf["index_labels"], {"unwired": check.UNWIRED, "unverified": check.UNVERIFIED, "deterministic": "deterministic"})
        self.assertEqual(sorted(enf["solution_flags"]), sorted(self.kc.SOLUTION_FLAGS))
        self.assertEqual((self.kc.UNWIRED, self.kc.UNVERIFIED), (check.UNWIRED, check.UNVERIFIED))

    def test_Engine_に足したテストが通る(self):
        if importlib.util.find_spec("pytest") is None:
            self.skipTest("pytest がない")
        r = subprocess.run([sys.executable, "-m", "pytest", "-q", "-p", "no:cacheprovider", FILES[2]], cwd=self.dir, capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)


if __name__ == "__main__":
    unittest.main()
