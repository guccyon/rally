"""衝突の続きのジョブの線（.factory-config/continuation.yaml の conflict）を読み、判定を返す（知見 K-0010 の担保。議決 D-0013・Issue #954）。

参照の実装。Engine の本物の処理をこの線に照らすのは engine_harness.py と test_engine.py。

  python3 policy.py [--config <continuation.yaml>]   # 設定を読めて形が正しいかを見る（読めなければ終了コード 2）
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

import yaml

DEFAULT_CONFIG = Path(__file__).resolve().parents[2] / ".factory-config" / "continuation.yaml"
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
    if c.get("agent_tier") not in TIERS:
        raise PolicyError(f"conflict.agent_tier は {', '.join(TIERS)} のどれか")
    return c


def enqueue_decision(policy: dict, attempts_so_far: int) -> str:
    """同じ PR でこれまでに積んだ conflict の続きのジョブの数から、次の衝突で enqueue（積む）か on_limit の値（積まずに人へ）かを返す"""
    return "enqueue" if attempts_so_far < policy["max_attempts_per_pr"] else policy["on_limit"]


def may_resume(policy: dict, attempt: int) -> bool:
    """attempt 回目（1 始まり）の conflict の続きのジョブがセッションを引き継いでよいか"""
    return attempt <= policy["resume_session_until_attempt"]


def is_mechanical_path(policy: dict, path: str) -> bool:
    return any(path.startswith(p) if p.endswith("/") else path == p for p in policy["mechanical"]["paths"])


def resolution(policy: dict, conflicted: list[str]) -> dict:
    """衝突したファイルから解き方を返す。{"mode": none | mechanical | agent, "tier": none | tier-*, "mechanical": [...], "agent": [...]}"""
    mech = sorted(f for f in conflicted if is_mechanical_path(policy, f))
    rest = sorted(f for f in conflicted if not is_mechanical_path(policy, f))
    if not conflicted:
        return {"mode": "none", "tier": "none", "mechanical": [], "agent": []}
    if not rest:
        return {"mode": "mechanical", "tier": policy["mechanical"]["agent_tier"], "mechanical": mech, "agent": []}
    return {"mode": "agent", "tier": policy["agent_tier"], "mechanical": mech, "agent": rest}


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
