"""policy.py の線（.factory-config/continuation.yaml の conflict）のテスト（知見 K-0010 の担保。議決 D-0013・Issue #954）。Engine は要らない"""
from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import policy  # noqa: E402


class ConfigTest(unittest.TestCase):
    def test_リポジトリの設定を読める(self):
        c = policy.load()
        self.assertEqual(c["max_attempts_per_pr"], 3)
        self.assertEqual(c["on_limit"], "stoppage")
        self.assertEqual(c["resume_session_until_attempt"], 1)
        self.assertIn("CHANGELOG.md", c["mechanical"]["paths"])
        self.assertIn("changelog.d/", c["mechanical"]["paths"])
        self.assertEqual(c["mechanical"]["verify"], "tools/changelog-check/check.mjs")   # K-0002 の検査
        self.assertTrue((policy.ROOT / c["mechanical"]["verify"]).is_file())

    def test_形が正しくない設定は読めないとして終了コード2(self):
        bad = {
            "上限がない": "conflict: {on_limit: stoppage}",
            "上限が0": "conflict: {max_attempts_per_pr: 0, on_limit: stoppage, resume_session_until_attempt: 1, mechanical: {paths: [CHANGELOG.md], agent_tier: none}, agent_tier: tier-large}",
            "機械的に解くのにエージェントを使う": "conflict: {max_attempts_per_pr: 3, on_limit: stoppage, resume_session_until_attempt: 1, mechanical: {paths: [CHANGELOG.md], agent_tier: tier-small}, agent_tier: tier-large}",
            "上限を超えたときの扱いが知らない値": "conflict: {max_attempts_per_pr: 3, on_limit: retry, resume_session_until_attempt: 1, mechanical: {paths: [CHANGELOG.md], agent_tier: none, verify: x.mjs}, agent_tier: tier-large}",
            "機械的な解消の検査がない": "conflict: {max_attempts_per_pr: 3, on_limit: stoppage, resume_session_until_attempt: 1, mechanical: {paths: [CHANGELOG.md], agent_tier: none}, agent_tier: tier-large}",
        }
        with tempfile.TemporaryDirectory() as tmp:
            for name, text in bad.items():
                path = Path(tmp) / "continuation.yaml"
                path.write_text(text, encoding="utf-8")
                with self.assertRaises(policy.PolicyError, msg=name):
                    policy.load(path)
                p = subprocess.run([sys.executable, "-I", str(HERE / "policy.py"), "--config", str(path)], capture_output=True, text=True)
                self.assertEqual(p.returncode, 2, name)


