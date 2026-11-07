// judge のジョブの入力に、検査役の指摘が引く rubric の本文が入っているかの線（知見 K-0011 の担保。議決 D-0033、Issue #1054）。LLM を使わない。
//
// 線: judge の入力（エージェントに渡すプロンプト）に、指摘（defects）が引くすべての rubric_id（PR-002・OV-001 など）について、
// 正本の rubric の本文（Engine の core-gates/rubric.yaml の check、製品の overlay のルールの check.text）が入っている。1 つでも欠けたら外れる。
//
// 見ること:
//   - rubric_id の正本の本文が見つからない（unknown_rubric。本文を確かめられないので通さない）
//   - 正本の本文が入力にない（missing_rubric_body）
// 入力の中の本文は JSON に埋め込まれていることがあるので、空白と JSON の逃がし（\n・\"・\\）を落として比べる。

/** 指摘が引く rubric_id（出た順に、同じ id は 1 回だけ。rubric_id のない指摘は数えない） */
export function requiredRubricIds(defects) {
  return [...new Set((defects ?? []).map((d) => d?.rubric_id).filter(Boolean))];
}

/** 比べるための形。空白と JSON の逃がしを落とす */
export function normalize(text) {
  return String(text ?? "")
    .replace(/\\[nrt]/g, "")
    .replace(/\\(["\\/])/g, "$1")
    .replace(/\s+/g, "");
}

/**
 * 入力が線を満たすか。外れたものを [{ rule, rubric_id, source }] で返す（空なら通る）。
 * defects: judge の入力の defects。prompt: judge が組んだ入力（エージェントに渡すプロンプト）。
 * sources: rubric_id ごとの正本 { source: "core" | "overlay" | null, body: string | null }（engine_harness.py が返す）
 */
export function checkInput(defects, prompt, sources) {
  const hay = normalize(prompt);
  const errors = [];
  for (const id of requiredRubricIds(defects)) {
    const src = sources?.[id];
    const body = normalize(src?.body);
    if (!src?.source || !body) errors.push({ rule: "unknown_rubric", rubric_id: id, source: src?.source ?? null });
    else if (!hay.includes(body)) errors.push({ rule: "missing_rubric_body", rubric_id: id, source: src.source });
  }
  return errors;
}
