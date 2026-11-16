"use client";

import { useEffect, useId, useRef, useState, type ReactNode } from "react";

import { Icon } from "./Icon";
import { Notice } from "./Notice";

/** 列の種類（specs/design/components.yaml の data_table.variants）。 */
export type DataTableVariant = "text" | "numeric" | "long_text";

/**
 * 並べ方の値の種類（specs/design/components.yaml の table_sort.directions・wording）。
 * datetime は「新しい順・古い順」、count は「多い順・少ない順」。どちらも最初に押すと descending（新しい順・多い順）。
 */
export type TableSortKind = "datetime" | "count";

export type TableSortDirection = "ascending" | "descending";

/** 今の並び。key は API の並べ方の引数に渡す値（画面の中で行を並べ替えない）。 */
export type TableSort = { key: string; direction: TableSortDirection };

export type DataTableColumn<Row> = {
  header: string;
  variant?: DataTableVariant;
  cell: (row: Row) => ReactNode;
  /** API に並べ方の引数がある列だけ付ける（table_sort）。ない列は素の見出し。 */
  sort?: { key: string; kind: TableSortKind };
};

/**
 * 見出しで並べ替える表の並び（table_sort）。並べ替えは onSort で API から読み直す。
 * 読み直すあいだは current を押した列の新しい向きにして loading、失敗したら current を前の並びに戻して error を渡す。
 */
export type DataTableSortProps = {
  current: TableSort;
  onSort: (next: TableSort) => void;
  /** 表の上の order_text の前に置く件数の文（「12 件」）。 */
  count?: string;
  loading?: boolean;
  error?: string;
};

/** table_sort.errors.sort_failed の文言。 */
export const TABLE_SORT_FAILED = "並べ替えられませんでした。もう一度見出しを押すと、読み直します。";

const SORT_WORDS: Record<TableSortKind, Record<TableSortDirection, string>> = {
  datetime: { descending: "新しい順", ascending: "古い順" },
  count: { descending: "多い順", ascending: "少ない順" },
};

/** 並びの言い方（「開始の新しい順」）。「昇順」「降順」「ソート」を使わない。 */
export function tableSortLabel(header: string, kind: TableSortKind, direction: TableSortDirection): string {
  return `${header}の${SORT_WORDS[kind][direction]}`;
}

/** 見出しを押したときの次の並び。同じ列なら逆、ほかの列なら descending（並べない 3 つ目の状態は作らない）。 */
export function nextTableSort(current: TableSort, key: string): TableSort {
  if (current.key !== key) return { key, direction: "descending" };
  return { key, direction: current.direction === "descending" ? "ascending" : "descending" };
}

/** 見出しとセルに共通の余白。左右 sm 8px で列のあいだは md 16px、先頭の左と最後の右は 0（Section の見出しと左をそろえる）。 */
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
 * sort を渡すと、sort の付いた列の見出しを押して並べ替えられる（table_sort）。今の並びは矢印と aria-sort と表の上の order_text で示す。
 * notes は値の注記（value_note）の語の意味を 1 語 1 文で表の下に出し、table に aria-describedby で結ぶ。
 * 注記の付いた値（ValueNote）があるときだけ渡す。
 */
