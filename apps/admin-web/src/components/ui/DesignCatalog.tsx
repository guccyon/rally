"use client";

import { CATALOG, catalogEntryName } from "./catalog";

/**
 * デザインシステムのカタログ（`/design`）。部品 × 種類 × 状態を、部品ごとにすべて並べる。
 * 本番のメニューには出さない。部品が変わると画面のキャプチャ（id `design-catalog`）に出る。
 */
export function DesignCatalog() {
  const components = [...new Set(CATALOG.map((entry) => entry.component))];
  return (
    <main className="mx-auto max-w-3xl p-6">
      <h1 className="mb-2 text-xl font-bold">デザインシステムの部品</h1>
      <p className="mb-6 text-sm text-gray-600">
        specs/design/components.yaml の部品を、種類と状態ごとに並べたものです。画面はこの部品で組みます。
      </p>
      {components.map((component) => (
        <section key={component} aria-label={component} className="mb-8">
          <h2 className="mb-3 text-lg font-semibold">{component}</h2>
          <div className="space-y-4">
            {CATALOG.filter((entry) => entry.component === component).map((entry) => {
              const name = catalogEntryName(entry);
              return (
                <div key={name} className="space-y-1">
                  <h3 className="text-xs text-gray-500">{name}</h3>
                  {entry.render()}
                </div>
              );
            })}
          </div>
        </section>
      ))}
    </main>
  );
}
