#!/usr/bin/env python3
"""知見の担保の配線と効き目の検査（知見 K-0004 の担保、議決 D-0015、Issue #962）。

.factory-config/knowledge/K-*.yaml の enforcement.solutions の各項目にある真偽の項目を見る。LLM を使わない。
  - wired: CI や呼び出しに配線済みか（.github/workflows/ や Engine の呼び出しから実際に走るか）
  - verified: 本物の記録で効き目を確かめたか（wired でないのに verified にはできない）
書いていない項目は落とさず false（未記入）として数え、未記入の件数を別に出す。

配線先の照合（議決 D-0021、Issue #986）: wired が true の解決策には wired_by（{workflow, job, runs} の列）を必須にし、
.github/workflows/ のそのファイルとジョブが実在して、ジョブの手順が runs の検査を呼んでいるかを
tools/sendback-check/order.mjs と同じやり方（注釈と行末の \ を畳んだ行、working-directory と cd、変数、npm scripts、
ラッパーのスクリプトを 3 段まで辿る）で照らし、なければ落とす。order.mjs と違い、迷う形は呼んでいない側に数える
（ここで見逃すと配線していない担保を配線済みと読ませるため）。

確認の記録の照合（議決 D-0034、Issue #1059）: verified が true の解決策には verified_by（本物の記録を指す case・pr・event の列。
evidence と同じく 1 項目に 1 つの参照）を必須にし、case を 1 件以上含めて、指す事例記録が .factory-config/knowledge/cases/ にあることと、
その解決策が wired: true であることを照らし、なければ落とす。

定期の配線の必須（議決 D-0039、Issue #1088）: REQUIRED_SCHEDULE の解決策（K-0004 の issue 1018 と K-0001 の issue 1106（議決 D-0044）。
branch protection の必須のチェックを protection.mjs で照らす）を wired: true にするときは、wired_by に、on.schedule（cron）のあるワークフローで、schedule でも走る
（if: が github.event_name を見るなら == 'schedule' を含み、!= 'schedule' を含まない）ジョブから runs の検査を呼ぶ項目を必須にする。手で走らせた記録だけでは wired にできない。
議決 D-0061（Issue #1163。起票時の番号は D-0058）で、schedule でも走るかの読み方を許可リストにした。ジョブと呼ぶ手順の if: は、無いか、
github.event_name == '<event>' を || でつないだだけで == 'schedule' を含む形（${{ }} で包んでもよい）だけを許し、それ以外の式
（&& や needs.*・always()・ほかの条件を || でつないだ飛ばし）は走らない側に数える。

呼ぶ検査の必須（議決 D-0042、Issue #1100）: REQUIRED_RUNS の解決策（K-0010 の issue 1034・1100。衝突の続きのジョブの検査）を wired: true にするときは、
wired_by に、.github/workflows/ の 1 つのジョブから挙げた検査（tools/conflict-check/test_watch.py・test_engine.py）をすべて呼ぶ項目を必須にする。
ほかの検査（test_policy.py だけなど）を呼ぶジョブだけでは wired にできない。

PR ごとの配線の必須（議決 D-0043、Issue #1103）: REQUIRED_PR の解決策（K-0010 の issue 1035 と 1103。job.finished の agent_resumed と
agent_tier を test_events.py の EngineFinishedTest で照らす）を wired: true にするときは、wired_by に、on.pull_request のあるワークフローで、
pull_request でも走る（if: が github.event_name を見るなら == 'pull_request' を含み、!= 'pull_request' を含まない）ジョブから
tools/conflict-check/test_events.py を呼ぶ項目を必須にする。ほかの検査（test_ci.py など）を呼ぶだけ、定期や手動だけでは wired にできない。
議決 D-0048（Issue #1129）で、K-0004 の issue 986・1059 の解決策（wired_by・verified_by の照合）も、pull_request のジョブから
この check.py を呼ぶ項目がないと wired にできないようにした（ci/knowledge-enforcement.yml を写した enforcement のジョブで満たす）。
議決 D-0052（Issue #1139）で、K-0004 の issue 1062 の解決策（README の一覧と K-*.yaml を照らす test_check.py のテスト）も、pull_request のジョブから
test_check.py を呼ぶ項目がないと wired にできないようにした（同じく enforcement のジョブで満たす）。
議決 D-0053（Issue #1144）で、K-0004 の issue 1063・1144 の解決策（Engine に移す差分を ENGINE_REF の版で確かめる engine-test のジョブ）も、
pull_request のジョブから test_engine_patch.py を呼ぶ項目がないと wired にできないようにした（ci/knowledge-enforcement.yml を写した engine-test の
unittest discover で満たす。check.py・test_check.py・test_ci.py を名指しで呼ぶ enforcement のジョブでは満たさない）。
議決 D-0059（Issue #1158。起票時の番号は D-0056）で、K-0004 の issue 1158 の解決策（protection.mjs を定期と PR ごとに走らせる）を
REQUIRED_SCHEDULE と REQUIRED_PR の両方に挙げ、schedule でも走るジョブと pull_request でも走るジョブの両方から
tools/sendback-check/protection.mjs を呼ぶ項目がないと wired にできないようにした（tools/sendback-check/ci/sendback-protection.yml を写した
protection と protection-pr のジョブで満たす）。

議決との照合（議決 D-0041、Issue #1094）: decided_by を書いた解決策は、それが committee:D-NNNN の形で、
.factory-config/knowledge/committee/D-NNNN.yaml があり、その議決の enforcement に同じ知見の id（knowledge）と
同じ種類（solution_kind が解決策の kind）の案があることを照らし、なければ decision_mismatch で落とす。

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
import re
import subprocess
import sys
from pathlib import Path

ROOT = ".factory-config/knowledge"
FLAGS = ("wired", "verified")
UNWIRED = "未配線"
UNVERIFIED = "deterministic（未確認）"
WORKFLOWS = ".github/workflows"
WIRED_BY_KEYS = ("workflow", "job", "runs")
VERIFIED_BY_KINDS = ("case", "pr", "event")
CASE_RE = re.compile(r"^C-\d{4,}$")
DECIDED_BY_RE = re.compile(r"^committee:(D-\d{4,})$")
DECISION_MISMATCH = "decision_mismatch"
EVENT_RE = re.compile(r"^[\w.-]+:\S+$")
# (知見の id, 解決策の issue) → schedule のジョブから呼ぶことを求める検査（議決 D-0039、Issue #1088。K-0001 は議決 D-0044、Issue #1106。
# K-0004 の issue 1158 は議決 D-0059（起票時の番号は D-0056）。REQUIRED_PR にも挙げ、schedule と pull_request の両方のジョブから呼ぶことを求める）
SENDBACK_PROTECTION = "tools/sendback-check/protection.mjs"
REQUIRED_SCHEDULE = {("K-0004", 1018): SENDBACK_PROTECTION,
                     ("K-0001", 1106): "tools/pr-body-check/protection.mjs",
                     ("K-0004", 1158): SENDBACK_PROTECTION}
# (知見の id, 解決策の issue) → (pull_request のジョブから呼ぶことを求める検査, 決めた議決)（議決 D-0043、Issue #1103。
# K-0004 の issue 986・1059 は議決 D-0048、Issue #1129。issue 1062 は議決 D-0052、Issue #1139。issue 1063・1144 は議決 D-0053、Issue #1144。
# issue 1158 は議決 D-0059（起票時の番号は D-0056）、Issue #1158）
KNOWLEDGE_CHECK = "tools/knowledge-enforcement-check/check.py"
KNOWLEDGE_TEST = "tools/knowledge-enforcement-check/test_check.py"
ENGINE_PATCH_TEST = "tools/knowledge-enforcement-check/test_engine_patch.py"
REQUIRED_PR = {("K-0010", 1035): ("tools/conflict-check/test_events.py", "D-0043"), ("K-0010", 1103): ("tools/conflict-check/test_events.py", "D-0043"),
               ("K-0004", 986): (KNOWLEDGE_CHECK, "D-0048"), ("K-0004", 1059): (KNOWLEDGE_CHECK, "D-0048"),
               ("K-0004", 1062): (KNOWLEDGE_TEST, "D-0052"),
               ("K-0004", 1063): (ENGINE_PATCH_TEST, "D-0053"), ("K-0004", 1144): (ENGINE_PATCH_TEST, "D-0053"),
               ("K-0004", 1158): (SENDBACK_PROTECTION, "D-0059")}
# (知見の id, 解決策の issue) → 1 つのジョブからすべて呼ぶことを求める検査（議決 D-0042、Issue #1100）
CONFLICT_TESTS = ("tools/conflict-check/test_watch.py", "tools/conflict-check/test_engine.py")
REQUIRED_RUNS = {("K-0010", 1034): CONFLICT_TESTS, ("K-0010", 1100): CONFLICT_TESTS}
SCRIPT_RE = re.compile(r"(?:^|[\s\"'=(])((?:\.{0,2}/)?[\w.\/-]+\.(?:py|sh|bash|mjs|cjs|js|ts))\b")


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


def logical_lines(text: str) -> list[str]:
    """shell の注釈（行頭か空白の後ろの #）を落とし、行末の \\ でつないだ行を 1 行にする（order.mjs の logicalLines と同じ）。"""
    out, cur = [], None
    for raw in text.splitlines():
        line = re.sub(r"(^|\s)#.*$", "", raw).rstrip()
        cur = line if cur is None else f"{cur} {line.strip()}"
        if cur.endswith("\\"):
            cur = cur[:-1]
        else:
            out.append(cur)
            cur = None
    if cur is not None:
        out.append(cur)
    return out


