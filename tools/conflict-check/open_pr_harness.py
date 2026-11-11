"""Engine の新しい PR の道を、GitHub とエージェントを差し替えて動かす（知見 K-0023 の担保。議決 D-0148・Issue #1383）。

test_open_pr.py から呼ぶ。orchestrator/worker/entrypoint.sh のうち、新しい PR の道（続きのジョブでないときの else から、gh pr create のあとの fi まで）と
stage_all をそのまま bash で動かし、PR を開いた時点で既定ブランチの最新がブランチに入っていたかを返す。

- 一時の bare リポジトリを origin にし、既定ブランチ（main）を浅く（--depth 1）クローンして作業のブランチを切り、実装の変更を置く（entrypoint.sh と同じ）
- 実装の間に main が進んだ形を作る: none = 進まない、advance = ほかのファイルが進む、conflict = 実装と同じ行が進む
- gh は偽物に差し替える。gh pr create が呼ばれた時点の origin で、PR のブランチが push 済みか・main の最新を含むか・衝突の印を含むかを記録する
- エージェント（AGENT）は偽物に差し替える。resolve = 衝突した両方の行を残して解く、leave = 何もしない、abort = 取り込みを中止する。
  呼ばれた引数と標準入力を記録する（STATE_ARGS を渡したか＝同じセッションで解いたかを見るため）

変えるのはファイルの置き場（/work/ を一時ディレクトリに）だけ。

  python3 open_pr_harness.py run <entrypoint.sh> [--main none|advance|conflict] [--agent resolve|leave|abort]
  python3 open_pr_harness.py order <entrypoint.sh>   # 新しい PR の道の中の、既定ブランチの取り込み・push・gh pr create の行の位置
  python3 open_pr_harness.py --check <entrypoint.sh>  # 取り出せるかだけを見る（取り出せなければ終了コード 1）
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ENTRYPOINT = "orchestrator/worker/entrypoint.sh"
BRANCH = "factory/issue-345-job-429"
STATE_DIR = "/handoff/agent"
PR_URL = "https://github.com/guccyon/rally/pull/345"
MERGE = re.compile(r'^\s*(if\s+!\s+|elif\s+!\s+)?git merge\b.*origin/\$\{DEFAULT_BRANCH\}')

FAKE_GH = r"""#!/usr/bin/env bash
printf '%s\n' "$*" >> "$HARNESS_ROOT/gh.log"
if [ "$1 $2" = "pr create" ]; then
  o="$HARNESS_ORIGIN" b="refs/heads/$HARNESS_BRANCH"
  pushed=false; contains=false; markers=false
  if git -C "$o" rev-parse --verify -q "$b" >/dev/null; then
    pushed=true
    git -C "$o" merge-base --is-ancestor refs/heads/main "$b" && contains=true
    git -C "$o" grep -qE '^(<<<<<<<|>>>>>>>) ' "$b" -- . && markers=true
  fi
  printf '{"pushed":%s,"contains_main":%s,"markers":%s}\n' "$pushed" "$contains" "$markers" >> "$HARNESS_ROOT/created.jsonl"
  echo "$HARNESS_PR_URL"
fi
"""

FAKE_AGENT = r"""import json, os, subprocess, sys
root = os.environ["HARNESS_ROOT"]
with open(os.path.join(root, "agent.jsonl"), "a", encoding="utf-8") as f:
    f.write(json.dumps({"argv": sys.argv[1:], "prompt": sys.stdin.read()}, ensure_ascii=False) + "\n")
mode = os.environ["HARNESS_AGENT"]
if mode == "resolve":
    files = subprocess.run(["git", "diff", "--name-only", "--diff-filter=U"], capture_output=True, text=True, check=True).stdout.split()
    for p in files:
        lines = open(p, encoding="utf-8").read().splitlines(keepends=True)
        open(p, "w", encoding="utf-8").writelines(l for l in lines if not l.startswith(("<<<<<<< ", "=======", ">>>>>>> ")))
elif mode == "abort":
    subprocess.run(["git", "merge", "--abort"], check=False)
