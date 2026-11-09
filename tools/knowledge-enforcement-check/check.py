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
議決 D-0072（Issue #1200。起票時の番号は D-0071）で、K-0004 の issue 1129 の解決策（この段そのもの）も REQUIRED_PR に挙げ、issue 986・1059・1129 の
解決策は REQUIRED_PR_WORKFLOW の .github/workflows/knowledge-enforcement.yml のジョブを指す wired_by でないと数えないようにした
（ほかのワークフローの pull_request のジョブから check.py を呼ぶだけでは、ひな形の写しがなくても wired と読ませてしまうため）。
議決 D-0072（Issue #1201。起票時の番号は D-0071）の test の案で、その線を知見と issue の列挙に頼らない形に広げた。wired: true の解決策はどれも、
KNOWLEDGE_GATE の .github/workflows/knowledge-enforcement.yml の pull_request でも走るジョブから check.py を呼ぶ項目が wired_by にないと
knowledge_gate_unwired で落とす（K-0004 の issue 986・1059・1129 は REQUIRED_PR・REQUIRED_PR_WORKFLOW から外し、この線で見る。
組を足し忘れたほかの知見の解決策が黙って通らないように）。
議決 D-0052（Issue #1139）で、K-0004 の issue 1062 の解決策（README の一覧と K-*.yaml を照らす test_check.py のテスト）も、pull_request のジョブから
test_check.py を呼ぶ項目がないと wired にできないようにした（同じく enforcement のジョブで満たす）。
議決 D-0075（Issue #1215）で、K-0004 の issue 1138 の解決策（.factory-config/knowledge/README.md の一覧も K-*.yaml と照らす test_check.py のテスト）も
REQUIRED_PR に挙げ、issue 1062・1138 の解決策は REQUIRED_PR_WORKFLOW の .github/workflows/knowledge-enforcement.yml のジョブを指す wired_by でないと
数えないようにした（ひな形 ci/knowledge-enforcement.yml の写しの必須チェックでなく、ほかのワークフローの pull_request のジョブから test_check.py を呼ぶだけでは、
一覧がずれた PR を止めるとは読めないため）。
議決 D-0053（Issue #1144）で、K-0004 の issue 1063・1144 の解決策（Engine に移す差分を ENGINE_REF の版で確かめる engine-test のジョブ）も、
pull_request のジョブから test_engine_patch.py を呼ぶ項目がないと wired にできないようにした（ci/knowledge-enforcement.yml を写した engine-test の
unittest discover で満たす。check.py・test_check.py・test_ci.py を名指しで呼ぶ enforcement のジョブでは満たさない）。
議決 D-0059（Issue #1158。起票時の番号は D-0056）で、K-0004 の issue 1158 の解決策（protection.mjs を定期と PR ごとに走らせる）を
REQUIRED_SCHEDULE と REQUIRED_PR の両方に挙げ、schedule でも走るジョブと pull_request でも走るジョブの両方から
tools/sendback-check/protection.mjs を呼ぶ項目がないと wired にできないようにした（tools/sendback-check/ci/sendback-protection.yml を写した
protection と protection-pr のジョブで満たす）。
議決 D-0060（Issue #1160。起票時の番号は D-0056）で、K-0004 の issue 1087（sendback-protection.yml のひな形）と issue 1018 の解決策は、
REQUIRED_SCHEDULE_WORKFLOW の .github/workflows/sendback-protection.yml のジョブを指す wired_by でないと数えないようにした
（ほかのワークフローの schedule のジョブから protection.mjs を呼ぶだけでは、ひな形の写しがなくても wired と読ませてしまうため）。
議決 D-0086（Issue #1247 の起票時の番号は D-0085。PR #1243）の lint の案で、その線を issue 番号の列挙に頼らない形に広げた。wired: true の解決策で、
wired_by に tools/sendback-check/protection.mjs を呼ぶ項目を持つものは、知見と issue を問わずどれも、SENDBACK_PROTECTION_WORKFLOW の
.github/workflows/sendback-protection.yml の schedule でも走るジョブから protection.mjs を呼ぶ項目がないと wired にできない
（別の issue 番号で足した見張りが、REQUIRED_SCHEDULE・REQUIRED_SCHEDULE_WORKFLOW に組を足し忘れると、ほかのワークフローや PR のジョブだけで通っていたため。
issue 1158 の解決策もワークフローの名前を問わなくなった）。
議決 D-0061（Issue #1162。起票時の番号は D-0058）で、K-0004 の issue 1162 の解決策（sendback-protection の定期の実行が最後に通ってから
2 日より長ければ落とす heartbeat.mjs）を REQUIRED_SCHEDULE に挙げ、schedule でも走るジョブから tools/sendback-check/heartbeat.mjs を呼ぶ項目がないと
wired にできないようにした（tools/sendback-check/ci/sendback-protection-heartbeat.yml を写した heartbeat のジョブで満たす）。
議決 D-0070（Issue #1182。起票時の番号は D-0067）で、K-0001 の issue 1182 の解決策（建設者が pr-body-engine.yml を写して pr-body-engine-test を
必須のチェックにし、protection.mjs を定期に走らせる）を REQUIRED_SCHEDULE と REQUIRED_PR の両方に挙げ、REQUIRED_SCHEDULE_WORKFLOW の
.github/workflows/pr-body-protection.yml の schedule でも走るジョブから tools/pr-body-check/protection.mjs を呼ぶ項目と、REQUIRED_PR_WORKFLOW の
.github/workflows/pr-body-engine.yml の pull_request でも走るジョブから tools/pr-body-check/engine.test.mjs を呼ぶ項目の両方がないと wired にできないようにした
（tools/pr-body-check/ci/pr-body-protection.yml を写した protection と、ci/pr-body-engine.yml を写した engine-test のジョブで満たす）。
議決 D-0100（Issue #1196。起票時の番号は D-0071）で、K-0012 の issue 1122・1196 の解決策と K-0006 の issue 906 の解決策（同じ check.mjs を使う
マージ前の必須チェック pr-body-check）を REQUIRED_PR に挙げ、REQUIRED_PR_WORKFLOW の .github/workflows/pr-body-check.yml の pull_request でも走るジョブから
tools/pr-body-check/check.mjs を呼ぶ項目がないと wired にできないようにした（tools/pr-body-check/ci/pr-body-check.yml を写した pr-body-check のジョブで満たす）。
K-0012 の 2 つは REQUIRED_PR_ALSO で .github/workflows/pr-body-engine.yml の pull_request でも走るジョブから tools/pr-body-check/engine.test.mjs
（(12) が merge_step の待ちを確かめる）を呼ぶ項目も求め、下の「Engine の merge_step の照合」も通らないと wired にできない。
議決 D-0100（Issue #1197。起票時の番号は D-0071）の test の案で、K-0012 の issue 1197 の解決策（recheck.test.mjs・engine.test.mjs・
ci.test.mjs を PR ごとに走らせ、ひな形と Engine の待つチェックの名前のずれと、待ちの線を外す書き換えをマージ前に落とす）を REQUIRED_PR に挙げ、
REQUIRED_PR_WORKFLOW の .github/workflows/pr-body-engine.yml の pull_request でも走るジョブから tools/pr-body-check/recheck.test.mjs を呼ぶ項目と、
REQUIRED_PR_ALSO で同じワークフローから tools/pr-body-check/engine.test.mjs と tools/pr-body-check/ci.test.mjs を呼ぶ項目がそれぞれないと
wired にできないようにした（ci/pr-body-engine.yml を写した engine-test の node --test で 3 つとも満たす）。
議決 D-0101（Issue #1207。起票時の番号は D-0073）で、K-0014 の issue 1136 の解決策（衝突したファイルの列を必須にした定義と schema.py）と
issue 1207 の解決策（それを直近の窓のイベント列に対して定期に走らせる）を REQUIRED_SCHEDULE に挙げ、REQUIRED_SCHEDULE_WORKFLOW の
.github/workflows/conflict-schema.yml の schedule でも走るジョブから tools/conflict-check/schema.py を呼ぶ項目がないと wired にできないようにした
（tools/conflict-check/ci/conflict-schema.yml を写した conflict-schema のジョブで満たす）。
議決 D-0076（Issue #1214。起票時の番号は D-0074）で、K-0014 の issue 1214 の解決策（events.py の missing_conflict_files を直近の窓に積まれた
衝突の続きのジョブに対して定期に走らせる）を REQUIRED_SCHEDULE に挙げ、REQUIRED_SCHEDULE_WORKFLOW の .github/workflows/conflict-schema.yml の
schedule でも走るジョブから tools/conflict-check/events.py を呼ぶ項目がないと wired にできないようにした（同じひな形の conflict-record のジョブで満たす）。
議決 D-0076（Issue #1213。起票時の番号は D-0074）で、K-0014 の issue 1213 の解決策（衝突の続きの handoff.saved にも conflict_files を求める定義を
Engine の handoff.saved の形で確かめた schema の解決策。照らすのは同じ schema.py）を REQUIRED_SCHEDULE に挙げ、REQUIRED_SCHEDULE_WORKFLOW の
.github/workflows/conflict-schema.yml の schedule でも走るジョブから tools/conflict-check/schema.py を呼ぶ項目がないと wired にできないようにした
（issue 1136・1207 と同じ conflict-schema のジョブで満たす）。