def norm(base: str, path: str) -> str | None:
    """base（リポジトリの根からの道）の下の path を根からの道にする。根の外に出る道は None。"""
    parts: list[str] = []
    for seg in f"{base}/{path}".split("/"):
        if seg in ("", "."):
            continue
        if seg == "..":
            if not parts:
                return None
            parts.pop()
        else:
            parts.append(seg)
    return "/".join(parts)


def runs_off(cond) -> bool:
    """if: が字面で偽（false・${{ false }}）なら走らない"""
    if cond is False:
        return True
    return isinstance(cond, str) and re.fullmatch(r"\s*(\$\{\{\s*)?false(\s*\}\})?\s*", cond) is not None


def expand(line: str, env: dict) -> str:
    """${{ env.NAME }}・${NAME}・$NAME を値に置き換える。github.workspace と GITHUB_WORKSPACE は根（空の道）にする。"""
    line = re.sub(r"\$\{\{\s*github\.workspace\s*\}\}/?|\$\{?GITHUB_WORKSPACE\}?/?", "", line)
    line = re.sub(r"\$\{\{\s*env\.(\w+)\s*\}\}", lambda m: str(env.get(m.group(1), m.group(0))), line)
    return re.sub(r"\$\{(\w+)\}|\$(\w+)", lambda m: str(env.get(m.group(1) or m.group(2), m.group(0))), line)


