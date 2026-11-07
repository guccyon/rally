// check.mjs の ci_check を配線する順序を決定論で見張る（知見 K-0004 の担保。議決 D-0012、Issue #953）。LLM を使わない。
//
//   node order.mjs [--repo <リポジトリの根>] [--json <出力先>] [--summary <GITHUB_STEP_SUMMARY>]   # --repo を省くとこのファイルから 2 つ上
//   終了コード: 0 = 通る（まだ配線していないか、順序を守って配線した）、1 = 外れた、2 = 読めなかった
//
// 順序（議決 D-0010、README の「配線の順序」）: Engine の basis を直して engine.test.mjs の todo を外し、engine.test.mjs を CI に配線してから、
// check.mjs --events の ci_check を配線する。todo のうちに配線すると、すべての承認が basis_not_split で落ちる。
//
// 見ること（.github/workflows/ に check.mjs --events を呼ぶ手順があるときだけ）:
//   - engine.test.mjs に todo が残っている（engine_test_todo）
//   - .github/workflows/ に engine.test.mjs を走らせる手順がない（engine_test_not_wired）
// check.mjs --events を GitHub Actions（GITHUB_ACTIONS=true）で動かしたときも、同じ見張りを先に通す（check.mjs の main）。
// 呼ぶ手順は npm scripts・シェルのラッパー・引数の順序を変えた呼び方も辿って見つける（議決 D-0017、Issue #967）。

