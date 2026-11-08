# PR 本文の見出しの検査

PR 本文の見出しの構造を決定論で（LLM を使わずに）見る。知見 `K-0001`（`.factory-config/knowledge/K-0001.yaml`）の担保で、委員会の議決 `D-0001` と Issue #826 で作り、議決 `D-0013` の案と Issue #956 で本文の更新の前の関所（`gate.mjs`）を足し、議決 `D-0019` の案と Issue #975 で関所を Engine に配線する差分（`engine/pr-body-gate.patch`）を足した。議決 `D-0032`（Issue #1037 の起票時の番号は `D-0030`。PR #1036）の案と Issue #1037 で、CI のひな形の見張り（`ci.test.mjs`）と必須のチェックの照合（`protection.mjs`）を足した。議決 `D-0044` の案と Issue #1106 で、`protection.mjs` を既定のブランチへの push と毎日走らせ、必須のチェックが外れたら落ちて Issue を開くか更新するひな形（`ci/pr-body-protection.yml`）を足した。議決 `D-0045` の ci_check の案と Issue #1109 で、通る側のテスト（`gate.test.mjs`・`engine.test.mjs` の (10)）が CI の `node --test` の対象から外れていないかの見張りを `ci.test.mjs` に足した。議決 `D-0068`（Issue #1180 の起票時の番号は `D-0066`。PR #1176）の test の案と Issue #1180 で、`ci/pr-body-protection.yml` の写しが schedule と `workflow_dispatch` で走り、トークンを読めないときに `protection_unreadable` で飛ばさず落ちることを `ci.test.mjs` で見るようにした。議決 `D-0070`（Issue #1182 の起票時の番号は `D-0067`。PR #1181）の ci_check の案と Issue #1182 で、写しと定期の照合のどちらかを欠いたまま `K-0001` の解決策を `wired: true` にできないよう、`tools/knowledge-enforcement-check/check.py` の配線の必須に足した（下の「配線の手順（建設者）」の 5）。議決 `D-0070`（Issue #1183 の起票時の番号は `D-0067`。PR #1181）の test の案と Issue #1183 で、その見張りを字面の照合だけに頼らず、`gate.test.mjs` と `engine.test.mjs` を `node --test --test-reporter=tap` で走らせた結果で、通る側のテストが pass と数えられ skip・todo が 0 かを見るようにした。議決 `D-0099`（Issue #1187 の起票時の番号は `D-0069`。PR #1184）の ci_check の案と Issue #1187 で、(10) の中の条件付きの早い `return`・短絡・三項・`try`/`catch` のように、飛ばさずに pass と数えられる外し方も `ci.test.mjs` で見るようにした。知見 `K-0006` の担保として、議決 `D-0006` と Issue #906 で見出しの定義（`template.json`）との照合を足した。知見 `K-0012` の担保として、議決 `D-0033`（Issue #1053 の起票時の番号は `D-0032`。PR #1051）の test の案と Issue #1053 で、本文の直しのあとの再検査を待たずにマージしていないかをイベント列で見る検査（`recheck.mjs`）を足した。議決 `D-0050`（Issue #1131 の起票時の番号は `D-0048`。PR #1128）の ci_check の案と Issue #1131 で、`recheck.test.mjs` に事例 `C-0003` の列を足し、ひな形 `ci/pr-body-engine.yml` の `engine-test` で走るテストに数えて、`ci.test.mjs` でファイルの側で外していないかを見張るようにした。議決 `D-0050` の ci_check の案と Issue #1122 で、K-0006 と同じ見出しの検査を本文だけの直し（`pull_request` の `edited`）でも走らせるマージ前の必須チェックのひな形（`ci/pr-body-check.yml`）と、本文の直しが終わって直した本文がそのチェックに通るまで Engine の supervisor がマージを待つ差分（`engine/pr-body-recheck.patch`）を足した（別の検査は作らず、K-0006 の #906 と同じ 1 つのチェックにまとめた）。議決 `D-0100`（Issue #1196 の起票時の番号は `D-0071`。PR #1192）の ci_check の案と Issue #1196 で、写しと Engine への差分の取り込みのどちらかを欠いたまま `K-0012` と `K-0006` のそのチェックの解決策を `wired: true` にできないよう、`tools/knowledge-enforcement-check/check.py` の配線の必須に足した（下の「マージ前の必須チェック（建設者）」）。同じ議決の test の案と Issue #1197 で、`engine.test.mjs` の (12)（`merge_step` の待ちの線）を TAP で走らせた結果で pass と数えられたかを `ci.test.mjs` で見張り、`recheck.test.mjs`・`engine.test.mjs`・`ci.test.mjs` を `.github/workflows/pr-body-engine.yml` の写しの PR ごとのジョブから走らせないまま、その解決策を `wired: true` にできないよう `check.py` の配線の必須に足した（下の「配線の手順（建設者）」）。知見 `K-0005` の担保として、Issue #1121（解決策の種類は config）で、見出しの正本（`template.json`）と、それに合わせる CLAUDE.md と Engine の rubric の `PR-002` を `followers.json` に書き、食い違ったら落とす照合（`followers.mjs`）を足した。議決 `D-0068`（Issue #1179 の起票時の番号は `D-0066`。PR #1176）の ci_check の案と Issue #1179 で、関所の配線の差分（`engine/pr-body-gate.patch`）が `ENGINE_REF` の版と Engine の既定のブランチの先頭に当たるかを照らす検査（`patch.mjs`）と、それを既定のブランチへの push・毎日・手動で走らせて当たらなければ Issue を開くひな形（`ci/pr-body-patch.yml`）を足した。

続きのジョブ（衝突の解消・CI 落ちの直しなど）の報告が、新しい PR と同じ見出しの組（`## 概要`・`## 対応内容` など）のまま本文の末尾に足されると、同じ見出しが 2 回出る。これを検査役（LLM）と判定役まで流さず、本文を更新した直後に止めるためのもの。差し戻しの直しで本文の末尾に追記するだけだと、見出しの順が崩れたり必須の見出しが欠けたりする（K-0006）ので、それも見る。

```sh
node check.mjs --body <本文のファイル> [--json <出力先>]
gh pr view <番号> --json body --jq .body | node check.mjs     # --body を省くと標準入力から読む
npm test
```

依存はない（Node.js だけ。`engine.test.mjs` は bash・jq・git・Python 3 と Engine のチェックアウトも使う。`protection.mjs` は branch protection の JSON の読み方を `tools/sendback-check/protection.mjs` から使う）。

## 見ること

| rule | 外れる本文 |
|---|---|
| `duplicate_h2` | 同じ `## ` 見出しが 2 回以上ある |
| `outside_factory_record` | `## 工場の記録` より後ろに `## ` か `# ` の見出しがある（そこから下は工場の記録の中なので `### ` 以下にする） |
| `continuation_outside_record` | 「続きのジョブ」を含む見出しが、`## 工場の記録` の下の `### ` 以下にない |
| `missing_required` | `template.json` で必須（`required: true`）の `## ` 見出しがない |
| `template_order` | `template.json` の `## ` 見出しが定義の順に並んでいない（前の見出しより定義で先のものが後ろに来たら、その見出しを 1 件） |
| `outside_template` | `template.json` にない `## ` か `# ` の見出し（`## 工場の記録`・続きのジョブの報告など）が、定義の見出しの最後のものより前にある |

コードブロック（```` ``` ```` と `~~~`）の中の行は見出しとして数えない。`### ` 以下の見出しは重なってもよく、定義の外でもよい。同じ `## ` 見出しの 2 回目は `duplicate_h2` だけで数え、順の違いや定義の見出しの終わりには数えない。

## 見出しの定義（`template.json`）

PR 本文の `## ` 見出しの順と、必須かどうかの 1 つの定義。順は `概要 / 背景 / 問題 / 対応内容 / 影響範囲・互換性 / 動作確認 / 補足 / 仮決め / 知見`。該当しない見出しは省くので、必須は `概要` と `対応内容` だけ。

