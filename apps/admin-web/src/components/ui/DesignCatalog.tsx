"use client";

import { CATALOG, NOT_IN_ADMIN_WEB, catalogEntryName } from "./catalog";

/**
 * デザインシステムのカタログ（`/design`）。specs/design/components.yaml の部品を、種類と状態ごとに部品ごとに並べる。
 * 本番のメニューには出さない。部品が変わると画面のキャプチャ（id `design-catalog`）に出る。
 */
export function DesignCatalog() {
  const components = [...new Set(CATALOG.map((entry) => entry.component))];
  return (
    <main className="mx-auto max-w-3xl p-6">
      <h1 className="mb-2 text-headline font-black">デザインシステムの部品</h1>
      <p className="mb-2 text-sm text-text-muted">
        specs/design/components.yaml の部品を、種類と状態ごとに並べたものです。画面はこの部品で組みます。
      </p>
      <ul aria-label="admin-web で使わない部品と種類" className="mb-10 list-disc pl-6 text-sm text-text-muted">
        {NOT_IN_ADMIN_WEB.map((item) => (
          <li key={[item.component, item.variant].filter(Boolean).join(" / ")}>
            {[item.component, item.variant].filter(Boolean).join(" / ")}: {item.reason}
          </li>
        ))}
      </ul>
      {components.map((component) => (
        <section key={component} aria-label={component} className="mb-10">
          <h2 className="mb-2 text-xl font-bold">{component}</h2>
          <div className="space-y-4">
            {CATALOG.filter((entry) => entry.component === component).map((entry) => {
              const name = catalogEntryName(entry);
              return (
                <div key={name} className="space-y-1">
                  <h3 className="text-xs text-text-muted">{name}</h3>
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