class LineTest(unittest.TestCase):
    def setUp(self):
        self.c = policy.load()

    def simulate(self, conflicts: list[tuple[list[str], bool | None]]) -> list[str]:
        """同じ PR の衝突を順に回す。各回は（衝突したファイル, 機械的に解いた結果が検査に通ったか）。積むかの判定を返す"""
        outcomes, decisions = [], []
        for files, verified in conflicts:
            d = policy.enqueue_decision(self.c, policy.agent_attempts(outcomes))
            decisions.append(d)
            if d != "enqueue":
                break   # 停止を開いたら新しい head は来ない
            outcomes.append(policy.resolution(self.c, files, verified))
        return decisions

    def test_エージェントで解く衝突が20回続けば3回だけ積み4回目からは停止を開いて人へ(self):
        self.assertEqual(self.simulate([(["apps/backend/src/x.rs"], None)] * 20), ["enqueue"] * 3 + ["stoppage"])

    def test_C0005と同じCHANGELOGだけの衝突が20回続いても機械的に解けていれば人の判断待ちに上がらない(self):
        for files in (["CHANGELOG.md"], ["CHANGELOG.md", "changelog.d/700.md"]):
            self.assertEqual(self.simulate([(files, True)] * 20), ["enqueue"] * 20, files)

    def test_機械的に解いた回は上限に数えずエージェントで解いた回だけ数える(self):
        mech, agent = (["CHANGELOG.md"], True), (["CHANGELOG.md", "src/a.rs"], None)
        self.assertEqual(self.simulate([mech, agent, mech, mech, agent, mech, agent, mech, mech]),
                         ["enqueue"] * 7 + ["stoppage"])   # エージェントの 3 回目（7 回目の衝突）のあとの衝突で人へ

    def test_CHANGELOGだけでも検査に通らずエージェントで解いた回は上限に数える(self):
        self.assertEqual(self.simulate([(["CHANGELOG.md"], False)] * 20), ["enqueue"] * 3 + ["stoppage"])

    def test_上限はエージェントで解いた回の数だけで決まる(self):
        n = self.c["max_attempts_per_pr"]
        self.assertEqual([policy.enqueue_decision(self.c, k) for k in range(n + 2)], ["enqueue"] * n + ["stoppage"] * 2)

    def test_2回目以降はセッションを引き継がない(self):
        self.assertTrue(policy.may_resume(self.c, 1))
        for attempt in (2, 3, 20):
            self.assertFalse(policy.may_resume(self.c, attempt), attempt)

    def test_CHANGELOGと断片だけの衝突はエージェントを使わずに機械的に解く(self):
        for files in (["CHANGELOG.md"], ["changelog.d/700.md"], ["CHANGELOG.md", "changelog.d/700.md", "changelog.d/701.md"]):
            for verified in (None, True):
                r = policy.resolution(self.c, files, verified)
                self.assertEqual((r["mode"], r["tier"], r["agent"], r["counts"]), ("mechanical", "none", [], False), files)

    def test_CHANGELOGだけでも機械的に解いた結果が検査に通らなければエージェントで解き上限に数える(self):
        r = policy.resolution(self.c, ["CHANGELOG.md", "changelog.d/700.md"], verified=False)
        self.assertEqual((r["mode"], r["tier"], r["counts"]), ("agent", "tier-large", True))
        self.assertEqual((r["mechanical"], r["agent"]), ([], ["CHANGELOG.md", "changelog.d/700.md"]))

    def test_ほかのファイルも衝突したらエージェントで解く(self):
        r = policy.resolution(self.c, ["CHANGELOG.md", "apps/backend/src/x.rs"])
        self.assertEqual((r["mode"], r["tier"], r["counts"]), ("agent", "tier-large", True))
        self.assertEqual((r["mechanical"], r["agent"]), (["CHANGELOG.md"], ["apps/backend/src/x.rs"]))

    def test_名前が似ているだけのファイルは機械的に解かない(self):
        for f in ("docs/CHANGELOG.md", "CHANGELOG.md.bak", "changelog.dx/1.md", "apps/changelog.d/1.md"):
            self.assertEqual(policy.resolution(self.c, [f])["mode"], "agent", f)

    def test_衝突がなければ何もしない(self):
        r = policy.resolution(self.c, [])
        self.assertEqual((r["mode"], r["counts"]), ("none", False))


def _git(cwd: Path, *args: str, check: bool = True) -> str:
    return subprocess.run(["git", *args], cwd=cwd, check=check, capture_output=True, text=True).stdout


def _changelog(unreleased: str) -> str:
    return f"# 変更履歴\n\n## [Unreleased]\n\n### 追加\n{unreleased}\n## [0.1.0]\n"


BASE = {"CHANGELOG.md": _changelog("- 前からある行（#1）\n"), "changelog.d/README.md": "# 断片\n",
        "changelog.d/10.md": "### 追加\n- 断片の行（#10）\n"}
NEED = [t for t in ("git", "node") if not shutil.which(t)]
SKIP_VERIFY = None if not NEED or os.environ.get("CI") == "true" else f"{'・'.join(NEED)} がない"