呼ぶ検査を解決策に書く（議決 D-0077（Issue #1216。起票時の番号は D-0075）の schema の案）: 上で REQUIRED_PR・REQUIRED_PR_WORKFLOW・REQUIRED_PR_ALSO に
(知見の id, 解決策の issue) の名前で並べていた組をやめ、解決策の requires_check（検査の道か、道か {runs, workflow} の列）に書く形に一般化した。
wired: true の解決策は、requires_check に挙げた検査ごとに、on.pull_request のあるワークフロー（workflow を書いた検査はそのファイル）の pull_request でも走る
ジョブからそれを呼ぶ wired_by の項目がないと requires_check_unwired で落とす。requires_check の形の誤りと、挙げた検査がリポジトリにないことは、
wired を問わず requires_check_invalid で落とす。上で REQUIRED_PR などに挙げたと書いた組は、今はそれぞれの解決策の requires_check に同じ中身で書いてある
（組を check.py に足し忘れると、その解決策が wired: true でも黙って通っていた。検査の求めを解決策そのものに置き、足す場所を 1 つにする）。
議決 D-0080（Issue #1227 の起票時の番号は D-0078）の lint の案で、wired: true の解決策は requires_check を書くか、REQUIRED_SCHEDULE・REQUIRED_RUNS に
挙げてあることを求め、どちらでもなければ requires_check_missing で落とすようにした（requires_check は書いた解決策しか照らさないので、書き忘れると
KNOWLEDGE_GATE の check.py の配線だけで wired と読ませてしまい、組の足し忘れと同じ穴が欄の書き忘れとして残っていた）。

Engine の merge_step の照合（議決 D-0100（起票時の番号は D-0071）、Issue #1196）: REQUIRED_ENGINE_WAIT の解決策（K-0012 の issue 1122・1196）が
wired: true なら、.github/workflows/pr-body-engine.yml の ENGINE_REF の版の Engine の orchestrator/dispatcher/supervisor.py で、merge_step が
body_recheck_pending（tools/pr-body-check/engine/pr-body-recheck.patch で足す、直した本文の pr-body-check を待つ関数）を定義どおりに呼んでいることを
構文木で照らし、呼んでいなければ engine_unwired で落とす。Engine は --engine か ENGINE_DIR の全履歴のチェックアウトから版のファイルを取り出す。
Engine のチェックアウトがない・写しや ENGINE_REF がない・版がない・読めないときは照らせないとして engine_unverifiable（<HoldUncheckable の kind>）で落とす。
Engine に差分を入れて ENGINE_REF を上げる前に、ワークフローの写しだけで wired: true にする変更を止める。

議決との照合（議決 D-0041、Issue #1094）: decided_by を書いた解決策は、それが committee:D-NNNN の形で、
.factory-config/knowledge/committee/D-NNNN.yaml があり、その議決の enforcement に同じ知見の id（knowledge）と
同じ種類（solution_kind が解決策の kind）の案があることを照らし、なければ decision_mismatch で落とす。
議決 D-0122（Issue #1232 の起票時の番号は D-0080。PR #1228）の lint の案で、番号を取り違えた先にも同じ知見と種類の案があると
通ってしまう穴を塞いだ。what の最初の「議決 D-NNNN」が decided_by の番号と違う、その直後の括弧に書いた「PR #N」が
指す議決の pr（…/pull/N）と違う（議決に pr がないときも）ときも decision_mismatch で落とす。what に議決や PR を書いていなければその照合はしない。

engine-head の記録の照合（議決 D-0047、Issue #1114）: HEAD_RECORD_SOLUTIONS の解決策（K-0004 の issue 977・1046・1047。engine-head の定期の実行と
その記録）が verified: true なら、--head-comments に渡した Issue #948 のコメント（gh api の出力）で tools/sendback-check/head-record.mjs を走らせ、
engine-head の記録がなければ head_record_missing、コメントを渡していない・読めない・head-record.mjs が結果を出さないときは head_record_unchecked で落とす。
Issue #948 に engine-head の記録が残る前に verified を true にする変更を止める。

review_by の先送りの照合（議決 D-0067（起票時の番号は D-0066）、Issue #1178）: --base に前の版（PR の base。例: origin/main）を渡すと、
K-0010 の review_by をその版より後ろへ動かす（日付でなくする場合も含む）ときに、tools/conflict-check/ci/conflict-check.yml（写しがあれば
.github/workflows/conflict-check.yml）の ENGINE_REF の版の Engine で、test_events.py の EngineFinishedTest が todo のまま（job.finished に
agent_resumed も agent_tier も出ていない）なら落とす（review_by を延ばすだけで、期限切れで落ちるのを先送りできないように）。Engine は --engine か
ENGINE_DIR の全履歴のチェックアウトから版の dispatcher.py を取り出し、tools/conflict-check/engine_harness.py の finished で欄を読む。
求める欄は test_events.py の FINISHED_FIELDS を読む。Engine のチェックアウトがない・版がない・欄を読めないときは照らせないとして落とす。
落とす行には種類の名前を添える（review_by_hold、照らせないときは review_by_unverifiable（<HoldUncheckable の kind>））。
--base を渡さないとき、前の版に K-0010 がないとき、前の版の review_by が日付でないときは見ない。

直した知見の記入の必須（議決 D-0071、Issue #1193）: --base を渡すと、PR の差分で変わった K-*.yaml（--base と HEAD の分かれ目（merge-base）の版から
作業ツリーで中身が変わったか、その版になかったもの）の enforcement.solutions の各項目に wired と verified の両方が書かれていなければ、
solution_fields_missing で落とす（未記入の解決策を残したまま知見を直す変更セットを止める）。変えていない知見の未記入は見ない。
未記入の解決策のある知見で分かれ目を求められない・その版を読めないときは solution_fields_unverifiable で落とす。