def calls(lines: list[str], target: str, cwd: str, env: dict, read, depth: int = 0) -> bool:
    """行の並び（1 つの手順の run か、ラッパーのスクリプト）が target（根からの道）の検査を呼ぶか。

    直の道（cwd か根から）、npm・pnpm の run（cwd の package.json の scripts）、unittest discover・node --test で
    target のあるディレクトリのテストを全部走らせる形、中身を読めるラッパーのスクリプト（3 段まで）を辿る。
    """
    env = dict(env)
    tdir, tname = target.rsplit("/", 1) if "/" in target else ("", target)
    is_test = re.match(r"test_.*\.py$|.*\.test\.[cm]?js$", tname) is not None
    for raw in lines:
        for m in re.finditer(r"(?:^|[;&|]\s*|\bexport\s+)([A-Za-z_]\w*)=([\"']?)([^\"'\s;]*)\2", raw):
            env[m.group(1)] = expand(m.group(3), env)
        line = expand(raw, env)
        for m in re.finditer(r"(?:^|[;&|]\s*)cd\s+[\"']?([^\"'\s;&|]+)", line):
            cwd = norm(cwd, m.group(1)) or ""
        for m in SCRIPT_RE.finditer(line):
            for base in dict.fromkeys((cwd, "")):
                rel = norm(base, m.group(1))
                if rel is None:
                    continue
                if rel == target:
                    return True
                inner = read(rel) if depth < 3 and rel.rsplit(".", 1)[-1] in ("sh", "bash") else None
                if inner is not None and calls(logical_lines(inner), target, cwd, env, read, depth + 1):
                    return True
        npm = re.search(r"\b(?:npm|pnpm)\b((?:\s+\S+)*?)\s+(?:run|run-script)\s+([\w:.-]+)|\b(?:npm|pnpm)\s+(test)\b", line)
        if npm and depth < 3:
            prefix = re.search(r"--prefix[=\s]+[\"']?([^\"'\s]+)", npm.group(1) or "")
            pdir = norm(cwd, prefix.group(1)) if prefix else cwd
            pkg = read(f"{pdir}/package.json" if pdir else "package.json") if pdir is not None else None
            try:
                body = (json.loads(pkg).get("scripts") or {}).get(npm.group(2) or npm.group(3)) if pkg else None
            except ValueError:
                body = None
            if isinstance(body, str) and calls(logical_lines(body), target, pdir, env, read, depth + 1):
                return True
        if is_test:
            node = re.search(r"\bnode\b.*--test\b(.*)$", line)
            if node and cwd == tdir and not re.search(r"\.[cm]?js\b", node.group(1)) and tname.endswith("js"):
                return True
            disc = re.search(r"\bunittest\s+discover\b(.*)$", line)
            if disc and tname.endswith(".py"):
                start = re.search(r"(?:-s|--start-directory)[=\s]+[\"']?([^\"'\s]+)", disc.group(1))
                if norm(cwd, start.group(1) if start else ".") == tdir:
                    return True
    return False


