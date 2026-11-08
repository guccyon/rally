// PR 本文の見出しの正本（template.json）に、ほかの見出しの一覧が合っているかを決定論で照らす（知見 K-0005 の担保。Issue #1121）。LLM を使わない。
// 正本と、正本に合わせる側（CLAUDE.md の「PR の説明」と Engine の rubric の PR-002 の check）は followers.json に書く（正本はオーナーが決めた。Issue #936）。
//
//   node followers.mjs [--engine-dir <Engine のチェックアウト>] [--json <出力先>]
//   Engine の道は --engine-dir、なければ環境変数 ENGINE_DIR、なければリポジトリの隣の ../engine
//   終了コード: 0 = 合っている、1 = 食い違った、2 = 合わせる側を読めなかった（照らしていない）、3 = followers.json か template.json が不正
//
// 見ること:
//   - 合わせる側の見出しの一覧が、template.json の見出しを定義の順に区切りでつないだものと同じ（headings_mismatch）
//   - 合わせる側に必須の一覧があれば、template.json の必須の見出しを同じ順につないだものと同じ（required_mismatch）
// 区切りで割らずに、つないだ文字列どうしで比べる（rubric の区切りの「・」が見出し「影響範囲・互換性」にも入るため）。

import { readFileSync, writeFileSync } from "node:fs";
import { parseArgs } from "node:util";
import { fileURLToPath } from "node:url";

import { TemplateError, loadTemplate } from "./check.mjs";

/** followers.json が不正なときに投げる。終了コード 3。 */
export class FollowersError extends Error {
  constructor(reason) {
    super(reason);
    this.name = "FollowersError";
    this.reason = reason;
  }
}

/** 合わせる側を読めない・一覧を取り出せないときに投げる。終了コード 2。 */
export class SourceError extends Error {
  constructor(reason) {
    super(reason);
    this.name = "SourceError";
    this.reason = reason;
  }
}

const ROOTS = ["repo", "engine"];

/** 一覧の取り出し方（pattern は捕まえる組が 1 つの正規表現、separator は見出しの区切り）を検証して返す。 */
function parseList(v, where) {
  const fail = (why) => { throw new FollowersError(`${where}: ${why}`); };
  if (v === null || typeof v !== "object" || Array.isArray(v)) fail("オブジェクトでない");
  const extra = Object.keys(v).filter((k) => k !== "pattern" && k !== "separator");
  if (extra.length > 0) fail(`知らない欄がある（${extra.join("・")}）`);
  if (typeof v.separator !== "string" || v.separator === "") fail("separator が空でない文字列でない");
  if (typeof v.pattern !== "string" || v.pattern === "") fail("pattern が空でない文字列でない");
  let re;
  try {
    re = new RegExp(v.pattern, "g");
  } catch (e) {
    fail(`pattern が正規表現でない（${e.message}）`);
  }
  if (new RegExp(`${v.pattern}|`).exec("").length !== 2) fail("pattern の捕まえる組が 1 つでない");
  return Object.freeze({ pattern: v.pattern, separator: v.separator, re });
}

/**
 * followers.json の中身を検証し、凍結した合わせる側の一覧を返す。canonical は template.json だけ（正本は 1 つ）。
 * 各要素は id・root（repo か engine）・path・headings を持ち、required と、rubric の項目を指す rubric_id と field は任意（2 つはそろえて書く）。
 */