写しの配線の記録（議決 D-0080（起票時の番号は D-0078）の ci_check の案、Issue #1226）: .github/workflows/knowledge-enforcement.yml（ひな形の写し）が
実在するのに、MIRROR_WIRED の解決策（K-0004 の issue 1063・1144。engine-test のジョブ）が wired: true でない（false・未記入・真偽でない値）なら
mirror_unwired で落とす。建設者が写したあとに記録の更新が遅れて、未配線と表示され続けるずれを止める。写しがなければ見ない。

担保の表示（この検査の出力に出す値。Engine の core-gates/knowledge_check.py に移したら INDEX.md の担保の列もこの値にする予定）:
  - level が deterministic でない: level のまま（context・checklist）
  - deterministic で、wired の解決策が 1 件もない: 未配線（deterministic とは出さない）
  - deterministic で、wired はあるが wired かつ verified が 1 件もない: deterministic（未確認）
  - deterministic で、wired かつ verified が 1 件以上: deterministic

使い方:
  python3 tools/knowledge-enforcement-check/check.py [--repo .] [--json out.json] [--summary "$GITHUB_STEP_SUMMARY"] [--base origin/main --engine .engine] [--head-comments comments.jsonl]
  終了コード: 0 = 緑、1 = 通らない、2 = 読めなかった

依存: python3（engine-head の記録の照合は node も）。YAML は PyYAML があればそれ、なければ ruby で読む（Engine の knowledge_check.py と同じ）
"""
from __future__ import annotations

import argparse
import ast
import json
import os
import re
import subprocess
import sys
import tempfile
from datetime import date
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
# what の最初の「議決 D-NNNN」と、その直後の括弧（入れ子なし）。括弧の中の「PR #N」を議決の pr と照らす（議決 D-0122（起票時の番号は D-0080）、Issue #1232）
WHAT_DECISION_RE = re.compile(r"議決 (D-\d{4,})(?:（([^（）]*)）)?")
WHAT_PR_RE = re.compile(r"PR #(\d+)")
DECISION_PR_RE = re.compile(r"/pull/(\d+)$")
EVENT_RE = re.compile(r"^[\w.-]+:\S+$")
# (知見の id, 解決策の issue) → schedule のジョブから呼ぶことを求める検査（議決 D-0039、Issue #1088。K-0001 は議決 D-0044、Issue #1106。
# K-0004 の issue 1158 は議決 D-0059（起票時の番号は D-0056）。requires_check にも書き、schedule と pull_request の両方のジョブから呼ぶことを求める。
# K-0004 の issue 1087 は議決 D-0060（起票時の番号は D-0056）、Issue #1160。
# K-0004 の issue 1162 は議決 D-0061（起票時の番号は D-0058）。sendback-protection が最後に通ってからの時間を照らす heartbeat.mjs。
# K-0001 の issue 1182 は議決 D-0070（起票時の番号は D-0067）。requires_check にも書く。
# K-0014 の issue 1136・1207 は議決 D-0101（起票時の番号は D-0073）、Issue #1207。衝突したファイルの列を直近の窓のイベント列で照らす schema.py。
# K-0014 の issue 1214 は議決 D-0076（起票時の番号は D-0074）、Issue #1214。直近の窓に積まれた衝突の続きのジョブを照らす events.py）
SENDBACK_PROTECTION = "tools/sendback-check/protection.mjs"
PR_BODY_PROTECTION = "tools/pr-body-check/protection.mjs"
SENDBACK_HEARTBEAT = "tools/sendback-check/heartbeat.mjs"
CONFLICT_SCHEMA = "tools/conflict-check/schema.py"
CONFLICT_EVENTS = "tools/conflict-check/events.py"
REQUIRED_SCHEDULE = {("K-0004", 1018): SENDBACK_PROTECTION,
                     ("K-0001", 1106): PR_BODY_PROTECTION,
                     ("K-0001", 1182): PR_BODY_PROTECTION,
                     ("K-0004", 1158): SENDBACK_PROTECTION,
                     ("K-0004", 1087): SENDBACK_PROTECTION,
                     ("K-0004", 1162): SENDBACK_HEARTBEAT,
                     ("K-0014", 1136): CONFLICT_SCHEMA,
                     ("K-0014", 1207): CONFLICT_SCHEMA,
                     ("K-0014", 1213): CONFLICT_SCHEMA,
                     ("K-0014", 1214): CONFLICT_EVENTS}
# (知見の id, 解決策の issue) → (REQUIRED_SCHEDULE の検査を呼ぶジョブがあることを求めるワークフロー, 落ちたときに挙げる議決)（議決 D-0060、Issue #1160）
SENDBACK_PROTECTION_WORKFLOW = f"{WORKFLOWS}/sendback-protection.yml"
REQUIRED_SCHEDULE_WORKFLOW = {("K-0004", 1018): (SENDBACK_PROTECTION_WORKFLOW, "D-0039・D-0060"),
                              ("K-0004", 1087): (SENDBACK_PROTECTION_WORKFLOW, "D-0039・D-0060"),
                              ("K-0001", 1182): (f"{WORKFLOWS}/pr-body-protection.yml", "D-0070"),
                              ("K-0014", 1136): (f"{WORKFLOWS}/conflict-schema.yml", "D-0101"),
                              ("K-0014", 1207): (f"{WORKFLOWS}/conflict-schema.yml", "D-0101"),
                              ("K-0014", 1213): (f"{WORKFLOWS}/conflict-schema.yml", "D-0076"),
                              ("K-0014", 1214): (f"{WORKFLOWS}/conflict-schema.yml", "D-0076")}
# wired_by に SENDBACK_PROTECTION を呼ぶ項目を持つ wired: true の解決策は、知見と issue を問わずどれも、SENDBACK_PROTECTION_WORKFLOW の
# schedule でも走るジョブから呼ぶ項目を要する（議決 D-0086（Issue #1247 の起票時の番号は D-0085）の lint の案。別の issue 番号で足した見張りが、
# REQUIRED_SCHEDULE・REQUIRED_SCHEDULE_WORKFLOW に組を足し忘れるとすり抜けないように）
SENDBACK_PROTECTION_DECISION = "D-0086"
KNOWLEDGE_CHECK = "tools/knowledge-enforcement-check/check.py"
# 解決策が pull_request でも走るジョブから呼ぶことを求める検査は、その解決策の requires_check に書く（議決 D-0077（起票時の番号は D-0075）の schema の案、Issue #1216）。
# 前は (知見の id, 解決策の issue) の名前で並べた組（REQUIRED_PR・REQUIRED_PR_WORKFLOW・REQUIRED_PR_ALSO）をここに持っていて、
# 組を足し忘れた解決策は wired: true にしても黙って通っていた。前の組は同じ中身でそれぞれの解決策の requires_check に移した（議決 D-0043・D-0052・D-0053・D-0059・D-0070・D-0074・D-0075・D-0100）
REQUIRES_CHECK = "requires_check"
REQUIRES_CHECK_KEYS = ("runs", "workflow")
REQUIRES_CHECK_DECISION = "D-0077"
REQUIRES_CHECK_INVALID = "requires_check_invalid"
REQUIRES_CHECK_UNWIRED = "requires_check_unwired"
# wired: true の解決策は、requires_check を書くか、REQUIRED_SCHEDULE・REQUIRED_RUNS（名前で並べた残りの求め）に挙げてあることを求める
# （議決 D-0080（Issue #1227 の起票時の番号は D-0078）の lint の案。requires_check を書き忘れた解決策が、check.py の配線だけで wired と読ませないように）
REQUIRES_CHECK_MISSING = "requires_check_missing"
REQUIRES_CHECK_MISSING_DECISION = "D-0080"
# wired: true の解決策は、知見と issue を問わずどれも、ひな形 ci/knowledge-enforcement.yml の写しの .github/workflows/knowledge-enforcement.yml の
# pull_request でも走るジョブから check.py を呼ぶ項目を wired_by に要する（議決 D-0072（起票時の番号は D-0071）の test の案、Issue #1201。
# 前は K-0004 の issue 986・1059・1129 を REQUIRED_PR に名前で挙げていて、ほかの知見の解決策は組を足し忘れると黙って通っていた。
# ほかのワークフローから check.py を呼ぶだけでは、必須チェックにする写しとは読めない（議決 D-0072、Issue #1200））
KNOWLEDGE_ENFORCEMENT_WORKFLOW = f"{WORKFLOWS}/knowledge-enforcement.yml"
KNOWLEDGE_GATE = (KNOWLEDGE_CHECK, KNOWLEDGE_ENFORCEMENT_WORKFLOW, "D-0072")
KNOWLEDGE_GATE_UNWIRED = "knowledge_gate_unwired"
# (知見の id, 解決策の issue)。KNOWLEDGE_ENFORCEMENT_WORKFLOW（ひな形の写し）が実在するなら wired: true であることを求める
# （議決 D-0080（起票時の番号は D-0078）の ci_check の案、Issue #1226。写したあとに記録の更新が遅れて、engine-test が PR ごとに走っているのに未配線と出続けるずれを止める）
MIRROR_WIRED = (("K-0004", 1063), ("K-0004", 1144))
MIRROR_WIRED_DECISION = "D-0080"
MIRROR_UNWIRED = "mirror_unwired"
# 知見 K-0012 の本文の直しのあとの再検査（issue 1122・1196）は Engine の merge_step の待ちも求める（REQUIRED_ENGINE_WAIT。議決 D-0100（起票時の番号は D-0071）、Issue #1196）
PR_BODY_ENGINE_WORKFLOW = f"{WORKFLOWS}/pr-body-engine.yml"
PR_BODY_RECHECK = (("K-0012", 1122), ("K-0012", 1196))
# (知見の id, 解決策の issue)。wired: true にするには、ENGINE_REF の Engine の merge_step が待ちの関数を呼んでいることを求める（議決 D-0100、Issue #1196）
REQUIRED_ENGINE_WAIT = set(PR_BODY_RECHECK)
ENGINE_SUPERVISOR = "orchestrator/dispatcher/supervisor.py"
ENGINE_MERGE_STEP = "merge_step"
ENGINE_RECHECK_WAIT = "body_recheck_pending"
ENGINE_UNWIRED = "engine_unwired"
ENGINE_UNVERIFIABLE = "engine_unverifiable"
# (知見の id, 解決策の issue) → 1 つのジョブからすべて呼ぶことを求める検査（議決 D-0042、Issue #1100）
CONFLICT_TESTS = ("tools/conflict-check/test_watch.py", "tools/conflict-check/test_engine.py")
REQUIRED_RUNS = {("K-0010", 1034): CONFLICT_TESTS, ("K-0010", 1100): CONFLICT_TESTS}
# (知見の id, 解決策の issue)。verified: true にするには Issue #948 に engine-head の記録が要る（議決 D-0047、Issue #1114）
HEAD_RECORD_SOLUTIONS = {("K-0004", 977), ("K-0004", 1046), ("K-0004", 1047)}
HEAD_RECORD = Path(__file__).resolve().parent.parent / "sendback-check" / "head-record.mjs"
HEAD_RECORD_MISSING = "head_record_missing"
HEAD_RECORD_UNCHECKED = "head_record_unchecked"
# review_by を前の版より後ろへ動かすときに、ENGINE_REF の Engine で test_events.py の EngineFinishedTest が todo のままでないことを求める知見
# （議決 D-0067（起票時の番号は D-0066）、Issue #1178）。ENGINE_REF は写しがあれば写し、なければひな形から読む（EngineFinishedTest を走らせる CI と同じ版）
REVIEW_BY_HOLD = "K-0010"
CONFLICT_CHECK_WORKFLOWS = (f"{WORKFLOWS}/conflict-check.yml", "tools/conflict-check/ci/conflict-check.yml")
CONFLICT_HARNESS = "tools/conflict-check/engine_harness.py"
CONFLICT_EVENTS_TEST = "tools/conflict-check/test_events.py"   # FINISHED_FIELDS（job.finished に求める欄）の定義元
ENGINE_DISPATCHER = "orchestrator/dispatcher/dispatcher.py"
# 出力の行に添える種類の名前。review_by_hold = Engine が今の形のまま延ばした、review_by_unverifiable = 照らせない（括弧に HoldUncheckable の kind）
REVIEW_BY_HOLD_ERROR = "review_by_hold"
REVIEW_BY_UNVERIFIABLE = "review_by_unverifiable"
SOLUTION_FIELDS_MISSING = "solution_fields_missing"
SOLUTION_FIELDS_UNVERIFIABLE = "solution_fields_unverifiable"
ENGINE_REF_RE = re.compile(r"^\s*ENGINE_REF:\s*['\"]?([^'\"\s#]*)['\"]?\s*(?:#.*)?$", re.M)
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


def calls_sendback_protection(s: dict) -> bool:
    """wired_by に SENDBACK_PROTECTION を呼ぶ項目があるか（議決 D-0086（起票時の番号は D-0085）、Issue #1247）"""
    wired_by = s.get("wired_by")
    return isinstance(wired_by, list) and any(
        isinstance(w, dict) and isinstance(w.get("runs"), str) and norm("", w["runs"]) == SENDBACK_PROTECTION for w in wired_by)


