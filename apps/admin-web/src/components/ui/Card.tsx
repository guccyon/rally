import type { ReactNode } from "react";

/** カード（components.yaml の card）。ひとまとまりの情報を枠で囲む。 */
export function Card({ label, children }: { label?: string; children: ReactNode }) {
  return (
    <div aria-label={label} role={label ? "group" : undefined} className="space-y-2 rounded border p-3">
      {children}
    </div>
  );
}
