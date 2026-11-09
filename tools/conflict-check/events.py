"""制御プレーンのイベント列から、PR ごとに conflict の続きのジョブを組み、policy.py の線に照らして落とす（知見 K-0010 の担保。議決 D-0023・Issue #972）。
衝突したファイルがその場で記録されなかった回も落とす（知見 K-0014 の担保。議決 D-0049・Issue #1135、議決 D-0101・Issue #1208）。
mechanical.verify の検査（K-0002 の check.mjs）に落ちる head を機械的に解いた回も落とす（知見 K-0015 の担保。議決 D-0078（Issue #1220 の
起票時の番号は D-0077）・Issue #1220）。

組むもの（PR ごと、continue.enqueued（reason=conflict）の順に 1 件ずつ）: 何回目か・agent_resumed・tier・衝突したファイル。
落とすもの（rule）:
  over_limit            同じ PR で max_attempts_per_pr を超えて積まれた（今の線では 4 回目以降）
  resumed_after_limit   resume_session_until_attempt より後の回（今の線では 2 回目以降）で agent_resumed=true
  agent_for_mechanical  衝突したファイルが mechanical.paths（CHANGELOG.md と changelog.d/）だけなのに、エージェントの tier（tier-large など）で解いた。
                        その head が mechanical.verify の検査に落ちる（--verify）なら、エージェントで解くのが線どおりなので外れにしない
  mechanical_without_verify  PR の head が mechanical.verify の検査（tools/changelog-check/check.mjs。CHANGELOG.md を直接書き換えていると落ちる）に
                        落ちる（--verify の終了コードが 0 でない）のに、mechanical.agent_tier（none。機械的）で衝突を解いた（知見 K-0015 の担保）
  missing_conflict_files  continue.enqueued の時点で、同じ job_id の conflict.files がその場で載っていない。ないか、backfill で後から足されたもの
                          （payload.source が backfill）か、ジョブが動き出したあと（同じ job_id の job.started か job.finished より後）に記録されたもの
                          しかない回。continue.enqueued の payload の conflict_files（conflict-files.yaml の形。空でない文字列の列で 1 件以上）は
                          積んだその場の記録として通す（議決 D-0101・Issue #1208）。終わらずに止まった回も照らす（知見 K-0014 の担保。
                          議決 D-0049・Issue #1135）。continuation.yaml の線ではないので check ではなく record_check が出し、watch.py の停止には使わない

あわせて、同じ PR・同じファイルの機械的な解消が max_attempts_per_pr（3 回）を超えたら、ファイルごとの回数を出す（外れにはしない。K-0015）。

記録がない項目（agent_resumed・tier・衝突したファイル）は照らさずに件数を出す。機械的に解いた回の検査の結果（--verify にない head）も同じく件数を出す。これは Engine が今の形（job.finished に agent_resumed と
解き方の tier（agent_tier）を出さない版）のときだけの扱いで、それらを出す版の Engine で agent_resumed か tier の照らせない件数が 1 以上なら
test_events.py の EngineFinishedTest が落とす（議決 D-0030・Issue #1035）

  python3 events.py --events <イベントの JSON か JSONL> [--pr <PR の URL>] [--config <continuation.yaml>] [--since <ISO 8601 の時刻>] [--verify <JSON>] [--record-only] [--json]
  --verify: PR の head ごとの mechanical.verify の検査の終了コード（node tools/changelog-check/check.mjs --base <既定ブランチ> --head <head_sha>）。
            { "<PR の URL>": { "<head_sha>": <終了コード> } } か、それを "verify" に持つオブジェクト（fixture は events と同じファイルに置く）。
            制御プレーンの events には検査の結果が載らない（Engine が mechanical.verify を呼んでいない）ので、検査を走らせた側が渡す（K-0015）
  --since: Engine に engine/conflict-files.patch を当てた時刻。occurred_at がそれより前のイベントは組まない（照らさない）。
           occurred_at がない・読めないイベントがあれば絞れないので終了コード 2（schema.py の --since と同じ。議決 D-0076・Issue #1214）
  --record-only: record_check（missing_conflict_files）だけを出し、continuation.yaml の線（check）は照らさない。直近の窓の本物のイベント列を
                 定期に照らすひな形 ci/conflict-schema.yml の conflict-record のジョブが使う（知見 K-0014 の担保。議決 D-0076・Issue #1214）
  終了コード: 0 = 通る、1 = 外れた、2 = 記録か設定（continuation.yaml・conflict-files.yaml）を読めなかった
"""
from __future__ import annotations

