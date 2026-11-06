import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import { OUT_DIR, buildHtml, readSpec, tokenVariables } from "./build.mjs";

const spec = readSpec();
const kebab = (name) => name.replaceAll("_", "-");
const html = buildHtml(spec);

test("置いてある catalog.html は今の正本から作ったものと同じ（正本を直したら npm run build:png で作り直す）", () => {
  assert.equal(readFileSync(join(OUT_DIR, "catalog.html"), "utf8"), html);
});

test("すべての部品に preview があり、カタログにすべての見本が出る", () => {
  for (const component of spec.components.components) {
    assert.ok(component.preview?.length > 0, component.id);
    assert.ok(html.includes(`id="c-${component.id}"`), component.id);
    for (const preview of component.preview) assert.ok(html.includes(preview.html), `${component.id} / ${preview.label}`);
  }
});

test("preview_css と preview が使う CSS の変数と書体の class は、すべて tokens.yaml から作ってある", () => {
  const vars = tokenVariables(spec.tokens);
  const used = new Set([...html.matchAll(/var\(--([\w-]+)\)/g)].map(([, name]) => name));
  for (const name of used) assert.ok(name in vars, name);
  assert.equal(vars["color-primary"], spec.tokens.color.roles.primary);
  assert.equal(vars["color-danger-fg"], spec.tokens.color.status.danger.fg);
  const steps = new Set(Object.keys(spec.tokens.typography.scale).map(kebab));
  const types = new Set([...html.matchAll(/class="[^"]*\btype-([\w-]+)/g)].map(([, name]) => name));
  for (const name of types) assert.ok(steps.has(name), name);
  assert.equal(vars["font-numeric"], spec.tokens.typography.families.numeric);
});

test("書体の見本は tokens.yaml の sample（日本語か、numeric の段は数字）で、Web フォントを読まない", () => {
  for (const [name, step] of Object.entries(spec.tokens.typography.scale)) {
    // numeric の段（日時と回数の数字）は数字の見本、ほかは日本語の見本
    assert.match(step.sample, step.family === "numeric" ? /\d/ : /[ぁ-んァ-ヶ一-龠]/, name);
    assert.ok(html.includes(`<div class="type-${kebab(name)}">${step.sample}</div>`), name);
  }
  assert.doesNotMatch(html, /@import|@font-face|fonts\.googleapis|<link /);
  assert.doesNotMatch(html, /https?:\/\//);
});
