"""Engine に移す差分が ENGINE_REF の版に当たるかの検査（知見 K-0004 の担保、議決 D-0024、Issue #1014）。

engine/knowledge-enforcement.patch を、.github/workflows/knowledge-check.yml の ENGINE_REF の版の Engine に git apply --check で当て、
当たらず、しかも Engine 側にまだ入っていない（git apply --reverse --check が通らず、差分で足した enforcement_label・check_wired_by・check_verified_by も
ない）ときは patch_stale で落とす。差分が当たるか、もう入っていれば（git apply --reverse --check が通るときだけ）通る。差分で足した関数は
あるのに --reverse --check が通らない版（差分の一部だけが入った・入れたあと変わった）は、入っているとは数えずに patch_partially_applied で
落とす（知見 K-0004 の担保、議決 D-0036、Issue #1077）。test_engine.py は ENGINE_DIR のチェックアウトの今の中身に当てるが、こちらは ENGINE_REF の版を
名指しで取り出して当てるので、チェックアウトが別の版でも製品の CI が使う版で確かめられる。
Engine のチェックアウトは環境変数 ENGINE_DIR で渡す（なければリポジトリの隣の ../engine）。チェックアウトがないか、ENGINE_REF の版を
含まないときは飛ばす。ただし ENGINE_DIR を渡したときと CI（CI=true）では飛ばさずに落とす（test_engine.py と同じ）。
環境変数 ENGINE_PATCH_REF に版を渡すと、ENGINE_REF の代わりにその版に当てる（ci/knowledge-enforcement-head.yml が Engine の既定のブランチの
先頭の版を渡し、ENGINE_REF が古いまま先頭だけが進んで差分が当たらなくなったことを定期に見る。議決 D-0036、Issue #1076）。

  python3 -m unittest discover -s tools/knowledge-enforcement-check
"""
from __future__ import annotations

import io
import os
import re
import subprocess
import tarfile
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
PATCH = HERE / "engine" / "knowledge-enforcement.patch"
WORKFLOW = REPO / ".github" / "workflows" / "knowledge-check.yml"
LOCK_REL = ".factory-config/engine.lock"   # knowledge-check.yml が ENGINE_REF を読むファイル（Engine ADR 0041）
# 差分で Engine に足す関数を探すファイル。どれかがあるのに --reverse --check が通らなければ、一部だけ入ったか入れたあと変わった（Issue #1077）
MARKER_FILE = "core-gates/knowledge_check.py"

ENGINE = Path(os.environ.get("ENGINE_DIR") or REPO.parent / "engine")
REQUIRED = bool(os.environ.get("ENGINE_DIR")) or os.environ.get("CI") == "true"
PATCH_REF = os.environ.get("ENGINE_PATCH_REF", "")

APPLIES, MERGED, STALE, PARTIAL = "applies", "merged", "patch_stale", "patch_partially_applied"


def engine_ref(workflow: Path = WORKFLOW) -> str:
    """ワークフローの ENGINE_REF（40 桁のコミット SHA）。env に ENGINE_REF の行があればその値、なければワークフローが読む
    .factory-config/engine.lock の版（コメントと空行を除いた最初の行。ワークフローと同じ読み方。Engine ADR 0041。議決 D-0080、Issue #1225）。"""
    text = workflow.read_text(encoding="utf-8")
    if re.search(r"^\s*ENGINE_REF:", text, re.M):
        m = re.search(r"^\s*ENGINE_REF:\s*['\"]?([0-9a-f]{40})['\"]?\s*$", text, re.M)
        if not m:
            raise ValueError(f"{workflow} に ENGINE_REF（40 桁のコミット SHA）がない")
        return m.group(1)
    if LOCK_REL not in text:
        raise ValueError(f"{workflow} に ENGINE_REF（40 桁のコミット SHA）も {LOCK_REL} の読み込みもない")
    lock = workflow.parents[2] / LOCK_REL
    lines = [l for l in lock.read_text(encoding="utf-8").splitlines() if not re.match(r"^\s*(#|$)", l)] if lock.is_file() else []
    ref = "".join(lines[0].split()) if lines else ""
    if not re.fullmatch(r"[0-9a-f]{40}", ref):
        raise ValueError(f"{workflow} が読む {LOCK_REL} に 40 桁のコミット SHA の 1 行がない")
    return ref


def target_ref() -> tuple[str, str]:
    """差分を当てる版と、その出どころ（ENGINE_PATCH_REF があればその版、なければ ENGINE_REF）。"""
    return (PATCH_REF, "ENGINE_PATCH_REF") if PATCH_REF else (engine_ref(), "ENGINE_REF")


