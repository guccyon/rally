// 表の並べ替えと値の注記（specs/design/components.yaml の table_sort・value_note。0.11.0、#1493）。
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { expect, test, vi } from "vitest";

import {
  DataTable,
  TABLE_SORT_FAILED,
  nextTableSort,
  tableSortLabel,
  type DataTableColumn,
  type TableSort,
} from "@/components/ui/DataTable";
import { ValueNote } from "@/components/ui/ValueNote";

type Row = { id: string; start: string; coach: string; booked: number };

const ROWS: Row[] = [
  { id: "a", start: "10/28（水）12:00", coach: "山田 花子", booked: 9 },
  { id: "b", start: "10/28（水）9:00", coach: "佐藤 健", booked: 4 },
];

const COLUMNS: DataTableColumn<Row>[] = [
  { header: "開始", cell: (row) => row.start, sort: { key: "start", kind: "datetime" } },
  { header: "コーチ", cell: (row) => row.coach },
  { header: "予約された枠", variant: "numeric", cell: (row) => `${row.booked} 件`, sort: { key: "booked", kind: "count" } },
];

const NEWEST: TableSort = { key: "start", direction: "descending" };

test("並びの言い方は日時が新しい順・古い順、数が多い順・少ない順。最初は descending、同じ列は逆で、3 つ目の状態はない", () => {
  expect(tableSortLabel("開始", "datetime", "descending")).toBe("開始の新しい順");
  expect(tableSortLabel("開始", "datetime", "ascending")).toBe("開始の古い順");
  expect(tableSortLabel("予約された枠", "count", "descending")).toBe("予約された枠の多い順");
  expect(tableSortLabel("予約された枠", "count", "ascending")).toBe("予約された枠の少ない順");
  expect(nextTableSort(NEWEST, "booked")).toEqual({ key: "booked", direction: "descending" });
  expect(nextTableSort(NEWEST, "start")).toEqual({ key: "start", direction: "ascending" });
  expect(nextTableSort({ key: "start", direction: "ascending" }, "start")).toEqual(NEWEST);
});

test("押せる列だけ見出しが button で、今の並びの th にだけ aria-sort と矢印、ほかは unfold_more。表の上に件数と並びの文", async () => {
  const onSort = vi.fn();
  render(
    <DataTable
      label="予約"
      columns={COLUMNS}
      rows={ROWS}
      rowKey={(row) => row.id}
      sort={{ current: NEWEST, onSort, count: "12 件" }}
    />,
  );
  const headers = screen.getAllByRole("columnheader");
  expect(headers[0]).toHaveAttribute("aria-sort", "descending");
  expect(headers[0]).toHaveClass("text-text");
  expect(headers[0]?.querySelector('svg[data-icon="arrow_downward"]')).toHaveAttribute("aria-hidden", "true");
  expect(headers[1]).not.toHaveAttribute("aria-sort");
  expect(within(headers[1] as HTMLElement).queryByRole("button")).toBeNull();
  expect(headers[2]).not.toHaveAttribute("aria-sort");
  expect(headers[2]).toHaveClass("text-text-muted", "text-right");
  expect(headers[2]?.querySelector('svg[data-icon="unfold_more"]')).not.toBeNull();
  expect(screen.getByRole("status")).toHaveTextContent("12 件・開始の新しい順");

  // button の名前は見出しの字。押すと次の並びを渡し、行は画面の中で並べ替えない
  await userEvent.click(screen.getByRole("button", { name: "予約された枠" }));
  expect(onSort).toHaveBeenCalledWith({ key: "booked", direction: "descending" });
  await userEvent.click(screen.getByRole("button", { name: "開始" }));
  expect(onSort).toHaveBeenLastCalledWith({ key: "start", direction: "ascending" });
  expect(screen.getAllByRole("row").slice(1).map((row) => row.textContent)).toEqual([
    "10/28（水）12:00山田 花子9 件",
    "10/28（水）9:00佐藤 健4 件",
  ]);
});

