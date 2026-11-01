// @vitest-environment node
// src/app/globals.css の @theme がデザイントークン（specs/design/tokens.yaml）の色と一致すること、
// 部品が Tailwind の既定の色を直書きせずトークンの名前を使うことを確かめる。
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { load } from "js-yaml";
import { describe, expect, test } from "vitest";

type Tokens = {
  tokens: Record<string, { value: string | number | null }>;
  apps: { "admin-web": { overrides: Record<string, string | number> } };
};

const tokens = load(
  readFileSync(new URL("../../../specs/design/tokens.yaml", import.meta.url), "utf8"),
) as Tokens;
const css = readFileSync(new URL("../src/app/globals.css", import.meta.url), "utf8");
const srcDir = fileURLToPath(new URL("../src", import.meta.url));

// tokens.yaml の冒頭の「アプリでの名前」の規則
function cssVariable(id: string): string {
  const [, group, ...rest] = id.split(".");
  if (group === "text") return `--text-color-${rest.join("-")}`;
  if (group === "surface") return `--background-color-${rest.join("-")}`;
  return `--color-${[group, ...rest].join("-")}`;
}

function themeVariables(): Map<string, string> {
  const block = /@theme\s*\{([^}]*)\}/.exec(css)?.[1] ?? "";
  return new Map([...block.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)].map((m) => [m[1]!, m[2]!.trim()]));
}

function sourceFiles(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return sourceFiles(path);
    return /\.(tsx?|css)$/.test(entry.name) ? [path] : [];
  });
}

describe("デザイントークン", () => {
  const overrides = tokens.apps["admin-web"].overrides;
  for (const id of Object.keys(overrides)) {
    test(`admin-web の上書き ${id} は tokens にある名前`, () => {
      expect(tokens.tokens).toHaveProperty([id]);
    });
  }

  test("@theme の色は tokens.yaml の色と同じ名前・同じ値で、過不足がない", () => {
    const expected = new Map(
      Object.entries(tokens.tokens)
        .filter(([id]) => id.startsWith("color."))
        .map(([id, token]) => [cssVariable(id), String(overrides[id] ?? token.value)]),
    );
    expect([...expected.values()]).not.toContain("null");
    expect(Object.fromEntries(themeVariables())).toEqual(Object.fromEntries(expected));
  });

  test("src に Tailwind の既定の色のクラスを直書きしない", () => {
    const palette =
      /\b(?:text|bg|border|ring|outline|divide|fill|stroke|from|via|to|placeholder|decoration)-(?:slate|gray|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose|white|black)(?:-\d+)?\b/g;
    const found = sourceFiles(srcDir).flatMap((file) =>
      [...readFileSync(file, "utf8").matchAll(palette)].map((m) => `${file.slice(srcDir.length + 1)}: ${m[0]}`),
    );
    expect(found).toEqual([]);
  });
});
