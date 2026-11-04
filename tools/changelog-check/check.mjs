// PR が CHANGELOG.md の [Unreleased] を直接書き換えていないかを見る（知見 K-0002、議決 D-0001、factory-engine の ADR 0027）。
//
//   node check.mjs --base origin/main --head HEAD [--summary <GITHUB_STEP_SUMMARY>]
//
// 比べるのは base と head の分かれ目（merge-base）と head。git diff base...head と同じ範囲で、既定ブランチ側の変更は見ない。
// 決定的（LLM を使わない）。依存は node と git だけ。
//
// 通す変更:
//   - [Unreleased] を変えていない（断片 changelog.d/<Issue 番号>.md だけを足す、ふつうの PR）
//   - 取りまとめ（core-gates/changelog_compile.py）: [Unreleased] に足した行がすべて、この PR で消した断片の行か種類の見出し（### 追加 など）で、[Unreleased] から消した行がない
//   - 版の切り出し: [Unreleased] から消した行がすべて、[Unreleased] の外（版の項）に移っていて、[Unreleased] に足した行がない
// それ以外で [Unreleased] を変えていたら落とす。断片を足していても、CHANGELOG.md を直接書き換えれば取りまとめで二重になるので落とす。
//
// 終了コード: 0 = 通る、1 = 通らない、2 = 読めなかった

import { execFileSync } from "node:child_process";
import { appendFileSync } from "node:fs";
import { parseArgs } from "node:util";
import { pathToFileURL } from "node:url";

export const CHANGELOG = "CHANGELOG.md";
export const FRAGMENT_DIR = "changelog.d";
const UNRELEASED = "## [Unreleased]";
const SKIP = new Set(["README.md"]);
const KIND_HEADING = /^###\s+\S/;

/** CHANGELOG の本文を [Unreleased] の中と外の行（空行を除く）に分ける。[Unreleased] がなければ inside は null。 */
export function splitUnreleased(text) {
  const lines = (text ?? "").split("\n").map((l) => l.trimEnd());
  const start = lines.findIndex((l) => l.trim() === UNRELEASED);
  if (start < 0) return { inside: null, outside: lines.filter((l) => l.trim()) };
  let end = lines.findIndex((l, i) => i > start && l.startsWith("## "));
  if (end < 0) end = lines.length;
  return {
    inside: lines.slice(start + 1, end).filter((l) => l.trim()),
    outside: [...lines.slice(0, start + 1), ...lines.slice(end)].filter((l) => l.trim()),
  };
}

/** a にあって b にない行（同じ行が何回あるかも数える）。 */
function minus(a, b) {
  const left = new Map();
  for (const l of b) left.set(l, (left.get(l) ?? 0) + 1);
  const out = [];
  for (const l of a) {
    const n = left.get(l) ?? 0;
    if (n > 0) left.set(l, n - 1);
    else out.push(l);
  }
  return out;
}