def job_calls(workflow: dict, job_id: str, target: str, read, off=runs_off) -> bool:
    """ワークフローの job_id のジョブの手順のどれかが target を呼ぶか（if: が字面で偽のジョブ・手順は走らないので数えない。off で差し替える）"""
    job = (workflow.get("jobs") or {}).get(job_id)
    if not isinstance(job, dict) or off(job.get("if")):
        return False
    wenv = {**(workflow.get("env") or {}), **(job.get("env") or {})}
    wd = (((job.get("defaults") or {}).get("run") or {}).get("working-directory")) or ""
    for step in job.get("steps") or []:
        if not isinstance(step, dict) or not isinstance(step.get("run"), str) or off(step.get("if")):
            continue
        env = {**wenv, **(step.get("env") or {})}
        cwd = norm("", expand(str(step.get("working-directory") or wd), env)) or ""
        if calls(logical_lines(step["run"]), target, cwd, env, read):
            return True
    return False


def repo_reader(repo: Path):
    """リポジトリの根からの道でファイルを読む関数（なければ None）"""
    def read(rel: str) -> str | None:
        p = repo / rel
        return p.read_text(encoding="utf-8") if rel and p.is_file() else None
    return read


def valid_wired_by(w) -> bool:
    """wired_by の項目が workflow・job・runs を空でない文字列で持つか"""
    return isinstance(w, dict) and all(isinstance(w.get(key), str) and w.get(key) for key in WIRED_BY_KEYS)


def is_workflow(path: str) -> bool:
    """.github/workflows/ の直下の YAML か（ひな形の ci/*.yml は配線ではない）"""
    return re.fullmatch(rf"{re.escape(WORKFLOWS)}/[^/]+\.ya?ml", path) is not None


def check_wiring(k: dict, repo: Path) -> list[str]:
    """wired が true の解決策の wired_by を照らす（議決 D-0021、Issue #986）"""
    f, e = k.get("_file", str(k.get("id"))), []
    read = repo_reader(repo)

    for i, s in enumerate(solutions(k)):
        if not isinstance(s, dict) or s.get("wired") is not True:
            continue
        where = f"{f}: enforcement.solutions[{i}]（{s.get('kind')}、issue {s.get('issue')}）"
        wb = s.get("wired_by")
        if not isinstance(wb, list) or not wb:
            e.append(f"{where}: wired が true なのに wired_by（配線先の workflow・job・runs の列）がない")
            continue
        for j, w in enumerate(wb):
            at = f"{where}: wired_by[{j}]"
            if not valid_wired_by(w):
                e.append(f"{at}: workflow・job・runs を文字列で書く（{w!r}）")
                continue
            wf, job, target = w["workflow"], w["job"], norm("", w["runs"])
            if not is_workflow(wf):
                e.append(f"{at}: workflow は {WORKFLOWS}/ の直下のファイル（{wf}。ひな形の ci/*.yml は配線ではない）")
                continue
            if read(wf) is None:
                e.append(f"{at}: {wf} がない")
                continue
            if not target or read(target) is None:
                e.append(f"{at}: runs の {w['runs']} がない")
                continue
            try:
                doc = load_yaml(repo / wf)
            except Exception as x:  # noqa: BLE001
                e.append(f"{at}: {wf} が読めない（{x}）")
                continue
            if not isinstance(doc, dict) or not isinstance((doc.get("jobs") or {}).get(job), dict):
                e.append(f"{at}: {wf} に job {job} がない")
            elif not job_calls(doc, job, target, read):
                e.append(f"{at}: {wf} の job {job} が {target} を呼んでいない")
    return e