print(json.dumps({"output": "## 概要\n衝突を解いた", "cost_usd": 0.25, "num_turns": 2, "usage": {}, "resumed": True}))
"""


def _git(cwd: Path, *args: str, check=True) -> str:
    return subprocess.run(["git", *args], cwd=cwd, check=check, capture_output=True, text=True).stdout


def _lines(entrypoint: Path) -> list[str]:
    if not entrypoint.is_file():
        raise SystemExit(f"entrypoint.sh がない: {entrypoint}")
    return entrypoint.read_text(encoding="utf-8").splitlines(keepends=True)


def stage_all(entrypoint: Path) -> str:
    """entrypoint.sh の stage_all の定義（stage_all() { から次の } まで）"""
    lines = _lines(entrypoint)
    try:
        start = next(i for i, l in enumerate(lines) if l.startswith("stage_all() {"))
        end = next(i for i in range(start, len(lines)) if lines[i].rstrip("\n") == "}")
    except StopIteration:
        raise SystemExit("entrypoint.sh に stage_all の定義が見つからない") from None
    return "".join(lines[start:end + 1])


def open_pr_block(entrypoint: Path) -> list[str]:
    """新しい PR の道の行。続きのジョブの「[worker] continued pr=」の echo のあとの else から、gh pr create のあとの最初の fi の前まで"""
    lines = _lines(entrypoint)
    try:
        cont = next(i for i, l in enumerate(lines) if 'echo "[worker] continued pr=' in l)
        start = next(i for i in range(cont, len(lines)) if lines[i].rstrip("\n") == "else") + 1
        create = next(i for i in range(start, len(lines)) if "gh pr create" in lines[i])
        end = next(i for i in range(create, len(lines)) if lines[i].rstrip("\n") == "fi")
    except StopIteration:
        raise SystemExit("entrypoint.sh に新しい PR の道（continued pr の echo の後の else から gh pr create の後の fi まで）が見つからない") from None
    return lines[start:end]


def order(entrypoint: Path) -> dict:
    """新しい PR の道の中で、既定ブランチの取り込み（git merge … origin/${DEFAULT_BRANCH}）・push・gh pr create が最初に出る行（道の中の 0 始まり）。ないものは null"""
    block = open_pr_block(entrypoint)

    def first(pred):
        return next((i for i, l in enumerate(block) if not l.lstrip().startswith("#") and pred(l)), None)

    return {"merge": first(lambda l: MERGE.search(l)), "fetch": first(lambda l: "git fetch" in l),
            "push": first(lambda l: re.search(r"\bgit push\b", l)), "create": first(lambda l: "gh pr create" in l)}


def _origin(root: Path) -> tuple[Path, Path]:
    origin = root / "origin.git"
    _git(root, "init", "-q", "--bare", "-b", "main", str(origin))
    seed = root / "seed"
    _git(root, "clone", "-q", str(origin), str(seed))
    _git(seed, "checkout", "-q", "-b", "main")
    (seed / "src").mkdir()
    (seed / "src/app.txt").write_text("value = 1\n", encoding="utf-8")
    (seed / "README.md").write_text("# rally\n", encoding="utf-8")
    _git(seed, "add", "-A")
    _git(seed, "commit", "-q", "-m", "base")
    _git(seed, "push", "-q", "origin", "main")
    return origin, seed


def run(entrypoint: Path, main: str = "advance", agent: str = "resolve") -> dict:
    """新しい PR の道を動かす。実装（src/app.txt を value = pr にする。コミットはしない）を置いたあと、main を進めてから道を動かす"""
    script = ("set -euo pipefail\n" + stage_all(entrypoint)
              + 'AGENT=("${HARNESS_PYTHON}" -I -B "${HARNESS_ROOT}/fake_agent.py")\n'
              + f'STATE_ARGS=(--state-dir {STATE_DIR})\nAGENT_TIMEOUT=()\n')
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        (root / ".gitconfig").write_text("[user]\n\tname = factory-worker\n\temail = factory-worker@users.noreply.github.com\n", encoding="utf-8")
        env = {"PATH": f"{root / 'bin'}:/usr/local/bin:/usr/bin:/bin", "HOME": str(root), "GIT_TERMINAL_PROMPT": "0",
               "HARNESS_ROOT": str(root), "HARNESS_BRANCH": BRANCH, "HARNESS_PR_URL": PR_URL, "HARNESS_AGENT": agent,
               "HARNESS_PYTHON": sys.executable}
        origin, seed = _origin(root)
        env["HARNESS_ORIGIN"] = str(origin)
        (root / "bin").mkdir()
        (root / "bin/gh").write_text(FAKE_GH, encoding="utf-8")
        (root / "bin/gh").chmod(0o755)
        (root / "fake_agent.py").write_text(FAKE_AGENT, encoding="utf-8")
        repo = root / "repo"
        subprocess.run(["git", "clone", "--quiet", "--depth", "1", "--branch", "main", f"file://{origin}", str(repo)],
                       check=True, capture_output=True, env=env)
        _git(repo, "checkout", "-q", "-b", BRANCH)
        (repo / "src/app.txt").write_text("value = pr\n", encoding="utf-8")   # 実装（エージェントの作業。コミットは道の中で）
        if main != "none":
            target = seed / ("src/app.txt" if main == "conflict" else "README.md")
            target.write_text("value = main\n" if main == "conflict" else "# rally\n\n進んだ行\n", encoding="utf-8")
            _git(seed, "commit", "-q", "-am", "main が進んだ")
            _git(seed, "push", "-q", "origin", "main")
        block = "".join(open_pr_block(entrypoint)).replace("/work/", f"{root}/")
        vars_ = {"BRANCH": BRANCH, "DEFAULT_BRANCH": "main", "REPO": "guccyon/rally", "TITLE": "実装", "ISSUE_NUMBER": "345",
                 "JOB_ID": "429", "INCOMPLETE": "", "CLAUDE_RC": "0", "LINK_KW": "Closes", "PR_TITLE": "実装 (#345)",
                 "SUMMARY": "## 概要\n実装した", "MODEL": "claude-opus", "TURNS": "10", "COST": "3.30", "USAGE": "{}",
                 "MAX_TURNS": "40", "MODEL_TIER": "tier-large"}
        p = subprocess.run(["bash", "-c", script + block], cwd=repo, env={**env, **vars_}, capture_output=True, text=True)
        read = lambda name: [json.loads(l) for l in (root / name).read_text(encoding="utf-8").splitlines()] if (root / name).is_file() else []  # noqa: E731
        gh = (root / "gh.log").read_text(encoding="utf-8").splitlines() if (root / "gh.log").is_file() else []
        result = next((json.loads(l[len("FACTORY_RESULT:"):]) for l in reversed(p.stdout.splitlines()) if l.startswith("FACTORY_RESULT:")), None)
        pushed = bool(_git(origin, "rev-parse", "--verify", "-q", f"refs/heads/{BRANCH}", check=False).strip())
        return {"exit": p.returncode, "created": read("created.jsonl"), "gh": gh, "pushed": pushed,
                "agent_calls": [{"argv": c["argv"], "resumes": STATE_DIR in c["argv"] and "--state-dir" in c["argv"],
                                 "prompt_head": c["prompt"][:200]} for c in read("agent.jsonl")],
                "result": result, "stdout": p.stdout[-1500:], "stderr": p.stderr[-1500:]}


def check(entrypoint: Path) -> dict:
    """動かさず、取り出す手順が entrypoint.sh にあるかだけを見る（CI で飛ばさずに落とすため）"""
    stage_all(entrypoint)
    return {"block_lines": len(open_pr_block(entrypoint)), "order": order(entrypoint)}


def main(argv: list[str]):
    opt = lambda name, default: argv[argv.index(name) + 1] if name in argv else default  # noqa: E731
    if len(argv) == 2 and argv[0] == "--check":
        out = check(Path(argv[1]))
    elif len(argv) == 2 and argv[0] == "order":
        out = order(Path(argv[1]))
    elif len(argv) >= 2 and argv[0] == "run":
        out = run(Path(argv[1]), opt("--main", "advance"), opt("--agent", "resolve"))
    else:
        raise SystemExit(__doc__)
    print(json.dumps(out, ensure_ascii=False))


if __name__ == "__main__":
    main(sys.argv[1:])
