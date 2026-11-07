// Engine の本物の judge の入力の組み立て（core-gates/judge.py の main）で、検査役の指摘が引くすべての rubric_id について
// rubric の本文が入力に入ることを確かめる（知見 K-0011 の担保。議決 D-0033、Issue #1054）。事例 C-0006（PR #821）の指摘 7 件を fixtures にする。
// Engine のチェックアウトは環境変数 ENGINE_DIR で渡す（なければリポジトリの隣の ../engine）。どちらもなければ飛ばす。
// ただし ENGINE_DIR を渡したときと CI（CI=true）では飛ばさずに落とす（sendback-check の engine.test.mjs と同じ。議決 D-0011）。
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { cpSync, existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { checkInput, requiredRubricIds } from "./line.mjs";

const harness = fileURLToPath(new URL("./engine_harness.py", import.meta.url));
const engineDir = process.env.ENGINE_DIR || fileURLToPath(new URL("../../../engine", import.meta.url));
const required = Boolean(process.env.ENGINE_DIR) || process.env.CI === "true";
const skip = required || existsSync(`${engineDir}/core-gates/judge.py`) ? false : `Engine のチェックアウトがない（ENGINE_DIR=${engineDir}）`;

const C0006 = JSON.parse(readFileSync(new URL("./fixtures/c0006.json", import.meta.url), "utf8"));

/** Engine の judge.py で入力を 1 回組み、プロンプトと rubric_id ごとの正本の本文を返す（dir を渡せばその Engine の写しで動かす） */
const engine = (record, dir = engineDir) =>
  JSON.parse(execFileSync("python3", ["-I", harness, dir], { input: JSON.stringify(record), encoding: "utf8" }));

test("(0) Engine のチェックアウトがあり、judge.py を差し替えて読み込める", { skip }, () => {
  let out;
  try {
    out = execFileSync("python3", ["-I", harness, "--check", engineDir], { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
  } catch (e) {
    assert.fail(`engine_harness.py が Engine の judge.py を読み込めない（ENGINE_DIR=${engineDir}）: ${e.stderr || e.message}`);
  }
  assert.deepEqual(JSON.parse(out).functions, ["main", "sh"]);
});

// 今の Engine の judge.py は Engine の core-gates/rubric.yaml だけから本文を引き、製品の overlay のルール（OV-001 など）の本文は渡さない。
// (1) は必須のテストだが、Engine がこの今の形（LEGACY。core の本文はすべてあり、overlay の本文が 1 つもない）のときだけ todo にする
// （外れても落ちないが結果に出る）。sendback-check の todoWhileLegacy と同じ扱い（議決 D-0011）。core の本文が欠けたとき、
// overlay の本文を一部だけ渡すようになったときは落ちる。建設者が judge.py で overlay の本文も渡すようにして ENGINE_DIR の版を上げたら、
// isLegacyRubric と todo を消してよい
const LEGACY_TODO = "Engine の judge.py が今の形（core-gates/rubric.yaml の本文だけを渡し、overlay のルールの本文を渡さない。建設者が Engine を直したら必須になる）";

/** 外れが、Engine の今の形（overlay の本文を 1 つも渡さない）によるものだけか */
export function isLegacyRubric(errors, sources) {
  const overlay = Object.keys(sources ?? {}).filter((id) => sources[id]?.source === "overlay");
  return errors.length > 0
    && errors.every((e) => e.rule === "missing_rubric_body" && e.source === "overlay")
    && overlay.every((id) => errors.some((e) => e.rubric_id === id));
}

test("(1) の todo は overlay の本文を 1 つも渡さない今の形にだけ効き、core の欠けと overlay の一部の欠けは必須にする", () => {
  const src = { "PR-002": { source: "core", body: "a" }, "OV-001": { source: "overlay", body: "b" }, "OV-002": { source: "overlay", body: "c" } };
  const miss = (id) => ({ rule: "missing_rubric_body", rubric_id: id, source: src[id].source });
  assert.ok(isLegacyRubric([miss("OV-001"), miss("OV-002")], src));
  assert.ok(!isLegacyRubric([], src), "外れがなければ todo にしない");
  assert.ok(!isLegacyRubric([miss("OV-001")], src), "overlay の本文を一部だけ渡す形は必須");
  assert.ok(!isLegacyRubric([miss("PR-002"), miss("OV-001"), miss("OV-002")], src), "core の本文の欠けは必須");
  assert.ok(!isLegacyRubric([{ rule: "unknown_rubric", rubric_id: "OV-001", source: null }], { "OV-001": { source: null, body: null } }));
});

test("(1) Engine の judge の入力に、C-0006 の指摘が引くすべての rubric の本文が入っている", { skip }, (t) => {
  const out = engine(C0006);
  const errors = checkInput(C0006.defects, out.prompt, out.rubric);
  if (isLegacyRubric(errors, out.rubric)) t.todo(LEGACY_TODO);
  assert.deepEqual(errors, []);
});

test("(2) C-0006 の fixtures は指摘 7 件で、core（PR-002）と overlay（OV-001）の rubric を引き、正本の本文を引ける", { skip }, () => {
  // (1) が見るものが空でないこと（fixtures や正本の置き場が変わって何も求めなくなると、(1) が黙って通る）
  assert.equal(C0006.provisional.length + C0006.defects.length, 7);
  assert.deepEqual(requiredRubricIds(C0006.defects), ["PR-002", "OV-001"]);
  const out = engine(C0006);
  assert.equal(out.rubric["PR-002"].source, "core");
  assert.equal(out.rubric["OV-001"].source, "overlay");
  for (const id of ["PR-002", "OV-001"]) assert.ok(out.rubric[id].body, `${id} の正本の本文がない`);
  assert.ok(!checkInput(C0006.defects, out.prompt, out.rubric).some((e) => e.rule === "unknown_rubric"));
});

test("(3) Engine の rubric.yaml から PR-002 を消した写しでは、todo にならずに外れる", { skip }, () => {
  // 1 つでも欠けたら落ちることを、本物の judge.py で確かめる（todo が core の欠けまで隠さないこと）
  const dir = mkdtempSync(join(tmpdir(), "judge-input-engine-"));
  cpSync(join(engineDir, "core-gates"), join(dir, "core-gates"), { recursive: true });
  const rubric = join(dir, "core-gates", "rubric.yaml");
  const src = readFileSync(rubric, "utf8");
  const cut = src.replace(/^ {2}- id: PR-002\n[\s\S]*?(?=^ {2}- id: )/m, "");
  assert.notEqual(cut, src, "rubric.yaml から PR-002 を切り出せない（rubric.yaml の形が変わったらこのテストを合わせる）");
  writeFileSync(rubric, cut);
  const out = engine(C0006, dir);
  // 正本の本文は元の Engine から引く（写しの rubric.yaml にはもうないので）
  const sources = engine(C0006).rubric;
  const errors = checkInput(C0006.defects, out.prompt, sources);
  assert.ok(errors.some((e) => e.rule === "missing_rubric_body" && e.rubric_id === "PR-002"), JSON.stringify(errors));
  assert.ok(!isLegacyRubric(errors, sources));
});