export function parseFollowers(data) {
  const fail = (why) => { throw new FollowersError(why); };
  if (data === null || typeof data !== "object" || Array.isArray(data)) fail("オブジェクトでない");
  if (data.canonical !== "template.json") fail("canonical が template.json でない（見出しの正本は template.json の 1 つ）");
  if (!Array.isArray(data.followers) || data.followers.length === 0) fail("followers が空でない配列でない");
  const ids = new Set();
  return Object.freeze(data.followers.map((f, i) => {
    const where = `followers[${i}]`;
    if (f === null || typeof f !== "object" || Array.isArray(f)) fail(`${where} がオブジェクトでない`);
    const known = ["id", "root", "path", "headings", "required", "rubric_id", "field"];
    const extra = Object.keys(f).filter((k) => !known.includes(k));
    if (extra.length > 0) fail(`${where} に知らない欄がある（${extra.join("・")}）`);
    if (typeof f.id !== "string" || f.id === "") fail(`${where}.id が空でない文字列でない`);
    if (ids.has(f.id)) fail(`${where}.id「${f.id}」が重なっている`);
    ids.add(f.id);
    if (!ROOTS.includes(f.root)) fail(`${where}.root が ${ROOTS.join(" か ")} でない`);
    if (typeof f.path !== "string" || f.path === "") fail(`${where}.path が空でない文字列でない`);
    if (("rubric_id" in f) !== ("field" in f)) fail(`${where} の rubric_id と field はそろえて書く`);
    if ("rubric_id" in f && (typeof f.rubric_id !== "string" || typeof f.field !== "string" || f.rubric_id === "" || f.field === "")) {
      fail(`${where} の rubric_id と field が空でない文字列でない`);
    }
    return Object.freeze({
      id: f.id, root: f.root, path: f.path,
      rubric_id: f.rubric_id ?? null, field: f.field ?? null,
      headings: parseList(f.headings, `${where}.headings`),
      required: "required" in f ? parseList(f.required, `${where}.required`) : null,
    });
  }));
}

/** followers.json を読んで検証する。読めない・JSON でない・不正なら FollowersError。 */
export function loadFollowers(url = new URL("./followers.json", import.meta.url)) {
  let data;
  try {
    data = JSON.parse(readFileSync(url, "utf8"));
  } catch (e) {
    throw new FollowersError(`followers.json を読めなかった（${e.message}）`);
  }
  return parseFollowers(data);
}