def schedule_requirements(k: dict, s: dict) -> dict[str, tuple[str | None, str]]:
    """解決策に求める schedule の配線を {検査の道: (ワークフロー（なければ None）, 落ちたときに挙げる議決)} にする。

    REQUIRED_SCHEDULE・REQUIRED_SCHEDULE_WORKFLOW の組に加えて、wired_by に SENDBACK_PROTECTION を呼ぶ項目を持つ解決策は
    知見と issue を問わず SENDBACK_PROTECTION_WORKFLOW のジョブを求める（議決 D-0086（起票時の番号は D-0085）、Issue #1247）"""
    key = (k.get("id"), s.get("issue"))
    out = {}
    if key in REQUIRED_SCHEDULE:
        out[REQUIRED_SCHEDULE[key]] = REQUIRED_SCHEDULE_WORKFLOW.get(key, (None, "D-0039"))
    if calls_sendback_protection(s) and out.get(SENDBACK_PROTECTION, (None,))[0] is None:
        out[SENDBACK_PROTECTION] = (SENDBACK_PROTECTION_WORKFLOW, SENDBACK_PROTECTION_DECISION)
    return out


def check_schedule(k: dict, repo: Path) -> list[str]:
    """REQUIRED_SCHEDULE の解決策が wired: true なら、schedule のジョブから検査を呼ぶ wired_by を求める（議決 D-0039、Issue #1088）。
    REQUIRED_SCHEDULE_WORKFLOW に挙げた解決策は、そのワークフローのジョブを指す項目だけを数える（議決 D-0060、Issue #1160）。
    wired_by に protection.mjs を呼ぶ項目を持つ解決策は、組に挙げていなくても sendback-protection.yml のジョブを求める（議決 D-0086、Issue #1247）"""
    f, e = k.get("_file", str(k.get("id"))), []
    read = repo_reader(repo)

    for i, s in enumerate(solutions(k)):
        if not isinstance(s, dict) or s.get("wired") is not True:
            continue
        for target, (workflow, decision) in schedule_requirements(k, s).items():
            e += schedule_errors(f, i, s, target, workflow, decision, repo, read)
    return e