/** 断片の項目の行（見出しと空行を除く）。changelog_compile.py の parse_fragment と同じく行はそのまま移る。 */
function fragmentLines(text) {
  return text
    .split("\n")
    .map((l) => l.trimEnd())
    .filter((l) => l.trim() && !/^#{2,4}\s+/.test(l));
}

/**
 * 差分を判定する。
 * @param {{ baseChangelog: string, headChangelog: string, baseFragments: Map<string, string>, headFragments: Map<string, string> }} input
 *   断片は changelog.d/ のファイル名 → 中身（README.md は除く）
 * @returns {{ ok: boolean, kind: string, message: string, added: string[], removed: string[], addedFragments: string[], removedFragments: string[] }}
 */
export function checkChangelog({ baseChangelog, headChangelog, baseFragments, headFragments }) {
  const base = splitUnreleased(baseChangelog);
  const head = splitUnreleased(headChangelog);
  const addedFragments = [...headFragments.keys()].filter((n) => !baseFragments.has(n)).sort();
  const removedFragments = [...baseFragments.keys()].filter((n) => !headFragments.has(n)).sort();
  const result = (ok, kind, message, added = [], removed = []) => ({ ok, kind, message, added, removed, addedFragments, removedFragments });

  if (base.inside === null) return result(true, "no_unreleased", `ベースの ${CHANGELOG} に ${UNRELEASED} がないので見ない`);
  if (head.inside === null) {
    return result(false, "unreleased_removed", `${CHANGELOG} の ${UNRELEASED} の見出しを消している。見出しは残す`);
  }
  const added = minus(head.inside, base.inside);
  const removed = minus(base.inside, head.inside);
  if (!added.length && !removed.length) return result(true, "untouched", `${CHANGELOG} の ${UNRELEASED} は変わっていない`);

  if (!removed.length && removedFragments.length) {
    const moved = removedFragments.flatMap((n) => fragmentLines(baseFragments.get(n)));
    const extra = minus(added.filter((l) => !KIND_HEADING.test(l)), moved);
    if (!extra.length) {
      return result(true, "compiled", `取りまとめ: 断片 ${removedFragments.length} 件を ${UNRELEASED} に移した`, added, removed);
    }
  }
  if (!added.length) {
    const released = minus(head.outside, base.outside);
    if (!minus(removed, released).length) {
      return result(true, "released", `版の切り出し: ${UNRELEASED} の ${removed.length} 行を版の項に移した`, added, removed);
    }
  }
  const hint = addedFragments.length
    ? `断片（${addedFragments.map((n) => `${FRAGMENT_DIR}/${n}`).join("・")}）を足しているので、${CHANGELOG} の変更は戻す（取りまとめで二重になる）`
    : `${CHANGELOG} の変更を戻し、${FRAGMENT_DIR}/<Issue 番号>.md に [Unreleased] に入れる行を書く（書き方は ${FRAGMENT_DIR}/README.md）`;
  return result(false, "direct_edit", `${CHANGELOG} の ${UNRELEASED} を直接書き換えている。${hint}`, added, removed);
}

function git(args, cwd) {
  return execFileSync("git", args, { cwd, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
}

/** ref の時点のファイル。なければ null。 */
function show(ref, path, cwd) {
  try {
    return execFileSync("git", ["show", `${ref}:${path}`], { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "ignore"], maxBuffer: 64 * 1024 * 1024 });
  } catch {
    return null;
  }
}

function fragmentsAt(ref, cwd) {
  const out = new Map();
  for (const path of git(["ls-tree", "--name-only", `${ref}:${FRAGMENT_DIR}`], cwd).split("\n").filter(Boolean)) {
    if (!path.endsWith(".md") || SKIP.has(path)) continue;
    out.set(path, show(ref, `${FRAGMENT_DIR}/${path}`, cwd) ?? "");
  }
  return out;
}

function fragmentsAtOrEmpty(ref, cwd) {
  try {
    return fragmentsAt(ref, cwd);
  } catch {
    return new Map();
  }
}

/** git の 2 つの ref から判定の入力を作る。base は分かれ目（merge-base）に置き換える。cwd は git のリポジトリ（既定は今のディレクトリ）。 */
export function readFromGit(baseRef, headRef, { cwd } = {}) {
  const mergeBase = git(["merge-base", baseRef, headRef], cwd).trim();
  return {
    baseChangelog: show(mergeBase, CHANGELOG, cwd) ?? "",
    headChangelog: show(headRef, CHANGELOG, cwd) ?? "",
    baseFragments: fragmentsAtOrEmpty(mergeBase, cwd),
    headFragments: fragmentsAtOrEmpty(headRef, cwd),
  };
}

export function renderSummary(r) {
  const out = ["## changelog-check", "", `${r.ok ? "通る" : "通らない"}（${r.kind}）: ${r.message}`, ""];
  if (r.addedFragments.length) out.push(`- 足した断片: ${r.addedFragments.join("、")}`);
  if (r.removedFragments.length) out.push(`- 消した断片: ${r.removedFragments.join("、")}`);
  const block = (title, lines) => out.push(...(out.at(-1) === "" ? [] : [""]), title, "", "```", ...lines, "```");
  if (!r.ok && r.added.length) block("[Unreleased] に直接足した行:", r.added);
  if (!r.ok && r.removed.length) block("[Unreleased] から直接消した行:", r.removed);
  return out.join("\n") + "\n";
}

function main() {
  const { values } = parseArgs({ options: { base: { type: "string" }, head: { type: "string", default: "HEAD" }, summary: { type: "string" } } });
  if (!values.base) {
    console.error("--base が要る（例: --base origin/main）");
    return 2;
  }
  let r;
  try {
    r = checkChangelog(readFromGit(values.base, values.head));
  } catch (e) {
    console.error(`読めなかった: ${e.message}`);
    return 2;
  }
  if (values.summary) appendFileSync(values.summary, renderSummary(r));
  if (r.ok) console.log(`changelog: ${r.message}`);
  else console.log(`::error file=${CHANGELOG}::${r.message}`);
  return r.ok ? 0 : 1;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) process.exit(main());