/** YAML の 1 行の値（二重引用・一重引用・引用なし）を文字列にする。複数行の形は読まない。 */
function scalar(raw, where) {
  const v = raw.trim();
  if (v.startsWith("\"")) {
    try {
      return JSON.parse(v);
    } catch {
      throw new SourceError(`${where} の二重引用の値を読めなかった`);
    }
  }
  if (v.startsWith("'")) {
    if (!v.endsWith("'") || v.length < 2) throw new SourceError(`${where} の一重引用の値を読めなかった`);
    return v.slice(1, -1).replaceAll("''", "'");
  }
  if (v === "" || /^[|>]/.test(v)) throw new SourceError(`${where} が 1 行の値でない（複数行の形は読まない）`);
  return v.replace(/\s+#.*$/, "");
}

/** rubric.yaml の本文から、id の項目の field の値を取り出す。項目がない・重なる・欄がないなら SourceError。 */
export function rubricField(text, id, field) {
  const lines = text.split(/\r?\n/);
  const starts = [];
  lines.forEach((l, i) => {
    const m = l.match(/^(\s*)- id:\s*(.+?)\s*$/);
    if (m && scalar(m[2], `id の行（${i + 1} 行目）`) === id) starts.push({ i, indent: m[1].length });
  });
  if (starts.length !== 1) throw new SourceError(`rubric に ${id} の項目が ${starts.length} 件ある（1 件のはず）`);
  const { i: start, indent } = starts[0];
  const key = new RegExp(`^ {${indent + 2}}${field}:(.*)$`);
  for (let i = start + 1; i < lines.length; i++) {
    const l = lines[i];
    if (l.trim() !== "" && l.search(/\S/) <= indent) break;
    const m = l.match(key);
    if (m) return scalar(m[1], `${id} の ${field}（${i + 1} 行目）`);
  }
  throw new SourceError(`rubric の ${id} に ${field} がない`);
}

/** 本文から一覧（捕まえた組）を取り出す。ちょうど 1 か所で当たらなければ SourceError。 */
function capture(text, list, what) {
  const hits = [...text.matchAll(list.re)];
  if (hits.length !== 1) throw new SourceError(`${what}が ${hits.length} か所で当たった（1 か所のはず。pattern: ${list.pattern}）`);
  return hits[0][1];
}

/** 合わせる側の本文（rubric なら項目の欄の値）から、見出しと必須の一覧の文字列を取り出す。 */
export function extract(follower, text) {
  const body = follower.rubric_id === null ? text : rubricField(text, follower.rubric_id, follower.field);
  return {
    headings: capture(body, follower.headings, `${follower.id} の見出しの一覧`),
    required: follower.required === null ? null : capture(body, follower.required, `${follower.id} の必須の一覧`),
  };
}

/**
 * 合わせる側を正本に照らし、食い違いの一覧を返す（空なら合っている）。read(follower) は合わせる側の本文を返す（読めなければ SourceError）。
 */
export function checkFollowers({ template = loadTemplate(), followers = loadFollowers(), read }) {
  const errors = [];
  for (const f of followers) {
    const got = extract(f, read(f));
    const where = f.rubric_id === null ? f.path : `${f.path} の ${f.rubric_id} の ${f.field}`;
    const headings = template.map((t) => t.text).join(f.headings.separator);
    if (got.headings !== headings) {
      errors.push({ rule: "headings_mismatch", follower: f.id, expected: headings, actual: got.headings,
        message: `${where} の見出しの一覧「${got.headings}」が template.json の「${headings}」と食い違う。template.json に合わせる` });
    }
    if (f.required !== null) {
      const required = template.filter((t) => t.required).map((t) => t.text).join(f.required.separator);
      if (got.required !== required) {
        errors.push({ rule: "required_mismatch", follower: f.id, expected: required, actual: got.required,
          message: `${where} の必須の見出し「${got.required}」が template.json の「${required}」と食い違う。template.json に合わせる` });
      }
    }
  }
  return errors;
}

/** root ごとの道から本文を読む関数を返す。読めなければ SourceError。 */
export function reader(roots) {
  return (f) => {
    const url = new URL(f.path, roots[f.root]);
    try {
      return readFileSync(url, "utf8");
    } catch (e) {
      throw new SourceError(`${f.id} の ${fileURLToPath(url)} を読めなかった（${e.message}）`);
    }
  };
}

/** ディレクトリの道を、末尾に / のある file: の URL にする。 */
export const dirUrl = (dir) => new URL(`file://${dir.endsWith("/") ? dir : `${dir}/`}`);

function main() {
  const { values } = parseArgs({ options: { "engine-dir": { type: "string" }, json: { type: "string" } } });
  const engineDir = values["engine-dir"] || process.env.ENGINE_DIR;
  const roots = {
    repo: new URL("../../", import.meta.url),
    engine: engineDir ? dirUrl(engineDir) : new URL("../../../engine/", import.meta.url),
  };
  let errors;
  try {
    errors = checkFollowers({ read: reader(roots) });
  } catch (e) {
    if (e instanceof SourceError) {
      console.error(`pr-body-followers: 合わせる側を読めなかった: ${e.reason}`);
      process.exit(2);
    }
    if (e instanceof FollowersError || e instanceof TemplateError) {
      console.error(`pr-body-followers: 定義が不正: ${e.reason}`);
      process.exit(3);
    }
    throw e;
  }
  if (values.json) writeFileSync(values.json, JSON.stringify({ ok: errors.length === 0, errors }, null, 2) + "\n");
  for (const e of errors) console.log(`${e.follower}: ${e.message}`);
  console.log(`pr-body-followers: 合わせる側 ${loadFollowers().length} 件、食い違い ${errors.length} 件`);
  process.exit(errors.length === 0 ? 0 : 1);
}

if (import.meta.url === `file://${process.argv[1]}`) main();
