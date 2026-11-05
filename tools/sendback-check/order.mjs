// check.mjs の ci_check を配線する順序を決定論で見張る（知見 K-0004 の担保。議決 D-0012、Issue #953）。LLM を使わない。
//
//   node order.mjs [--repo <リポジトリの根>] [--engine-report <engine.test.mjs の TAP>] [--json <出力先>] [--summary <GITHUB_STEP_SUMMARY>]   # --repo を省くとこのファイルから 2 つ上
//   終了コード: 0 = 通る（まだ配線していないか、順序を守って配線した）、1 = 外れた、2 = 読めなかった
//
// 順序（議決 D-0010、README の「配線の順序」）: Engine の basis を直して engine.test.mjs の todo を外し、engine.test.mjs を CI に配線してから、
// check.mjs --events の ci_check を配線する。todo のうちに配線すると、すべての承認が basis_not_split で落ちる。
//
// 見ること（.github/workflows/ に check.mjs --events を呼ぶ手順があるときだけ）:
//   - engine.test.mjs に todo が残っている（engine_test_todo）
//   - .github/workflows/ に engine.test.mjs を走らせる手順がない（engine_test_not_wired）
//   - --engine-report を渡したとき、ENGINE_REF の Engine で走らせた engine.test.mjs の結果で (3) か (c) が通っていない
//     （LEGACY_BASIS の todo・飛ばした・落ちた・結果にない。engine_basis_not_passed。議決 D-0016、Issue #964）
// check.mjs --events を GitHub Actions（GITHUB_ACTIONS=true）で動かしたときも、同じ見張りを先に通す（check.mjs の main）。

