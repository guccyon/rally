"""Engine の本物の judge の入力の組み立て（core-gates/judge.py の main）を、GitHub・git・エージェントを差し替えて動かす
（知見 K-0011 の担保。議決 D-0033、Issue #1054）。

engine.test.mjs から呼ぶ。judge.py の main をそのまま動かし、エージェントに渡すはずだったプロンプト（judge の入力）を返す。
あわせて、指摘が引く rubric_id ごとに、正本の rubric の本文（Engine の core-gates/rubric.yaml の check、
製品の .factory-config/overlay/rules/<id>.yaml の check.text）を返す。入力にその本文が入っているかは line.mjs が見る。

差し替えるもの（judge.py の中身には手を入れない）:
- agent_runner（エージェントの呼び出し）: プロンプトを覚えて、空の裁きを返す
- pr_diff（PR の差分）: 決まった文字列を返す
- judge.sh（gh・git）: gh pr view は入力の pr（title・body）を返し、git は何もしない。ほかの呼び出しは落とす
- judge.Path: PR の head のクローン（/work/product）を、製品のチェックアウト（第 2 引数。既定はこのリポジトリ）に読み替える

  python3 engine_harness.py <Engine のチェックアウト> [<製品のチェックアウト>] < case.json > result.json
  python3 engine_harness.py --check <Engine のチェックアウト>   # judge.py を読み込めて main・sh があるかだけを見る（なければ終了コード 1）

差し替えの知らない呼び出し（gh pr view と git 以外のコマンド）を Engine が引いたら、標準エラーに
「harness_unknown_call: <呼び出し>」を出して終了コード 3 で落ちる（tools/sendback-check/engine_harness.py の harness_unknown_query と同じ扱い）。

入力: { "pr_url", "mode", "provisional": [...], "defects": [{ "rubric_id", ... }], "pr": { "title", "body" } }（judge.py の入力と同じ形に pr を足したもの）
出力: { "prompt": str, "rubric": { "<rubric_id>": { "source": "core" | "overlay" | null, "body": str | null } } }
"""
from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import sys
import types
from pathlib import Path

JUDGE = "core-gates/judge.py"
CORE_RUBRIC = "core-gates/rubric.yaml"
OVERLAY_RULES = ".factory-config/overlay/rules"
CLONE = "/work/product"   # judge.py が PR の head を取るクローンの場所
REPO = Path(__file__).resolve().parent.parent.parent

UNKNOWN_CALL_MARK = "harness_unknown_call"
UNKNOWN_CALL_EXIT = 3


class HarnessUnknownCall(AssertionError):
    """差し替えが知らない呼び出し（Engine の入力の変化ではなく、ハーネスのずれ）"""


def load_judge(engine: Path, captured: dict):
    """agent_runner と pr_diff を差し替えて judge.py を読み込む"""
    path = engine / JUDGE
    if not path.is_file():
        raise SystemExit(f"Engine のチェックアウトに {JUDGE} がない（ENGINE_DIR={engine}）")

    def run(stage, prompt, schema=None, **kw):
        captured["prompt"] = prompt
        return types.SimpleNamespace(model="harness", cost_usd=0, usage={}, output={"provisional": [], "defects": [], "summary": ""})

    sys.modules["agent_runner"] = types.SimpleNamespace(run=run)
    sys.modules["pr_diff"] = types.SimpleNamespace(fetch=lambda repo, number, sh: "（engine_harness の差分）")
    # judge.py は同じ core-gates/ のモジュール（knowledge_context など）を import する。-I で走らせるので道を足す
    if str(path.parent) not in sys.path:
        sys.path.insert(0, str(path.parent))
    spec = importlib.util.spec_from_file_location("engine_judge", path)
    judge = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(judge)  # type: ignore[union-attr]
    missing = [n for n in ("main", "sh") if not callable(getattr(judge, n, None))]
    if missing:
        raise SystemExit(f"Engine の {JUDGE} に {', '.join(missing)} がない（名前が変わったら engine_harness.py を合わせる）")
    return judge


