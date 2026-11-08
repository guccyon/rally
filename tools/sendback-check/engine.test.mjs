// Engine の本物の判定（judgment.record_judged → dispatcher.act_on_judgment、supervisor.inspector_approved）を
// check.test.mjs と同じ 3 つの場合で確かめる（知見 K-0004 の担保。議決 D-0009・D-0010・D-0011・D-0014・D-0028・D-0041、Issue #935・#944・#945・#949・#961・#1025・#1093）。
// 参照の実装（check.mjs）ではなく本物の判定の担保で、check.mjs の ci_check の配線より先に入れる（D-0010）。
// Engine のチェックアウトは環境変数 ENGINE_DIR で渡す（なければリポジトリの隣の ../engine）。どちらもなければ飛ばす。
// ただし ENGINE_DIR を渡したときと CI（CI=true）では飛ばさずに落とす（議決 D-0011、Issue #948）。
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { approvalBasis, checkRecord } from "./check.mjs";
import { BASIS_TESTS, LEGACY_BASIS_NOT_FOUND, NO_ENGINE_CHECKOUT, skipLines, todoLines } from "./order.mjs";

const harness = fileURLToPath(new URL("./engine_harness.py", import.meta.url));
const script = fileURLToPath(new URL("./check.mjs", import.meta.url));
const engineDir = process.env.ENGINE_DIR || fileURLToPath(new URL("../../../engine", import.meta.url));
const required = Boolean(process.env.ENGINE_DIR) || process.env.CI === "true";
const skip = required || existsSync(`${engineDir}/orchestrator/dispatcher/supervisor.py`)
  ? false : `${NO_ENGINE_CHECKOUT}（ENGINE_DIR=${engineDir}）`;

/** Engine の判定を 1 回動かし、起きた後続と basis を返す（dir を渡せばその Engine の写しで動かす） */
const engine = (record, dir = engineDir) =>
  JSON.parse(execFileSync("python3", ["-I", harness, dir], { input: JSON.stringify(record), encoding: "utf8" }));

const provisional = (verdict) => ({ subject: "provisional", verdict, defect: null });
const defect = (verdict, rubric_id, extra = {}) => ({ subject: "defect", verdict, defect: { rubric_id, location: "PR 本文", ...extra } });

/** PR #395 の裁き（C-0002）: 仮決め 3 件を adopt、欠陥 2 件（OV-001・PR-002）を reject。マージ済み。 */
const PR_395 = {
  pr_url: "https://github.com/guccyon/rally/pull/395",
  merged: true,
  judgments: [provisional("adopt"), provisional("adopt"), provisional("adopt"),
    defect("reject", "OV-001", { shadow: true }), defect("reject", "PR-002")],
};

// 取り出す関数（engine_harness.py の JUDGMENT_FUNCTIONS・ENGINE_FUNCTIONS）。Engine で名前や置き場が変わったら落ちる
const ENGINE_FUNCTIONS = ["record_judged", "act_on_judgment", "file_sendback", "enqueue_body_fix", "is_body_only", "inspector_approved"];

