import type { ReactNode } from "react";

/** 一覧の行（ListRow）を並べる枠。label は一覧の名前（支援技術が読む）。 */
export function List({ label, children }: { label: string; children: ReactNode }) {
  return (
    <ul aria-label={label} className="divide-y rounded border text-sm">
      {children}
    </ul>
  );
}
