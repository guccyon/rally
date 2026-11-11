// デザインシステムのカタログ（/design）が specs/design/components.yaml の部品と合っていること。
// components.yaml に部品・種類・状態を足したら、カタログ（src/components/ui/catalog.tsx）にも足す。
// admin-web で使わない部品と種類は catalog.tsx の NOT_IN_ADMIN_WEB に理由とともに書く。部品そのものの振る舞いは designComponents.test.tsx。
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { render, screen } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import { parse } from "yaml";

import { DesignCatalog } from "@/components/ui/DesignCatalog";
import { CATALOG, NOT_IN_ADMIN_WEB, catalogEntryName } from "@/components/ui/catalog";

type Keys = Record<string, unknown> | string[] | undefined;
type Spec = { components: { id: string; variants?: Keys; states?: Keys }[] };

// テストは apps/admin-web で動く（npm test）
const spec = parse(readFileSync(resolve(process.cwd(), "../../specs/design/components.yaml"), "utf8")) as Spec;

/** components.yaml の variants・states は、辞書ならそのキー、配列ならその要素が名前。 */
const keysOf = (node: Keys): string[] => (Array.isArray(node) ? node : Object.keys(node ?? {}));

const notInApp = (component: string, variant?: string) =>
  NOT_IN_ADMIN_WEB.some((item) => item.component === component && item.variant === variant);

describe("components.yaml とカタログの対応", () => {
  const components = new Map(spec.components.map((component) => [component.id, component]));

  test("admin-web で使う部品と種類は、すべてカタログに見本がある", () => {
    for (const component of spec.components) {
      if (notInApp(component.id)) continue;
      const entries = CATALOG.filter((entry) => entry.component === component.id);
      expect(entries.length, component.id).toBeGreaterThan(0);
      for (const variant of keysOf(component.variants)) {
        if (notInApp(component.id, variant)) continue;
        expect(
          entries.some((entry) => entry.variant === variant),
          `${component.id} / ${variant}`,
        ).toBe(true);
      }
    }
  });

  test("カタログの部品・種類・状態は components.yaml にあるものだけで、枠の名前は重ならない", () => {
    for (const entry of CATALOG) {
      const component = components.get(entry.component);
      expect(component, entry.component).toBeDefined();
      if (entry.variant) expect(keysOf(component?.variants), catalogEntryName(entry)).toContain(entry.variant);
      // default は「状態を付けない見た目」。components.yaml の states に書いていなくてよい
      if (entry.state && entry.state !== "default") {
        expect(keysOf(component?.states), catalogEntryName(entry)).toContain(entry.state);
      }
    }
    const names = CATALOG.map(catalogEntryName);
    expect(new Set(names).size).toBe(names.length);
  });

  test("使わないとした部品と種類は components.yaml にあり、カタログには置かない", () => {
    for (const item of NOT_IN_ADMIN_WEB) {
      const component = components.get(item.component);
      expect(component, item.component).toBeDefined();
      if (item.variant) expect(keysOf(component?.variants)).toContain(item.variant);
      expect(
        CATALOG.some(
          (entry) => entry.component === item.component && (!item.variant || entry.variant === item.variant),
        ),
        [item.component, item.variant].filter(Boolean).join(" / "),
      ).toBe(false);
    }
  });

  test("/design のページに、部品ごとの欄とすべての枠の見出しが出る", () => {
    render(<DesignCatalog />);
    // getByRole を枠ごとに呼ぶとページ全体のアクセシビリティツリーを毎回なめ直し、CI で 5 秒を超える。
    // 欄と見出しはそれぞれ 1 回だけ集め、名前ごとにちょうど 1 つあることを確かめる（getByRole と同じ判定）。
    // 名前は aria-label があればそれ、なければ中身の文字（欄は aria-label、h3 は中身で名前が付く）
    const countByName = (elements: HTMLElement[]) => {
      const counts = new Map<string, number>();
      for (const element of elements) {
        const name = (element.getAttribute("aria-label") ?? element.textContent ?? "").trim();
        counts.set(name, (counts.get(name) ?? 0) + 1);
      }
      return counts;
    };
    const regions = countByName(screen.getAllByRole("region"));
    for (const component of new Set(CATALOG.map((entry) => entry.component))) {
      expect(regions.get(component), component).toBe(1);
    }
    const headings = countByName(screen.getAllByRole("heading", { level: 3 }));
    for (const entry of CATALOG) {
      expect(headings.get(catalogEntryName(entry)), catalogEntryName(entry)).toBe(1);
    }
  });
});
