"""制御プレーンのイベント列を events.py で照らし、外れた PR に衝突の停止（ci:<PR の URL>）を開いて人の判断待ちにする（知見 K-0010 の担保。議決 D-0030・Issue #1034）。

supervisor の周期ごと（または conflict の続きのジョブを積むたび）に、開いている工場の PR のイベント列に対して走らせる。
over_limit・resumed_after_limit・agent_for_mechanical のどれかが出た PR ごとに、停止を 1 つ開く。
停止を開くのは Engine の stoppage.open_stoppage（同じ key が開いていれば開き直さない）。ワーカーは Engine を変えられないので、
supervisor からの呼び出し（apply）は建設者が配線する（README の「supervisor の周期ごとの見張り」）。

  python3 watch.py --events <イベントの JSON か JSONL> [--pr <PR の URL>] [--config <continuation.yaml>] [--json]
  終了コード: 0 = 開く停止がない、1 = 開く停止がある（標準出力に出す）、2 = 記録か設定を読めなかった
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

import yaml

sys.path.insert(0, str(Path(__file__).resolve().parent))   # python3 -I で動かしても隣の events.py・policy.py を読む
import events  # noqa: E402
import policy  # noqa: E402

CATEGORY = "ticket"           # supervisor の ci_failure_step が開く衝突の停止と同じ種類
DETECTED_BY = "conflict-check"


def stoppages(policy_: dict, evs: list[dict], pr_url: str | None = None) -> list[dict]:
    """外れた PR ごとに、開く停止 { key, category, what, waiting_for, detected_by, correlation_id, job_id, pr_url, rules, errors } を返す。
    外れのない PR と、記録がなく照らせなかっただけの PR には開かない"""
    corr = {e.get("job_id"): e.get("correlation_id") for e in evs
            if e["name"] == "continue.enqueued" and e.get("job_id") is not None and isinstance(e.get("correlation_id"), str)}
    out = []
    for url, record in events.records_from_events(evs, pr_url).items():
        errors = events.check(policy_, record)
        if not errors:
            continue
        rules = sorted({e["rule"] for e in errors})
        first = errors[0]   # 線から外れた最初の回（そこで止めるべきだった）
        out.append({
            "key": f"ci:{url}",
            "category": CATEGORY,
            "what": f"工場の PR の衝突の続きのジョブが continuation.yaml の conflict の線から外れた（{', '.join(rules)}）。"
                    f"最初は {first['attempt']} 回目: {first['message']}",
            "waiting_for": "人の判断待ち。builder が衝突を解くか、差し戻す（K-0010）",
            "detected_by": DETECTED_BY,
            "correlation_id": next((corr[e["job_id"]] for e in errors if e["job_id"] in corr), None),
            "job_id": first["job_id"] if isinstance(first["job_id"], int) else None,
            "pr_url": url,
            "rules": rules,
            "errors": errors,
        })
    return out


def apply(cur, stoppage, found: list[dict]) -> list[str]:
    """found の停止を Engine の stoppage.open_stoppage で開き、新しく開いた key を返す（同じ key が開いていれば開き直さない）"""
    opened = []
    for s in found:
        if stoppage.open_stoppage(cur, s["key"], s["category"], s["what"], waiting_for=s["waiting_for"], detected_by=s["detected_by"],
                                  correlation_id=s["correlation_id"], job_id=s["job_id"]):
            opened.append(s["key"])
    return opened


def main(argv: list[str]) -> int:
    def arg(name: str) -> str | None:
        return argv[argv.index(name) + 1] if name in argv and argv.index(name) + 1 < len(argv) else None

    path, config = arg("--events"), Path(arg("--config") or policy.DEFAULT_CONFIG)
    if not path:
        print("conflict-check watch: --events <イベントの JSON か JSONL> を渡す", file=sys.stderr)
        return 2
    try:
        policy_ = policy.load(config)
    except (OSError, yaml.YAMLError, policy.PolicyError) as e:
        print(f"conflict-check watch: {config} を読めない: {e}", file=sys.stderr)
        return 2
    try:
        evs = events.parse_events(Path(path).read_text(encoding="utf-8"))
    except (OSError, events.EventsError) as e:
        print(f"conflict-check watch: {path} を読めない: {e}", file=sys.stderr)
        return 2
    found = stoppages(policy_, evs, arg("--pr"))
    if "--json" in argv:
        json.dump({"stoppages": found}, sys.stdout, ensure_ascii=False)
        print()
    else:
        for s in found:
            print(f"open {s['key']} ({', '.join(s['rules'])}): {s['what']}")
        print(f"conflict-check watch: 開く停止 {len(found)} 件")
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