def triggers(workflow: dict):
    """on: の値（PyYAML も ruby も on: を true と読むので両方を見る）"""
    return next((workflow[key] for key in ("on", True, "true") if key in workflow), None)


def has_schedule(workflow: dict) -> bool:
    """on.schedule に cron が 1 つ以上あるか"""
    on = triggers(workflow)
    sched = on.get("schedule") if isinstance(on, dict) else None
    return isinstance(sched, list) and any(isinstance(c, dict) and isinstance(c.get("cron"), str) and c["cron"].strip() for c in sched)


def off_event(event: str):
    """event では走らないと数える if: を見る関数（字面で偽か、github.event_name を見るのに == '<event>' がないか != '<event>' がある）。迷う形は走らない側に数える"""
    def off(cond) -> bool:
        if runs_off(cond):
            return True
        if not isinstance(cond, str) or "event_name" not in cond:
            return False
        return (not re.search(rf"event_name\s*==\s*['\"]{event}['\"]", cond)) or re.search(rf"!=\s*['\"]{event}['\"]", cond) is not None
    return off


SCHEDULE_IF_TERM_RE = re.compile(r"\s*github\.event_name\s*==\s*'([\w-]+)'\s*")


def off_schedule(cond) -> bool:
    """schedule では走らないと数える if:（許可リスト。議決 D-0061（起票時の番号は D-0058）、Issue #1163）。

    if: が無いか、github.event_name == '<event>' を || でつないだだけの式（${{ }} で包んでもよい）で、その中に == 'schedule' があるときだけ
    走ると数える。それ以外の式（&& や needs・always()・ほかの条件を || でつないだもの、真偽の値も）は走らない側に数える。
    """
    if cond is None:
        return False
    if not isinstance(cond, str):
        return True
    m = re.fullmatch(r"\s*\$\{\{(.*)\}\}\s*", cond, re.S)
    terms = [SCHEDULE_IF_TERM_RE.fullmatch(t) for t in (m.group(1) if m else cond).split("||")]
    return not all(terms) or "schedule" not in {t.group(1) for t in terms}


def has_pull_request(workflow: dict) -> bool:
    """on に pull_request があるか（on: pull_request・on: [pull_request, ...]・on: {pull_request: ...} のどれでも）"""
    on = triggers(workflow)
    return on == "pull_request" or (isinstance(on, (list, dict)) and "pull_request" in on)


def check_schedule(k: dict, repo: Path) -> list[str]:
    """REQUIRED_SCHEDULE の解決策が wired: true なら、schedule のジョブから検査を呼ぶ wired_by を求める（議決 D-0039、Issue #1088）"""
    f, e = k.get("_file", str(k.get("id"))), []
    read = repo_reader(repo)

    for i, s in enumerate(solutions(k)):
        if not isinstance(s, dict) or s.get("wired") is not True:
            continue
        target = REQUIRED_SCHEDULE.get((k.get("id"), s.get("issue")))
        if target is None:
            continue
        ok = False
        for w in s.get("wired_by") if isinstance(s.get("wired_by"), list) else []:
            if not valid_wired_by(w) or norm("", w["runs"]) != target or not is_workflow(w["workflow"]) or read(w["workflow"]) is None:
                continue
            try:
                doc = load_yaml(repo / w["workflow"])
            except Exception:  # noqa: BLE001  読めないことは check_wiring が落とす
                continue
            if isinstance(doc, dict) and has_schedule(doc) and job_calls(doc, w["job"], target, read, off_schedule):
                ok = True
                break
        if not ok:
            where = f"{f}: enforcement.solutions[{i}]（{s.get('kind')}、issue {s.get('issue')}）"
            e.append(f"{where}: wired にするには wired_by に、on.schedule（cron）のある {WORKFLOWS}/ のワークフローで schedule でも走るジョブから"
                     f" {target} を呼ぶ項目が要る（手で走らせた記録だけでは wired にできない。議決 D-0039。"
                     f"ジョブと手順の if: は無いか、github.event_name == 'schedule' を || でつないだ決まった形だけを許す。議決 D-0061）")
    return e


