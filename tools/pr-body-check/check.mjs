// PR 本文の見出しの構造を決定論で検査する（知見 K-0001 の担保。議決 D-0001、Issue #826）。LLM を使わない。
//
//   node check.mjs [--body <本文のファイル>] [--json <出力先>]    # --body を省くと標準入力から読む
//   終了コード: 0 = 通る、1 = 外れた、2 = 読めなかった
//
// 見ること:
//   - 同じ「## 」見出しが 2 回以上ない（続きのジョブが新しい PR と同じ見出しの組を末尾に足すと重なる）
//   - 「## 工場の記録」より後ろに「## 」以上の見出しがない（続きのジョブの記録は「## 工場の記録」の下の「### 」に入れる）
//   - 「続きのジョブ」の見出しが「## 工場の記録」の下の「### 」以下にある
// コードブロック（``` と ~~~）の中の行は見出しとして数えない。
//
// 呼び出し: 本文の更新の前の関所 gate.mjs（議決 D-0013、Issue #956）がこの checkBody を使う。外れたら本文を書かずに body_fix に回す。

import { readFileSync, writeFileSync } from "node:fs";
import { parseArgs } from "node:util";

/** 工場の記録の見出し（Engine の worker/entrypoint.sh が新しい PR の本文に付ける）。 */
export const FACTORY_RECORD = "工場の記録";
/** 続きのジョブの記録の見出しに含まれる語。 */
export const CONTINUATION = "続きのジョブ";

const HEADING = /^ {0,3}(#{1,6})[ \t]+(.*?)(?:[ \t]+#+)?[ \t]*$/;
const FENCE = /^ {0,3}(`{3,}|~{3,})/;

/** 本文の見出しを、行番号（1 始まり）・階層・文字列の組で返す。コードブロックの中は飛ばす。 */
export function headings(body) {
  const out = [];
  let fence = null;
  body.split(/\r?\n/).forEach((line, i) => {
    const f = line.match(FENCE);
    if (f) {
      if (fence === null) fence = f[1];
      else if (f[1][0] === fence[0] && f[1].length >= fence.length) fence = null;
      return;
    }
    if (fence !== null) return;
    const m = line.match(HEADING);
    if (m) out.push({ line: i + 1, level: m[1].length, text: m[2].trim() });
  });
  return out;
}

/** 本文を検査し、外れた箇所の一覧を返す（空なら通る）。 */
export function checkBody(body) {
  const hs = headings(body);
  const errors = [];

  const seen = new Map();
  for (const h of hs.filter((h) => h.level === 2)) {
    if (seen.has(h.text)) {
      errors.push({ rule: "duplicate_h2", line: h.line, heading: h.text,
        message: `「## ${h.text}」が 2 回以上ある（${seen.get(h.text)} 行目と ${h.line} 行目）。続きの報告は「## ${FACTORY_RECORD}」の下の ### に書く` });
    } else {
      seen.set(h.text, h.line);
    }
  }

  const record = hs.find((h) => h.level === 2 && h.text === FACTORY_RECORD);
  for (const h of hs) {
    const afterRecord = record !== undefined && h.line > record.line;
    if (afterRecord && h.level <= 2) {
      errors.push({ rule: "outside_factory_record", line: h.line, heading: h.text,
        message: `「## ${FACTORY_RECORD}」より後ろに「${"#".repeat(h.level)} ${h.text}」がある。ここから下は ### 以下の見出しにする` });
    } else if (h.text.includes(CONTINUATION) && !(afterRecord && h.level >= 3)) {
      errors.push({ rule: "continuation_outside_record", line: h.line, heading: h.text,
        message: `続きのジョブの見出し「${"#".repeat(h.level)} ${h.text}」が「## ${FACTORY_RECORD}」の下の ### 以下にない` });
    }
  }

  return errors.sort((a, b) => a.line - b.line);
}

function main() {
  const { values } = parseArgs({ options: { body: { type: "string" }, json: { type: "string" } } });
  let body;
  try {
    body = readFileSync(values.body ?? 0, "utf8");
  } catch (e) {
    console.error(`pr-body-check: 本文を読めなかった: ${e.message}`);
    process.exit(2);
  }
  const errors = checkBody(body);
  if (values.json) writeFileSync(values.json, JSON.stringify({ ok: errors.length === 0, errors }, null, 2) + "\n");
  for (const e of errors) console.log(`${e.line} 行目: ${e.message}`);
  console.log(`pr-body-check: 見出し ${headings(body).length} 件、外れ ${errors.length} 件`);
  process.exit(errors.length === 0 ? 0 : 1);
}

if (import.meta.url === `file://${process.argv[1]}`) main();