test("(0) Engine のチェックアウトがあり、判定の関数をすべて取り出せる", { skip }, () => {
  let out;
  try {
    out = execFileSync("python3", ["-I", harness, "--check", engineDir], { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
  } catch (e) {
    assert.fail(`engine_harness.py が Engine から関数を取り出せない（ENGINE_DIR=${engineDir}）: ${e.stderr || e.message}`);
  }
  const { functions } = JSON.parse(out);
  for (const name of ENGINE_FUNCTIONS) assert.ok(functions.includes(name), `${name} を取り出せない`);
});

/** 後続の判定の線だけを見る（basis は (3) で見る） */
const followupRules = (record) => checkRecord(record).map((e) => e.rule).filter((r) => !r.startsWith("basis_"));

test("(1) Engine は PR #395 と同じ入力では差し戻しも続きのジョブも起こさない", { skip }, () => {
  const out = engine(PR_395);
  assert.deepEqual(out.followups, []);
  assert.equal(out.approved, true);
  assert.deepEqual(followupRules({ ...PR_395, followups: out.followups }), []);
});

test("(2) Engine は直っていない採用欠陥があれば、マージの前後に関わらず起こす", { skip }, () => {
  const judgments = [...PR_395.judgments, defect("adopt", "BE-003", { location: "apps/backend/src/x.rs" })];
  for (const merged of [false, true]) {
    const out = engine({ ...PR_395, merged, judgments });
    // マージ前は同じ担当の続きのジョブ（ADR 0017）、マージ後は差し戻しの Issue で直す
    assert.deepEqual(out.followups, [{ kind: merged ? "sendback" : "review_fix" }], `merged=${merged}`);
    assert.deepEqual(followupRules({ merged, judgments, followups: out.followups }), [], `merged=${merged}`);
  }
});

test("(2) Engine は overlay の shadow のルールの指摘は adopt でも起こさず、本文だけの欠陥は本文の直しで直す", { skip }, () => {
  assert.deepEqual(engine({ ...PR_395, merged: false, judgments: [defect("adopt", "OV-001", { shadow: true })] }).followups, []);
  assert.deepEqual(engine({ ...PR_395, merged: false, judgments: [defect("adopt", "PR-002")] }).followups, [{ kind: "body_fix" }]);
});

// 議決 D-0011 は、Engine の basis を直す PR で todo を外すとした（Issue #949）。Engine は別のリポジトリで、basis の文を approvalBasis() の形に
// 合わせるのは建設者なので、ここでは todo を「Engine が今の形（adopt の合計だけ。LEGACY_BASIS）を書くときだけ」に狭めて必須のテストにした。
// Engine の basis が今の形から変わったら、その版からは approvalBasis() と合わないと落ちる（分けて書いても数や文がずれれば落ちる）。
// 今の形のうちは外れても落ちないが結果に出る。todo のうちに check.mjs の ci_check を配線すると、すべての承認が basis_not_split で落ちる（D-0010 の順序）
const LEGACY_BASIS = /^judge が \d+ 件を裁いた（採用 \d+。採用した指摘は続きのジョブか差し戻しで直す）$/;
const LEGACY_TODO = "Engine の basis が adopt の合計だけ（今の形。建設者が Engine を直したら必須になる）";

/** Engine の basis が今の形（adopt の合計だけ）なら、そのテストを todo にする。ほかの形なら必須のまま */
const todoWhileLegacy = (t, basis) => { if (LEGACY_BASIS.test(basis ?? "")) t.todo(LEGACY_TODO); };
// (3) と (c) の名前は order.mjs の BASIS_TESTS に置く。CI はこのテストの TAP を order.mjs --engine-report に渡し、
// check.mjs --events を配線したのに (3) か (c) が通っていなければ落とす（議決 D-0016、Issue #964）

test("(3) の todo は Engine の今の basis の形（adopt の合計だけ）にだけ効き、ほかの形は必須にする", () => {
  assert.ok(LEGACY_BASIS.test("judge が 6 件を裁いた（採用 4。採用した指摘は続きのジョブか差し戻しで直す）"));
  const judgments = [...PR_395.judgments, defect("adopt", "BE-003", { location: "apps/backend/src/x.rs" })];
  assert.ok(!LEGACY_BASIS.test(approvalBasis(judgments)));
  assert.ok(!LEGACY_BASIS.test("judge が 6 件を裁いた（採用 4）"));
  assert.ok(!LEGACY_BASIS.test(""));
});

/** (3) の入力（PR #395 の裁きに、直っていない採用欠陥を 1 件足す） */
const BASIS_JUDGMENTS = [...PR_395.judgments, defect("adopt", "BE-003", { location: "apps/backend/src/x.rs" })];

/** (3) の中身。t は todo を付けられる相手（node:test の t か、下の写しのテストの記録係）、dir は Engine のチェックアウトか写し */
function basisCase(t, dir = engineDir) {
  const out = engine({ ...PR_395, merged: false, judgments: BASIS_JUDGMENTS }, dir);
  todoWhileLegacy(t, out.basis);
  assert.equal(out.basis, approvalBasis(BASIS_JUDGMENTS));
  assert.deepEqual(checkRecord({ merged: false, judgments: BASIS_JUDGMENTS, followups: out.followups, basis: out.basis }), []);
}

test(BASIS_TESTS["(3)"], { skip }, (t) => basisCase(t));

// 議決 D-0014 は、Engine の basis を直す PR で todo を外して必須にするとした（Issue #961）。Engine は別のリポジトリなので、このリポジトリで
// その PR に当たるのは ENGINE_REF を直した版に上げる PR。Engine が今の形の basis を書かなくなったのに todo（todoWhileLegacy・t.todo(…) など。
// order.mjs の todoLines と同じ数え方）が残っていたら、その PR で落とす。todo を消し忘れると order.mjs の engine_test_todo で ci_check を配線できない。
// 写しを作れないときの legacy_basis_not_found の skip（下の写しのテスト）も、order.mjs の skipLines（todoLines と同じ数え方）で数えて、残っていれば同じく落とす
// （議決 D-0041、Issue #1093）。字面で数え漏らす別名や動的な skip は、order.test.mjs がこのファイルを走らせた TAP の SKIP の理由と件数で照らす
// （order.mjs の skipReportProblems。議決 D-0061、Issue #1168）
const OWN_SOURCE = readFileSync(fileURLToPath(import.meta.url), "utf8");

/** ENGINE_REF を上げる PR で消すもの（todo と skip）の行番号 */
const legacyLeftovers = (source) => [...new Set([...todoLines(source), ...skipLines(source)])].sort((a, b) => a - b);

/** todo を外すことを求める中身。dir は Engine のチェックアウトか写し、source は engine.test.mjs の中身 */
function todoRemovalCase(dir = engineDir, source = OWN_SOURCE) {
  const { basis } = engine({ ...PR_395, merged: false, judgments: BASIS_JUDGMENTS }, dir);
  if (LEGACY_BASIS.test(basis ?? "")) return;   // 今の形のうちは todo を残してよい
  const left = legacyLeftovers(source);
  assert.deepEqual(left, [], `Engine の basis が今の形でない（${basis}）のに engine.test.mjs に todo か skip が残っている（${left.join("・")} 行目）。`
    + "LEGACY_BASIS・todoWhileLegacy と todo、写しのテスト（legacy_basis_not_found の skip）を消して必須のテストにする");
}

test("(3) Engine の basis が今の形（adopt の合計だけ）でなくなったら、同じ PR で engine.test.mjs の todo を外す", { skip }, () => todoRemovalCase());

// 議決 D-0028 は、今の形かどうかを LEGACY_BASIS の文と合うかで見ているので、Engine が別の言い回しで adopt の合計だけを書くように変わったときに
// todo を外すことを求め、そのあと (3) が必須で落ちる流れを写しで確かめるとした（Issue #1025）。Engine の inspector_approved の basis の文を、
// LEGACY_BASIS と文が違っても欠陥の採用数と仮決めの承認数を分けて書いていない言い回しに書き換えた写しを作り、上の 2 つの中身をその写しで動かす。
// 写しの元にする Engine が今の形を書かなくなったら（建設者が basis を直したら）写しは作れないので、LEGACY_BASIS・todoWhileLegacy と一緒にこのテストも消す。
// それまでに写しを作れなくなったら、黙って通さず t.skip に legacy_basis_not_found を付けて飛ばし（結果に SKIP で出る）、その skip の行が残っていれば
// 上の「todo を外す」のテストで落とす（議決 D-0041、Issue #1093）。Engine がまだ今の形の basis を書くのに行だけが見つからないときは飛ばさずに落とす
const SUPERVISOR = "orchestrator/dispatcher/supervisor.py";
const LEGACY_BASIS_LINE = /return True, f"judge が \{len\(verdicts\)\} 件を裁いた（採用 \{n_adopt\}。採用した指摘は続きのジョブか差し戻しで直す）"/;
const LEGACY_BASIS_SOURCE = 'f"judge が {len(verdicts)} 件を裁いた（採用 {n_adopt}。採用した指摘は続きのジョブか差し戻しで直す）"';
/** adopt の合計だけを書く別の言い回し（Python の f 文字列。len(verdicts) と n_adopt は inspector_approved の変数） */
const ADOPT_TOTAL_VARIANTS = [
  'f"judge が {len(verdicts)} 件を裁き、採用は {n_adopt} 件（採用した指摘は続きのジョブか差し戻しで直す）"',
  'f"judge が {len(verdicts)} 件を裁いた（採用 {n_adopt}。採用した欠陥は続きのジョブか差し戻しで直す）"',
  'f"judge の裁き {len(verdicts)} 件・採用 {n_adopt} 件"',
  'f"採用 {n_adopt}/{len(verdicts)}（judge が裁いた。採用した指摘は続きのジョブか差し戻しで直す）"',
];

/** Engine の判定のファイルを一時ディレクトリに写し、inspector_approved の basis の文を basis に書き換えた写しの道を返す */
function engineWithBasis(basis) {
  const dir = mkdtempSync(join(tmpdir(), "sendback-engine-variant-"));
  cpSync(join(engineDir, "orchestrator/dispatcher"), join(dir, "orchestrator/dispatcher"), { recursive: true });
  const supervisor = join(dir, SUPERVISOR);
  const source = readFileSync(supervisor, "utf8");
  assert.match(source, LEGACY_BASIS_LINE, "Engine の inspector_approved に今の形の basis の行がない（写しを作れない）");
  writeFileSync(supervisor, source.replace(LEGACY_BASIS_LINE, `return True, ${basis}`));
  return dir;
}

/** todo を付けたかを覚えるだけの t の代わり（node:test の t.todo と同じ名前の関数を持つ） */
const todoRecorder = () => { const marks = []; return { marks, todo(reason) { marks.push(reason); } }; };

/** source から ENGINE_REF を上げる PR で消すもの（todo と skip）の行を除いた中身 */
const withoutLeftovers = (source) => source.split(/\r?\n/).filter((_, i) => !legacyLeftovers(source).includes(i + 1)).join("\n");

test("(3) ENGINE_REF を上げる PR で消すものには、写しを作れないときの legacy_basis_not_found の skip も数える", () => {
  const skipAt = OWN_SOURCE.split(/\r?\n/).findIndex((l) => /\bt\.skip\(/.test(l) && l.includes("LEGACY_BASIS_NOT_FOUND")) + 1;
  assert.ok(skipAt > 0, "写しのテストに legacy_basis_not_found の t.skip がない（写しのテストごと消したなら、このテストも消す）");
  assert.ok(skipLines(OWN_SOURCE).includes(skipAt));
  assert.ok(legacyLeftovers(OWN_SOURCE).includes(skipAt));
  // Engine のチェックアウトがないときの { skip } は数えない
  assert.ok(!skipLines(OWN_SOURCE).some((n) => /\{ skip \}/.test(OWN_SOURCE.split(/\r?\n/)[n - 1])));
  assert.deepEqual(legacyLeftovers(withoutLeftovers(OWN_SOURCE)), []);
});

test("(3) Engine が LEGACY_BASIS と違う言い回しで adopt の合計だけを書くと、todo を外すことを求められ、外すと (3) が必須で落ちる", { skip }, (t) => {
  if (!LEGACY_BASIS_LINE.test(readFileSync(join(engineDir, SUPERVISOR), "utf8"))) {
    const { basis } = engine({ ...PR_395, merged: false, judgments: BASIS_JUDGMENTS });
    assert.ok(!LEGACY_BASIS.test(basis ?? ""), `Engine は今の形の basis（${basis}）を書くのに、inspector_approved に今の形の basis の行がない`
      + "（写しを作れない。LEGACY_BASIS_LINE と LEGACY_BASIS_SOURCE を Engine の行に合わせる）");
    t.skip(`${LEGACY_BASIS_NOT_FOUND}: Engine の inspector_approved に今の形の basis の文がなく写しを作れない（basis: ${basis}）。`
      + "ENGINE_REF を上げる PR で LEGACY_BASIS・todoWhileLegacy と一緒にこのテストを消す");
    return;
  }
  const withoutTodos = withoutLeftovers(OWN_SOURCE);
  assert.notDeepEqual(todoLines(OWN_SOURCE), [], "engine.test.mjs に todo がない（LEGACY_BASIS ごと消したなら、このテストも消す）");
  assert.deepEqual(legacyLeftovers(withoutTodos), []);
  for (const variant of ADOPT_TOTAL_VARIANTS) {
    const dir = engineWithBasis(variant);
    const out = engine({ ...PR_395, merged: false, judgments: BASIS_JUDGMENTS }, dir);
    assert.equal(out.approved, true, variant);
    // 写しの basis は今の形（LEGACY_BASIS）と見なされず、欠陥の採用数と仮決めの承認数も分けて書いていない
    assert.ok(!LEGACY_BASIS.test(out.basis), `${variant}: ${out.basis}`);
    assert.deepEqual(checkRecord({ merged: false, judgments: BASIS_JUDGMENTS, followups: out.followups, basis: out.basis }).map((e) => e.rule),
      ["basis_not_split"], `${variant}: ${out.basis}`);
    // todo が残っていれば、todo を外すことを求めて落ちる。todo と skip を外せば通る
    assert.throws(() => todoRemovalCase(dir), /todo か skip が残っている/, variant);
    todoRemovalCase(dir, withoutTodos);
    // (3) は todo にならずに（必須のまま）落ちる
    const t = todoRecorder();
    assert.throws(() => basisCase(t, dir), assert.AssertionError, variant);
    assert.deepEqual(t.marks, [], `${variant}: (3) が todo になった`);
  }
  // 比べのために、今の形の文のままの写しでは (3) が todo になる（写しの作り方で外れていないこと）
  const legacy = todoRecorder();
  assert.throws(() => basisCase(legacy, engineWithBasis(LEGACY_BASIS_SOURCE)));
  assert.deepEqual(legacy.marks, [LEGACY_TODO]);
});

// イベントの経路（Issue #945）: Engine が記録したイベント列を、README の SQL と同じ形で check.mjs に渡す。
// ci_check がワークフローで走らせるのと同じ呼び方（--events --pr --summary、GITHUB_ACTIONS=true）で CLI を動かす

/**
 * 配線の順序を守ったリポジトリの写し（ci/sendback-engine.yml を写し、todo のない engine.test.mjs を置く）。check.mjs --events は GitHub Actions では
 * 動いていること自体を配線と数えて配線の順序も見る（Issue #1084）ので、イベントの経路だけを見るためにこの写しを GITHUB_WORKSPACE に渡す
 */
function wiredWorkspace(dir) {
  mkdirSync(join(dir, ".github/workflows"), { recursive: true });
  mkdirSync(join(dir, "tools/sendback-check"), { recursive: true });
  cpSync(fileURLToPath(new URL("./ci/sendback-engine.yml", import.meta.url)), join(dir, ".github/workflows/sendback-engine.yml"));
  writeFileSync(join(dir, "tools/sendback-check/engine.test.mjs"), "// 配線の順序を守った写し（todo なし）\n");
  return dir;
}

/** イベント列を ci_check と同じ呼び方で check.mjs に渡し、終了コード・標準出力・JSON・要約を返す */
function ciCheck(events, prUrl = PR_395.pr_url) {
  const dir = mkdtempSync(join(tmpdir(), "sendback-engine-events-"));
  const workspace = wiredWorkspace(join(dir, "repo"));
  writeFileSync(join(dir, "events.json"), JSON.stringify(events, null, 2));
  writeFileSync(join(dir, "summary.md"), "");
  const r = spawnSync("node", [script, "--events", join(dir, "events.json"), "--pr", prUrl, "--json", join(dir, "out.json"),
    "--summary", join(dir, "summary.md")], { env: { ...process.env, GITHUB_ACTIONS: "true", GITHUB_WORKSPACE: workspace }, encoding: "utf8" });
  return { status: r.status, stdout: r.stdout, result: JSON.parse(readFileSync(join(dir, "out.json"), "utf8")),
    summary: readFileSync(join(dir, "summary.md"), "utf8") };
}

const adoptedDefect = defect("adopt", "BE-003", { location: "apps/backend/src/x.rs" });
/** Engine の今の判定で後続が線に合う場合（PR #395 と同じ入力、直っていない採用欠陥のマージ前・後、本文だけの欠陥） */
const ENGINE_CASES = [
  ["PR #395 と同じ入力", PR_395],
  ["採用欠陥・マージ前", { ...PR_395, merged: false, judgments: [...PR_395.judgments, adoptedDefect] }],
  ["採用欠陥・マージ後", { ...PR_395, merged: true, judgments: [...PR_395.judgments, adoptedDefect] }],
  ["本文だけの欠陥", { ...PR_395, merged: false, judgments: [defect("adopt", "PR-002")] }],
];

test("イベントの経路: Engine が記録したイベント列を check.mjs --events にそのまま渡すと、1 つの head に組めて後続の判定は線に合う", { skip }, () => {
  for (const [name, record] of ENGINE_CASES) {
    const out = engine(record);
    const { result, stdout } = ciCheck(out.events);
    assert.deepEqual(result.errors.map((e) => e.rule).filter((r) => !r.startsWith("basis_")), [], name);
    assert.match(stdout, new RegExp(`head 1 個・裁き ${record.judgments.length} 件`), name);
  }
});

test("イベントの経路: PR #395 の流れ（直すもののない差し戻し）を Engine の形のイベント列で落とす", { skip }, () => {
  // PR #395 と同じ入力の Engine のイベント列に、Engine が差し戻しで記録する sendback.filed（採用欠陥のマージ後の場合から取る）を足す
  const sendback = engine(ENGINE_CASES[2][1]).events.find((e) => e.name === "sendback.filed");
  assert.ok(sendback, "Engine が採用欠陥のマージ後に sendback.filed を記録していない");
  const { status, stdout, result, summary } = ciCheck([...engine(PR_395).events, sendback]);
  assert.equal(status, 1);
  assert.ok(result.errors.some((e) => e.rule === "followup_without_defect" && e.pr_url === PR_395.pr_url && e.head_sha === "head1"));
  assert.match(stdout, /^::error title=sendback-check followup_without_defect::https:\/\/github\.com\/guccyon\/rally\/pull\/395@head1 /m);
  assert.match(summary, /外れた: https:\/\/github\.com\/guccyon\/rally\/pull\/395@head1（裁き 5 件。欠陥の採用 0・仮決めの承認 3）/);
});

// 今の Engine の basis は adopt の合計だけなので、承認のある head はすべて basis_not_split で落ちる（ci_check を配線してはいけない状態）。
// (3) と同じく、todo は Engine が今の形の basis を書くときだけ。Engine の basis が直ってこれが通ってから ci_check を配線する（D-0010 の順序）
test(BASIS_TESTS["(c)"], { skip }, (t) => {
  for (const [name, record] of ENGINE_CASES) {
    const out = engine(record);
    todoWhileLegacy(t, out.events.find((e) => e.name === "supervisor.inspector_approved")?.payload.basis);
    const { status, result } = ciCheck(out.events);
    assert.deepEqual(result.errors, [], name);
    assert.equal(status, 0, name);
  }
});
