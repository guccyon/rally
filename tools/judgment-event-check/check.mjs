// judge の裁き（judgment.judged）と差し戻しの incident の形を、スキーマで決定論に検査する（知見 K-0003 の担保。議決 D-0004、Issue #903）。LLM を使わない。
//
//   node check.mjs --event <judgment.judged | incident.sendback> [--file <JSON Lines か JSON 配列>] [--json <出力先>]    # --file を省くと標準入力から読む
//   終了コード: 0 = 通る、1 = 外れた、2 = 読めなかった
//
// 見ること（スキーマは schema/ の JSON Schema。ここはその一部のキーワードだけを解釈する）:
//   - judgment.judged: verdict が adopt なら subject（provisional = 仮決めの承認、defect = 欠陥の採用）がある
//   - incident.sendback: adopted_defects と approved_provisionals が分かれてあり、2 つを混ぜた合計（adopted）がない
//
// 想定する呼び出し: Engine が judgment.judged を書く前と、supervisor が差し戻しの incident を数える前。外れたら書かずに止める。

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { parseArgs } from "node:util";
import { fileURLToPath } from "node:url";

const SCHEMA_DIR = join(dirname(fileURLToPath(import.meta.url)), "schema");

/** 検査できるイベントの名前。schema/<名前>.schema.json に対応する。 */
export const EVENTS = ["judgment.judged", "incident.sendback"];

/** イベントの名前からスキーマを読む。 */
export function loadSchema(event) {
  if (!EVENTS.includes(event)) throw new Error(`知らないイベント: ${event}（${EVENTS.join(" | ")}）`);
  return JSON.parse(readFileSync(join(SCHEMA_DIR, `${event}.schema.json`), "utf8"));
}

function typeOf(v) {
  if (v === null) return "null";
  if (Array.isArray(v)) return "array";
  if (typeof v === "number") return Number.isInteger(v) ? "integer" : "number";
  return typeof v;
}

const matchesType = (v, t) => typeOf(v) === t || (t === "number" && typeOf(v) === "integer");

/**
 * 値をスキーマで検査し、外れた箇所の一覧を返す（空なら通る）。
 * 解釈するキーワードは type・enum・const・required・properties・minimum・maximum・minLength・if/then/else・not・allOf だけ。
 */
export function validate(value, schema, path = "$") {
  const errors = [];
  const err = (message) => errors.push({ path, message });

  if (schema.type !== undefined) {
    const types = [schema.type].flat();
    if (!types.some((t) => matchesType(value, t))) {
      err(`型が ${types.join(" | ")} でない（${typeOf(value)}）`);
      return errors;
    }
  }
  if (schema.enum !== undefined && !schema.enum.includes(value)) {
    err(`${JSON.stringify(value)} は ${schema.enum.join(" | ")} のどれでもない`);
  }
  if (schema.const !== undefined && value !== schema.const) {
    err(`${JSON.stringify(value)} は ${JSON.stringify(schema.const)} でない`);
  }
  if (typeof value === "number") {
    if (schema.minimum !== undefined && value < schema.minimum) err(`${value} は ${schema.minimum} より小さい`);
    if (schema.maximum !== undefined && value > schema.maximum) err(`${value} は ${schema.maximum} より大きい`);
  }
  if (typeof value === "string" && schema.minLength !== undefined && value.length < schema.minLength) {
    err(`文字数が ${schema.minLength} に満たない`);
  }
  const isObject = typeOf(value) === "object";
  if (isObject) {
    for (const key of schema.required ?? []) {
      if (!(key in value)) err(`${key} がない`);
    }
    for (const [key, sub] of Object.entries(schema.properties ?? {})) {
      if (key in value) errors.push(...validate(value[key], sub, `${path}.${key}`));
    }
  }
  for (const sub of schema.allOf ?? []) errors.push(...validate(value, sub, path));
  if (schema.if !== undefined) {
    const branch = validate(value, schema.if, path).length === 0 ? schema.then : schema.else;
    if (branch !== undefined) {
      const why = branch.description ? `（${branch.description}）` : "";
      errors.push(...validate(value, branch, path).map((e) => ({ ...e, message: e.message + why })));
    }
  }
  if (schema.not !== undefined && validate(value, schema.not, path).length === 0) {
    err(schema.not.description ?? "当てはまってはいけない形に当てはまる");
  }
  return errors;
}

/** JSON Lines か JSON の配列を、行番号（配列なら何件目か。1 始まり）付きのイベントの一覧にする。 */
export function parseEvents(text) {
  const trimmed = text.trim();
  if (trimmed.startsWith("[")) return JSON.parse(trimmed).map((value, i) => ({ line: i + 1, value }));
  return text.split(/\r?\n/)
    .map((raw, i) => ({ line: i + 1, raw: raw.trim() }))
    .filter(({ raw }) => raw !== "")
    .map(({ line, raw }) => ({ line, value: JSON.parse(raw) }));
}

/** イベントの一覧を検査し、外れた箇所の一覧を返す。payload を包んだ行（{ name, payload }）は payload を見る。 */
export function checkEvents(event, events) {
  const schema = loadSchema(event);
  return events.flatMap(({ line, value }) => {
    const payload = typeOf(value) === "object" && typeOf(value.payload) === "object" ? value.payload : value;
    return validate(payload, schema).map((e) => ({ event, line, ...e }));
  });
}

function main() {
  const { values } = parseArgs({ options: { event: { type: "string" }, file: { type: "string" }, json: { type: "string" } } });
  let events;
  try {
    loadSchema(values.event);
    events = parseEvents(readFileSync(values.file ?? 0, "utf8"));
  } catch (e) {
    console.error(`judgment-event-check: 読めなかった: ${e.message}`);
    process.exit(2);
  }
  const errors = checkEvents(values.event, events);
  if (values.json) writeFileSync(values.json, JSON.stringify({ ok: errors.length === 0, errors }, null, 2) + "\n");
  for (const e of errors) console.log(`${e.line} 行目 ${e.path}: ${e.message}`);
  console.log(`judgment-event-check: ${values.event} ${events.length} 件、外れ ${errors.length} 件`);
  process.exit(errors.length === 0 ? 0 : 1);
}

if (import.meta.url === `file://${process.argv[1]}`) main();
