"use client";

import { useEffect, useRef, useState, type ReactNode } from "react";

/** 列の種類（specs/design/components.yaml の data_table.variants）。 */
export type DataTableVariant = "text" | "numeric" | "long_text";

export type DataTableColumn<Row> = {
  header: string;
  variant?: DataTableVariant;
  cell: (row: Row) => ReactNode;
};

/** 見出しとセルに共通の余白。左右 sm 8px で列のあいだは md 16px、先頭の左と最後の右は 0（Section の padding にそろえる）。 */
const EDGE_CLASSES = "px-2 py-2 first:pl-0 last:pr-0";

/** 種類ごとの寄せと折り返し。折り返してよいのは long_text の列だけ（文節で折る）。 */
const VARIANT_CLASSES: Record<DataTableVariant, string> = {
  text: "whitespace-nowrap",
  numeric: "whitespace-nowrap text-right tabular-nums",
  long_text: "min-w-[12em] max-w-[40em] whitespace-normal [word-break:auto-phrase]",
};

/**
 * 表（specs/design/components.yaml の data_table）。並べ方は渡した順のまま（API が返した順）。
 * 幅に収まらないときは包みだけが横に動き、ページ全体は動かない。包みが横に動くときは role="region" と
 * aria-label と tabindex="0" を付け、キーボードの矢印でも動かせるようにする。
 * 行全体は押せるようにしない（詳細へは cell にリンクを置く）。急ぎの行は highlightRow で danger.container の面にする
 * （色だけで伝えないよう、状態のセルに文字のしるしを置く）。
 */
export function DataTable<Row>({
  label,
  columns,
  rows,
  rowKey,
  highlightRow,
}: {
  label: string;
  columns: DataTableColumn<Row>[];
  rows: Row[];
  rowKey: (row: Row) => string;
  highlightRow?: (row: Row) => boolean;
}) {
  const wrapper = useRef<HTMLDivElement>(null);
  const [scrollable, setScrollable] = useState(false);

  useEffect(() => {
    const element = wrapper.current;
    if (!element) return;
    const update = () => setScrollable(element.scrollWidth > element.clientWidth);
    update();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(update);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  return (
    <div
      ref={wrapper}
      className="max-w-full overflow-x-auto"
      {...(scrollable ? { role: "region", "aria-label": label, tabIndex: 0 } : {})}
    >
      <table aria-label={label} className="w-full min-w-max border-collapse text-left text-sm">
        <thead>
          <tr>
            {columns.map((column) => (
              <th
                key={column.header}
                scope="col"
                className={`${EDGE_CLASSES} whitespace-nowrap border-b align-bottom font-bold ${
                  column.variant === "numeric" ? "text-right" : ""
                }`}
              >
                {column.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={rowKey(row)} className={highlightRow?.(row) ? "bg-danger-container" : undefined}>
              {columns.map((column) => (
                <td
                  key={column.header}
                  className={`${EDGE_CLASSES} h-12 border-t align-middle ${VARIANT_CLASSES[column.variant ?? "text"]}`}
                >
                  {column.cell(row)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