def schedule_errors(f: str, i: int, s: dict, target: str, workflow: str | None, decision: str, repo: Path, read) -> list[str]:
    """解決策の wired_by に、on.schedule のある（workflow を書けばその）ワークフローの schedule でも走るジョブから target を呼ぶ項目がなければ落とす"""
    for w in s.get("wired_by") if isinstance(s.get("wired_by"), list) else []:
        if not valid_wired_by(w) or norm("", w["runs"]) != target or not is_workflow(w["workflow"]) or read(w["workflow"]) is None:
            continue
        if workflow is not None and norm("", w["workflow"]) != workflow:
            continue
        try:
            doc = load_yaml(repo / w["workflow"])
        except Exception:  # noqa: BLE001  読めないことは check_wiring が落とす
            continue
        if isinstance(doc, dict) and has_schedule(doc) and job_calls(doc, w["job"], target, read, off_schedule):
            return []
    where = f"{f}: enforcement.solutions[{i}]（{s.get('kind')}、issue {s.get('issue')}）"
    place = f"on.schedule（cron）のある {workflow} で" if workflow else f"on.schedule（cron）のある {WORKFLOWS}/ のワークフローで"
    return [f"{where}: wired にするには wired_by に、{place} schedule でも走るジョブから"
            f" {target} を呼ぶ項目が要る（手で走らせた記録だけでは wired にできない。議決 {decision}。"
            f"ジョブと手順の if: は無いか、github.event_name == 'schedule' を || でつないだ決まった形だけを許す。議決 D-0061）"]


def required_checks(s: dict) -> tuple[list[tuple[str, str | None]], list[str]]:
    """解決策の requires_check を (検査の道, ワークフロー（なければ None）) の列にする（議決 D-0077（起票時の番号は D-0075）、Issue #1216）。

    値は検査の道の文字列か、その列。列の項目は道の文字列か {runs, workflow}（workflow は書けばそのワークフローのジョブを指す wired_by だけを数える）。
    書いていなければ空の列。形の誤りは 2 つ目に返す"""
    if REQUIRES_CHECK not in s:
        return [], []
    v = s[REQUIRES_CHECK]
    entries = [v] if isinstance(v, str) else v
    if not isinstance(entries, list) or not entries:
        return [], [f"{REQUIRES_CHECK} は検査の道か、道か {{runs, workflow}} の空でない列（{v!r}）"]
    out, bad = [], []
    for j, x in enumerate(entries):
        if isinstance(x, str):
            x = {"runs": x}
        ok = (isinstance(x, dict) and set(x) <= set(REQUIRES_CHECK_KEYS) and "runs" in x
              and all(isinstance(x[key], str) and x[key] for key in x))
        runs = norm("", x["runs"]) if ok else None
        if not runs:
            bad.append(f"{REQUIRES_CHECK}[{j}]: 検査の道の文字列か、runs（とあれば workflow）を空でない文字列で書いた項目（{entries[j]!r}）")
        elif "workflow" in x and not is_workflow(x["workflow"]):
            bad.append(f"{REQUIRES_CHECK}[{j}]: workflow は {WORKFLOWS}/ の直下のファイル（{x['workflow']}）")
        else:
            out.append((runs, x.get("workflow")))
    return out, bad


def check_pull_request(k: dict, repo: Path) -> list[str]:
    """requires_check を書いた解決策を照らす（議決 D-0077（起票時の番号は D-0075）の schema の案、Issue #1216）。

    形が誤っているか、挙げた検査がリポジトリにないなら requires_check_invalid で落とす（wired を問わない）。
    wired: true なら、挙げた検査ごとに、pull_request でも走るジョブからそれを呼ぶ wired_by の項目（workflow を書いた検査はそのワークフローの項目だけ）を求め、
    なければ requires_check_unwired で落とす。前は (知見の id, 解決策の issue) の組を REQUIRED_PR に並べていて、組を足し忘れると黙って通っていた"""
    f, e = k.get("_file", str(k.get("id"))), []
    read = repo_reader(repo)

    for i, s in enumerate(solutions(k)):
        if not isinstance(s, dict):
            continue
        where = f"{f}: enforcement.solutions[{i}]（{s.get('kind')}、issue {s.get('issue')}）"
        required, bad = required_checks(s)
        e += [f"{where}: {REQUIRES_CHECK_INVALID}: {b}" for b in bad]
        for target, _ in required:
            if read(target) is None:
                e.append(f"{where}: {REQUIRES_CHECK_INVALID}: {REQUIRES_CHECK} の {target} がない")
        if s.get("wired") is not True:
            continue
        for target, workflow in required:
            if read(target) is None or pr_job_calls(s, repo, read, target, workflow):
                continue
            place = workflow or f"{WORKFLOWS}/ のワークフロー"
            e.append(f"{where}: {REQUIRES_CHECK_UNWIRED}: wired にするには wired_by に、on.pull_request のある {place} で pull_request でも走るジョブから"
                     f" {target} を呼ぶ項目が要る（{REQUIRES_CHECK} に挙げた検査。ほかの検査を呼ぶだけ・定期や手動だけでは wired にできない。"
                     f"議決 {REQUIRES_CHECK_DECISION}）")
    return e


def check_requires_check_missing(k: dict) -> list[str]:
    """wired: true の解決策で、requires_check を書いておらず、REQUIRED_SCHEDULE・REQUIRED_RUNS にも挙げていないなら requires_check_missing で落とす
    （議決 D-0080（Issue #1227 の起票時の番号は D-0078）の lint の案）。

    requires_check は書いた解決策しか照らさないので、書き忘れると、どの解決策にも求める check.py の配線（knowledge_gate）だけで wired: true にできてしまう。
    名前で組を並べていたころの足し忘れと同じ穴が、欄の書き忘れとして残っていた。形の誤りは check_pull_request が落とす"""
    f, e = k.get("_file", str(k.get("id"))), []
    for i, s in enumerate(solutions(k)):
        if not isinstance(s, dict) or s.get("wired") is not True or REQUIRES_CHECK in s:
            continue
        key = (k.get("id"), s.get("issue"))
        if key in REQUIRED_SCHEDULE or key in REQUIRED_RUNS:
            continue
        e.append(f"{f}: enforcement.solutions[{i}]（{s.get('kind')}、issue {s.get('issue')}）: {REQUIRES_CHECK_MISSING}: wired にするには {REQUIRES_CHECK} に、"
                 f"pull_request でも走るジョブから呼ぶ検査の道（例: {REQUIRES_CHECK}: tools/knowledge-enforcement-check/test_engine_patch.py）を書く"
                 f"（書かないと {KNOWLEDGE_CHECK} の配線だけで wired と読ませてしまう。議決 {REQUIRES_CHECK_MISSING_DECISION}）")
    return e


def check_knowledge_gate(k: dict, repo: Path) -> list[str]:
    """wired: true の解決策はどれも、KNOWLEDGE_GATE のワークフローの pull_request のジョブから check.py を呼ぶ wired_by を求める
    （議決 D-0072（起票時の番号は D-0071）の test の案、Issue #1201。REQUIRED_PR の列挙に頼らない）"""
    f, e = k.get("_file", str(k.get("id"))), []
    read = repo_reader(repo)
    target, workflow, decision = KNOWLEDGE_GATE

    for i, s in enumerate(solutions(k)):
        if not isinstance(s, dict) or s.get("wired") is not True or pr_job_calls(s, repo, read, target, workflow):
            continue
        e.append(f"{f}: enforcement.solutions[{i}]（{s.get('kind')}、issue {s.get('issue')}）: {KNOWLEDGE_GATE_UNWIRED}: wired にするには wired_by に、"
                 f"on.pull_request のある {workflow} で pull_request でも走るジョブから {target} を呼ぶ項目が要る"
                 f"（wired: true の解決策はどれも、この検査が PR ごとに走っていないと wired と読ませない。議決 {decision}）")
    return e