def has_commit(engine: Path, ref: str) -> bool:
    r = subprocess.run(["git", "-C", str(engine), "cat-file", "-e", f"{ref}^{{commit}}"], capture_output=True)
    return r.returncode == 0


def added_functions(patch: Path = PATCH, file: str = MARKER_FILE) -> tuple[tuple[str, str], ...]:
    """差分が file に足す関数（+def name( の行）を、(file, "def name(") の組で差分に出てくる順に返す。名前を手で書くと、
    差分に関数を足したときに一覧から漏れて、その関数だけが入った版を patch_stale と取り違える（知見 K-0004 の担保、議決 D-0054、Issue #1149）。"""
    found, cur = [], None
    for line in patch.read_text(encoding="utf-8").splitlines():
        if line.startswith("diff --git "):   # 中身の行は空白・+・-・\ で始まるので、+++ の行と違って足した行と取り違えない
            cur = line.rsplit(" b/", 1)[-1]
        elif cur == file and (m := re.match(r"^\+(def [A-Za-z_]\w*\()", line)):
            found.append((file, m.group(1)))
    return tuple(found)


MARKERS = added_functions()


def patch_state(engine: Path, ref: str, patch: Path = PATCH,
                markers: tuple[tuple[str, str], ...] | None = None) -> tuple[str, str]:
    """ref の版の Engine に対する差分の状態と、git apply の出力。APPLIES = 当たる、MERGED = もう入っている（--reverse --check が通る）、
    PARTIAL = 差分で足した関数はあるが --reverse --check が通らない、STALE = どれでもない。markers を渡さなければ patch から取り出す。"""
    if markers is None:
        markers = added_functions(patch)
    tar = subprocess.run(["git", "-C", str(engine), "archive", "--format=tar", ref], capture_output=True, check=True).stdout
    with tempfile.TemporaryDirectory() as t:
        with tarfile.open(fileobj=io.BytesIO(tar)) as f:
            f.extractall(t, filter="data") if hasattr(tarfile, "data_filter") else f.extractall(t)
        apply = lambda *a: subprocess.run(["git", "apply", "--check", *a, str(patch)], cwd=t, capture_output=True, text=True)  # noqa: E731
        fwd = apply()
        if fwd.returncode == 0:
            return APPLIES, ""
        if apply("--reverse").returncode == 0:
            return MERGED, ""
        found = [name.removeprefix("def ").rstrip("(") for f, name in markers
                 if (Path(t) / f).is_file() and name in (Path(t) / f).read_text(encoding="utf-8")]
        if found:
            return PARTIAL, f"{', '.join(found)} はあるが git apply --reverse --check が通らない"
        return STALE, fwd.stderr.strip()


def git(cwd: Path, *args: str) -> str:
    env = {**os.environ, "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@example.invalid",
           "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@example.invalid"}
    return subprocess.run(["git", *args], cwd=cwd, env=env, capture_output=True, text=True, check=True).stdout.strip()