見出しの正はこのファイルに置く（知見 `K-0005` の線でオーナーが決めた。Issue #936）。CLAUDE.md の「書き方」の「PR の説明は … の見出しで」は同じ順にそろえてあり、`check.test.mjs` が両者の一致を確かめる。Engine の rubric の `PR-002`（`core-gates/rubric.yaml`）の見出しの列（概要・背景・問題・対応内容・影響範囲・動作確認・補足）は同じ順だが、`影響範囲` の名前と `仮決め`・`知見` がない点がまだ食い違う。Engine は別のリポジトリなので、建設者がこの定義に合わせる。Engine の実装役の報告の型（`orchestrator/worker/entrypoint.sh` の「最後の出力を次の見出しだけで構成する」の見出しの列）も、`背景`・`問題` がない点がまだ食い違う。正本とそれに合わせる側は `followers.json` に書き、`followers.mjs` が照らす（下の「正本に合わせる側の照合」）。

`check.mjs` は初めて照らすときに定義を読んで検証する（`h2` が空でない配列で、各要素は前後に空白のない 1 行の `text` と真偽値の `required` だけを持ち、`text` が重ならない）。読めない・JSON でない・外れたら専用の `TemplateError` を投げ、本文を照らさない。モジュールの読み込み時には投げないので、表示と終了コード（3）は境界の `main` が組む。検証した定義は配列も要素も凍結して `loadTemplate()` が返すので、後から書き換えられない。

## 正本に合わせる側の照合（`followers.json`・`followers.mjs`）

知見 `K-0005`（PR 本文の見出しの定義が、rubric の `PR-002` とリポジトリの決まりで割れていると、PR ごとに判断が割れる）を決定論で担保する（Issue #1121。解決策の種類は config。議決 `D-0071` の config の案と Issue #1194 で、実装役の報告の型を合わせる側に足した）。見出しの正本を `template.json` の 1 つと `followers.json` に書き、それに合わせる側の見出しの一覧が正本と食い違ったら落とす。

- `followers.json`: `canonical` は `template.json` だけ（ほかを書くと定義が不正）。`followers` に合わせる側を並べる。今は CLAUDE.md の「PR の説明は … の見出しで」（区切り ` / `）と「（必須は … 。」（区切り ` と `）、Engine の `core-gates/rubric.yaml` の `PR-002` の `check` の「テンプレートの見出し（…）」（区切り `・`）、Engine の `orchestrator/worker/entrypoint.sh` の実装役の報告の型（「最後の出力を次の見出しだけで構成する」の次から続く `「## …」` の行。区切り ` / `）。各要素は `id`・`root`（`repo` か `engine`）・`path`・`headings`（`pattern` は捕まえる組が 1 つの正規表現、`separator` は区切り）と、任意の `required`、rubric の項目を指す `rubric_id`・`field`（そろえて書く）。一覧には任意の `item`（捕まえる組が 1 つの正規表現）を書け、書くと `pattern` で捕まえた範囲から `item` の組を出てくる順に拾い、`separator` でつないだものを一覧にする（実装役の報告の型のように、見出しの間に注釈が挟まる形のため。1 つも拾えなければ読めなかったことにする）
- `followers.mjs`: 合わせる側から一覧をちょうど 1 か所で取り出し、`template.json` の見出しを定義の順に同じ区切りでつないだ文字列と比べる（区切りで割らない。rubric の区切りの `・` が `影響範囲・互換性` にも入るため）。`required` があれば必須の見出しも同じく比べる。rubric は `- id: PR-002` の項目の中から `check` の 1 行の値（引用あり・なし）を読む

```sh
node followers.mjs [--engine-dir <Engine のチェックアウト>] [--json <出力先>]   # Engine は --engine-dir、なければ ENGINE_DIR、なければリポジトリの隣の ../engine
```

| rule | 外れる形 |
|---|---|
| `headings_mismatch` | 合わせる側の見出しの一覧が、`template.json` の見出しの順・名前・数と食い違う |
| `required_mismatch` | 合わせる側の必須の一覧が、`template.json` の必須の見出しと食い違う |

終了コードは 0 = 合っている、1 = 食い違った、2 = 合わせる側を読めなかった・一覧がちょうど 1 か所で取り出せなかった（照らしていない。黙って通さない）、3 = `followers.json` か `template.json` が不正。`--json` は `{ "ok": bool, "errors": [{ "rule", "follower", "expected", "actual", "message" }] }`。

照らすのは CI で、`followers.test.mjs` が `ci/pr-body-engine.yml` の `node --test` の中で走り、`ENGINE_DIR` の Engine の rubric・実装役の報告の型と今の CLAUDE.md を照らす（`ENGINE_DIR` を渡したときと `CI=true` では、Engine がなくても飛ばさずに落とす）。今の Engine（手元の 57069a8）の `PR-002` は `headings_mismatch` で食い違うが、`PR-002` の `check` が Issue #1121 の時点の形（`概要・背景・問題・対応内容・影響範囲・動作確認・補足`）のままのときだけ、この照合を `todo` にして CI を止めない。実装役の報告の型も同じで、Issue #1194 の時点の形（`概要 / 対応内容 / 影響範囲・互換性 / 動作確認 / 補足 / 仮決め / 知見`）のままの `headings_mismatch` だけを `todo` に数える。Engine の食い違いがどれもこの 2 つの今の形のときだけ `todo` にし、ほかの形の食い違いや、rubric・`entrypoint.sh` を読めない・一覧を取り出せないときは `todo` にせず落とす。建設者が Engine の `PR-002` の見出しの列を `template.json` に合わせ（`概要・背景・問題・対応内容・影響範囲・互換性・動作確認・補足・仮決め・知見`）、実装役の報告の型に `背景`・`問題` を足し）て `ENGINE_REF` を上げると、`todo` が外れて必須の照合になる（両方を合わせた写しで通ることを確かめた）。正本を変えるときは、同じ PR で CLAUDE.md をそろえ、Engine の `PR-002` と実装役の報告の型を合わせる。

## 結果

- 終了コード: 0 = 通る、1 = 外れた、2 = 本文を読めなかった、3 = 見出しの定義（`template.json`）が不正（本文は照らしていない。本文を直しても通らないので、定義を直す）
- 3 のときは標準エラーに「pr-body-check: 見出しの定義が不正: 理由」を出し、標準出力と `--json` には何も書かない
- 標準出力: 外れた箇所ごとに「N 行目: 理由」（本文全体の外れ、つまり `missing_required` は「本文: 理由」）と、最後に件数の 1 行
- `--json`: `{ "ok": bool, "errors": [{ "rule", "line", "heading", "message" }] }`。`missing_required` の `line` は `null`

## 本文の更新の前の関所（`gate.mjs`）

知見 `K-0001` を決定論で担保する配線の入口（議決 `D-0013` の案、Issue #956）。続きのジョブの summary を本文に組み込んだあと、`gh pr edit` で書く前の本文を `checkBody` に照らし、外れたら本文を書かずに本文の直し（body_fix）に回す。

```sh
printf '%s' "${NEW_BODY}" | node tools/pr-body-check/gate.mjs --pr-url "${PR_URL}"   # リポジトリの根から
```

- 終了コード: 0 = 本文を書いてよい、1 = 本文を書かずに body_fix に回す、2 = 読めなかった（本文は書かない）、3 = 見出しの定義が不正（本文は書かない。body_fix には回さず、定義を直す）
- 1 のときの標準出力: body_fix の入力の JSON 1 行。Engine の `dispatcher.enqueue_body_fix` が `job.enqueued` の `input` に入れる形（`{ pr_url, defects: [{ rubric_id, severity, location, evidence, remediation_hint }] }`）
  - `rubric_id` は本文の構成の指摘 `PR-002`（Engine の `core-gates/rubric.yaml`）、`severity` は `should`
  - `location` は「PR 本文 N 行目」（Engine の `is_body_only` が本文だけの指摘と読む形）。`evidence` は「pr-body-check <rule>: 理由」