import functools
import json
import sys
from collections import Counter
from pathlib import Path

import yaml

sys.path.insert(0, str(Path(__file__).resolve().parent))   # python3 -I で動かしても隣の policy.py を読む
import policy  # noqa: E402

NAMES = ("pr.conflicted", "continue.enqueued", "job.started", "job.finished", "conflict.files")
INLINE = "conflict_files"   # continue.enqueued の payload に載せる衝突したファイルの列（conflict-files.yaml・engine/conflict-files.patch の欄）


class EventsError(ValueError):
    """イベント列の形が正しくない"""


def parse_verify(text: str) -> dict[str, dict[str, int]]:
    """--verify の JSON（{ pr_url: { head_sha: 終了コード } } か、それを "verify" に持つオブジェクト）を読む。形が違えば EventsError"""
    try:
        whole = json.loads(text)
    except json.JSONDecodeError as e:
        raise EventsError(f"検査の結果が JSON として読めない: {e}") from e
    if isinstance(whole, dict) and "verify" in whole:
        whole = whole["verify"]
    if not isinstance(whole, dict):
        raise EventsError("検査の結果が { PR の URL: { head_sha: 終了コード } } でない")
    for url, heads in whole.items():
        if not isinstance(heads, dict) or not all(isinstance(c, int) and not isinstance(c, bool) for c in heads.values()):
            raise EventsError(f"{url} の検査の結果が {{ head_sha: 終了コード }} でない")
    return whole


def parse_events(text: str) -> list[dict]:
    """イベントの JSON（配列か { "events": [...] }）か JSONL を読む。各件は { name, payload, job_id?, id? }。id があれば id の順に並べる"""
    t = text.strip()
    try:
        whole = json.loads(t) if t else []
    except json.JSONDecodeError:
        try:
            whole = [json.loads(l) for l in t.splitlines() if l.strip()]
        except json.JSONDecodeError as e:
            raise EventsError(f"JSON としても JSONL としても読めない: {e}") from e
    if isinstance(whole, dict):
        whole = whole.get("events") if "events" in whole else [whole]
    if not isinstance(whole, list):
        raise EventsError("events が配列でない")
    for i, e in enumerate(whole):
        if not isinstance(e, dict) or not isinstance(e.get("name"), str):
            raise EventsError(f"{i} 件目に name がない")
        if e.get("payload") is not None and not isinstance(e["payload"], dict):
            raise EventsError(f"{i} 件目の payload がオブジェクトでない")
    if all(isinstance(e.get("id"), int) for e in whole):
        whole = sorted(whole, key=lambda e: e["id"])
    return whole


def _tier(enq: dict, started: dict, finished: dict) -> str | None:
    """その回を解いた tier。記録された agent_tier を先に見て、なければ費用 0 で終わった回は none（エージェントを使わなかった。
    worker の entrypoint.sh は機械的に解いた回の cost_usd を 0 にする）、ほかは job.started の model_tier。分からなければ None"""
    for p in (enq, finished):
        if isinstance(p.get("agent_tier"), str):
            return p["agent_tier"]
    cost = finished.get("cost_usd")
    if isinstance(cost, (int, float)) and not isinstance(cost, bool) and cost == 0:
        return "none"
    for p in (started, enq):
        if isinstance(p.get("model_tier"), str):
            return p["model_tier"]
    return None


@functools.cache
def _inline_spec() -> tuple:
    """conflict-files.yaml の continue.enqueued の conflict_files の形と、それを照らす schema.field_error。
    形の判定は schema.py が持つ（schema.py がこのモジュールを取り込むので、使うときに取り込む）。定義のファイルを読めなければ OSError、
    YAML として読めない・形が違う・continue.enqueued の conflict_files の項目がなければ schema.DefsError"""
    import schema
    try:
        defs = schema.load()
    except yaml.YAMLError as e:
        raise schema.DefsError(f"YAML として読めない: {e}") from e
    spec = defs.get("continue.enqueued", {}).get("fields", {}).get(INLINE)
    if spec is None:
        raise schema.DefsError(f"continue.enqueued.fields.{INLINE} がない")
    return spec, schema.field_error