class PatchStateTest(unittest.TestCase):
    """作った Engine の写し（git の版）で、当たる・入っている・一部だけ入った・入れたあと変わった・どれでもないの判定を確かめる。
    Engine は要らない。"""

    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        d = cls.dir = Path(cls.tmp.name) / "engine"
        (d / "core-gates").mkdir(parents=True)
        git(d, "init", "-q")
        f = d / "core-gates" / "knowledge_check.py"
        mid = "".join(f"m{i}\n" for i in range(10))   # 2 つの関数を別の hunk に分ける
        f.write_text(f"a\nb\nc\n{mid}x\ny\nz\n", encoding="utf-8")
        git(d, "add", ".")
        git(d, "commit", "-qm", "base")
        cls.base = git(d, "rev-parse", "HEAD")
        f.write_text(f"a\nb\ndef enforcement_label(k):\nc\n{mid}x\ny\ndef check_wired_by(x):\nz\n", encoding="utf-8")
        cls.patch = Path(cls.tmp.name) / "p.patch"
        cls.patch.write_text(git(d, "diff") + "\n", encoding="utf-8")
        git(d, "commit", "-qam", "merged")
        cls.merged = git(d, "rev-parse", "HEAD")
        f.write_text(f"a\nB\ndef enforcement_label(k, v):\nc\n{mid}x\ny\ndef check_wired_by(x, y):\nz\n", encoding="utf-8")
        git(d, "commit", "-qam", "merged-then-changed")
        cls.changed = git(d, "rev-parse", "HEAD")
        f.write_text(f"a\nb\ndef enforcement_label(k):\nc\n{mid}x\ny\nz\n", encoding="utf-8")
        git(d, "commit", "-qam", "label-only")
        cls.partial = git(d, "rev-parse", "HEAD")
        f.write_text(f"a\nb\nc\n{mid}X\ny\ndef check_wired_by(x):\nz\n", encoding="utf-8")
        git(d, "commit", "-qam", "wired-by-only-then-changed")
        cls.wired_only = git(d, "rev-parse", "HEAD")
        f.write_text("p\nq\nr\n", encoding="utf-8")
        git(d, "commit", "-qam", "moved")
        cls.moved = git(d, "rev-parse", "HEAD")

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def state(self, ref):
        return patch_state(self.dir, ref, self.patch)[0]

    def test_当たる版は通る(self):
        self.assertEqual(self.state(self.base), APPLIES)

    def test_もう入っている版は通る(self):
        self.assertEqual(self.state(self.merged), MERGED)

    def test_入れたあと変わった版は関数があっても_patch_partially_applied(self):
        state, err = patch_state(self.dir, self.changed, self.patch)
        self.assertEqual(state, PARTIAL)
        self.assertIn("enforcement_label", err)
        self.assertIn("check_wired_by", err)

    def test_一部だけ入った版は_patch_partially_applied(self):
        state, err = patch_state(self.dir, self.partial, self.patch)
        self.assertEqual(state, PARTIAL)
        self.assertIn("enforcement_label", err)
        self.assertNotIn("check_wired_by", err)
        self.assertEqual(self.state(self.wired_only), PARTIAL)

    def test_当たらずまだ入っていない版は_patch_stale(self):
        state, err = patch_state(self.dir, self.moved, self.patch)
        self.assertEqual(state, STALE)
        self.assertIn("core-gates/knowledge_check.py", err)

    def test_ENGINE_REF_を読む(self):
        self.assertRegex(engine_ref(), r"^[0-9a-f]{40}$")
        w = Path(self.tmp.name) / "w.yml"
        w.write_text("env:\n  ENGINE_REF: main\n", encoding="utf-8")
        with self.assertRaises(ValueError):
            engine_ref(w)

    def test_ENGINE_REF_の行がなければ_engine_lock_の版を読む(self):
        root = Path(self.tmp.name) / "r"
        w = root / ".github" / "workflows" / "knowledge-check.yml"
        w.parent.mkdir(parents=True)
        w.write_text(f"run: |\n  lock={LOCK_REL}\n", encoding="utf-8")
        lock = root / LOCK_REL
        lock.parent.mkdir(parents=True)
        for body, want in ((f"# 版\n\n  {'c' * 40}  \n{'d' * 40}\n", "c" * 40), ("# 版\nmain\n", None), ("# 版だけ\n", None), (None, None)):
            with self.subTest(body):
                if body is None:
                    lock.unlink()
                else:
                    lock.write_text(body, encoding="utf-8")
                if want:
                    self.assertEqual(engine_ref(w), want)
                else:
                    with self.assertRaises(ValueError):
                        engine_ref(w)
        w.write_text("run: echo\n", encoding="utf-8")   # ENGINE_REF の行も engine.lock の読み込みもない
        with self.assertRaises(ValueError):
            engine_ref(w)