- 外れた箇所は標準エラーに「N 行目: 理由」で出す。`--json <出力先>` で `{ ok, errors, body_fix }` を書く

## 本文の直しのあとの再検査（`recheck.mjs`）

知見 `K-0012` の担保（議決 `D-0033` の test の案、Issue #1053）。本文だけの直し（body_fix）が終わる前か、直した本文を検査する前に supervisor がマージしていないかを、制御プレーンのイベント列で見る。

```sh
node recheck.mjs --events <制御プレーンのイベントの JSON か JSONL> [--pr <PR の URL>] [--json <出力先>]
```

- イベント列は古い順に並べて渡す。形と読み方は `tools/sendback-check/check.mjs --events` と同じ（`parseEvents` を使う）。本文の直しの積みと終わりは `job_id`（events の列）で結ぶ
- 同じ PR の `supervisor.merged` ごとに、それより前を見て外す:

| rule | 外れる列 |
|---|---|
| `merged_without_body_recheck` | `pr.body_amended` のあと、本文の検査（`gate.evaluated`）がないまま `supervisor.merged` が来た（事例 `C-0006`、PR #821: 直し 14:25:04、マージ 14:25:06） |
| `merged_before_body_fix_finished` | `job.enqueued`（`kind: body_fix`）のあと、その終わり（`pr.body_amended` か `pr.body_amend_failed`）より前に `supervisor.merged` が来た（事例 `C-0005`、PR #659: マージ 20:08:43、直し 20:08:50） |
| `merged_after_body_fix_failed` | 本文の直しが失敗で終わり（`pr.body_amend_failed`）、そのあと直しが成功（`pr.body_amended`）しないまま `supervisor.merged` が来た |

- 本文の検査は `gate.evaluated` で数え、`gate` は問わない（検査役の `rubric` も、`check.mjs` の結果を `gate: "pr-body-check"` で記録したものも数える）。Engine の supervisor が直した本文の `pr-body-check` が通ったのを確かめて残す `supervisor.body_rechecked`（下の「本文の直しのあとの再検査を待つ supervisor（建設者）」）も数える。直す前の検査とほかの PR の検査は数えない
- 終了コード: 0 = 通る、1 = 外れた、2 = イベント列を読めなかった。標準出力に外れごとに「PR の URL rule: 理由」と、最後に件数の 1 行。`--json` は `{ ok, errors: [{ rule, pr_url, message }] }`
- `recheck.mjs` はまだどこからも呼んでいない（CI にも Engine にも配線していない）。テスト（`recheck.test.mjs`）は `npm test` と、ひな形 `ci/pr-body-engine.yml` の `engine-test`（道を渡さない `node --test`）で走る。建設者がひな形を `.github/workflows/pr-body-engine.yml` に写すと PR ごとに走る（議決 `D-0050`、Issue #1131）。マージを止めるのは `recheck.mjs` ではなく、Engine の supervisor の待ち（`engine/pr-body-recheck.patch`）とマージ前の必須チェック（`ci/pr-body-check.yml`）で、`recheck.mjs` はあとから記録を見て外れを数える側

## テスト

`npm test` で 8 つを走らせる。

- `check.test.mjs`: 見出しの検査（`check.mjs`）
- `gate.test.mjs`: 関所。事例 `C-0005`（PR #659）の 21 回の追記（conflict が 20 回、ci_failed が 1 回。どれも新しい PR と同じ `## 概要`・`## 対応内容`・`## 動作確認` を持つ summary）と同じ入力で落ち、1 回目の追記でもう落ちることを確かめる。入力は `c0005.mjs` が組む（本文そのものは記録にないので、事例に書かれた形を組み直したもの）。通る側の写しとして、同じ 21 回の報告を「## 工場の記録」の下の `### 続きのジョブ（job N）の記録` に、見出しを `###`・`####` に下げて、または `## ` 見出しのない形で足した本文では、毎回 `gate` が書いてよいと返し、CLI が 0 で何も出さないことも確かめる（誤検知がないこと。議決 `D-0032` の test の案、Issue #1038）
- `engine.test.mjs`: Engine の本物で確かめる。Engine のチェックアウトは環境変数 `ENGINE_DIR` で渡す（なければリポジトリの隣の `../engine`）。どちらもなければ飛ばす。ただし `ENGINE_DIR` を渡したときと CI（`CI=true`）では飛ばさずに落とす（`tools/sendback-check` と同じ。議決 `D-0011`）
  - (0) Engine の `orchestrator/worker/entrypoint.sh` から、新しい PR の本文（`PR_BODY`）・続きのジョブの組み込み（`SECTION_TITLE` から `NEW_BODY` まで）・本文の更新（`gh pr view` から `continued` のログまで）の節を取り出せ、`engine_harness.py` が `dispatcher.py` の `is_body_only`・`enqueue_body_fix` を取り出せる
  - (1) Engine の組み込みで C-0005 の 21 回の追記をした本文が関所で落ちる（Engine が組み込み方を直して重ならなくなったら、それでよい）
  - (2) 関所の body_fix の入力が Engine の `is_body_only` で本文だけの指摘と読まれ、`enqueue_body_fix` で body_fix のジョブが積まれる
  - (3) Engine の本文の更新の節を、偽の `gh`（`pr view` と `pr edit --body` だけを真似る）で C-0005 の 21 回分動かし、関所に外れる本文を一度も書かない。必須のテストで、`todo` はない（議決 `D-0019`、Issue #975 で外した）。Engine の `entrypoint.sh` が関所（`tools/pr-body-check/gate.mjs`）を呼ばない形なら落ちるので、`engine/pr-body-gate.patch` を入れていない Engine では落ちる
  - (4) `ENGINE_DIR` の Engine の写しに `engine/pr-body-gate.patch` を当て（もう関所を呼んでいれば当てない）、(3) と同じ 21 回で外れる本文を一度も書かず、外れた回ごとに引き継ぎ（`HANDOFF_DIR/jobs/<job>/body_gate.json`）に関所の結果（`rc`・`body_fix`・書かなかった報告の全文 `pending_entry`・そのファイルの場所 `handoff`）を残す。差分が当たらなければ落ちる
  - (5) 同じ写しで、`## ` 見出しを持たない続きのジョブの報告は止めずに「## 工場の記録」の下の `### 続きのジョブの記録` に足して書く（誤検知しない）
  - (6) 同じ写しの dispatcher の `act_on_body_gate` が、`rc` 1 の結果で body_fix を 1 件積み（関所の指摘に、書かなかった報告の全文と引き継ぎの場所を「PR 本文 末尾」の指摘として足す）、`rc` 2・3 では積まずに人に回す（停止を開く）
  - (7) 4000 字を超える報告も切らずに、引き継ぎの `body_gate.json` と body_fix の指摘に全文が渡る
  - (8) 未完了の続きが仕上がった回（`CONTINUE_REASON=incomplete`・`INCOMPLETE` が空）に関所で外れても、報告を足さない本文で仕上げ（未完了の印を消す・`Refs` を `Closes` に直す・題を戻す・ドラフトを外す）だけは済ませ、`finished.applied` を true で残す。報告は body_fix に回し、人には回さない
  - (9) 同じ回で報告を足さない本文も関所で外れたら、仕上げも書かず `finished.applied` を false で残し、dispatcher が停止（`body_gate_finish:<PR の URL>`、human）を開いて人に回す
  - (10) `ENGINE_DIR`（CI では `ENGINE_REF` の版）の Engine の `entrypoint.sh` そのものの本文の更新の節で（差分を当てた写しではなく）、C-0005 の 21 回の続きのジョブの報告の見出しを `###`・`####` に下げたか `## ` 見出しを持たない形なら、毎回関所を 1 回呼んで 0 が返り（`node` を包んで終了コードを残す）、`body_gate.json` を残さずに本文を書き、書いた本文が関所に通る（正しい更新まで止める誤検知がないこと。議決 `D-0032` の test の案、Issue #1038。議決 `D-0045` の test の案、Issue #1110 で写しから本物に替えた）。必須のテストで、関所を呼ばない Engine では (3) と同じく落ちる。議決 `D-0044` の test の案（Issue #1107）で、この通る側を必須のテストとして担保に記録した
  - (11) Engine がまだ関所を呼ばない間だけ、(10) と同じことを `engine/pr-body-gate.patch` を当てた写しで確かめる（差分そのものの誤検知がないこと）。Engine が関所を呼ぶようになったら (10) が本物で確かめるので飛ばす。Engine の行が変わって `engine/pr-body-gate.patch` が当たらなくなると (4)〜(9) と (11) がそろって落ち、差分を確かめられなくなるので、そのときは同じ PR で差分を今の Engine に合わせて作り直す
  - (12) `ENGINE_DIR` の Engine の写しに `engine/pr-body-recheck.patch` を当て（もう `body_recheck_pending` があれば当てない）、`engine_harness.py --body-recheck` で `supervisor.py` の `body_fix_pending`・`body_recheck_pending` を `merge_step` と同じ順に動かす。入力は事例 `C-0006`（PR #821）の記録の順（本文の直し job 4299 を 14:24:41 に積み、マージ 14:25:06）で、GitHub の `pr-body-check` の実行は時刻と結果だけを真似る。`merge_step` が両方を待つ（`if …: continue`）のがマージより前にあること、直しが走っている間は待つこと、直しが終わっても直しのあとに始まった `pr-body-check` が通るまでは待つこと（走っていない・直す前の実行だけ・実行中・落ちた・取り消された・ワークフローを読めない）、通ったら `supervisor.body_rechecked` を 1 回だけ残し、それを挟んだ C-0006 の列が `recheck.mjs` に通ること、直しが失敗・取り消しで終わったら停止（`body_recheck:<PR の URL>`、human）を開いて待つこと、直しのない PR と `pr-body-check` のワークフローを持たない製品では待たないことを確かめる