def fake_sh(case: dict, calls: list[str]):
    pr = case.get("pr") or {}

    def sh(*a: str, retries: int = 3, cwd: str | None = None) -> str:
        if a[:3] == ("gh", "pr", "view"):
            return json.dumps({"title": pr.get("title", "PR"), "body": pr.get("body", ""), "closingIssuesReferences": []}, ensure_ascii=False)
        if a[:1] == ("git",):
            return ""
        calls.append(" ".join(a))
        raise HarnessUnknownCall(f"{UNKNOWN_CALL_MARK}: engine_harness が知らない呼び出し: {' '.join(a)}")

    return sh


def remap_path(product: Path):
    """judge.py の Path。PR の head のクローンを製品のチェックアウトに読み替える（ほかはそのまま）"""

    def path(*a):
        p = Path(*a)
        if str(p) == CLONE or str(p).startswith(CLONE + "/"):
            return product / p.relative_to(CLONE)
        return p

    return path


def _yaml():
    import yaml  # type: ignore
    return yaml


def rubric_sources(engine: Path, product: Path, ids: list[str]) -> dict:
    """rubric_id ごとの正本の本文。Engine の rubric.yaml の check、なければ製品の overlay のルールの check.text"""
    yaml = _yaml()
    core = {r["id"]: r for r in yaml.safe_load((engine / CORE_RUBRIC).read_text(encoding="utf-8"))["rules"]}
    out = {}
    for rid in ids:
        if rid in core:
            out[rid] = {"source": "core", "body": core[rid].get("check")}
            continue
        f = product / OVERLAY_RULES / f"{rid}.yaml"
        if f.is_file():
            check = (yaml.safe_load(f.read_text(encoding="utf-8")) or {}).get("check")
            out[rid] = {"source": "overlay", "body": check.get("text") if isinstance(check, dict) else check}
            continue
        out[rid] = {"source": None, "body": None}
    return out


def run(engine: Path, product: Path, case: dict, calls: list[str]) -> dict:
    captured: dict = {}
    judge = load_judge(engine, captured)
    judge.sh = fake_sh(case, calls)
    judge.Path = remap_path(product)
    stdin, sys.stdin = sys.stdin, io.StringIO(json.dumps({k: v for k, v in case.items() if k != "pr"}, ensure_ascii=False))
    try:
        with contextlib.redirect_stdout(io.StringIO()):
            judge.main()
    finally:
        sys.stdin = stdin
    if "prompt" not in captured:
        raise SystemExit("judge.py の main がエージェントを呼ばなかった（engine_harness.py を合わせる）")
    ids = list(dict.fromkeys(d["rubric_id"] for d in case.get("defects", []) if d.get("rubric_id")))
    return {"prompt": captured["prompt"], "rubric": rubric_sources(engine, product, ids)}


def check(engine: Path) -> dict:
    """入力は組まず、judge.py を読み込めて差し替える名前があるかだけを見る（CI で飛ばさずに落とすため）"""
    load_judge(engine, {})
    if not (engine / CORE_RUBRIC).is_file():
        raise SystemExit(f"Engine のチェックアウトに {CORE_RUBRIC} がない（ENGINE_DIR={engine}）")
    return {"functions": ["main", "sh"]}


def main():
    if len(sys.argv) == 3 and sys.argv[1] == "--check":
        json.dump(check(Path(sys.argv[2])), sys.stdout, ensure_ascii=False)
        return
    if len(sys.argv) not in (2, 3):
        raise SystemExit("使い方: python3 engine_harness.py [--check] <Engine のチェックアウト> [<製品のチェックアウト>] < case.json")
    case = json.load(sys.stdin)
    product = Path(sys.argv[2]) if len(sys.argv) == 3 else REPO
    calls: list[str] = []
    out = sys.stdout
    try:
        result = run(Path(sys.argv[1]), product, case, calls)
    except Exception:
        if not calls:
            raise
        result = None
    if calls:   # 組んだ入力は信じられないので出さない（Engine が例外を握りつぶして通っても落とす）
        for c in dict.fromkeys(calls):
            print(f"{UNKNOWN_CALL_MARK}: engine_harness が知らない呼び出し（Engine の先頭に合わせて engine_harness.py を直す）: {c}", file=sys.stderr)
        raise SystemExit(UNKNOWN_CALL_EXIT)
    json.dump(result, out, ensure_ascii=False)


if __name__ == "__main__":
    main()
