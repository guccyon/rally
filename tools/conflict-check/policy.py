"""衝突の続きのジョブの線（.factory-config/continuation.yaml の conflict）を読み、判定を返す（知見 K-0010 の担保。議決 D-0013・Issue #954）。

参照の実装。Engine の本物の処理をこの線に照らすのは engine_harness.py と test_engine.py。
上限（max_attempts_per_pr）に数えるのは、エージェントで解いた回だけ（resolution の counts）。機械的に解いた回は、
解いた結果が mechanical.verify の検査（K-0002 の tools/changelog-check/check.mjs）に通ったときだけ機械的な解消として扱い、数えない。

  python3 policy.py [--config <continuation.yaml>]   # 設定を読めて形が正しいかを見る（読めなければ終了コード 2）
"""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_CONFIG = ROOT / ".factory-config" / "continuation.yaml"
ON_LIMIT = ("stoppage",)
TIERS = ("tier-small", "tier-large")


class PolicyError(ValueError):
    """設定の形が正しくない"""


def load(path: Path = DEFAULT_CONFIG) -> dict:
    """continuation.yaml の conflict を読み、形を確かめて返す"""
    doc = yaml.safe_load(Path(path).read_text(encoding="utf-8")) or {}
    c = doc.get("conflict")
    if not isinstance(c, dict):
        raise PolicyError("conflict がない")
    n = c.get("max_attempts_per_pr")
    if not isinstance(n, int) or isinstance(n, bool) or n < 1:
        raise PolicyError("conflict.max_attempts_per_pr は 1 以上の整数")
    if c.get("on_limit") not in ON_LIMIT:
        raise PolicyError(f"conflict.on_limit は {', '.join(ON_LIMIT)} のどれか")
    r = c.get("resume_session_until_attempt")
    if not isinstance(r, int) or isinstance(r, bool) or r < 0:
        raise PolicyError("conflict.resume_session_until_attempt は 0 以上の整数")
    m = c.get("mechanical") or {}
    paths = m.get("paths")
    if not isinstance(paths, list) or not paths or not all(isinstance(p, str) and p for p in paths):
        raise PolicyError("conflict.mechanical.paths は空でないパスの配列")
    if m.get("agent_tier") != "none":
        raise PolicyError("conflict.mechanical.agent_tier は none（機械的に解くときはエージェントを使わない）")
    v = m.get("verify")
    if not isinstance(v, str) or not v:
        raise PolicyError("conflict.mechanical.verify は機械的に解いた結果を確かめる検査のパス（リポジトリの根から）")
    if c.get("agent_tier") not in TIERS:
        raise PolicyError(f"conflict.agent_tier は {', '.join(TIERS)} のどれか")
    return c


def enqueue_decision(policy: dict, agent_attempts_so_far: int) -> str:
    """同じ PR でこれまでにエージェントで解いた conflict の続きのジョブの数（agent_attempts）から、
    次の衝突で enqueue（積む）か on_limit の値（積まずに人へ）かを返す。機械的に解いた回は数に入れない"""
    return "enqueue" if agent_attempts_so_far < policy["max_attempts_per_pr"] else policy["on_limit"]


def agent_attempts(outcomes: list[dict]) -> int:
    """同じ PR のこれまでの conflict の続きのジョブの解き方（resolution の戻り値）のうち、上限に数える回（エージェントで解いた回）の数"""
    return sum(1 for o in outcomes if o["counts"])


def may_resume(policy: dict, attempt: int) -> bool:
    """attempt 回目（1 始まり）の conflict の続きのジョブがセッションを引き継いでよいか"""
    return attempt <= policy["resume_session_until_attempt"]


def is_mechanical_path(policy: dict, path: str) -> bool:
    return any(path.startswith(p) if p.endswith("/") else path == p for p in policy["mechanical"]["paths"])


def resolution(policy: dict, conflicted: list[str], verified: bool | None = None) -> dict:
    """衝突したファイルから解き方を返す。
    {"mode": none | mechanical | agent, "tier": none | tier-*, "mechanical": [...], "agent": [...], "counts": 上限に数えるか}

    verified は、機械的に解いた結果が mechanical.verify の検査に通ったか（None はまだ検査していない。衝突したファイルだけで決める）。
    False なら、衝突したファイルが mechanical.paths だけでもエージェント（agent_tier）で解き、その回は上限に数える"""
    mech = sorted(f for f in conflicted if is_mechanical_path(policy, f))
    rest = sorted(f for f in conflicted if not is_mechanical_path(policy, f))
    if not conflicted:
        return {"mode": "none", "tier": "none", "mechanical": [], "agent": [], "counts": False}
    if not rest and verified is not False:
        return {"mode": "mechanical", "tier": policy["mechanical"]["agent_tier"], "mechanical": mech, "agent": [], "counts": False}
    return {"mode": "agent", "tier": policy["agent_tier"], "mechanical": mech if rest else [], "agent": rest or mech, "counts": True}


def verify_mechanical(policy: dict, repo: Path, base: str, head: str = "HEAD") -> dict:
    """機械的に解いてコミットした結果（head）を、既定ブランチ（base）に対して mechanical.verify の検査
    （K-0002 の tools/changelog-check/check.mjs）にかける。{"ok": 終了コードが 0 か, "returncode", "output"}。
    読めなかった（終了コード 2）ときも通らないとして扱う（エージェントに回す）"""
    p = subprocess.run(["node", str(ROOT / policy["mechanical"]["verify"]), "--base", base, "--head", head],
                       cwd=repo, capture_output=True, text=True)
    return {"ok": p.returncode == 0, "returncode": p.returncode, "output": (p.stdout + p.stderr).strip()}


def main(argv: list[str]) -> int:
    path = Path(argv[argv.index("--config") + 1]) if "--config" in argv else DEFAULT_CONFIG
    try:
        policy = load(path)
    except (OSError, yaml.YAMLError, PolicyError) as e:
        print(f"conflict-check: {path} を読めない: {e}", file=sys.stderr)
        return 2
    json.dump(policy, sys.stdout, ensure_ascii=False)
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