def check_pull_request(k: dict, repo: Path) -> list[str]:
    """REQUIRED_PR の解決策が wired: true なら、pull_request のジョブから検査を呼ぶ wired_by を求める（議決 D-0043、Issue #1103）"""
    f, e = k.get("_file", str(k.get("id"))), []
    read = repo_reader(repo)

    for i, s in enumerate(solutions(k)):
        if not isinstance(s, dict) or s.get("wired") is not True:
            continue
        required = REQUIRED_PR.get((k.get("id"), s.get("issue")))
        if required is None:
            continue
        target, decision = required
        ok = False
        for w in s.get("wired_by") if isinstance(s.get("wired_by"), list) else []:
            if not valid_wired_by(w) or norm("", w["runs"]) != target or not is_workflow(w["workflow"]) or read(w["workflow"]) is None:
                continue
            try:
                doc = load_yaml(repo / w["workflow"])
            except Exception:  # noqa: BLE001  読めないことは check_wiring が落とす
                continue
            if isinstance(doc, dict) and has_pull_request(doc) and job_calls(doc, w["job"], target, read, off_event("pull_request")):
                ok = True
                break
        if not ok:
            where = f"{f}: enforcement.solutions[{i}]（{s.get('kind')}、issue {s.get('issue')}）"
            e.append(f"{where}: wired にするには wired_by に、on.pull_request のある {WORKFLOWS}/ のワークフローで pull_request でも走るジョブから"
                     f" {target} を呼ぶ項目が要る（ほかの検査を呼ぶだけ・定期や手動だけでは wired にできない。議決 {decision}）")
    return e


def check_required_runs(k: dict, repo: Path) -> list[str]:
    """REQUIRED_RUNS の解決策が wired: true なら、1 つのジョブから挙げた検査をすべて呼ぶ wired_by を求める（議決 D-0042、Issue #1100）"""
    f, e = k.get("_file", str(k.get("id"))), []
    read = repo_reader(repo)

    for i, s in enumerate(solutions(k)):
        if not isinstance(s, dict) or s.get("wired") is not True:
            continue
        targets = REQUIRED_RUNS.get((k.get("id"), s.get("issue")))
        if targets is None:
            continue
        jobs: dict[tuple[str, str], set[str]] = {}
        for w in s.get("wired_by") if isinstance(s.get("wired_by"), list) else []:
            if valid_wired_by(w) and is_workflow(w["workflow"]) and read(w["workflow"]) is not None:
                jobs.setdefault((w["workflow"], w["job"]), set()).add(norm("", w["runs"]) or "")
        ok = False
        for (wf, job), runs in jobs.items():
            if not set(targets) <= runs:
                continue
            try:
                doc = load_yaml(repo / wf)
            except Exception:  # noqa: BLE001  読めないことは check_wiring が落とす
                continue
            if isinstance(doc, dict) and all(job_calls(doc, job, t, read) for t in targets):
                ok = True
                break
        if not ok:
            where = f"{f}: enforcement.solutions[{i}]（{s.get('kind')}、issue {s.get('issue')}）"
            e.append(f"{where}: wired にするには wired_by に、{WORKFLOWS}/ の 1 つのジョブから {'・'.join(targets)} をすべて呼ぶ項目"
                     f"（検査ごとに 1 項目、同じ workflow・job）が要る（議決 D-0042）")
    return e