class AddedFunctionsTest(unittest.TestCase):
    """一部だけ入ったことを見る関数の名前を手で書かず、差分の core-gates/knowledge_check.py に足す行（+def ...）から取り出す
    （知見 K-0004 の担保、議決 D-0054、Issue #1149）。差分に関数を 1 つ足した作った版で、その関数だけが入った Engine が
    patch_stale ではなく patch_partially_applied で落ちることを確かめる。Engine は要らない。"""

    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        d = cls.dir = Path(cls.tmp.name) / "engine"
        (d / "core-gates" / "tests").mkdir(parents=True)
        git(d, "init", "-q")
        f = d / "core-gates" / "knowledge_check.py"
        t = d / "core-gates" / "tests" / "test_knowledge_check.py"
        gap = lambda p: "".join(f"{p}{i}\n" for i in range(10))  # noqa: E731  関数ごとに別の hunk に分ける
        body = lambda *fs: f"a\n{fs[0]}{gap('m')}x\n{fs[1]}{gap('n')}y\n{fs[2]}z\n"  # noqa: E731
        f.write_text(body("", "", ""), encoding="utf-8")
        t.write_text("t\n", encoding="utf-8")
        git(d, "add", ".")
        git(d, "commit", "-qm", "base")
        cls.base = git(d, "rev-parse", "HEAD")
        added = ("def enforcement_label(k):\n", "def check_wired_by(x):\n", "def check_added(y):\n")
        f.write_text(body(*added), encoding="utf-8")
        t.write_text("t\ndef test_check_added():\n", encoding="utf-8")
        cls.patch = Path(cls.tmp.name) / "p.patch"
        cls.patch.write_text(git(d, "diff") + "\n", encoding="utf-8")
        git(d, "checkout", "-q", ".")
        f.write_text(body("", "", added[2]), encoding="utf-8")
        git(d, "commit", "-qam", "added-only")
        cls.added_only = git(d, "rev-parse", "HEAD")

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def test_差分の_knowledge_check_py_に足す関数だけを取り出す(self):
        self.assertEqual(added_functions(self.patch), tuple((MARKER_FILE, f"def {n}(")
                                                           for n in ("enforcement_label", "check_wired_by", "check_added")))

    def test_差分に足した関数だけが入った版は_patch_stale_ではなく_patch_partially_applied(self):
        state, err = patch_state(self.dir, self.added_only, self.patch)
        self.assertEqual(state, PARTIAL)
        self.assertIn("check_added", err)
        self.assertNotIn("enforcement_label", err)
        # 名前を手で書いた一覧（足した関数が漏れる）なら patch_stale と取り違える
        old = ((MARKER_FILE, "def enforcement_label("), (MARKER_FILE, "def check_wired_by("))
        self.assertEqual(patch_state(self.dir, self.added_only, self.patch, old)[0], STALE)

    def test_Engine_に移す差分の関数を取り出す(self):
        added = {line[1:].split("(")[0].removeprefix("def ") for line in
                 PATCH.read_text(encoding="utf-8").split("diff --git ")[1].splitlines() if line.startswith("+def ")}
        self.assertTrue(MARKERS)
        self.assertEqual({name.removeprefix("def ").rstrip("(") for _, name in MARKERS}, added)
        self.assertTrue(all(f == MARKER_FILE for f, _ in MARKERS))


REF, REF_FROM = target_ref()
HAS_ENGINE = (ENGINE / ".git").exists() and has_commit(ENGINE, REF)


@unittest.skipUnless(HAS_ENGINE or REQUIRED, f"{REF_FROM} の版を含む Engine のチェックアウトがない（ENGINE_DIR={ENGINE}）")
class EngineRefPatchTest(unittest.TestCase):
    def fail_with_annotation(self, msg: str):
        """GitHub Actions では msg の 1 行目を差分のファイルへの ::error にも出して落とす。"""
        if os.environ.get("GITHUB_ACTIONS") == "true":
            print(f"::error file={PATCH.relative_to(REPO)}::{msg.splitlines()[0]}")
        self.fail(msg)

    def test_差分が_ENGINE_REF_の版に当たるかもう入っている(self):
        ref = REF
        if not HAS_ENGINE:
            if REF_FROM == "ENGINE_PATCH_REF":
                raise AssertionError(f"Engine のチェックアウト（ENGINE_DIR={ENGINE}）に ENGINE_PATCH_REF {ref} の版がない")
            raise AssertionError(f"Engine のチェックアウト（ENGINE_DIR={ENGINE}）に knowledge-check.yml の ENGINE_REF {ref} の版がない。"
                                 "ci/knowledge-enforcement.yml の ENGINE_REF を knowledge-check.yml と同じ版にそろえる")
        state, err = patch_state(ENGINE, ref)
        if state == STALE:
            msg = (f"patch_stale: {PATCH.relative_to(REPO)} が {REF_FROM} {ref} の Engine に当たらず、Engine 側にもまだ入っていない。"
                   f"今の Engine に合わせて作り直す: {err}")
            self.fail_with_annotation(msg)
        if state == PARTIAL:
            msg = (f"patch_partially_applied: {REF_FROM} {ref} の Engine に {err}（差分の一部だけが入ったか、入れたあと変わった）。"
                   f"{PATCH.relative_to(REPO)} を今の Engine に合わせて作り直す（全部入れて ENGINE_REF を上げたなら差分を消す）")
            self.fail_with_annotation(msg)
        self.assertIn(state, (APPLIES, MERGED))


if __name__ == "__main__":
    unittest.main()
