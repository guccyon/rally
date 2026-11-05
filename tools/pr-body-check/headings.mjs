// PR 本文の ## 見出しの定義が 1 つに揃っているかを決定論で確かめる（知見 K-0005 の担保。議決 D-0006、Issue #905）。LLM を使わない。
//
//   node headings.mjs [--repo <リポジトリ>] [--engine <factory-engine の置き場>]
//   終了コード: 0 = 揃っている、1 = ずれた、2 = 読めなかった
//
// 正は headings.json の headings。次の 3 つをそれと照らす:
//   - claude_md: CLAUDE.md の「PR の説明は … の見出しで」（判定役が決まりとして読む）
//   - worker:    Engine の orchestrator/worker/entrypoint.sh の「最後の出力を次の見出しだけで構成する」（実装役の報告の型）
//   - rubric:    Engine の core-gates/rubric.yaml の PR-002 の check の「見出し（…）」（検査役）
// Engine 側のずれで人の承認を待っているものは headings.json の known_drift に今の値を書く。その値のときだけ通し、
// それ以外の値になったら（直って正と一致したときも）落として記録を直させる。

import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { parseArgs } from "node:util";

/** 正の定義（headings.json）を読む。 */
export function loadDefinition(path = new URL("./headings.json", import.meta.url)) {
  const def = JSON.parse(readFileSync(path, "utf8"));
  const { $comment: _, ...drift } = def.known_drift ?? {};
  return { headings: def.headings, knownDrift: drift };
}

/** CLAUDE.md の「PR の説明は A / B / … の見出しで」から見出しの並びを取る。見つからなければ null。 */
export function fromClaudeMd(text) {
  const m = text.match(/^- PR の説明は (.+?) の見出しで/m);
  return m ? m[1].split(" / ").map((s) => s.trim()) : null;
}

/** 実装役への指示（entrypoint.sh）の「最後の出力を次の見出しだけで構成する」の後の「## X」を順に取る。見つからなければ null。 */
export function fromWorkerPrompt(text) {
  const lines = text.split(/\r?\n/);
  const start = lines.findIndex((l) => l.includes("最後の出力を次の見出しだけで構成する"));
  if (start < 0) return null;
  const block = [];
  for (const line of lines.slice(start + 1)) {
    if (/^\s*-\s/.test(line) || line.trim() === "" || line.startsWith("EOF")) break;
    block.push(line);
  }
  const out = [...block.join("\n").matchAll(/「## ([^」]+)」/g)].map((m) => m[1].trim());
  return out.length > 0 ? out : null;
}

/**
 * rubric.yaml の PR-002 の check にある「見出し（A・B・…）」から並びを取る。見つからなければ null。
 * 区切りの「・」は見出しの中にも出る（影響範囲・互換性）ので、つないで known のどれかになる組は 1 つにまとめる。
 */
export function fromRubric(text, known = []) {
  const block = text.split(/^\s*- id:\s*/m).find((b) => /^"?PR-002"?\s*$/m.test(b.split("\n")[0]));
  const check = block?.match(/^\s*check:\s*(.+)$/m)?.[1];
  const list = check?.match(/見出し（([^）]+)）/)?.[1];
  if (!list) return null;
  const tokens = list.split("・").map((s) => s.trim());
  const out = [];
  for (let i = 0; i < tokens.length; i++) {
    let j = tokens.length;
    while (j > i + 1 && !known.includes(tokens.slice(i, j).join("・"))) j--;
    out.push(tokens.slice(i, j).join("・"));
    i = j - 1;
  }
  return out;
}

/** 2 つの並びの違いを文で返す（同じなら null）。 */
export function diffHeadings(expected, actual) {
  const missing = expected.filter((h) => !actual.includes(h));
  const extra = actual.filter((h) => !expected.includes(h));
  const parts = [];
  if (missing.length) parts.push(`足りない: ${missing.join("・")}`);
  if (extra.length) parts.push(`正にない: ${extra.join("・")}`);
  if (!parts.length && expected.join("\n") !== actual.join("\n")) parts.push(`順が違う: ${actual.join(" / ")}`);
  return parts.length ? parts.join("、") : null;
}

/** 1 つの定義の場所を正と照らし、外れなら理由を返す（通れば null）。drift は known_drift にある今の値。 */
export function checkSource(name, actual, headings, drift) {
  if (actual === null) return `${name}: 見出しの一覧が見つからない（書き方が変わったなら headings.mjs の読み方を直す）`;
  const diff = diffHeadings(headings, actual);
  if (drift === undefined) return diff && `${name}: 正（headings.json）とずれている。${diff}`;
  if (diff === null) return `${name}: 正と一致したので、headings.json の known_drift.${name} を消す`;
  if (diffHeadings(drift, actual) !== null) {
    return `${name}: known_drift に記録した値から変わった（今: ${actual.join(" / ")}）。正と揃えるか、記録を直す`;
  }
  return null;
}

/** リポジトリと Engine の定義を読んで照らす。engine が null なら CLAUDE.md だけ。{ errors, checked } を返す。 */
export function checkAll({ repo, engine, def = loadDefinition() }) {
  const sources = [["claude_md", () => fromClaudeMd(readFileSync(join(repo, "CLAUDE.md"), "utf8"))]];
  if (engine !== null) {
    sources.push(
      ["worker", () => fromWorkerPrompt(readFileSync(join(engine, "orchestrator/worker/entrypoint.sh"), "utf8"))],
      ["rubric", () => fromRubric(readFileSync(join(engine, "core-gates/rubric.yaml"), "utf8"), def.headings)],
    );
  }
  const errors = [];
  for (const [name, read] of sources) {
    const e = checkSource(name, read(), def.headings, def.knownDrift[name]);
    if (e) errors.push(e);
  }
  return { errors, checked: sources.map(([name]) => name) };
}

/** Engine の置き場。環境変数 FACTORY_ENGINE_DIR か、ワーカーに載っている /work/engine。無ければ null。 */
export function engineDir(env = process.env) {
  const dir = env.FACTORY_ENGINE_DIR ?? "/work/engine";
  return existsSync(join(dir, "core-gates/rubric.yaml")) ? dir : null;
}

function main() {
  const { values } = parseArgs({ options: { repo: { type: "string" }, engine: { type: "string" } } });
  const repo = values.repo ?? new URL("../..", import.meta.url).pathname;
  const engine = values.engine ?? engineDir();
  let result;
  try {
    result = checkAll({ repo, engine });
  } catch (e) {
    console.error(`pr-body-headings: 定義を読めなかった: ${e.message}`);
    process.exit(2);
  }
  for (const e of result.errors) console.log(e);
  if (engine === null) console.log("pr-body-headings: Engine が見つからないので CLAUDE.md だけ見た（--engine か FACTORY_ENGINE_DIR で渡す）");
  console.log(`pr-body-headings: ${result.checked.join("・")} を見て、ずれ ${result.errors.length} 件`);
  process.exit(result.errors.length === 0 ? 0 : 1);
}

if (import.meta.url === `file://${process.argv[1]}`) main();