def check_mirror_wired(k: dict, repo: Path) -> list[str]:
    """KNOWLEDGE_ENFORCEMENT_WORKFLOW が実在するのに、MIRROR_WIRED の解決策が wired: true でない（false・未記入・真偽でない値）なら
    mirror_unwired で落とす（議決 D-0080（起票時の番号は D-0078）の ci_check の案、Issue #1226）。
    写しがなければ見ない。wired: true にしたあとの wired_by の照合は check_wiring・check_pull_request・check_knowledge_gate が行う"""
    if not (repo / KNOWLEDGE_ENFORCEMENT_WORKFLOW).is_file():
        return []
    f, e = k.get("_file", str(k.get("id"))), []
    for i, s in enumerate(solutions(k)):
        if not isinstance(s, dict) or (k.get("id"), s.get("issue")) not in MIRROR_WIRED or s.get("wired") is True:
            continue
        now = f"wired: {s['wired']!r}" if "wired" in s else "未記入"
        e.append(f"{f}: enforcement.solutions[{i}]（{s.get('kind')}、issue {s.get('issue')}）: {MIRROR_UNWIRED}: {KNOWLEDGE_ENFORCEMENT_WORKFLOW} が"
                 f"あるのに wired が true でない（{now}）。写した PR で wired: true にし、wired_by に engine-test のジョブの"
                 f" test_engine_patch.py と enforcement のジョブの check.py の項目を書く（議決 {MIRROR_WIRED_DECISION}）")
    return e


def pr_job_calls(s: dict, repo: Path, read, target: str, workflow: str | None) -> bool:
    """解決策の wired_by に、pull_request でも走るジョブから target を呼ぶ項目があるか（workflow を渡したらそのワークフローの項目だけを数える）"""
    for w in s.get("wired_by") if isinstance(s.get("wired_by"), list) else []:
        if not valid_wired_by(w) or norm("", w["runs"]) != target or not is_workflow(w["workflow"]) or read(w["workflow"]) is None:
            continue
        if workflow is not None and norm("", w["workflow"]) != workflow:
            continue
        try:
            doc = load_yaml(repo / w["workflow"])
        except Exception:  # noqa: BLE001  読めないことは check_wiring が落とす
            continue
        if isinstance(doc, dict) and has_pull_request(doc) and job_calls(doc, w["job"], target, read, off_event("pull_request")):
            return True
    return False


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
    what に書いた最初の議決の番号と、その直後の括弧の PR の番号も、decided_by と議決の pr に照らす（議決 D-0122（起票時の番号は D-0080）、Issue #1232）。
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
        cited = WHAT_DECISION_RE.search(s["what"]) if isinstance(s.get("what"), str) else None
        if cited and cited.group(1) != did:
            e.append(f"{where}: what は議決 {cited.group(1)} を挙げるが decided_by は {dec}（議決の番号の取り違え）")
            continue
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
            continue
        pr = WHAT_PR_RE.search(cited.group(2) or "") if cited else None
        if pr:
            got = DECISION_PR_RE.search(d.get("pr", "")) if isinstance(d.get("pr"), str) else None
            if not got or got.group(1) != pr.group(1):
                e.append(f"{where}: what は議決 {did} を PR #{pr.group(1)} の議決と書くが、議決 {did} の pr は {d.get('pr')!r}（議決の番号の取り違え）")
    return e