test("読み直すあいだは全部の見出しを押せず、table に aria-busy と「〜に並べ替えています…」。終わると「〜に並べ替えました」を読む", async () => {
  const onSort = vi.fn();
  const { rerender } = render(
    <DataTable
      label="予約"
      columns={COLUMNS}
      rows={ROWS}
      rowKey={(row) => row.id}
      sort={{ current: { key: "start", direction: "ascending" }, onSort, count: "12 件", loading: true }}
    />,
  );
  expect(screen.getByRole("table")).toHaveAttribute("aria-busy", "true");
  expect(screen.getAllByRole("columnheader")[0]).toHaveAttribute("aria-sort", "ascending");
  expect(screen.getAllByRole("columnheader")[0]?.querySelector('svg[data-icon="arrow_upward"]')).not.toBeNull();
  for (const button of screen.getAllByRole("button")) {
    expect(button).toHaveAttribute("aria-disabled", "true");
    await userEvent.click(button);
  }
  expect(onSort).not.toHaveBeenCalled();
  expect(screen.getByRole("status")).toHaveTextContent("開始の古い順に並べ替えています…");
  // 行は前のまま
  expect(screen.getAllByRole("row")).toHaveLength(3);

  rerender(
    <DataTable
      label="予約"
      columns={COLUMNS}
      rows={ROWS}
      rowKey={(row) => row.id}
      sort={{ current: { key: "start", direction: "ascending" }, onSort, count: "12 件" }}
    />,
  );
  expect(screen.getByRole("table")).not.toHaveAttribute("aria-busy");
  expect(screen.getByRole("status")).toHaveTextContent("12 件・開始の古い順");
  expect(screen.getByRole("status")).toHaveTextContent("開始の古い順に並べ替えました");
});

test("失敗したら表の上に danger の知らせを出し、並びを前に戻して行は消さない", () => {
  function Screen() {
    const [sort] = useState<TableSort>(NEWEST);
    return (
      <DataTable
        label="予約"
        columns={COLUMNS}
        rows={ROWS}
        rowKey={(row) => row.id}
        sort={{ current: sort, onSort: () => {}, count: "12 件", error: TABLE_SORT_FAILED }}
      />
    );
  }
  render(<Screen />);
  expect(screen.getByRole("alert")).toHaveTextContent(TABLE_SORT_FAILED);
  expect(screen.getByRole("status")).toHaveTextContent("12 件・開始の新しい順");
  expect(screen.getByRole("status")).not.toHaveTextContent("並べ替えました");
  expect(screen.getAllByRole("row")).toHaveLength(3);
});

test("sort を渡さない表は今までどおり素の見出しで、表の上の文も出さない", () => {
  render(<DataTable label="予約" columns={COLUMNS} rows={ROWS} rowKey={(row) => row.id} />);
  expect(screen.queryByRole("button")).toBeNull();
  expect(screen.queryByRole("status")).toBeNull();
  expect(screen.getAllByRole("columnheader")[0]).not.toHaveAttribute("aria-sort");
});

test("値の注記は字の語で、数の列は値の左・ほかは値の右。語の意味は表の下に出し、table に aria-describedby で結ぶ", () => {
  const note = "（参考）は、公開した枠が 5 件未満で、割合が動きやすい値です。";
  render(
    <DataTable
      label="コーチごとの予約枠の需給"
      columns={[
        { header: "コーチ", cell: (row: Row) => <ValueNote note="（参考）">{row.coach}</ValueNote> },
        {
          header: "埋まった割合",
          variant: "numeric",
          cell: () => (
            <ValueNote note="（参考）" numeric>
              67%
            </ValueNote>
          ),
        },
      ]}
      rows={ROWS.slice(0, 1)}
      rowKey={(row) => row.id}
      notes={[note]}
    />,
  );
  const cells = screen.getAllByRole("cell");
  expect(cells[0]).toHaveTextContent(/^山田 花子（参考）$/);
  expect(cells[1]).toHaveTextContent(/^（参考）67%$/);
  const label = within(cells[1] as HTMLElement).getByText("（参考）");
  expect(label).toHaveClass("text-sm", "text-text-muted", "mr-1");
  expect(within(cells[0] as HTMLElement).getByText("（参考）")).toHaveClass("ml-1");

  const table = screen.getByRole("table");
  const describedBy = table.getAttribute("aria-describedby");
  expect(describedBy).toBeTruthy();
  expect(document.getElementById(describedBy ?? "")).toHaveTextContent(note);
  expect(document.getElementById(describedBy ?? "")).toHaveClass("mt-2", "text-sm", "text-text-muted");
});

test("注記がない表には表の下の説明と aria-describedby を出さない", () => {
  render(<DataTable label="予約" columns={COLUMNS} rows={ROWS} rowKey={(row) => row.id} notes={[]} />);
  expect(screen.getByRole("table")).not.toHaveAttribute("aria-describedby");
});