import { appendFileSync, existsSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { parseArgs } from "node:util";
import { fileURLToPath } from "node:url";

/** このファイルのあるリポジトリの根（tools/sendback-check から 2 つ上）。 */
export const REPO_ROOT = fileURLToPath(new URL("../../", import.meta.url));
/** Engine の本物の判定のテスト（リポジトリの根からの道）。 */
export const ENGINE_TEST = "tools/sendback-check/engine.test.mjs";

const TOOL_DIR = "tools/sendback-check";

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

/** scripts を渡さないときの既定（tools/sendback-check/package.json の check と同じ）。ファイルは読まない（読むのは loadRepo だけ）。 */
export const DEFAULT_SCRIPTS = Object.freeze({ check: "node check.mjs" });

/** tools/sendback-check/package.json の scripts（npm run の名前から中身を引く）。ファイルがなければ既定。loadRepo から呼ぶ。 */
function readToolScripts(root) {
  const path = join(root, TOOL_DIR, "package.json");
  if (!existsSync(path)) return DEFAULT_SCRIPTS;
  return JSON.parse(readFileSync(path, "utf8")).scripts ?? {};
}

/** 手順の行に出てくるスクリプトのファイルの道（シェルや node のラッパー）。check.mjs・order.mjs そのものとテスト（*.test.*）は除く。 */
const scriptPaths = (line) => [...line.matchAll(/(?:^|[\s"'=(])((?:\.{0,2}\/)?[\w.\/-]+\.(?:sh|bash|mjs|cjs|js))\b/g)]
  .map((m) => m[1]).filter((p) => !/(^|\/)(check|order)\.mjs$|\.test\.[cm]?js$/.test(p));

/** 道をリポジトリの根からの道にする（working-directory の下か、根から）。根の外に出る道は捨てる。 */
function resolvePaths(wd, path) {
  const out = [];
  for (const base of [wd, ""]) {
    const parts = [];
    let ok = true;
    for (const seg of `${base}/${path}`.split("/")) {
      if (seg === "" || seg === ".") continue;
      if (seg === "..") { if (parts.length === 0) { ok = false; break; } parts.pop(); } else parts.push(seg);
    }
    if (ok && !out.includes(parts.join("/"))) out.push(parts.join("/"));
  }
  return out;
}

/** 行の npm・pnpm の run（run-script）が呼ぶ scripts の名前（--prefix などの前置きは飛ばす）。 */
function npmRun(line) {
  const m = line.match(/\b(?:npm|pnpm)\b(?:\s+\S+)*?\s+(?:run|run-script)\s+([\w:.-]+)/);
  return m ? m[1] : null;
}

/**
 * 手順（行の並び）が check.mjs を呼ぶか（calls）と、呼び方に --events が入っているか（events）。
 * 直に呼ぶ（道・working-directory・cd・変数・シェルの関数で）ほか、tools/sendback-check の npm scripts と、
 * リポジトリの中のラッパーのスクリプト（readFile で読める）を中まで辿る。--events は手順のどこにあっても数える（引数を後から足す形のため）。
 */
function invocation(lines, { wd, mentionsTool, vars, scripts, readFile, depth }) {
  const text = lines.join("\n");
  let events = /--events\b/.test(text);
  let calls = false;
  const toolCtx = mentionsTool || wd.includes("sendback-check") || text.includes("sendback-check");   // cd してから呼ぶ形も
  for (const l of lines) {
    if (/sendback-check\/check\.mjs\b/.test(l) || (toolCtx && /\bcheck\.mjs\b/.test(l))) calls = true;
    if (vars.some((v) => new RegExp(`\\$\\{?${v}\\b|\\benv\\.${v}\\b`).test(l))) calls = true;
    const name = toolCtx ? npmRun(l) : null;
    if (name) {
      const body = scripts[name];
      // 名前が scripts にない（読めない）ときは、取りこぼさないよう check を名乗るものを呼ぶ側に数える
      if (body === undefined ? /^check\b/.test(name) : /\bcheck\.mjs\b/.test(body)) calls = true;
      if (body !== undefined && /--events\b/.test(body)) events = true;
    }
    if (depth < 3) {
      for (const p of scriptPaths(l)) {
        for (const rel of resolvePaths(wd, p)) {
          const inner = readFile(rel);
          if (inner == null) continue;
          const dir = rel.includes("/") ? rel.slice(0, rel.lastIndexOf("/")) : "";
          const r = invocation(logicalLines(inner), { wd: dir, mentionsTool: inner.includes("sendback-check") || dir.includes(TOOL_DIR),
            vars: [...vars, ...pathVars(inner)], scripts, readFile, depth: depth + 1 });
          if (r.calls) { calls = true; events ||= r.events; }
        }
      }
    }
  }
  return { calls, events };
}

/** check.mjs の道（tools/sendback-check/check.mjs）を入れた変数の名前（YAML の env と shell の代入）。 */
function pathVars(text) {
  return [...new Set(logicalLines(text).flatMap((l) =>
    [...l.matchAll(/\b([A-Za-z_]\w*)\s*[:=]\s*["']?[^"'\s]*sendback-check\/check\.mjs/g)].map((m) => m[1])))];
}

/**
 * ワークフローに check.mjs --events を呼ぶ手順があるか。取りこぼすと見張れないので、迷う形は呼ぶ側に数える。
 * 呼び方は直の node のほか、npm scripts（--prefix・run-script を含む）、シェルのラッパー、引数の順序を変えたもの、変数や関数を経たものも見る
 * （議決 D-0017、Issue #967）。scripts は tools/sendback-check/package.json の scripts、readFile はリポジトリの根からの道で中身（なければ null）を返す。
 */
export function callsEventsCheck(text, { scripts = DEFAULT_SCRIPTS, readFile = () => null } = {}) {
  const mentionsTool = logicalLines(text).some((l) => l.includes("sendback-check"));
  const vars = pathVars(text);
  return steps(text).some((s) => {
    const r = invocation(s.lines, { wd: s.wd, mentionsTool, vars, scripts, readFile, depth: 0 });
    return r.calls && r.events;
  });
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

/**
 * 配線の順序を見る。workflows は [{ path, text }]、engineTest は engine.test.mjs の中身（なければ null）。
 * scripts・readFile は callsEventsCheck と同じ（npm scripts とラッパーのスクリプトを辿るため）。
 */
export function checkOrder({ workflows, engineTest, scripts = DEFAULT_SCRIPTS, readFile = () => null }) {
  const callers = workflows.filter((w) => callsEventsCheck(w.text, { scripts, readFile })).map((w) => w.path);
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
  if (!workflows.some((w) => runsEngineTest(w.text))) {
    errors.push({ rule: "engine_test_not_wired", message: `${where} が check.mjs --events を呼ぶのに、.github/workflows/ に ${ENGINE_TEST} を走らせる手順がない`
      + "（ci/sendback-engine.yml を先に写す）" });
  }
  return { wired: callers, errors };
}

/** リポジトリの .github/workflows/ と engine.test.mjs、tools/sendback-check/package.json の scripts を読み、ラッパーのスクリプトを読む関数を添える。 */
export function loadRepo(root) {
  const dir = join(root, ".github/workflows");
  const workflows = existsSync(dir)
    ? readdirSync(dir).filter((f) => /\.ya?ml$/.test(f)).sort()
      .map((f) => ({ path: `.github/workflows/${f}`, text: readFileSync(join(dir, f), "utf8") }))
    : [];
  const testPath = join(root, ENGINE_TEST);
  const readFile = (rel) => {
    const path = join(root, rel);
    return rel !== "" && existsSync(path) && statSync(path).isFile() ? readFileSync(path, "utf8") : null;
  };
  return { workflows, engineTest: existsSync(testPath) ? readFileSync(testPath, "utf8") : null, scripts: readToolScripts(root), readFile };
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
  const { values } = parseArgs({ options: { repo: { type: "string" }, json: { type: "string" }, summary: { type: "string" } } });
  let result;
  try {
    result = checkOrder(loadRepo(values.repo ?? REPO_ROOT));
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
