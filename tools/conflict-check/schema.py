"""制御プレーンのイベント列を、衝突したファイルの列（conflict_files）を必須にした定義に照らす（知見 K-0014 の担保。議決 D-0049・Issue #1136）。

定義は conflict-files.yaml（Engine の definitions/events.yaml の events と同じ形の抜き書き）。engine/conflict-files.patch が Engine の定義に同じ欄を足す。
照らすイベント: pr.conflicted（いつも）・continue.enqueued（reason=conflict のとき）・handoff.saved（continue_reason=conflict のとき）。
落とすもの（rule）:
  missing_field  要る欄がない（null も含む）
  bad_field      欄の形が違う（conflict_files なら、空でない文字列の列で 1 件以上でない。空の列も含む）

  python3 schema.py --events <イベントの JSON か JSONL> [--defs <conflict-files.yaml か Engine の events.yaml>] [--json]
  終了コード: 0 = 通る、1 = 外れた、2 = 記録か定義を読めなかった
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

import yaml

sys.path.insert(0, str(Path(__file__).resolve().parent))   # python3 -I で動かしても隣の events.py を読む
import events as ev  # noqa: E402

DEFAULT_DEFS = Path(__file__).resolve().parent / "conflict-files.yaml"
TYPES = ("paths",)


class DefsError(ValueError):
    """定義の形が正しくない"""


def load(path: Path = DEFAULT_DEFS) -> dict[str, dict]:
    """定義の events を読んで形を確かめる。{ イベント名: { required, required_when, fields } }"""
    d = yaml.safe_load(path.read_text(encoding="utf-8"))
    if not isinstance(d, dict) or not isinstance(d.get("events"), dict):
        raise DefsError("events がない")
    out = {}
    for name, e in d["events"].items():
        if not isinstance(e, dict):
            raise DefsError(f"{name} がオブジェクトでない")
        req, when, fields = e.get("required") or [], e.get("required_when") or [], e.get("fields") or {}
        if not isinstance(req, list) or not all(isinstance(f, str) for f in req):
            raise DefsError(f"{name}.required が文字列の列でない")
        if not isinstance(when, list) or not all(isinstance(w, dict) and isinstance(w.get("when"), dict) and w["when"]
                                                 and isinstance(w.get("fields"), list) and all(isinstance(f, str) for f in w["fields"])
                                                 for w in when):
            raise DefsError(f"{name}.required_when が {{ when: {{欄: 値}}, fields: [欄] }} の列でない")
        if not isinstance(fields, dict):
            raise DefsError(f"{name}.fields がオブジェクトでない")
        for f, spec in fields.items():
            if not isinstance(spec, dict) or spec.get("type") not in TYPES:
                raise DefsError(f"{name}.fields.{f} の type が {', '.join(TYPES)} でない")
            n = spec.get("min_items", 0)
            if not isinstance(n, int) or isinstance(n, bool) or n < 0:
                raise DefsError(f"{name}.fields.{f}.min_items が 0 以上の整数でない")
        out[name] = {"required": req, "required_when": when, "fields": fields}
    return out


def required_fields(defn: dict, payload: dict) -> list[str]:
    """その payload で要る欄（required と、when の値がすべて合う required_when の fields）"""
    out = list(defn["required"])
    for w in defn["required_when"]:
        if all(payload.get(k) == v for k, v in w["when"].items()):
            out += [f for f in w["fields"] if f not in out]
    return out


def field_error(spec: dict, value) -> str | None:
    """欄の値が形に合わなければ理由"""
    if spec["type"] == "paths":
        if not isinstance(value, list) or not all(isinstance(x, str) and x.strip() for x in value):
            return "空でない文字列（ファイルの道）の列でない"
        if len(value) < spec.get("min_items", 0):
            return f"{spec.get('min_items', 0)} 件以上でない（{len(value)} 件）"
    return None


def check(defs: dict[str, dict], events: list[dict]) -> list[dict]:
    """定義にあるイベントを 1 件ずつ照らし、外れたものを [{ rule, index, name, job_id, pr_url, field, message }] で返す。
    要る欄は、ないか null なら missing_field、あれば形を見る。要らない欄でも、あって null でなければ形を見る"""
    errors = []
    for i, e in enumerate(events):
        defn = defs.get(e["name"])
        if defn is None:
            continue
        p = e.get("payload") or {}
        need = required_fields(defn, p)

        def err(rule: str, field: str, message: str):
            errors.append({"rule": rule, "index": i, "name": e["name"], "job_id": e.get("job_id"), "pr_url": p.get("pr_url"),
                           "field": field, "message": message})

        for f in need:
            if p.get(f) is None:
                err("missing_field", f, f"{e['name']} に {f} がない（null も含む）")
        for f, spec in defn["fields"].items():
            if p.get(f) is None:
                continue
            why = field_error(spec, p[f])
            if why:
                err("bad_field", f, f"{e['name']} の {f} が {why}")
    return errors


def main(argv: list[str]) -> int:
    def arg(name: str) -> str | None:
        return argv[argv.index(name) + 1] if name in argv and argv.index(name) + 1 < len(argv) else None

    path, defs_path = arg("--events"), Path(arg("--defs") or DEFAULT_DEFS)
    if not path:
        print("conflict-check schema: --events <イベントの JSON か JSONL> を渡す", file=sys.stderr)
        return 2
    try:
        defs = load(defs_path)
    except (OSError, yaml.YAMLError, DefsError) as e:
        print(f"conflict-check schema: {defs_path} を読めない: {e}", file=sys.stderr)
        return 2
    try:
        events = ev.parse_events(Path(path).read_text(encoding="utf-8"))
    except (OSError, ev.EventsError) as e:
        print(f"conflict-check schema: {path} を読めない: {e}", file=sys.stderr)
        return 2
    errors = check(defs, events)
    if "--json" in argv:
        json.dump({"ok": not errors, "errors": errors}, sys.stdout, ensure_ascii=False)
        print()
    else:
        for x in errors:
            print(f"{x['pr_url']} {x['name']} job={x['job_id']} {x['rule']}: {x['message']}")
        print(f"conflict-check schema: イベント {len(events)} 件・外れ {len(errors)} 件")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