def head_record(comments: str | None, script: Path = HEAD_RECORD) -> tuple[str, str] | None:
    """Issue #948 のコメント（gh api の出力のファイル）を head-record.mjs で照らす。記録があれば None、なければ (rule, 理由)。
    head-record.mjs が --json に ok: true を書いて 0 で終わったときだけ通す（結果を書かずに 0 で抜けた形も通さない）"""
    if not comments or not Path(comments).is_file():
        return HEAD_RECORD_UNCHECKED, f"--head-comments に Issue #948 のコメント（gh api の出力）を渡していないか、ファイルがない（{comments}）"
    with tempfile.TemporaryDirectory() as t:
        out = Path(t) / "head-record.json"
        try:
            p = subprocess.run(["node", str(script), "--comments", str(Path(comments).resolve()), "--json", str(out)],
                               capture_output=True, text=True, timeout=120)
        except (OSError, subprocess.SubprocessError) as x:
            return HEAD_RECORD_UNCHECKED, f"{script.name} を走らせられない（{x}）"
        try:
            result = json.loads(out.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            result = None
    if not isinstance(result, dict):
        return HEAD_RECORD_UNCHECKED, f"{script.name} が結果を出さなかった（終了コード {p.returncode}: {p.stderr.strip()}）"
    if p.returncode == 0 and result.get("ok") is True:
        return None
    if p.returncode == 1 and any(isinstance(x, dict) and x.get("rule") == HEAD_RECORD_MISSING for x in result.get("errors") or []):
        return HEAD_RECORD_MISSING, "Issue #948 に engine-head の先頭の Engine での実行の記録（github-actions[bot] の印と実行の URL）がない"
    return HEAD_RECORD_UNCHECKED, f"{script.name} の結果が読めない（終了コード {p.returncode}）"


def check_head_record(k: dict, comments: str | None, cache: dict | None = None, script: Path = HEAD_RECORD) -> list[str]:
    """HEAD_RECORD_SOLUTIONS の解決策が verified: true なら、Issue #948 に engine-head の記録があることを求める（議決 D-0047、Issue #1114）。
    head-record.mjs は 1 回だけ走らせる（cache に覚える）"""
    f, e = k.get("_file", str(k.get("id"))), []
    cache = {} if cache is None else cache
    for i, s in enumerate(solutions(k)):
        if not isinstance(s, dict) or s.get("verified") is not True or (k.get("id"), s.get("issue")) not in HEAD_RECORD_SOLUTIONS:
            continue
        if "result" not in cache:
            cache["result"] = head_record(comments, script)
        if cache["result"] is None:
            continue
        rule, why = cache["result"]
        where = f"{f}: enforcement.solutions[{i}]（{s.get('kind')}、issue {s.get('issue')}）: {rule}"
        e.append(f"{where}: {why}。.github/workflows/sendback-engine-head.yml を schedule で走らせて Issue #948 に記録が残るまで verified を true にしない（議決 D-0047）")
    return e


class HoldUncheckable(Exception):
    """review_by の先送りの照合と Engine の merge_step の照合で、前の版・ENGINE_REF・Engine の中身を読めず照らせないこと。kind は読めなかったものの種類
    （base_unreadable・engine_missing・engine_ref_invalid・engine_ref_missing・finished_unreadable・fields_unreadable・supervisor_unreadable）"""

    def __init__(self, kind: str, detail: str):
        super().__init__(f"{kind}: {detail}")
        self.kind, self.detail = kind, detail


def git(cwd: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["git", "-C", str(cwd), *args], capture_output=True, text=True)


def as_date(v) -> date | None:
    """review_by の値を日付にする（PyYAML は日付で、ruby で読むと文字列で返る）。読めなければ None"""
    if isinstance(v, date):
        return v
    try:
        return date.fromisoformat(str(v))
    except ValueError:
        return None


def base_knowledge(repo: Path, base: str, rel: str):
    """base の版の知見のファイルを読む。その版にファイルがなければ None。base が版として読めないか、ファイルを YAML として読めなければ HoldUncheckable"""
    if git(repo, "rev-parse", "--verify", "--quiet", f"{base}^{{commit}}").returncode != 0:
        raise HoldUncheckable("base_unreadable", f"前の版 {base} がリポジトリにない（fetch-depth: 0 で取る）")
    if git(repo, "cat-file", "-e", f"{base}:{rel}").returncode != 0:
        return None
    with tempfile.TemporaryDirectory() as tmp:
        p = Path(tmp) / Path(rel).name
        p.write_text(git(repo, "show", f"{base}:{rel}").stdout, encoding="utf-8")
        try:
            return load_yaml(p)
        except Exception as x:  # noqa: BLE001  PyYAML と ruby で投げる例外が違うので、読むところだけで受けて種類を付ける
            raise HoldUncheckable("base_unreadable", f"前の版 {base} の {rel} を読めない（{x}）") from x


def conflict_engine_ref(repo: Path) -> tuple[str, str]:
    """EngineFinishedTest を走らせる CI の ENGINE_REF と、読んだファイル（写しがあれば写し、なければひな形）。読めなければ HoldUncheckable"""
    return engine_ref(repo, CONFLICT_CHECK_WORKFLOWS)


def engine_ref(repo: Path, rels: tuple[str, ...]) -> tuple[str, str]:
    """rels のうち最初にあるファイルの ENGINE_REF と、読んだファイル。読めなければ HoldUncheckable"""
    for rel in rels:
        p = repo / rel
        if p.is_file():
            refs = ENGINE_REF_RE.findall(p.read_text(encoding="utf-8"))
            if len(refs) != 1 or not re.fullmatch(r"[0-9a-f]{40}", refs[0]):
                raise HoldUncheckable("engine_ref_invalid", f"{rel} の ENGINE_REF が 40 桁のコミット SHA の 1 行でない（{refs!r}）")
            return refs[0], rel
    raise HoldUncheckable("engine_ref_invalid", f"ENGINE_REF を読むファイル（{'・'.join(rels)}）がない")


def finished_fields_required(repo: Path) -> tuple[str, ...]:
    """test_events.py の FINISHED_FIELDS（EngineFinishedTest が job.finished に求める欄）を構文木から読む（取り込むと Engine のテストまで読み込むため）"""
    try:
        tree = ast.parse((repo / CONFLICT_EVENTS_TEST).read_text(encoding="utf-8"))
        for node in tree.body:
            if isinstance(node, ast.Assign) and any(isinstance(t, ast.Name) and t.id == "FINISHED_FIELDS" for t in node.targets):
                return tuple(ast.literal_eval(node.value))
    except (OSError, SyntaxError, ValueError) as x:
        raise HoldUncheckable("fields_unreadable", f"{CONFLICT_EVENTS_TEST} の FINISHED_FIELDS を読めない（{x}）") from x
    raise HoldUncheckable("fields_unreadable", f"{CONFLICT_EVENTS_TEST} に FINISHED_FIELDS がない")


def engine_finished_legacy(repo: Path, engine: Path, ref: str) -> tuple[bool, list[str]]:
    """ENGINE_REF の版の Engine の job.finished が今の形（FINISHED_FIELDS をどれも出さない。EngineFinishedTest が todo になる）かと、その欄。
    Engine のチェックアウト（全履歴）から版の dispatcher.py を取り出し、engine_harness.py の finished で欄を読む。読めなければ HoldUncheckable"""
    if git(engine, "rev-parse", "--verify", "--quiet", f"{ref}^{{commit}}").returncode != 0:
        raise HoldUncheckable("engine_ref_missing", f"Engine のチェックアウト（{engine}）に ENGINE_REF {ref} の版がない（fetch-depth: 0 で取る）")
    src = git(engine, "show", f"{ref}:{ENGINE_DISPATCHER}")
    if src.returncode != 0:
        raise HoldUncheckable("engine_ref_missing", f"Engine の {ref} の版に {ENGINE_DISPATCHER} がない")
    with tempfile.TemporaryDirectory() as tmp:
        p = Path(tmp) / ENGINE_DISPATCHER
        p.parent.mkdir(parents=True)
        p.write_text(src.stdout, encoding="utf-8")
        out = subprocess.run([sys.executable, "-I", "-B", str(repo / CONFLICT_HARNESS), "finished", tmp], capture_output=True, text=True)
    if out.returncode != 0:
        raise HoldUncheckable("finished_unreadable", f"{CONFLICT_HARNESS} finished が落ちた: {(out.stderr or out.stdout).strip()}")
    try:
        fields = json.loads(out.stdout)["fields"]
    except (ValueError, KeyError, TypeError) as x:
        raise HoldUncheckable("finished_unreadable", f"{CONFLICT_HARNESS} finished の出力に fields がない（{x!r}）") from x
    if not isinstance(fields, list):
        raise HoldUncheckable("finished_unreadable", f"{CONFLICT_HARNESS} finished の fields が列でない（{fields!r}）")
    return not set(finished_fields_required(repo)) & set(fields), fields


def check_review_by_hold(k: dict, repo: Path, base: str | None, engine: Path | None) -> list[str]:
    """REVIEW_BY_HOLD の知見の review_by を base の版より後ろへ動かすなら、ENGINE_REF の Engine で EngineFinishedTest が todo のまま
    （job.finished に FINISHED_FIELDS をどれも出さない）でないことを求める（議決 D-0067（起票時の番号は D-0066）、Issue #1178）。
    base を渡さないとき・base にその知見がないとき・base の review_by が日付でないときは見ない。
    落とす行には種類の名前を添える（REVIEW_BY_HOLD_ERROR、照らせないときは REVIEW_BY_UNVERIFIABLE と HoldUncheckable の kind）"""
    if k.get("id") != REVIEW_BY_HOLD or not base:
        return []
    f = k.get("_file", str(k.get("id")))
    rel = f"{ROOT}/{f}"
    try:
        prev = base_knowledge(repo, base, rel)
    except HoldUncheckable as x:
        return [f"{f}: {REVIEW_BY_UNVERIFIABLE}（{x.kind}）: review_by の先送りを照らせない（{x.detail}。議決 D-0067）"]
    before = as_date(prev.get("review_by")) if isinstance(prev, dict) else None
    if before is None:
        return []
    after = as_date(k.get("review_by"))
    if after is not None and after <= before:
        return []
    moved = f"review_by を {before} から {k.get('review_by')} へ動かす"
    try:
        if engine is None:
            raise HoldUncheckable("engine_missing", "Engine のチェックアウトを --engine か ENGINE_DIR で渡す")
        ref, src = conflict_engine_ref(repo)
        legacy, fields = engine_finished_legacy(repo, engine, ref)
        required = finished_fields_required(repo)
    except HoldUncheckable as x:
        return [f"{f}: {REVIEW_BY_UNVERIFIABLE}（{x.kind}）: {moved}が、ENGINE_REF の Engine で EngineFinishedTest が todo のままかを照らせない"
                f"（{x.detail}。照らせないときは落とす。議決 D-0067）"]
    if legacy:
        return [f"{f}: {REVIEW_BY_HOLD_ERROR}: {moved}が、ENGINE_REF（{ref}。{src}）の Engine の job.finished（{', '.join(fields)}）に"
                f" {' と '.join(required)} がなく、{CONFLICT_EVENTS_TEST} の EngineFinishedTest が todo のまま。"
                "review_by を延ばすだけで期限切れの失敗を先送りしない。Engine を直して ENGINE_REF を上げてから延ばす（議決 D-0067）"]
    return []


def engine_merge_waits(engine: Path, ref: str) -> bool:
    """ref の版の Engine の supervisor.py で、merge_step が body_recheck_pending を呼び、その関数がモジュールの直下に定義されているか。
    Engine のチェックアウト（全履歴）から版のファイルを取り出して構文木で見る（取り込まない）。読めなければ HoldUncheckable"""
    if git(engine, "rev-parse", "--verify", "--quiet", f"{ref}^{{commit}}").returncode != 0:
        raise HoldUncheckable("engine_ref_missing", f"Engine のチェックアウト（{engine}）に ENGINE_REF {ref} の版がない（fetch-depth: 0 で取る）")
    src = git(engine, "show", f"{ref}:{ENGINE_SUPERVISOR}")
    if src.returncode != 0:
        raise HoldUncheckable("engine_ref_missing", f"Engine の {ref} の版に {ENGINE_SUPERVISOR} がない")
    try:
        tree = ast.parse(src.stdout)
    except SyntaxError as x:
        raise HoldUncheckable("supervisor_unreadable", f"Engine の {ref} の版の {ENGINE_SUPERVISOR} を読めない（{x}）") from x
    defs = {n.name: n for n in tree.body if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef))}
    if ENGINE_RECHECK_WAIT not in defs or ENGINE_MERGE_STEP not in defs:
        return False
    return any(isinstance(n, ast.Call) and isinstance(n.func, ast.Name) and n.func.id == ENGINE_RECHECK_WAIT
               for n in ast.walk(defs[ENGINE_MERGE_STEP]))