import { appendFileSync, existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { parseArgs } from "node:util";
import { fileURLToPath } from "node:url";

/** このファイルのあるリポジトリの根（tools/sendback-check から 2 つ上）。 */
export const REPO_ROOT = fileURLToPath(new URL("../../", import.meta.url));
/** Engine の本物の判定のテスト（リポジトリの根からの道）。 */
export const ENGINE_TEST = "tools/sendback-check/engine.test.mjs";

const TOOL_DIR = "tools/sendback-check";

/** engine.test.mjs の basis の場合のテストの名前。engine.test.mjs もこれを使うので、名前を変えても結果から引ける。 */
export const BASIS_TESTS = {
  "(3)": "(3) Engine の inspector_approved の basis に欠陥の採用数と仮決めの承認数を分けて書く",
  "(c)": "イベントの経路: Engine の正しい判定のイベント列は check.mjs --events で通る（ci_check を配線してよい合図）",
};

/** YAML と shell の注釈（行頭か空白の後ろの #）を落とし、行末の \ でつないだ行を 1 行にする。 */
function logicalLines(text) {
  const lines = text.split(/\r?\n/).map((l) => l.replace(/(^|\s)#.*$/, "").trimEnd());
  const out = [];
  let cur = null;
  for (const l of lines) {
    cur = cur === null ? l : `${cur} ${l.trim()}`;
    if (cur.endsWith("\\")) cur = cur.slice(0, -1);
    else { out.push(cur); cur = null; }
  }
  if (cur !== null) out.push(cur);
  return out;
}

/** ワークフローを手順（`- ` で始まる列の要素）ごとに分け、手順の working-directory と行を返す。 */
function steps(text) {
  const out = [];
  let cur = { wd: "", lines: [] };
  for (const line of logicalLines(text)) {
    if (/^\s*-\s+[\w-]+:/.test(line)) { out.push(cur); cur = { wd: "", lines: [] }; }
    const wd = line.match(/^\s*-?\s*working-directory:\s*["']?([^"'\s]+)/);
    if (wd) cur.wd = wd[1];
    cur.lines.push(line);
  }
  out.push(cur);
  return out;
}

/** その手順の行が tools/sendback-check の中で動くか（working-directory か、行の中の道で）。 */
const inToolDir = (step, line) => step.wd.replace(/\/$/, "").endsWith(TOOL_DIR) || line.includes(TOOL_DIR);

/** ワークフローに check.mjs --events を呼ぶ手順があるか。取りこぼすと見張れないので、迷う形は呼ぶ側に数える。 */
export function callsEventsCheck(text) {
  const mentionsTool = text.includes("sendback-check");
  return steps(text).some((s) => s.lines.some((l) =>
    /--events\b/.test(l) && (/sendback-check\/check\.mjs/.test(l) || (mentionsTool && /\bcheck\.mjs\b|\bnpm\s+run\s+check\b/.test(l)))));
}

/** ワークフローに engine.test.mjs を走らせる手順があるか（engine.test.mjs を名指しするか、tools/sendback-check で npm test・node --test を全部走らせる）。 */
export function runsEngineTest(text) {
  return steps(text).some((s) => s.lines.some((l) => {
    if (!inToolDir(s, l)) return false;
    if (/\bnode\b.*--test\b.*\bengine\.test\.mjs\b/.test(l)) return true;
    if (/\bnpm\b.*\btest\b/.test(l)) return true;
    const nodeTest = l.match(/\bnode\b.*--test\b(.*)$/);
    return Boolean(nodeTest) && !/\.mjs\b/.test(nodeTest[1]);   // ファイルを名指しすると engine.test.mjs が入らないことがある
  }));
}

/** engine.test.mjs に残っている todo（t.todo(…)・test.todo(…)・{ todo: … }）の行番号（1 始まり）。注釈の中は数えない。 */
export function todoLines(source) {
  const out = [];
  source.split(/\r?\n/).forEach((line, i) => {
    const code = line.replace(/\/\/.*$/, "");
    if (/\.todo\s*\(|\btodo\s*:/.test(code)) out.push(i + 1);
  });
  return out;
}

/** node --test の TAP の出力から、上の段のテストの結果を { name: "pass" | "fail" | "todo" | "skip" } で返す。 */
export function parseTapReport(text) {
  const out = {};
  for (const line of text.split(/\r?\n/)) {
    const m = line.match(/^(not )?ok \d+ - (.*)$/);
    if (!m) continue;
    // 名前の中の # は \# に逃がしてあるので、逃がしていない「 # 」から後ろが指示（TODO・SKIP）
    const d = m[2].match(/^((?:[^#\\]|\\.)*?)(?:\s+#\s*(TODO|SKIP)\b.*)?$/i);
    const name = (d ? d[1] : m[2]).replace(/\\(.)/g, "$1").trim();
    const directive = d?.[2]?.toUpperCase();
    out[name] = directive === "TODO" ? "todo" : directive === "SKIP" ? "skip" : m[1] ? "fail" : "pass";
  }
  return out;
}

const STATUS_TEXT = { todo: " todo になった（Engine の basis がまだ LEGACY_BASIS の形）", skip: "飛ばされた", fail: "落ちた" };

/** 配線の順序を見る。workflows は [{ path, text }]、engineTest は engine.test.mjs の中身（なければ null）、
 * engineReport は ENGINE_REF の Engine で走らせた engine.test.mjs の TAP（渡さなければ結果は見ない）。 */
export function checkOrder({ workflows, engineTest, engineReport }) {
  const callers = workflows.filter((w) => callsEventsCheck(w.text)).map((w) => w.path);
  if (callers.length === 0) return { wired: [], errors: [] };
  const errors = [];
  const where = callers.join("・");
  if (engineTest == null) {
    errors.push({ rule: "engine_test_todo", message: `${where} が check.mjs --events を呼ぶのに、${ENGINE_TEST} がない` });
  } else {
    const todos = todoLines(engineTest);
    if (todos.length > 0) {
      errors.push({ rule: "engine_test_todo", message: `${where} が check.mjs --events を呼ぶのに、${ENGINE_TEST} に todo が残っている`
        + `（${todos.join("・")} 行目）。Engine の basis を直して todo を外してから配線する` });
    }
  }
  if (engineReport != null) {
    const results = parseTapReport(engineReport);
    for (const [label, name] of Object.entries(BASIS_TESTS)) {
      const status = results[name];
      if (status === "pass") continue;
      errors.push({ rule: "engine_basis_not_passed", message: `${where} が check.mjs --events を呼ぶのに、ENGINE_REF の Engine で ${ENGINE_TEST} の ${label}`
        + `（${name}）が${STATUS_TEXT[status] ?? "結果にない"}。Engine の basis を approvalBasis() の形に直して ENGINE_REF を上げてから配線する` });
    }
  }
  if (!workflows.some((w) => runsEngineTest(w.text))) {
    errors.push({ rule: "engine_test_not_wired", message: `${where} が check.mjs --events を呼ぶのに、.github/workflows/ に ${ENGINE_TEST} を走らせる手順がない`
      + "（ci/sendback-engine.yml を先に写す）" });
  }
  return { wired: callers, errors };
}

/** リポジトリの .github/workflows/ と engine.test.mjs を読む。 */
export function loadRepo(root) {
  const dir = join(root, ".github/workflows");
  const workflows = existsSync(dir)
    ? readdirSync(dir).filter((f) => /\.ya?ml$/.test(f)).sort()
      .map((f) => ({ path: `.github/workflows/${f}`, text: readFileSync(join(dir, f), "utf8") }))
    : [];
  const testPath = join(root, ENGINE_TEST);
  return { workflows, engineTest: existsSync(testPath) ? readFileSync(testPath, "utf8") : null };
}

/** 結果を GitHub Actions の ::error（annotate）か「rule: 理由」の行にする。 */
export function formatErrors(errors, annotate) {
  return errors.map((e) => annotate ? `::error title=sendback-check ${e.rule}::${e.message}` : `${e.rule}: ${e.message}`);
}

export function renderSummary({ wired, errors }) {
  const out = ["## sendback-check の配線の順序", ""];
  if (wired.length === 0) out.push("check.mjs --events はまだ .github/workflows/ に配線されていない（見張ることはない）");
  else if (errors.length === 0) out.push(`通る: ${wired.join("・")} が check.mjs --events を呼び、engine.test.mjs は todo なしで CI に配線されている`);
  for (const e of errors) out.push(`- 外れた \`${e.rule}\`: ${e.message}`);
  return out.join("\n") + "\n";
}

function main() {
  const { values } = parseArgs({ options: { repo: { type: "string" }, "engine-report": { type: "string" }, json: { type: "string" },
    summary: { type: "string" } } });
  let result;
  try {
    // 結果のファイルがない（Engine のテストまで進まなかった）ときは結果が空として見る。配線していれば (3) と (c) が「結果にない」で外れる
    const report = values["engine-report"];
    const engineReport = report == null ? undefined : existsSync(report) ? readFileSync(report, "utf8") : "";
    result = checkOrder({ ...loadRepo(values.repo ?? REPO_ROOT), engineReport });
  } catch (e) {
    console.error(`sendback-check order: 読めなかった: ${e.message}`);
    process.exit(2);
  }
  if (values.json) writeFileSync(values.json, JSON.stringify({ ok: result.errors.length === 0, ...result }, null, 2) + "\n");
  if (values.summary) appendFileSync(values.summary, renderSummary(result));
  for (const line of formatErrors(result.errors, process.env.GITHUB_ACTIONS === "true")) console.log(line);
  console.log(`sendback-check order: check.mjs --events を呼ぶワークフロー ${result.wired.length} 個、外れ ${result.errors.length} 件`);
  process.exit(result.errors.length === 0 ? 0 : 1);
}

if (import.meta.url === `file://${process.argv[1]}`) main();
