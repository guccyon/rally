import type { ReactNode } from "react";

/**
 * 欄（specs/design/patterns.yaml の common_layout.admin_web）。background のまま面で囲わず、title（17px・900）の見出しの下に中身。
 * 欄と欄のあいだは 40px（0.5.0 で surface の面・md の角丸・padding 16px をやめた）。
 */
export function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section aria-label={title} className="mb-10">
      <h2 className="mb-2 text-xl font-black">{title}</h2>
      {children}
    </section>
  );
}