def check_verification(k: dict, repo: Path) -> list[str]:
    """verified が true の解決策の verified_by を照らす（議決 D-0034、Issue #1059）。wired が true であることは check_item が見る"""
    f, e = k.get("_file", str(k.get("id"))), []
    for i, s in enumerate(solutions(k)):
        if not isinstance(s, dict) or s.get("verified") is not True:
            continue
        where = f"{f}: enforcement.solutions[{i}]（{s.get('kind')}、issue {s.get('issue')}）"
        vb = s.get("verified_by")
        if not isinstance(vb, list) or not vb:
            e.append(f"{where}: verified が true なのに verified_by（確かめた本物の記録を指す case・pr・event の列）がない")
            continue
        cases = [v["case"] for v in vb if isinstance(v, dict) and len(v) == 1 and "case" in v]
        for j, v in enumerate(vb):
            at = f"{where}: verified_by[{j}]"
            if not isinstance(v, dict) or len(v) != 1 or next(iter(v)) not in VERIFIED_BY_KINDS:
                e.append(f"{at}: 1 項目に 1 つの参照で、種類は {'・'.join(VERIFIED_BY_KINDS)} のどれか（例: - case: C-0001）（{v!r}）")
                continue
            (kind, val), = v.items()
            if kind == "case":
                if not isinstance(val, str) or not CASE_RE.match(val):
                    e.append(f"{at}: case は C-NNNN（{val!r}）")
                elif not (repo / ROOT / "cases" / f"{val}.md").is_file():
                    e.append(f"{at}: 事例記録 {val} が {ROOT}/cases/ にない")
            elif kind == "pr" and (isinstance(val, bool) or not isinstance(val, int) or val <= 0):
                e.append(f"{at}: pr は PR の番号（正の整数）（{val!r}）")
            elif kind == "event" and (not isinstance(val, str) or not EVENT_RE.match(val)):
                e.append(f"{at}: event は <種類>:<id>（例: case.analyzed:40376）（{val!r}）")
        if not cases:
            e.append(f"{where}: verified_by に {ROOT}/cases/ にある事例記録（case）が 1 件もない（pr・event だけでは本物の記録か照らせない）")
    return e


def check_decision(k: dict, repo: Path, cache: dict | None = None) -> list[str]:
    """decided_by を書いた解決策が、指す議決の enforcement の案（同じ knowledge と solution_kind）に当たるかを照らす（議決 D-0041、Issue #1094）。
    decided_by を書いていない解決策は見ない"""
    f, e = k.get("_file", str(k.get("id"))), []
    cache = {} if cache is None else cache
    for i, s in enumerate(solutions(k)):
        if not isinstance(s, dict) or "decided_by" not in s:
            continue
        where = f"{f}: enforcement.solutions[{i}]（{s.get('kind')}、issue {s.get('issue')}）: {DECISION_MISMATCH}"
        dec = s["decided_by"]
        m = DECIDED_BY_RE.match(dec) if isinstance(dec, str) else None
        if not m:
            e.append(f"{where}: decided_by は committee:D-NNNN（{dec!r}）")
            continue
        did = m.group(1)
        if did not in cache:
            p = repo / ROOT / "committee" / f"{did}.yaml"
            try:
                cache[did] = load_yaml(p) if p.is_file() else None
            except Exception as x:  # noqa: BLE001
                cache[did] = x
        d = cache[did]
        if d is None:
            e.append(f"{where}: 議決 {did} が {ROOT}/committee/ にない")
            continue
        if isinstance(d, Exception):
            e.append(f"{where}: 議決 {did} が読めない（{d}）")
            continue
        plans = d.get("enforcement") if isinstance(d, dict) else None
        if not any(isinstance(p, dict) and p.get("knowledge") == k.get("id") and p.get("solution_kind") == s.get("kind")
                   for p in (plans if isinstance(plans, list) else [])):
            e.append(f"{where}: 議決 {did} の enforcement に {k.get('id')} の {s.get('kind')} の案がない")
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
    repo, decisions = Path(a.repo), {}
    errors = [line for k in items for line in check_item(k) + check_wiring(k, repo) + check_schedule(k, repo) + check_pull_request(k, repo) + check_required_runs(k, repo) + check_verification(k, repo) + check_decision(k, repo, decisions)]
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
