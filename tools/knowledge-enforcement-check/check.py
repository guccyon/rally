#!/usr/bin/env python3
"""知見の担保の配線と効き目の検査（知見 K-0004 の担保、議決 D-0015、Issue #962）。

.factory-config/knowledge/K-*.yaml の enforcement.solutions の各項目にある真偽の項目を見る。LLM を使わない。
  - wired: CI や呼び出しに配線済みか（.github/workflows/ や Engine の呼び出しから実際に走るか）
  - verified: 本物の記録で効き目を確かめたか（wired でないのに verified にはできない）
書いていない項目は落とさず false（未記入）として数え、未記入の件数を別に出す。

担保の表示（この検査の出力に出す値。Engine の core-gates/knowledge_check.py に移したら INDEX.md の担保の列もこの値にする予定）:
  - level が deterministic でない: level のまま（context・checklist）
  - deterministic で、wired の解決策が 1 件もない: 未配線（deterministic とは出さない）
  - deterministic で、wired はあるが wired かつ verified が 1 件もない: deterministic（未確認）
  - deterministic で、wired かつ verified が 1 件以上: deterministic

使い方:
  python3 tools/knowledge-enforcement-check/check.py [--repo .] [--json out.json] [--summary "$GITHUB_STEP_SUMMARY"]
  終了コード: 0 = 緑、1 = 通らない、2 = 読めなかった

依存: python3。YAML は PyYAML があればそれ、なければ ruby で読む（Engine の knowledge_check.py と同じ）
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

ROOT = ".factory-config/knowledge"
FLAGS = ("wired", "verified")
UNWIRED = "未配線"
UNVERIFIED = "deterministic（未確認）"


def load_yaml(path: Path):
    try:
        import yaml  # type: ignore
        return yaml.safe_load(path.read_text(encoding="utf-8"))
    except ImportError:
        out = subprocess.run(["ruby", "-ryaml", "-rjson", "-e", "puts JSON.generate(YAML.load_file(ARGV[0], permitted_classes: [Date]))", str(path)],
                             capture_output=True, text=True, check=True).stdout
        return json.loads(out)


def solutions(k: dict) -> list:
    enf = k.get("enforcement") or {}
    return (enf.get("solutions") or []) if isinstance(enf, dict) else []


def label(k: dict) -> str:
    """担保の列に出す値"""
    enf = k.get("enforcement") or {}
    level = enf.get("level", "context") if isinstance(enf, dict) else "context"
    if level != "deterministic":
        return str(level)
    sols = [s for s in solutions(k) if isinstance(s, dict)]
    if not any(s.get("wired") is True for s in sols):
        return UNWIRED
    if not any(s.get("wired") is True and s.get("verified") is True for s in sols):
        return UNVERIFIED
    return "deterministic"


def check_item(k: dict) -> list[str]:
    f, e = k.get("_file", str(k.get("id"))), []
    for i, s in enumerate(solutions(k)):
        if not isinstance(s, dict):
            continue  # 形は Engine の knowledge_check.py が見る
        where = f"{f}: enforcement.solutions[{i}]（{s.get('kind')}、issue {s.get('issue')}）"
        for flag in FLAGS:
            if flag in s and not isinstance(s[flag], bool):
                e.append(f"{where}: {flag} は true か false（{s[flag]!r}）")
        if s.get("verified") is True and s.get("wired") is not True:
            e.append(f"{where}: wired でないのに verified が true（配線してから本物の記録で確かめる）")
    return e


def row(k: dict) -> dict:
    sols = [s for s in solutions(k) if isinstance(s, dict)]
    return {
        "id": k.get("id"),
        "level": (k.get("enforcement") or {}).get("level", "context") if isinstance(k.get("enforcement"), dict) else "context",
        "label": label(k),
        "solutions": len(sols),
        "wired": sum(1 for s in sols if s.get("wired") is True),
        "verified": sum(1 for s in sols if s.get("wired") is True and s.get("verified") is True),
        "unrecorded": sum(1 for s in sols if any(flag not in s for flag in FLAGS)),
    }


def load(repo: Path) -> list[dict]:
    items = []
    for p in sorted((repo / ROOT).glob("K-*.yaml")):
        k = load_yaml(p)
        if isinstance(k, dict):
            k["_file"] = p.name
            items.append(k)
    return items


def summary_md(rows: list[dict], errors: list[str]) -> str:
    out = ["## knowledge-enforcement-check", "", "| id | 強さ | 担保の表示 | 解決策 | 配線済み | 確認済み | 未記入 |", "|---|---|---|---|---|---|---|"]
    for r in rows:
        if r["solutions"]:
            out.append(f"| {r['id']} | {r['level']} | {r['label']} | {r['solutions']} | {r['wired']} | {r['verified']} | {r['unrecorded']} |")
    out += [""] + [f"- {line}" for line in errors]
    return "\n".join(out) + "\n"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--repo", default=".")
    ap.add_argument("--json")
    ap.add_argument("--summary")
    a = ap.parse_args(argv)
    try:
        items = load(Path(a.repo))
    except Exception as e:  # noqa: BLE001
        print(f"読めなかった: {e}", file=sys.stderr)
        return 2
    errors = [line for k in items for line in check_item(k)]
    rows = [row(k) for k in items]
    if a.json:
        Path(a.json).write_text(json.dumps({"ok": not errors, "errors": errors, "items": rows}, ensure_ascii=False, indent=1), encoding="utf-8")
    if a.summary:
        with open(a.summary, "a", encoding="utf-8") as fh:
            fh.write(summary_md(rows, errors))
    for line in errors:
        print(f"::error::{line}")
    for r in rows:
        if r["solutions"]:
            print(f"{r['id']}: {r['label']}（解決策 {r['solutions']} 件、配線済み {r['wired']}、確認済み {r['verified']}、未記入 {r['unrecorded']}）")
    print(f"knowledge-enforcement: {len(items)} 件、エラー {len(errors)} 件")
    return 0 if not errors else 1


if __name__ == "__main__":
    sys.exit(main())