def check_engine_wait(k: dict, repo: Path, engine: Path | None, cache: dict | None = None) -> list[str]:
    """REQUIRED_ENGINE_WAIT の解決策が wired: true なら、.github/workflows/pr-body-engine.yml の ENGINE_REF の Engine の merge_step が
    body_recheck_pending を呼んでいることを求める（議決 D-0100（起票時の番号は D-0071）、Issue #1196）。
    落とす行には種類の名前を添える（ENGINE_UNWIRED、照らせないときは ENGINE_UNVERIFIABLE と HoldUncheckable の kind）"""
    f, e = k.get("_file", str(k.get("id"))), []
    cache = {} if cache is None else cache
    for i, s in enumerate(solutions(k)):
        if not isinstance(s, dict) or s.get("wired") is not True or (k.get("id"), s.get("issue")) not in REQUIRED_ENGINE_WAIT:
            continue
        if "result" not in cache:
            try:
                if engine is None:
                    raise HoldUncheckable("engine_missing", "Engine のチェックアウトを --engine か ENGINE_DIR で渡す")
                ref, rel = engine_ref(repo, (PR_BODY_ENGINE_WORKFLOW,))
                cache["result"] = (engine_merge_waits(engine, ref), ref, rel, None)
            except HoldUncheckable as x:
                cache["result"] = (None, None, None, x)
        waits, ref, rel, x = cache["result"]
        where = f"{f}: enforcement.solutions[{i}]（{s.get('kind')}、issue {s.get('issue')}）"
        if x is not None:
            e.append(f"{where}: {ENGINE_UNVERIFIABLE}（{x.kind}）: wired にするには ENGINE_REF の Engine の {ENGINE_MERGE_STEP} が"
                     f" {ENGINE_RECHECK_WAIT} を呼んでいることを照らす（{x.detail}。照らせないときは落とす。議決 D-0100）")
        elif not waits:
            e.append(f"{where}: {ENGINE_UNWIRED}: ENGINE_REF（{ref}。{rel}）の Engine の {ENGINE_SUPERVISOR} の {ENGINE_MERGE_STEP} が"
                     f" {ENGINE_RECHECK_WAIT} を呼んでいない。tools/pr-body-check/engine/pr-body-recheck.patch を Engine に入れて ENGINE_REF を上げてから"
                     " wired にする（議決 D-0100）")
    return e


def merge_base(repo: Path, base: str) -> str | None:
    """base と HEAD の分かれ目（PR の差分の起点）。求められなければ None"""
    out = git(repo, "merge-base", base, "HEAD")
    return out.stdout.strip() if out.returncode == 0 and out.stdout.strip() else None


def check_solution_fields(k: dict, repo: Path, base: str | None, cache: dict | None = None) -> list[str]:
    """PR の差分で変わった知見の enforcement.solutions の各項目に wired と verified の両方が書かれていることを求める（議決 D-0071、Issue #1193）。
    base を渡さないとき・未記入の解決策がないとき・分かれ目の版から変わっていないときは見ない。
    落とす行には種類の名前を添える（SOLUTION_FIELDS_MISSING、照らせないときは SOLUTION_FIELDS_UNVERIFIABLE）"""
    if not base:
        return []
    f = k.get("_file", str(k.get("id")))
    missing = [(i, s, [flag for flag in FLAGS if flag not in s]) for i, s in enumerate(solutions(k)) if isinstance(s, dict)]
    missing = [m for m in missing if m[2]]
    if not missing:
        return []
    cache = {} if cache is None else cache
    if base not in cache:
        cache[base] = merge_base(repo, base)
    mb, rel = cache[base], f"{ROOT}/{f}"
    if mb is None:
        return [f"{f}: {SOLUTION_FIELDS_UNVERIFIABLE}: 未記入の解決策があるが、前の版 {base} と HEAD の分かれ目を求められず、"
                "この変更セットで直したかを照らせない（fetch-depth: 0 で取る。照らせないときは落とす。議決 D-0071）"]
    if git(repo, "cat-file", "-e", f"{mb}:{rel}").returncode == 0:
        prev = git(repo, "show", f"{mb}:{rel}")
        if prev.returncode != 0:
            return [f"{f}: {SOLUTION_FIELDS_UNVERIFIABLE}: 分かれ目の版 {mb} の {rel} を読めない（{prev.stderr.strip()}。議決 D-0071）"]
        if prev.stdout == (repo / rel).read_text(encoding="utf-8"):
            return []
    return [f"{f}: {SOLUTION_FIELDS_MISSING}: enforcement.solutions[{i}]（{s.get('kind')}、issue {s.get('issue')}）に {'・'.join(flags)} がない。"
            "この変更セットで知見を直すので、実際の配線と確認の状態を真偽で書く（配線していない・確かめていないなら false。議決 D-0071）"
            for i, s, flags in missing]


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
    ap.add_argument("--head-comments", help="Issue #948 のコメント（gh api の出力）。engine-head に関わる解決策が verified: true のときに照らす")
    ap.add_argument("--base", help="前の版（例: origin/main）。渡すと K-0010 の review_by の先送り（議決 D-0067）と、差分で変わった知見の wired・verified の記入（議決 D-0071）を照らす")
    ap.add_argument("--engine", default=os.environ.get("ENGINE_DIR") or None, help="Engine のチェックアウト（全履歴。既定は ENGINE_DIR）")
    a = ap.parse_args(argv)
    try:
        items = load(Path(a.repo))
    except Exception as e:  # noqa: BLE001
        print(f"読めなかった: {e}", file=sys.stderr)
        return 2
    repo, decisions, heads, bases, waits = Path(a.repo), {}, {}, {}, {}
    errors = [line for k in items for line in check_item(k) + check_wiring(k, repo) + check_schedule(k, repo) + check_pull_request(k, repo) + check_requires_check_missing(k) + check_knowledge_gate(k, repo) + check_mirror_wired(k, repo) + check_required_runs(k, repo) + check_verification(k, repo) + check_decision(k, repo, decisions)
              + check_head_record(k, a.head_comments, heads)
              + check_review_by_hold(k, repo, a.base, Path(a.engine) if a.engine else None)
              + check_engine_wait(k, repo, Path(a.engine) if a.engine else None, waits)
              + check_solution_fields(k, repo, a.base, bases)]
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