- `ci.test.mjs`: CI のひな形 `ci/pr-body-engine.yml` の形と、`gate.test.mjs`・`engine.test.mjs` の (10)・`recheck.test.mjs`（skip・todo・only がなく、`C-0003`・`C-0005`・`C-0006` の列のテストがある）が実行の対象から外れていないこと、`gate.test.mjs`・`engine.test.mjs` を TAP で走らせた結果で通る側のテストと (10)・(12) が pass と数えられたこと（`engine.test.mjs` で飛ばしてよいのは、共通の skip と、Engine が関所を呼ぶようになったときの (11) だけ）（下の「配線の手順（建設者）」）、`ci/pr-body-protection.yml` と `ci/pr-body-patch.yml` の形と手順の振る舞い、`ci/pr-body-protection.yml` の写しがあれば写しそのものが定期・手動で走りトークンを読めないときに落ちること（下の「配線の手順（建設者）」「定期の照合」「差分の当たりの照合」）、`ci/pr-body-check.yml` の形（下の「マージ前の必須チェック（建設者）」）
- `patch.test.mjs`: 差分の当たりの照合（`patch.mjs`）。作った Engine の写し（git の版）で、当たる・もう入っている・入れたあと変わった・一部だけ入った・どれでもないの判定と、チェックアウトの今の中身ではなく名指しした版で照らすこと、CLI の終了コードを確かめる。本物の Engine では、差分が `ENGINE_REF` の版に当たるかもう入っていることを確かめる（`engine.test.mjs` と同じ飛ばし方。`ENGINE_DIR` を渡したときと `CI=true` では落とす）
- `protection.test.mjs`: 必須のチェックの照合（`protection.mjs`）
- `followers.test.mjs`: 正本に合わせる側の照合（`followers.mjs`）。今の CLAUDE.md が通り、順・名前・数・必須を変えると落ち、`template.json` だけを変えるとすべての合わせる側が落ち、Issue #1121 の時点の Engine の `PR-002` の形と Issue #1194 の時点の実装役の報告の型（`背景`・`問題` がない）が `headings_mismatch` で落ち、報告の型の見出しの順・名前・数を変えると落ちて注釈だけの変更は通り、一覧を取り出せない形は照らさずに読めなかったことにする。本物の Engine の rubric の照合は `engine.test.mjs` と同じ飛ばし方（`ENGINE_DIR` を渡したときと `CI=true` では落とす）。Engine の食い違いが `PR-002` の Issue #1121 の時点の形と実装役の報告の型の Issue #1194 の時点の形だけのときに `todo`（その判定も、ほかの形や読めない Engine では `todo` にしないことを確かめる）
- `recheck.test.mjs`: 本文の直しのあとの再検査（`recheck.mjs`）。事例 `C-0005`（PR #659）と `C-0006`（PR #821）の記録に書かれた時刻・job・順をイベントの形に組み直した列が、それぞれ `merged_before_body_fix_finished` と `merged_without_body_recheck` で落ち、事例 `C-0003`（PR #396。最後の本文の直しの job 572 のあと、検査を通さずに 14:23:30 にマージ）の列も `merged_without_body_recheck` で落ち、直しの終わりと再検査を待ってからマージした列は通ることを確かめる。Engine の supervisor が残す `supervisor.body_rechecked` も再検査に数え、直す前とほかの PR のものは数えないことも確かめる
- 節の取り出し方（`snippets`）は `entrypoint.sh` の今の行の形に合わせている。Engine で形が変わって取り出せなくなったら (0) が落ちるので、同じ PR で合わせる

```sh
npm test
ENGINE_DIR=/path/to/factory-engine node --test engine.test.mjs
python3 engine_harness.py --check /path/to/factory-engine   # 関数を取り出せるかだけを見る
python3 engine_harness.py --body-gate /path/to/factory-engine < gate.json   # act_on_body_gate を動かす（差分を当てた Engine）
python3 engine_harness.py --body-recheck /path/to/factory-engine < recheck.json   # body_fix_pending・body_recheck_pending を動かす（engine/pr-body-recheck.patch を当てた Engine）
```

## マージ前の必須チェック（建設者）

head_sha の変わらない本文だけの直しも通すため、CI では `pull_request` の `opened`・`synchronize`・`reopened` に加えて `edited` でも起動させ、マージ前の必須チェックにする（知見 `K-0006` の議決 D-0006、Issue #906）。知見 `K-0012` の担保（議決 `D-0050` の ci_check の案、Issue #1122）も同じ 1 つのチェックにまとめ、別の検査は作らない。本文は `github.event.pull_request.body` を環境変数で受けて標準入力に渡す（`${{ }}` で run に直接埋めない）。ひな形は `ci/pr-body-check.yml`（ジョブ `pr-body-check`、チェックの名前 `pr-body-check`）。

```yaml
on:
  pull_request:
    types: [opened, synchronize, reopened, edited]
jobs:
  pr-body-check:
    name: pr-body-check
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - env:
          BODY: ${{ github.event.pull_request.body }}
        run: printf '%s' "$BODY" | node tools/pr-body-check/check.mjs
```

`ci.test.mjs` が、ひな形の形（`on` が `pull_request` の 4 つの `types` だけで、`paths`・`branches` で絞らない、`if:`・`continue-on-error`・`needs` がない、`name:` が `pr-body-check`、本文を `env` の `BODY` で受けて `printf '%s' "$BODY" | node tools/pr-body-check/check.mjs` の 1 行で照らす）と、Engine の supervisor（`engine/pr-body-recheck.patch`）が待つチェックの名前とワークフローの道がこのひな形に合うこと、写しがあればひな形と同じことを見る。外す直し（`edited` を外す・`paths` で絞る・`|| true`・本文を run に埋めるなど）が外れになることも確かめる。