export function DataTable<Row>({
  label,
  columns,
  rows,
  rowKey,
  highlightRow,
  sort,
  notes,
}: {
  label: string;
  columns: DataTableColumn<Row>[];
  rows: Row[];
  rowKey: (row: Row) => string;
  highlightRow?: (row: Row) => boolean;
  sort?: DataTableSortProps;
  notes?: string[];
}) {
  const wrapper = useRef<HTMLDivElement>(null);
  const [scrollable, setScrollable] = useState(false);
  const noteId = useId();
  const sorted = sort ? columns.find((column) => column.sort?.key === sort.current.key) : undefined;
  const sortedLabel =
    sort && sorted?.sort ? tableSortLabel(sorted.header, sorted.sort.kind, sort.current.direction) : undefined;
  const announcement = useSortAnnouncement(sort, sortedLabel);

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

  const table = (
    <div
      ref={wrapper}
      className="max-w-full overflow-x-auto"
      {...(scrollable ? { role: "region", "aria-label": label, tabIndex: 0 } : {})}
    >
      <table
        aria-label={label}
        aria-busy={sort?.loading || undefined}
        aria-describedby={notes?.length ? noteId : undefined}
        className="w-full min-w-max border-collapse text-left text-sm"
      >
        <thead>
          <tr>
            {columns.map((column) =>
              sort && column.sort ? (
                <SortHeader key={column.header} column={column} sort={sort} />
              ) : (
                <th
                  key={column.header}
                  scope="col"
                  className={`${EDGE_CLASSES} ${HEADER_CLASSES} text-text-muted ${
                    column.variant === "numeric" ? "text-right" : ""
                  }`}
                >
                  {column.header}
                </th>
              ),
            )}
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

  if (!sort && !notes?.length) return table;
  return (
    <div>
      {sort?.error && (
        <div className="mb-2">
          <Notice message={sort.error} status="danger" />
        </div>
      )}
      {sort && sortedLabel && (
        <p role="status" className="mb-2 text-sm text-text-muted">
          {sort.loading
            ? `${sortedLabel}に並べ替えています…`
            : [sort.count, sortedLabel].filter(Boolean).join("・")}
          {announcement && <span className="sr-only">{announcement}</span>}
        </p>
      )}
      {table}
      {notes && notes.length > 0 && (
        <div id={noteId} className="mt-2 text-sm text-text-muted">
          {notes.map((note) => (
            <p key={note}>{note}</p>
          ))}
        </div>
      )}
    </div>
  );
}

/** 見出しのセルの字（small 700）。並べ替えの見出しも余白と字は同じ。 */
const HEADER_CLASSES = "whitespace-nowrap border-b-2 border-b-primary align-bottom font-bold";

/**
 * 並べ替えの見出し（table_sort）。th の中の button の名前は見出しの字、印は読み上げない。
 * 押下は th の面を primary_container、フォーカスは focus_ring 2px を th の内側に。読み直すあいだは全部を押せなくする。
 */
function SortHeader<Row>({ column, sort }: { column: DataTableColumn<Row>; sort: DataTableSortProps }) {
  const key = column.sort?.key ?? "";
  const current = sort.current.key === key;
  const disabled = sort.loading === true;
  const icon = !current ? "unfold_more" : sort.current.direction === "descending" ? "arrow_downward" : "arrow_upward";
  return (
    <th
      scope="col"
      aria-sort={current ? sort.current.direction : undefined}
      className={`${EDGE_CLASSES} ${HEADER_CLASSES} has-[button:active]:bg-primary-container has-[button:focus-visible]:outline-2 has-[button:focus-visible]:-outline-offset-2 has-[button:focus-visible]:outline-focus-ring ${
        current && !disabled ? "text-text" : "text-text-muted"
      } ${column.variant === "numeric" ? "text-right" : ""}`}
    >
      <button
        type="button"
        aria-disabled={disabled || undefined}
        onClick={() => {
          if (!disabled) sort.onSort(nextTableSort(sort.current, key));
        }}
        className="inline-flex min-h-12 items-center gap-1 font-bold focus-visible:outline-none"
      >
        {column.header}
        <Icon name={icon} size={16} className={current && !disabled ? "text-text" : "text-text-muted"} />
      </button>
    </th>
  );
}

/** 読み直しが終わって失敗でなければ、「開始の古い順に並べ替えました」を 1 回読む（role=status の中の見えない字）。 */
function useSortAnnouncement(sort: DataTableSortProps | undefined, sortedLabel: string | undefined): string {
  const loading = sort?.loading === true;
  const failed = sort?.error !== undefined;
  const [wasLoading, setWasLoading] = useState(loading);
  const [announcement, setAnnouncement] = useState("");
  if (wasLoading !== loading) {
    setWasLoading(loading);
    setAnnouncement(!loading && !failed && sortedLabel ? `${sortedLabel}に並べ替えました` : "");
  }
  return announcement;
}
