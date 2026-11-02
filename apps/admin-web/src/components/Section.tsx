import type { ReactNode } from "react";

/** 欄（specs/design/patterns.yaml の common_layout.admin_web）。surface の面・md の角丸・padding 16px で、欄と欄のあいだは 40px。 */
export function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section aria-label={title} className="mb-10 rounded-md bg-surface p-4">
      <h2 className="mb-2 text-xl font-bold">{title}</h2>
      {children}
    </section>
  );
}

/** 契約に API がまだない機能の表示。 */
export function NotImplemented({ children }: { children: ReactNode }) {
  return <p className="rounded-md bg-surface p-2 text-sm text-text-muted">未実装: {children}</p>;
}