なお、Engine の `orchestrator/worker/entrypoint.sh` は今、続きのジョブの報告を `## 工場の記録` の後ろに `## 続きのジョブの記録` として足すので、そのままでは `outside_factory_record`・`continuation_outside_record` で外れる。必須チェックにする前に、Engine 側でその見出しを `### ` に下げる（K-0001 の線。Engine への作用なので人の承認が要る）。下げる直しは `engine/pr-body-gate.patch` に入れてある。

このワークフローとブランチ保護の必須チェックは、ワーカーの権限の外なので建設者が入れる。`ci/pr-body-check.yml` を `.github/workflows/pr-body-check.yml` にそのまま写し（道と `name:` を変えると Engine の supervisor が待たない・待ち続ける）、`pr-body-check` を必須のチェックにする。

写して必須のチェックにし、下の差分を Engine に入れて `ENGINE_REF`（`.github/workflows/pr-body-engine.yml`）を上げたら、同じ変更セットで知見 `K-0012` の issue 1122・1196 の解決策と、同じ `check.mjs` を使う `K-0006` の issue 906 の解決策を `wired: true` にする（議決 `D-0100`（起票時の番号は `D-0071`）、Issue #1196）。`wired_by` には 3 つとも `{workflow: .github/workflows/pr-body-check.yml, job: pr-body-check, runs: tools/pr-body-check/check.mjs}` を、`K-0012` の 2 つにはさらに `{workflow: .github/workflows/pr-body-engine.yml, job: engine-test, runs: tools/pr-body-check/engine.test.mjs}`（(12) が `merge_step` の待ちを確かめる）を書く。書かないと `tools/knowledge-enforcement-check/check.py` が落とす（`REQUIRED_PR`・`REQUIRED_PR_WORKFLOW`・`REQUIRED_PR_ALSO`）。`ci/pr-body-engine.yml` を写して `pr-body-engine-test` を必須のチェックにしたら、`K-0012` の issue 1197（test）の解決策も `wired: true` にし、`wired_by` に `{workflow: .github/workflows/pr-body-engine.yml, job: engine-test, runs: …}` を `runs` が `tools/pr-body-check/recheck.test.mjs`・`tools/pr-body-check/engine.test.mjs`・`tools/pr-body-check/ci.test.mjs` の 3 つで書く（議決 `D-0100`（起票時の番号は `D-0071`）、Issue #1197。1 つでも欠けると `check.py` が落とす）。Engine の `merge_step` の道は `wired_by` の形では書けないので、`check.py` が `ENGINE_REF` の Engine の `orchestrator/dispatcher/supervisor.py` で `merge_step` が `body_recheck_pending` を呼んでいるかを照らし、差分を入れる前に `wired: true` にすると `engine_unwired` で落とす（`REQUIRED_ENGINE_WAIT`。`tools/knowledge-enforcement-check/README.md` の「Engine の merge_step の照合」）。

### 本文の直しのあとの再検査を待つ supervisor（建設者）

知見 `K-0012` の担保（議決 `D-0050` の ci_check の案、Issue #1122）。必須のチェックだけでは、本文の直しのジョブが終わってから `edited` のチェックが始まるまでの数秒の間に、前の本文で通ったチェックのままマージできる（事例 `C-0006`、PR #821: 直しの記録の 2 秒後にマージ）。また Engine の今の `body_fix_pending` は検査役の承認（`inspector_approved`）の中で直しの順番待ちと作業中しか見ず、直しが終わったあとと、検査役の承認が直しを見ずに返る経路（検査なし・裁くものがない）は待たない。そこで Engine の supervisor に入れる差分を `engine/pr-body-recheck.patch` に置いた（`orchestrator/dispatcher/supervisor.py`・`definitions/events.yaml`。Engine のチェックアウトの根で `git apply` する。`engine/pr-body-gate.patch` と一緒に当てられる）。

- `merge_step` で、検査役の承認と委員会の採用のあと、マージの前に `body_fix_pending`（直しの順番待ちか作業中）と `body_recheck_pending` を呼び、どちらかが待つなら次の周回まで待つ
- `body_recheck_pending` は、その PR の最後の本文の直し（`jobs` の `kind` が `body_fix`）を見る。直しがなければ待たない。成功で終わっていなければ（`failed`・`cancelled`）停止（`body_recheck:<PR の URL>`、human）を開いて待つ（直していない本文のままマージしない。PR が閉じたら閉じる）。成功で終わっていれば、製品に `.github/workflows/pr-body-check.yml` があるとき（なければ待たない）、head の `pr-body-check` のチェックの実行のうち最後に始まったものが、直しのジョブが走り始めた（なければ積んだ）時刻より後に始まって `success` で終わるまで待つ。通ったら `supervisor.body_rechecked`（`pr_url`・`head_sha`・`gate`・`job_id`・`check_run`・`started_at`。PR と直しのジョブごとに 1 回）を残し、以後は GitHub を引かない
- 直しの終わりは `pr.body_amended` ではなくジョブの状態で見る（ジョブの状態を書いてから `pr.body_amended` を記録するまでの間もマージさせないため）
- 入れて `ENGINE_REF` を上げるまで、`engine.test.mjs` の (12) は写しに差分を当てて確かめる。入れたら、本物の本文の直しで `supervisor.body_rechecked` のあとに `supervisor.merged` が来たこと（と `recheck.mjs` が過去の PR #659・#821 の本物のイベント列で落ちること）を記録してから、知見 `K-0012` のこの解決策の `wired`・`verified` を `true` にする

## Engine に入れること（建設者）

Engine は別のリポジトリなので、次は建設者が入れる。そのための差分を `engine/pr-body-gate.patch` に置いた（Engine の `orchestrator/worker/entrypoint.sh`・`orchestrator/dispatcher/dispatcher.py`・`definitions/events.yaml`。Engine のチェックアウトの根で `git apply` する）。入れて `ENGINE_REF` を上げるまで、`engine.test.mjs` の (3) は落ちる。

1. `worker/entrypoint.sh` の続きのジョブの節で、`NEW_BODY` を組んだあと `gh pr edit` の前に、リポジトリに `tools/pr-body-check/gate.mjs` があれば `NEW_BODY` を照らす。1（か 2）なら `gh pr edit` で本文を書かず、標準出力の body_fix の入力と組んだ本文（続きのジョブの報告）を引き継ぎ（`HANDOFF_DIR`）と `FACTORY_RESULT` に残す。3（見出しの定義が不正）なら本文を書かず、body_fix には回さずに人に回す（本文を直しても通らない）。push した head（`pr.head_updated`）と引き継ぎ（`handoff.saved`）は今のまま
2. dispatcher が、その結果を見て `enqueue_body_fix` で body_fix を積む（判定役を待たない。検査役にも回さない）。body_fix には、書かなかった続きのジョブの報告を「## 工場の記録」の下の `###` に入れて本文を組み直すよう渡す
3. body_fix が直した本文も関所に照らし、まだ外れたら人に回す（同じ直しを繰り返さない）

差分が入れるのは 1 と 2（3 は入れていない）:

- `entrypoint.sh`: 続きのジョブの記録の見出しを `### 続きのジョブの記録`（「## 工場の記録」の下）にする（前の `## 続きのジョブの記録` がある本文にはその下に足す）。未完了の印を外す直しのあとの `NEW_BODY` を、`gh pr edit` の前に関所に照らし、0 以外なら `gh pr edit`・`gh pr ready` を呼ばず、`{ rc, body_fix, pending_entry, finished, handoff }`（`pending_entry` は書かなかった報告の全文で切らない。`finished` は未完了の続きが仕上がった回だけで、仕上げを済ませたか（`applied`）と題・Issue の番号。`handoff` はそれを残した引き継ぎのファイル）を `FACTORY_RESULT` の `body_gate` と引き継ぎの `jobs/<job>/body_gate.json` に残す。仕上がった回は、報告を足さない本文が関所に通れば、それで仕上げ（未完了の印・`Closes`・題・`gh pr ready`）だけを先に書く。関所のないリポジトリでは今までどおり書く
- `dispatcher.py`: 続きのジョブの結果に `body_gate` があれば `act_on_body_gate` を呼ぶ。`rc` 1 は判定役を待たずに `enqueue_body_fix` で body_fix を積み、書かなかった報告を切らずに全文で、引き継ぎの場所を添えて「PR 本文 末尾」の指摘として渡す。`rc` 2・3 は停止（`body_gate:<PR の URL>`、human）を開く。仕上げを書けなかった（`finished.applied` が false）ときは、body_fix では行を消したりドラフトを外したりできないので、停止（`body_gate_finish:<PR の URL>`、human）を開いてやることを待ち事項に書く。どちらも `pr.body_gate_blocked` を残す
- `definitions/events.yaml`: `pr.body_gate_blocked` を足す

