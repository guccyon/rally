#!/usr/bin/env python3
"""`.factory-config/knowledge/README.md` を断片（`readme.d/*.md`）から作る。

README.md は生成物（`.factory-config/aggregates.yaml` の `compile: generate`）。
手で書き換えず、`readme.d/` の断片を直してこのスクリプトで作り直す。
断片はファイル名の昇順に連結する（1 断片 = README の 1 行。空行も断片）。

  python3 tools/knowledge-readme/build.py          # README.md を作り直す
  python3 tools/knowledge-readme/build.py --check  # 作り直しても差分が出ないことを確かめる（出れば 1）
"""
import argparse
import sys
from pathlib import Path

KNOWLEDGE = Path(".factory-config/knowledge")


def render(repo: Path) -> str:
    frags = sorted((repo / KNOWLEDGE / "readme.d").glob("*.md"))
    if not frags:
        raise SystemExit("readme.d/ に断片がない")
    return "".join(f.read_text(encoding="utf-8") for f in frags)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", default=".")
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()
    repo = Path(args.repo)
    target = repo / KNOWLEDGE / "README.md"
    out = render(repo)
    if args.check:
        if not target.exists() or target.read_text(encoding="utf-8") != out:
            print("README.md が readme.d/ の断片と違う。手で書き換えず、断片を直して build.py で作り直す", file=sys.stderr)
            return 1
        return 0
    target.write_text(out, encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
