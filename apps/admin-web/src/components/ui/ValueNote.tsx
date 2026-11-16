import type { ReactNode } from "react";

/**
 * 値の注記（specs/design/components.yaml の value_note）。そのまま読むと誤る値に、small text_muted の短い語（「（参考）」）を添える。
 * 値の色と太さは変えない。数の列（numeric）は値の左に置いて数字の右端をそろえ、ほかは値の右。あいだは xs 4px。
 * 語は API が印を返した値にだけ付け、しきい値を画面で計算しない。語の意味は DataTable の notes で表の下に 1 文で書く。
 * tooltip に隠さず、見える字のまま値と一緒に読む。
 */
export function ValueNote({
  note,
  numeric = false,
  children,
}: {
  note: string;
  numeric?: boolean;
  children: ReactNode;
}) {
  const label = <span className={`text-sm text-text-muted ${numeric ? "mr-1" : "ml-1"}`}>{note}</span>;
  return (
    <>
      {numeric && label}
      {children}
      {!numeric && label}
    </>
  );
}