`ci/pr-body-engine.yml` は、Engine を固定の版で取って `ENGINE_DIR` を向け、このディレクトリのテストを PR ごとに走らせる CI。ワーカーは `.github/workflows/` を変えられないので、建設者が `.github/workflows/pr-body-engine.yml` に写し、`engine-test`（チェックの名前は `pr-body-engine-test`）をブランチ保護の必須チェックにする（議決 `D-0019`。手順と見張りは下の「配線の手順（建設者）」）。(3) に `todo` がないので、関所を呼ばない Engine の版（今の `ENGINE_REF`）のままだと落ちる。差分を Engine に入れて `ENGINE_REF` を上げるのと同じ PR で写す。この CI では PR 本文そのものは見ない（落ちると `ci_failed` の続きのジョブが積まれ、その summary がまた本文に足されて、この知見の形を増やすため）。本文は上の「マージ前の必須チェック」で見るが、同じ理由で、そのチェックが落ちたときは `ci_failed` の続きのジョブを積まず、関所と同じく body_fix に回す（建設者が Engine の dispatcher で分ける）。

## 配線の手順（建設者）

議決 `D-0032`（Issue #1037）の案。Engine に差分を入れて `ENGINE_REF` を上げ、同じ PR で `ci/pr-body-engine.yml` を `.github/workflows/pr-body-engine.yml` に写して、`engine-test` のジョブを必須のチェックにする。ワーカーの権限で入れたのは、その配線が効く形かの見張り（`ci.test.mjs`）と、必須になったかの照合（`protection.mjs`）まで。

`ci.test.mjs`（`engine-test` の `node --test` の中で走る）が、ひな形と写しで次を見て、外れたら落とす。

- `on` が絞り込みのない `pull_request` だけ（`paths`・`paths-ignore`・`branches`・`types` は中身を問わず外れ）。`if:`・`continue-on-error`・`engine-test` の `needs` がない
- `ENGINE_REF` がワークフローの `env` に 40 桁のコミット SHA で 1 行だけあり（ジョブや手順の `env`・`GITHUB_ENV` で上書きしない）、Engine を取る手順の `ref` が `${{ env.ENGINE_REF }}`
- テストの手順が `tools/pr-body-check` で `ENGINE_DIR` を渡し、道を渡さずに `node --test` で全部のテストを走らせ（`--test-reporter` のほかの引数、`--test-name-pattern`・`--test-skip-pattern`・`--test-only` などを付けない。ワークフローのどこにも `NODE_OPTIONS` を置かない）、`exit $rc` で落ちる
- 通る側のテストが実行の対象から外れていない（議決 `D-0045` の ci_check の案、Issue #1109）。同じ `node --test` で必ず走らせるのは、`gate.test.mjs` の関所と CLI のテスト（`test("CLI …")` が 3 件以上、通る側の CLI のテストを含む）と、`engine.test.mjs` の (10)（通る側の 3 通りの形 `PASSING_REPORTS` を回すテスト。形が 3 通り以上）。`gate.test.mjs` には `skip` を置かない。`engine.test.mjs` の飛ばしは、`ENGINE_DIR` を渡すと `false` になる共通の `skip`（`required` の行と `skip` の宣言は 1 つ。テストには `{ skip }` だけを渡す）にかぎる。どちらにも `todo`・`only`・`test.skip`・`t.skip` などを置かない。注釈の行は見ない。外す直し（`{ skip: true }`・`{ todo: true }`・`test.skip`・`t.skip()`・(10) だけ別の `skip`・`required` から `ENGINE_DIR` を外す・ループを空にするなど）が外れになることも確かめる
- 通る側のテストが実際に走って pass と数えられた（議決 `D-0070`（Issue #1183 の起票時の番号は `D-0067`。PR #1181）の test の案、Issue #1183）。上の見張りはファイルの字面を見るだけなので、列挙していない飛ばし方（別名で呼ぶ `t.skip`、`describe.skip`、`ENGINE_DIR` が空の文字列で渡るなど）はすり抜けうる。`ci.test.mjs` が `gate.test.mjs` と `engine.test.mjs` をそれぞれ `node --test --test-reporter=tap` で走らせ（外の `node --test` の `NODE_TEST_CONTEXT` は渡さない）、`gate.test.mjs` の通る側のテスト（書いてよい本文の 2 件、通る側の 3 通りの形、通る側の CLI のテスト）と `engine.test.mjs` の (10)（3 通りの形）が上の段に名前のとおりにあって、`SKIP`・`TODO` の指示のない `ok` で、ファイルごとの集計の `skipped`（古い node は `skip`）と `todo` が 0 であることを見る。集計がない（途中で止まった）ときも落とす。`engine.test.mjs` のほかのテスト（関所を呼ばない Engine で落ちる (3) など）の失敗は同じ `node --test` で落ちるのでここでは見ない。Engine のチェックアウトがなく `ENGINE_DIR` も `CI=true` もない手元では `engine.test.mjs` の側だけ飛ばす（`engine.test.mjs` と同じ飛ばし方）。字面の見張りを通る別名の `t.skip` を `gate.test.mjs` の写しに入れると、走らせた結果で外れになることも確かめる。テストの中の条件付きの早い `return` のように、飛ばさずに pass と数えられる外し方はここでは捕まえられないので、次の見張りで見る
- (10) に条件付きの飛ばしがない（議決 `D-0099`（Issue #1187 の起票時の番号は `D-0069`。PR #1184）の ci_check の案、Issue #1187）。`engine.test.mjs` の (10) の本体と、そこから呼ぶ `assertWritesEveryTime` の本体に、分かれ道（`return`・`if`・`else`・`switch`・`try`・`catch`・`break`・`continue`・三項・`&&`・`||`・`??`）と環境変数・引数の読み取り（子のプロセスに渡す `process.env.PATH` は除く）と `skip`・`todo`・`only` がないこと、(10) が本体の上の段（入れ子の関数の中でなく）で `need("update")` と `assertWritesEveryTime(parts.update, report)` を呼ぶこと、`assertWritesEveryTime` が `REASONS.forEach((reason, i) => {` で C-0005 の 21 回を絞らずに回し、`c0005.mjs` の `REASONS` が 21 回であることを見る。注釈の行は見ない。(10) や `assertWritesEveryTime` の頭や 21 回の途中での条件付きの `return`、短絡・三項・`try`/`catch` で確かめを通さない、確かめを入れ子の関数に入れて呼ばない、回数を絞るなどの 11 通りの直しが外れになり、そのうち条件付きの `return` と `try`/`catch` は上の字面の見張りをすり抜けることも確かめる
- `recheck.test.mjs`（知見 `K-0012` の担保）が実行の対象から外れていない（議決 `D-0050` の ci_check の案、Issue #1131）。`skip`・`todo`・`only` を置かず、事例 `C-0003`・`C-0005`・`C-0006` の列が落ちるテスト（`test("C-NNNN の列は、…")`）が残っている。注釈の行は見ない
- `engine.test.mjs` の (12)（本文の直しの終わりと直した本文の `pr-body-check` をマージの前に待つ `merge_step` の線。知見 `K-0012` の担保）が実際に走って pass と数えられた（議決 `D-0100`（Issue #1197 の起票時の番号は `D-0071`。PR #1192）の test の案、Issue #1197）。`engine.test.mjs` の (12) のテストの名前が `ci.test.mjs` の一覧（6 件）と同じで、TAP で走らせた結果でそれぞれが `SKIP`・`TODO` の指示のない `ok` で、集計の `skipped`（(11) を除く）と `todo` が 0 であることを見る。(12) を消す・名前を変える・飛ばす・`todo` にする・落ちたまま（差分から `body_recheck_pending` の待ちを外すなど）にする書き換えが外れになることも確かめる。(12) の中の条件付きの早い `return` のように、飛ばさずに pass と数えられる外し方はここでは捕まえない（(10) の見張りと違い、まだ見ていない）
- `engine-test` の `name:` が `pr-body-engine-test`。`tools/knowledge-enforcement-check/ci/knowledge-enforcement.yml` にも id のままの `engine-test` のジョブがあるので（`tools/sendback-check/ci/sendback-engine.yml` の `engine-test` は `sendback-engine-test` を報告する。議決 `D-0038`）、同じ名前だと必須のチェックの「engine-test」がどちらの報告でも満たされ、この写しを消しても止まらない
- ひな形の `ENGINE_REF` が `tools/sendback-check/ci/sendback-engine.yml` と同じ版。写しがあれば、`ENGINE_REF` の行のほかがひな形と同じで、`.github/workflows/sendback-engine.yml` の写しとも同じ版。GitHub Actions で写しがなければ落ちる