def _inline_files(payload: dict) -> list[str] | None:
    """continue.enqueued の payload の conflict_files が conflict-files.yaml の形（schema.py の field_error で外れがない）ならその列。でなければ None"""
    v = payload.get(INLINE)
    if v is None:
        return None
    spec, field_error = _inline_spec()
    return v if field_error(spec, v) is None else None


def records_from_events(events: list[dict], pr_url: str | None = None, verify: dict[str, dict[str, int]] | None = None) -> dict[str, dict]:
    """PR ごとに { pr_url, conflicted_heads, attempts: [{ attempt, job_id, head_sha, agent_resumed, tier, files, files_source, files_before_start,
    files_inline }] } を組む。job.started・job.finished・conflict.files は job_id で continue.enqueued に結ぶ。files_source は conflict.files の payload.source
    （Engine の dispatcher がワーカーのログから記録したら worker_log、factoryctl learn backfill-conflicts で後から足したら backfill）。
    files_before_start は conflict.files がジョブの動き出す前（同じ job_id の job.started・job.finished より前。並びはイベント列の順）に記録されたか。
    files_inline は continue.enqueued の payload に conflict_files が形どおりに載っていたか。載っていれば files はその列（conflict.files より先に見る）。
    verified は、その回の head（continue.enqueued の head_sha）が verify（parse_verify の形）で mechanical.verify の検査に通ったか（終了コード 0）。
    verify にその head がなければ None"""
    by_job: dict[int, dict[str, dict]] = {}
    at: dict[int, dict[str, int]] = {}   # job_id ごとの、各イベントが最初に出たイベント列の位置
    for i, e in enumerate(events):
        if e["name"] in ("job.started", "job.finished", "conflict.files") and e.get("job_id") is not None:
            by_job.setdefault(e["job_id"], {}).setdefault(e["name"], e.get("payload") or {})
            at.setdefault(e["job_id"], {}).setdefault(e["name"], i)
    prs: dict[str, dict] = {}

    def pr(url: str) -> dict:
        return prs.setdefault(url, {"pr_url": url, "conflicted_heads": [], "attempts": []})

    for e in events:
        p = e.get("payload") or {}
        url = p.get("pr_url")
        if not isinstance(url, str) or (pr_url and url != pr_url):
            continue
        if e["name"] == "pr.conflicted":
            heads = pr(url)["conflicted_heads"]
            if p.get("head_sha") not in heads:
                heads.append(p.get("head_sha"))
        elif e["name"] == "continue.enqueued" and p.get("reason") == "conflict":
            r = pr(url)
            linked = by_job.get(e.get("job_id"), {}) if e.get("job_id") is not None else {}
            started, finished = linked.get("job.started", {}), linked.get("job.finished", {})
            resumed = next((x["agent_resumed"] for x in (p, finished) if isinstance(x.get("agent_resumed"), bool)), None)
            files, source = linked.get("conflict.files", {}).get("files"), linked.get("conflict.files", {}).get("source")
            inline = _inline_files(p)
            pos = at.get(e.get("job_id"), {})
            before_start = "conflict.files" in pos and all(pos["conflict.files"] < pos[k] for k in ("job.started", "job.finished") if k in pos)
            code = (verify or {}).get(url, {}).get(p.get("head_sha"))
            n = len(r["attempts"]) + 1
            if isinstance(p.get("attempt"), int) and not isinstance(p.get("attempt"), bool):
                n = max(n, p["attempt"])   # イベント列が途中から始まっていても、Engine の数えた回を下回らない
            r["attempts"].append({"attempt": n, "job_id": e.get("job_id"), "head_sha": p.get("head_sha"), "agent_resumed": resumed,
                                  "tier": _tier(p, started, finished),
                                  "files": inline if inline is not None else files if isinstance(files, list) else None,
                                  "files_source": source if isinstance(source, str) else None, "files_before_start": before_start,
                                  "files_inline": inline is not None, "verified": None if code is None else code == 0})
    return prs


