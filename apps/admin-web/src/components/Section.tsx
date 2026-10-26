import type { ReactNode } from "react";

export function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section aria-label={title} className="mb-8 rounded border p-4">
      <h2 className="mb-3 text-lg font-semibold">{title}</h2>
      {children}
    </section>
  );
}

/** 契約に API がまだない機能の表示。 */
export function NotImplemented({ children }: { children: ReactNode }) {
  return <p className="rounded bg-gray-100 p-2 text-sm text-gray-600">未実装: {children}</p>;
}