写して `engine-test` を必須のチェックにしたら、知見 `K-0012` の Issue #1131 の解決策を `wired: true` にし、`wired_by` に `{workflow: .github/workflows/pr-body-engine.yml, job: engine-test, runs: tools/pr-body-check/recheck.test.mjs}` を書く（`tools/knowledge-enforcement-check/check.py` は、ファイルを名指ししない `node --test` を `tools/pr-body-check` で走らせる形を `recheck.test.mjs` の呼び出しに数える）。

`protection.mjs` は、`gh api` で取り出した branch protection（古い形か rulesets）の JSON を読み、写しの `engine-test` が報告するチェックの名前（`pr-body-engine-test`）が必須のチェックにあるかを照らす。branch protection は PR の `GITHUB_TOKEN` では読めないので、PR ごとには走らせず建設者が走らせる。

```sh
gh api repos/{owner}/{repo}/branches/main/protection > protection.json   # 古い形
gh api repos/{owner}/{repo}/rules/branches/main > rules.json             # rulesets
node tools/pr-body-check/protection.mjs --protection protection.json --protection rules.json   # リポジトリの根から
```

`--protection` は何度でも渡せる。古い形と rulesets の両方を渡すと、どちらかで必須になっているチェックを必須に数える（議決 `D-0044`、Issue #1106）。

| rule | 外れる形 |
|---|---|
| `workflow_not_copied` | `.github/workflows/pr-body-engine.yml` がないか、`engine-test` のジョブがない |
| `check_name_changed` | `engine-test` のチェックの名前が `pr-body-engine-test` でない |
| `required_check_missing` | そのチェックの名前が必須のチェックにない（`engine-test` の id だけを必須にしても外れる） |
| `check_name_shared` | `.github/workflows/` のほかのジョブも同じチェックの名前を報告する |

終了コードは 0 = 通る、1 = 外れた、2 = 読めなかった。今のリポジトリには写しがないので `workflow_not_copied` で落ちる。

順序:

1. 写す前に、今の `ENGINE_REF`（関所を呼ばない Engine）を `ENGINE_DIR` にして `engine.test.mjs` の (3) が落ちることを記録する（`CI=true ENGINE_DIR=<Engine> node --test engine.test.mjs`）
2. `engine/pr-body-gate.patch` を Engine に入れ、`ENGINE_REF` をその版に上げる（`ci/pr-body-engine.yml` と `tools/sendback-check/ci/sendback-engine.yml` の両方。どちらのテストも通る版にする）。同じ PR で `ci/pr-body-engine.yml` を写し、`pr-body-engine-test` が通ることを確かめる
3. `pr-body-engine-test` を必須のチェックにし、`protection.mjs` が 0 で終わることを記録する。`ci/pr-body-protection.yml` を `.github/workflows/pr-body-protection.yml` に写してシークレット `PR_BODY_PROTECTION_TOKEN` を置き、手動で走らせて通ることを確かめる（下の「定期の照合」。議決 `D-0044`、Issue #1106）
4. 本物の続きのジョブで、関所に外れた本文が書かれず（`pr.body_gate_blocked`）、body_fix が積まれた（`job.enqueued`）ことを記録する
5. 1〜4 の記録を Issue #1037（と Issue #1106）に残してから、知見 `K-0001` のこの解決策の `wired` を `true`（`wired_by` に `.github/workflows/pr-body-engine.yml`・`engine-test`・`tools/pr-body-check/engine.test.mjs`）に、`verified` を `true` にする。Issue #1182 の解決策（議決 `D-0070`）は、`wired_by` に `{workflow: .github/workflows/pr-body-engine.yml, job: engine-test, runs: tools/pr-body-check/engine.test.mjs}` と `{workflow: .github/workflows/pr-body-protection.yml, job: protection, runs: tools/pr-body-check/protection.mjs}` の両方を書かないと、`tools/knowledge-enforcement-check/check.py` が落とす（`REQUIRED_PR`・`REQUIRED_PR_WORKFLOW` と `REQUIRED_SCHEDULE`・`REQUIRED_SCHEDULE_WORKFLOW`。branch protection は PR の `GITHUB_TOKEN` では読めないので、`protection.mjs` は `engine-test` と同じジョブでなく定期の `protection` のジョブで走らせる）
6. 上げた `ENGINE_REF` の Engine で `engine.test.mjs` の (10)（3 通りの形の 21 回で関所が毎回 0 を返して本文を書く）が通り、(11) が飛ぶ（差分の写しではなく本物で確かめた）ことを記録してから、知見 `K-0001` の Issue #1038 と Issue #1110 の解決策の `wired` と `verified` を `true` にする（議決 `D-0045`）

### 定期の照合（`ci/pr-body-protection.yml`）

`ci.test.mjs` は守りたい `engine-test` の中で走るので、写しが消えたり `pr-body-engine-test` が必須のチェックから外されたりしたときは自分も走らず、その崩れを見張れない。建設者が一度 `protection.mjs` を走らせるだけでも、後で外されたことに気づけない（議決 `D-0044`、Issue #1106）。`ci/pr-body-protection.yml` の `protection` のジョブ（チェックの名前 `pr-body-protection`）が、`protection.mjs` を既定のブランチ（`main`）への push・1 日 1 回（UTC 20:30、JST 5:30）・手動（`workflow_dispatch`）で走らせる。push で走るので、写しを消す PR がマージされた直後にも落ちる。