def check(policy_: dict, record: dict) -> list[dict]:
    """1 つの PR の記録を線に照らし、外れたものを [{ rule, attempt, job_id, message }] で返す。分からない項目（None）は照らさない"""
    errors = []
    for a in record["attempts"]:
        def err(rule: str, message: str):
            errors.append({"rule": rule, "attempt": a["attempt"], "job_id": a["job_id"], "message": message})

        if policy.enqueue_decision(policy_, a["attempt"] - 1) != "enqueue":
            err("over_limit", f"{a['attempt']} 回目の conflict の続きのジョブが積まれた（上限は {policy_['max_attempts_per_pr']} 回。"
                              f"超えたら積まずに {policy_['on_limit']}）")
        if a["agent_resumed"] is True and not policy.may_resume(policy_, a["attempt"]):
            err("resumed_after_limit", f"{a['attempt']} 回目でセッションを引き継いだ（agent_resumed=true。引き継いでよいのは "
                                       f"{policy_['resume_session_until_attempt']} 回目まで）")
        if a["files"] is not None and a["tier"] is not None:
            want = policy.resolution(policy_, a["files"], a["verified"])
            if want["mode"] == "mechanical" and a["tier"] != want["tier"]:
                err("agent_for_mechanical", f"衝突したのが {', '.join(a['files'])} だけなのに {a['tier']} で解いた（機械的に解く。tier は {want['tier']}）")
        if _mechanical(policy_, a) and a["verified"] is False:
            err("mechanical_without_verify", f"head {a['head_sha']} は {policy_['mechanical']['verify']} の検査に落ちる（CHANGELOG.md を直接書き換えている）のに、"
                                             f"{a['tier']}（機械的）で衝突を解いた（検査に落ちたらエージェントで解き、上限に数える。K-0015）")
    return errors


def _mechanical(policy_: dict, a: dict) -> bool:
    """その回を機械的に（mechanical.agent_tier で）解いたか。衝突がなかった回（files が空の列）は解いていない"""
    return a["tier"] == policy_["mechanical"]["agent_tier"] and a["files"] != []


def mechanical_repeats(policy_: dict, record: dict) -> dict[str, int]:
    """同じ PR で、同じファイルの機械的な解消が max_attempts_per_pr を超えたファイルと回数（K-0015。外れにはせず件数を出す）。
    衝突したファイルの記録がない回は数えない"""
    n = Counter(f for a in record["attempts"] if _mechanical(policy_, a) and a["files"] for f in a["files"])
    return {f: c for f, c in sorted(n.items()) if c > policy_["max_attempts_per_pr"]}


def unverified(policy_: dict, record: dict) -> int:
    """機械的に解いた回のうち、検査の結果（--verify）がなく mechanical_without_verify を照らせなかった回の数"""
    return sum(1 for a in record["attempts"] if _mechanical(policy_, a) and a["verified"] is None)


BACKFILL = "backfill"   # Engine の process_learn.record_conflict_files の source のうち、事後に埋めたもの（factoryctl learn backfill-conflicts）


def record_check(record: dict) -> list[dict]:
    """1 つの PR の記録で、continue.enqueued の時点で衝突したファイルがその場で記録されていない回を [{ rule, attempt, job_id, message }] で返す
    （知見 K-0014・議決 D-0049）。終わっていない回・終わらずに止まった回も照らす（積んだ時点で載っているはずなので、終わるのを待たない）。
    continue.enqueued の payload に conflict_files が形どおりに載っていれば、積んだその場の記録として通す（議決 D-0101・Issue #1208）"""
    errors = []
    for a in record["attempts"]:
        if a["files_inline"]:
            continue
        if a["files"] is None:
            why = "conflict.files がない"
        elif a["files_source"] == BACKFILL:
            why = "conflict.files が backfill で後から足されたものしかない"
        elif not a["files_before_start"]:
            why = "conflict.files がジョブの動き出したあと（job.started か job.finished より後）に記録された"
        else:
            continue
        errors.append({"rule": "missing_conflict_files", "attempt": a["attempt"], "job_id": a["job_id"],
                       "message": f"{a['attempt']} 回目の conflict の続きのジョブに、衝突したファイルがその場で記録されていない（{why}。"
                                  "同じファイルの衝突の型が集計で見えない。K-0014）"})
    return errors


