// @vitest-environment node
// globals.css の @theme が正本（specs/design/tokens.yaml）の色・角丸・書体と同じ値を持ち、Tailwind の既定の色を残していないこと。
import { readFileSync } from "node:fs";

import { expect, test } from "vitest";

const tokens = readFileSync(new URL("../../../specs/design/tokens.yaml", import.meta.url), "utf8");
const css = readFileSync(new URL("../src/app/globals.css", import.meta.url), "utf8");

const kebab = (name: string) => name.replaceAll("_", "-");

function cssVar(name: string): string | undefined {
  return css.match(new RegExp(`--${name}:\\s*([^;]+);`))?.[1]?.trim().toLowerCase();
}

function section(start: string, end: string): string {
  const from = tokens.indexOf(start);
  return tokens.slice(from, tokens.indexOf(end, from));
}

test("色の役割は tokens.yaml の color.roles と同じ", () => {
  const roles = [...section("  roles:", "  status:").matchAll(/^ {4}(\w+): "(#[0-9A-F]{6})"/gm)];
  expect(roles.length).toBe(17);
  for (const [, name, hex] of roles) {
    expect(cssVar(`color-${kebab(name ?? "")}`), name).toBe(hex?.toLowerCase());
  }
  expect(cssVar("color-scrim")).toBe("rgba(16, 19, 26, 0.56)");
});

test("状態の色は tokens.yaml の color.status と同じ", () => {
  const statuses = [
    ...section("  status:", "  status_rule:").matchAll(
      /^ {4}(\w+): +\{fg: "(#\w+)", container: "(#\w+)", on_container: "(#\w+)"\}/gm,
    ),
  ];
  expect(statuses.map(([, name]) => name)).toEqual(["info", "success", "warning", "danger"]);
  for (const [, name, fg, container, onContainer] of statuses) {
    expect(cssVar(`color-${name}`), name).toBe(fg?.toLowerCase());
    expect(cssVar(`color-${name}-container`), name).toBe(container?.toLowerCase());
    expect(cssVar(`color-${name}-on-container`), name).toBe(onContainer?.toLowerCase());
  }
});

test("角丸と書体は tokens.yaml と同じで、Tailwind の既定の色・角丸・影は消してある", () => {
  expect(tokens).toMatch(/radius_px: \{sm: 6, md: 12, pill: 999\}/);
  expect(cssVar("radius-sm")).toBe("6px");
  expect(cssVar("radius-md")).toBe("12px");
  expect(cssVar("radius-pill")).toBe("999px");
  expect(cssVar("font-sans")).toBe('"noto sans cjk jp", "noto sans jp", sans-serif');
  expect(cssVar("spacing")).toBe("4px");
  for (const reset of ["--color-*: initial", "--radius-*: initial", "--shadow-*: initial"]) {
    expect(css).toContain(reset);
  }
  // Web フォントを読まない
  expect(css).not.toMatch(/@import url|fonts\.googleapis/);
});