@unittest.skipIf(SKIP_VERIFY, SKIP_VERIFY)
class MechanicalVerifyTest(unittest.TestCase):
    """両方の行を残す解き方（git merge-file --union。Engine の entrypoint.sh と同じ）と K-0002 の検査（check.mjs）を一緒に確かめる（議決 D-0023）"""

    def setUp(self):
        self.c = policy.load()
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)

    def merged(self, pr: dict, main: dict) -> tuple[Path, list[str]]:
        """BASE から PR のブランチ（pr）と既定ブランチ（main）が分かれて変え、pr に main を取り込んで、衝突したファイルを両方の行を残して解いてコミットする。
        pr・main は {パス: 中身（None は消す）}。リポジトリと、解く前に衝突していたファイルを返す"""
        repo = Path(self.tmp.name) / "repo"
        repo.mkdir()
        _git(repo, "init", "-q", "-b", "main")
        _git(repo, "config", "user.email", "test@example.invalid")
        _git(repo, "config", "user.name", "test")

        def commit(files: dict):
            for f, text in files.items():
                path = repo / f
                if text is None:
                    path.unlink()
                else:
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_text(text, encoding="utf-8")
            _git(repo, "add", "-A")
            _git(repo, "commit", "-q", "-m", "x")

        commit(BASE)
        _git(repo, "checkout", "-q", "-b", "pr")
        commit(pr)
        _git(repo, "checkout", "-q", "main")
        commit(main)
        _git(repo, "checkout", "-q", "pr")
        _git(repo, "merge", "--no-edit", "-q", "main", check=False)
        conflicted = sorted(_git(repo, "diff", "--name-only", "--diff-filter=U").split())
        for f in conflicted:
            stage = {n: subprocess.run(["git", "show", f":{n}:{f}"], cwd=repo, capture_output=True, text=True) for n in (1, 2, 3)}
            paths = {n: Path(self.tmp.name) / f"stage{n}" for n in stage}
            for n, p in stage.items():
                paths[n].write_text(p.stdout if p.returncode == 0 else "", encoding="utf-8")   # add/add は共通の祖先がない
            subprocess.run(["git", "merge-file", "--union", str(paths[2]), str(paths[1]), str(paths[3])], check=True)
            (repo / f).write_text(paths[2].read_text(encoding="utf-8"), encoding="utf-8")
            _git(repo, "add", f)
        _git(repo, "commit", "-q", "--no-edit")
        self.assertFalse(_git(repo, "diff", "--name-only", "--diff-filter=U").split())
        return repo, conflicted

    def settle(self, repo: Path, conflicted: list[str]) -> tuple[dict, dict]:
        v = policy.verify_mechanical(self.c, repo, "main")
        return v, policy.resolution(self.c, conflicted, v["ok"])

    def test_取りまとめどうしのCHANGELOGの衝突は両方の行を残してK0002の検査に通り機械的な解消になる(self):
        repo, conflicted = self.merged(
            pr={"CHANGELOG.md": _changelog("- 前からある行（#1）\n- 断片の行（#10）\n"), "changelog.d/10.md": None},   # 取りまとめ（断片を移した）
            main={"CHANGELOG.md": _changelog("- 前からある行（#1）\n- 先に取りまとめた行（#20）\n")})
        self.assertEqual(conflicted, ["CHANGELOG.md"], "衝突を起こせている確かめ")
        text = (repo / "CHANGELOG.md").read_text(encoding="utf-8")
        self.assertIn("- 断片の行（#10）", text)
        self.assertIn("- 先に取りまとめた行（#20）", text)
        self.assertNotIn("<<<<<<<", text)
        v, r = self.settle(repo, conflicted)
        self.assertTrue(v["ok"], v)
        self.assertEqual((r["mode"], r["tier"], r["counts"]), ("mechanical", "none", False))

    def test_PRがUnreleasedを直接書き換えていればK0002の検査に通らずエージェントで解き上限に数える(self):
        repo, conflicted = self.merged(pr={"CHANGELOG.md": _changelog("- 前からある行（#1）\n- PR が直接足した行（#2）\n")},
                                       main={"CHANGELOG.md": _changelog("- 前からある行（#1）\n- 先に取りまとめた行（#20）\n")})
        self.assertEqual(conflicted, ["CHANGELOG.md"], "衝突を起こせている確かめ")
        v, r = self.settle(repo, conflicted)
        self.assertEqual(v["returncode"], 1, v)
        self.assertIn("直接書き換えている", v["output"])
        self.assertEqual((r["mode"], r["tier"], r["counts"]), ("agent", "tier-large", True))

    def test_断片どうしの衝突は両方の行を残してK0002の検査に通る(self):
        repo, conflicted = self.merged(pr={"changelog.d/700.md": "### 追加\n- PR の断片（#700）\n"},
                                       main={"changelog.d/700.md": "### 追加\n- main の断片（#700）\n"})
        self.assertEqual(conflicted, ["changelog.d/700.md"], "衝突を起こせている確かめ")
        text = (repo / "changelog.d/700.md").read_text(encoding="utf-8")
        self.assertIn("- PR の断片（#700）", text)
        self.assertIn("- main の断片（#700）", text)
        v, r = self.settle(repo, conflicted)
        self.assertTrue(v["ok"], v)
        self.assertEqual((r["mode"], r["counts"]), ("mechanical", False))

    def test_検査が読めなかったときも通らないとしてエージェントに回す(self):
        repo, conflicted = self.merged(pr={"changelog.d/700.md": "- a\n"}, main={"changelog.d/700.md": "- b\n"})
        v = policy.verify_mechanical(self.c, repo, "no-such-branch")
        self.assertEqual((v["ok"], v["returncode"]), (False, 2), v)
        self.assertEqual(policy.resolution(self.c, conflicted, v["ok"])["mode"], "agent")


if __name__ == "__main__":
    unittest.main()