def unknown(record: dict) -> dict:
    """照らせなかった項目の件数（記録がない）"""
    return {k: sum(1 for a in record["attempts"] if a[k] is None) for k in ("agent_resumed", "tier", "files")}


def main(argv: list[str]) -> int:
    def arg(name: str) -> str | None:
        return argv[argv.index(name) + 1] if name in argv and argv.index(name) + 1 < len(argv) else None

    import schema   # schema.py がこのモジュールを取り込むので、使うときに取り込む（_inline_spec と同じ）
    path, config = arg("--events"), Path(arg("--config") or policy.DEFAULT_CONFIG)
    if not path:
        print("conflict-check: --events <イベントの JSON か JSONL> を渡す", file=sys.stderr)
        return 2
    try:
        policy_ = policy.load(config)
    except (OSError, yaml.YAMLError, policy.PolicyError) as e:
        print(f"conflict-check: {config} を読めない: {e}", file=sys.stderr)
        return 2
    try:
        _inline_spec()
    except (OSError, schema.DefsError) as e:
        print(f"conflict-check: conflict-files.yaml の continue.enqueued の {INLINE} を読めない: {e}", file=sys.stderr)
        return 2
    since = None
    if "--since" in argv:
        try:
            since = schema.parse_time(arg("--since"))
        except ValueError as e:
            print(f"conflict-check: --since を時差つきの ISO 8601 の時刻で渡す: {e}", file=sys.stderr)
            return 2
    try:
        events = parse_events(Path(path).read_text(encoding="utf-8"))
        skip = schema.before(events, since)
        verify = parse_verify(Path(arg("--verify")).read_text(encoding="utf-8")) if arg("--verify") else None
    except (OSError, EventsError, schema.ev.EventsError) as e:   # コマンドで動かすと schema.py が取り込む events はこの __main__ と別のモジュール
        print(f"conflict-check: {path} か --verify を読めない: {e}", file=sys.stderr)
        return 2
    prs = records_from_events([e for i, e in enumerate(events) if i not in skip], arg("--pr"), verify)
    rules = record_check if "--record-only" in argv else (lambda r: check(policy_, r) + record_check(r))
    results = [{"pr_url": url, "attempts": len(r["attempts"]), "conflicted_heads": len(r["conflicted_heads"]),
                "unknown": unknown(r), "errors": rules(r),
                "mechanical": {"repeats": mechanical_repeats(policy_, r), "unverified": unverified(policy_, r)}} for url, r in prs.items()]
    errors = [e for r in results for e in r["errors"]]
    if "--json" in argv:
        json.dump({"ok": not errors, "prs": results}, sys.stdout, ensure_ascii=False)
        print()
    else:
        for r in results:
            for e in r["errors"]:
                print(f"{r['pr_url']} attempt={e['attempt']} job={e['job_id']} {e['rule']}: {e['message']}")
            u = ", ".join([f"{k} {n} 件" for k, n in r["unknown"].items() if n]
                          + ([f"機械的に解いた回の検査の結果 {r['mechanical']['unverified']} 件"] if r["mechanical"]["unverified"] else []))
            if r["mechanical"]["repeats"]:
                print(f"{r['pr_url']}: 同じファイルの機械的な解消が {policy_['max_attempts_per_pr']} 回を超えた: "
                      + ", ".join(f"{f} {n} 回" for f, n in r["mechanical"]["repeats"].items()) + "（衝突の元が PR に残っていないかを見る。K-0015）")
            print(f"{r['pr_url']}: conflict の続きのジョブ {r['attempts']} 回・衝突した head {r['conflicted_heads']} 個・外れ {len(r['errors'])} 件"
                  + (f"（記録がなく照らせなかった: {u}）" if u else ""))
        tail = f"（{since.isoformat()} より前の {len(skip)} 件は照らさない）" if since else ""
        print(f"conflict-check: PR {len(results)} 件・外れ {len(errors)} 件" + tail)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
