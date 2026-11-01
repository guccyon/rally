// デザインシステムの部品（src/components/ui/）とカタログ（/design）が specs/design/ の正本と合っていること。
// components.yaml に部品・種類・状態を足したら、部品とカタログ（src/components/ui/catalog.tsx）にも足す。
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, test, vi } from "vitest";
import { parse } from "yaml";

import { Button } from "@/components/ui/Button";
import { ConfirmStep } from "@/components/ui/ConfirmStep";
import { DesignCatalog } from "@/components/ui/DesignCatalog";
import { TextInput } from "@/components/ui/Input";
import { CATALOG, catalogEntryName } from "@/components/ui/catalog";

type Spec = {
  state_vocabulary: { id: string }[];
  components: { id: string; variants?: { id: string }[]; states: string[]; tokens: string[] }[];
};
type Patterns = {
  patterns: { id: string; layout: { component: string; variant?: string }[]; components: string[] }[];
};

// テストは apps/admin-web で動く（npm test）
const specPath = (name: string) => resolve(process.cwd(), "../../specs/design", name);
const spec = parse(readFileSync(specPath("components.yaml"), "utf8")) as Spec;
const patterns = parse(readFileSync(specPath("patterns.yaml"), "utf8")) as Patterns;

/** components.yaml から、部品 × 種類 × 状態の名前の全部。 */
function expectedEntries(): string[] {
  return spec.components.flatMap((component) =>
    (component.variants ?? [undefined]).flatMap((variant) =>
      component.states.map((state) => catalogEntryName({ component: component.id, variant: variant?.id, state })),
    ),
  );
}

describe("components.yaml とカタログの対応", () => {
  test("カタログは components.yaml の部品 × 種類 × 状態をちょうど 1 つずつ持つ", () => {
    const actual = CATALOG.map(catalogEntryName);
    expect(new Set(actual).size).toBe(actual.length);
    expect([...actual].sort()).toEqual(expectedEntries().sort());
  });

  test("状態は state_vocabulary の id だけを使う", () => {
    const vocabulary = new Set(spec.state_vocabulary.map((state) => state.id));
    for (const component of spec.components) {
      for (const state of component.states) expect(vocabulary, `${component.id} の ${state}`).toContain(state);
    }
  });

  test("/design のページに、部品ごとの欄とすべての枠の見出しが出る", () => {
    render(<DesignCatalog />);
    for (const component of spec.components) {
      expect(screen.getByRole("region", { name: component.id })).toBeInTheDocument();
    }
    for (const name of expectedEntries()) {
      expect(screen.getByRole("heading", { level: 3, name })).toBeInTheDocument();
    }
  });

  test.skipIf(!existsSync(specPath("tokens.yaml")))("使うトークンは tokens.yaml にある", () => {
    // tokens.yaml の形に依らず、入れ子のキーをつないだ名前（color.action.primary）と id の値をトークンの名前として集める
    const names = new Set<string>();
    const walk = (node: unknown, path: string[]) => {
      if (Array.isArray(node)) return node.forEach((item) => walk(item, path));
      if (node === null || typeof node !== "object") return;
      for (const [key, value] of Object.entries(node)) {
        if (key === "id" && typeof value === "string") names.add(value);
        names.add([...path, key].join("."));
        walk(value, [...path, key]);
      }
    };
    walk(parse(readFileSync(specPath("tokens.yaml"), "utf8")), []);
    for (const component of spec.components) {
      for (const token of component.tokens) expect(names, `${component.id} の ${token}`).toContain(token);
    }
  });
});

describe("patterns.yaml", () => {
  const components = new Map(spec.components.map((component) => [component.id, component]));

  test.each(patterns.patterns.map((pattern) => [pattern.id, pattern] as const))(
    "%s の部品は components.yaml にあり、並びに出る部品と一致する",
    (_id, pattern) => {
      for (const step of pattern.layout) {
        const component = components.get(step.component);
        expect(component, step.component).toBeDefined();
        if (step.variant) expect(component?.variants?.map((variant) => variant.id)).toContain(step.variant);
      }
      expect([...new Set(pattern.layout.map((step) => step.component))].sort()).toEqual([...pattern.components].sort());
    },
  );
});

describe("部品の振る舞い", () => {
  test("処理中のボタンは押せず、文言が処理中のものになる", () => {
    render(
      <Button busy busyLabel="送っています…">
        送るのを確定
      </Button>,
    );
    const button = screen.getByRole("button", { name: "送っています…" });
    expect(button).toBeDisabled();
    expect(button).toHaveAttribute("aria-busy", "true");
  });

  test("誤りのある入力は aria-invalid で、項目の下の理由を指す", () => {
    render(<TextInput label="題" error="題を入れてください。" />);
    const input = screen.getByRole("textbox", { name: "題" });
    expect(input).toHaveAttribute("aria-invalid", "true");
    expect(input).toHaveAccessibleDescription("題を入れてください。");
  });

  test("確認の段は内容を見せ、確定と直すを呼ぶ。送っている間は両方押せない", async () => {
    const onConfirm = vi.fn();
    const onCancel = vi.fn();
    const { rerender } = render(
      <ConfirmStep
        label="送る前の確認"
        message="次の内容で送りますか？"
        items={[{ term: "題", detail: "お知らせ" }]}
        confirmLabel="送るのを確定"
        busyLabel="送っています…"
        onConfirm={onConfirm}
        onCancel={onCancel}
      />,
    );
    const group = screen.getByRole("group", { name: "送る前の確認" });
    expect(within(group).getByText("お知らせ")).toBeInTheDocument();
    await userEvent.click(within(group).getByRole("button", { name: "送るのを確定" }));
    await userEvent.click(within(group).getByRole("button", { name: "直す" }));
    expect(onConfirm).toHaveBeenCalledOnce();
    expect(onCancel).toHaveBeenCalledOnce();

    rerender(
      <ConfirmStep
        label="送る前の確認"
        message="次の内容で送りますか？"
        confirmLabel="送るのを確定"
        busyLabel="送っています…"
        busy
        onConfirm={onConfirm}
        onCancel={onCancel}
      />,
    );
    expect(screen.getByRole("button", { name: "送っています…" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "直す" })).toBeDisabled();
  });
});