- branch protection は `GITHUB_TOKEN` では読めないので、読める権限のトークン（GitHub App か `administration:read` の fine-grained PAT）をシークレット `PR_BODY_PROTECTION_TOKEN` に置く（建設者。`tools/sendback-check/ci/sendback-protection.yml` の `SENDBACK_PROTECTION_TOKEN` と同じ値でよい）。トークンを渡すのは branch protection を読む手順だけ
- `main` の古い形（`branches/main/protection`）と rulesets（`rules/branches/main`）の両方を読み、`--protection` に 2 つとも渡す。古い形が保護なし（404 の `Branch not protected`）のときはそのまま渡す（rulesets にもなければ `required_check_missing`）
- トークンがない・読めなかった（権限がないなど）ときは `::error title=pr-body-protection protection_unreadable::…` で落ちる。必須のチェックの有無が分からないので Issue は開かない
- `workflow_not_copied`・`check_name_changed`・`required_check_missing`・`check_name_shared` のどれかで外れたら、ジョブは落ち、題が `ISSUE_TITLE` の開いている Issue がなければ開き、あればコメントで足す。記録の最初の行は外れ方ごとの印（`<!-- pr-body-protection: <外れの行の sha256 の先頭 16 桁> -->`）で、同じ印が Issue の本文かコメントにすでにあれば足さない。開いている Issue を探せなかったときは開く側に倒し、Issue を開けない・コメントできないときはその手順も落ちる
- 通ったら、開いている Issue に通ったことを書いて閉じる
- PR では走らせない（PR のコードをトークンと `issues: write` で動かさない）。建設者が `.github/workflows/pr-body-protection.yml` にそのまま写す。`ci.test.mjs` がこのひな形の形（push は `main` だけ・schedule・手動のほかで走らない・トークンは読む手順だけ・2 つの JSON を渡す・Issue の手順は `if: always()`・チェックの名前が `pr-body-engine-test` と重ならない）と、手順を偽の `gh` で動かした振る舞い（そろっていれば通る・rulesets だけでも通る・4 つの外れ方のどれでも Issue を開く・開いていればコメントで足し同じ外れ方は足さない・通ったら閉じる・読めなければ照らさない）、写しがあればひな形と同じかを見る
- 写しがひな形と同じかを見るだけでは、ひな形と写しを一緒に変えると、定期の照合そのものが黙って止まる（議決 `D-0068` の test の案、Issue #1180）。`ci.test.mjs` は、ひな形と、写しがあれば写しそのものについても、`on` の `schedule` に 5 項の `cron` があり `workflow_dispatch` があること、`continue-on-error` がなく、`protection` のジョブと `PR_BODY_PROTECTION_TOKEN` を読む手順に `if:` がないこと、その手順を偽の `gh` で動かし、トークンがない（`gh` が認証の誤りで 4 を返す）・権限がない（403）・rulesets を読めないのどれでも `::error title=pr-body-protection protection_unreadable::…` を出して 0 でない終了コードで落ちることを見る。トークンは読む手順の `env` に `${{ secrets.PR_BODY_PROTECTION_TOKEN }}` のまま渡す形にかぎる。schedule・cron・`workflow_dispatch` を外す直し、ジョブや読む手順の `if:`・`continue-on-error`、読めないときに 0 で抜ける直しが外れになることも確かめる
- 写してシークレットを置き、手動で 1 回走らせて通ったら、`K-0001` の Issue #1106 の解決策の `wired` を `true`（`wired_by` に `.github/workflows/pr-body-protection.yml` の `protection` と `tools/pr-body-check/protection.mjs`、`.github/workflows/pr-body-engine.yml` の `engine-test` と `tools/pr-body-check/engine.test.mjs`）にする。`tools/knowledge-enforcement-check/check.py` の `REQUIRED_SCHEDULE` に入れたので、schedule のジョブから `protection.mjs` を呼ぶ `wired_by` がないと `wired: true` にできない。上の「順序」の 1 と 4（今の `ENGINE_REF` で (3) が落ちること、本物の続きのジョブで外れた本文が書かれず body_fix が積まれたこと）を Issue #1106 に記録してから `verified` を `true` にする

### 差分の当たりの照合（`patch.mjs`・`ci/pr-body-patch.yml`）

`engine/pr-body-gate.patch` は建設者が Engine に入れるまで rally に置いたままなので、Engine の先頭が進むと黙って当たらなくなる（Engine `476573e` で `FACTORY_RESULT` の行が変わり、`engine.test.mjs` の (4)〜(10) が「差分が当たらない」で落ちていた。Issue #1107）。`engine.test.mjs` は PR ごとに `ENGINE_REF` の版でしか走らず、当たらないのか、もう入っているのか、一部だけ入ったのかも分けない。議決 `D-0068`（Issue #1179 の起票時の番号は `D-0066`。PR #1176）の ci_check の案で、`tools/knowledge-enforcement-check/test_engine_patch.py`（知見 `K-0004`。議決 `D-0024`・`D-0036`）と同じ形の照合を足した（Issue #1179）。

```sh
node patch.mjs --engine <Engine のチェックアウト（全部の履歴）> [--workflow <ENGINE_REF を読むワークフロー>] [--json <出力先>] [--summary <GITHUB_STEP_SUMMARY>]
```

- 照らす版は、`ENGINE_REF`（写し `.github/workflows/pr-body-engine.yml` があればそれ、なければひな形 `ci/pr-body-engine.yml` の `env`）と、`--engine` のチェックアウトの `HEAD`（既定のブランチの先頭）の 2 つ。版ごとに差分が触るファイルを git から取り出し（チェックアウトの今の中身は使わない）、`git apply --check` と `git apply --reverse --check` で照らす
- 版ごとの状態は 4 つ。`applies`（当たる）・`merged`（もう入っている。`--reverse --check` が通る）・`patch_partially_applied`（差分で足したもの（`entrypoint.sh` の `tools/pr-body-check/gate.mjs`・`dispatcher.py` の `act_on_body_gate`・`events.yaml` の `pr.body_gate_blocked`）はあるのに `--reverse --check` が通らない。一部だけ入ったか、入れたあと変わった）・`patch_stale`（当たらず、足したものもない）
- 終了コードは 0 = どちらの版でも `applies` か `merged`、1 = どちらかが `patch_stale` か `patch_partially_applied`、2 = 照らせなかった（`ENGINE_REF` を読めない・その版がチェックアウトにない。`patch_check_failed`）。GitHub Actions では外れを `::error title=pr-body-patch <rule>::…` で出す

`ci/pr-body-patch.yml` の `patch` のジョブ（チェックの名前 `pr-body-patch`）が、Engine を ref なし・全部の履歴（`fetch-depth: 0`）で取り、`patch.mjs` を既定のブランチ（`main`）への push・1 日 1 回（UTC 19:50、JST 4:50）・手動（`workflow_dispatch`）で走らせる。

- `patch_stale` か `patch_partially_applied` なら、ジョブは落ち、題が `ISSUE_TITLE` の開いている Issue がなければ開き、あればコメントで足す。記録には版ごとの状態と `git apply` の出力を書き、最初の行は外れ方と版ごとの印（`<!-- pr-body-patch: <外れの行（rule・どちらの版か・SHA）の sha256 の先頭 16 桁> -->`）で、同じ印が Issue の本文かコメントにすでにあれば足さない。Issue を見たら、建設者が Engine に入れる前に、差分を今の Engine に合わせて作り直す変更を起こす（Issue #1107 と同じ直し方。全部入れて `ENGINE_REF` を上げたなら差分を消す）
- どちらの版でも当たるかもう入っていれば、開いている Issue に通ったことを書いて閉じる
- 照らせなかったとき（Engine を取れなかった・`ENGINE_REF` の版がない）は `patch_check_failed` で落ちるが、差分の古さではないので Issue は開かない
- PR では走らせない（先頭の Engine の変化で PR を止めない。PR のコードを `issues: write` で動かさない）。シークレットは使わない。建設者が `.github/workflows/pr-body-patch.yml` にそのまま写す。`ci.test.mjs` がこのひな形の形（push は `main` だけ・schedule・手動のほかで走らない・Engine は ref なしで全部の履歴・照らす手順の外れを握りつぶさない・Issue の手順は `if: always()`・題が `pr-body-protection` と重ならない）と、偽の Engine と偽の `gh` で手順を動かした振る舞い（どちらの版でも当たるかもう入っていれば通る・`ENGINE_REF` の版でも先頭でも `patch_stale` と `patch_partially_applied` で Issue を開く・開いていればコメントで足し同じ記録は足さない・通ったら閉じる・照らせなければ Issue を開かない・Issue を開けなければ落ちる）、写しがあればひな形と同じかを見る
- 写して手動で 1 回走らせて通ったら、`K-0001` の Issue #1179 の解決策の `wired` を `true`（`wired_by` に `.github/workflows/pr-body-patch.yml` の `patch` と `tools/pr-body-check/patch.mjs`）にする。Engine の先頭が進んで差分が当たらなくなったときに Issue が開かれ、それを受けた直しがマージされたことを記録してから `verified` を `true` にする
