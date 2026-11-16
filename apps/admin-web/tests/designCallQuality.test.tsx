// 通話品質の状態のタグと、行を選ぶ表（specs/design/components.yaml の status_tag.call_quality_state と data_table の selectable。#1527）。
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";
import { parse } from "yaml";

import { DataTable } from "@/components/ui/DataTable";
import { StatusTag } from "@/components/ui/StatusTag";
import { CALL_QUALITY_STATE_STATUS } from "@/components/ui/status";

type Spec = { components: { id: string; call_quality_state?: Record<string, string> }[] };
type Glossary = { terms: { id: string; states?: Record<string, string> }[] };

// テストは apps/admin-web で動く（npm test）
const read = (path: string) => parse(readFileSync(resolve(process.cwd(), "../../specs", path), "utf8")) as unknown;
const components = read("design/components.yaml") as Spec;
const glossary = read("glossary.yaml") as Glossary;

test("通話品質の色の対応は components.yaml の call_quality_state と同じで、danger は low だけ（state-color-map）", () => {
  const spec = components.components.find((component) => component.id === "status_tag")?.call_quality_state;
  expect(CALL_QUALITY_STATE_STATUS).toEqual(spec);
  expect(Object.entries(CALL_QUALITY_STATE_STATUS).filter(([, status]) => status === "danger")).toEqual([
    ["low", "danger"],
  ]);
});

test("通話品質の値は glossary の call_quality_score.states と同じキーを持つ", () => {
  const states = glossary.terms.find((term) => term.id === "call_quality_score")?.states;
  expect(Object.keys(states ?? {}).sort()).toEqual(Object.keys(CALL_QUALITY_STATE_STATUS).sort());
});

test.each([
  ["low", "低品質", "bg-danger", "text-text"],
  ["fair", "やや低い", "bg-warning", "text-text"],
  ["ok", "問題なし", "bg-border-strong", "text-text-muted"],
  ["no_data", "指標なし", "bg-border-strong", "text-text-muted"],
] as const)("通話品質 %s は「%s」を点（%s）と字で出し、urgent にしない", (state, label, dot, text) => {
  render(<StatusTag label={label} status={CALL_QUALITY_STATE_STATUS[state]} />);
  expect(screen.getByText(label)).toHaveClass(text);
  expect(screen.getByTestId("status-dot")).toHaveClass(dot);
});

type Row = { coach: string; score: number; calls: number; urgent?: boolean };

const ROWS: Row[] = [
  { coach: "佐藤 健", score: 62, calls: 14, urgent: true },
  { coach: "山田 花子", score: 74, calls: 9, urgent: true },
  { coach: "鈴木 一郎", score: 90, calls: 0 },
];

function renderTable(selected: string | undefined, onSelect = vi.fn()) {
  render(
    <DataTable
      label="コーチごとの通話品質（直近 30 日）"
      columns={[
        { header: "コーチ", cell: (row: Row) => row.coach },
        { header: "品質スコア", variant: "numeric", cell: (row: Row) => row.score },
      ]}
      rows={ROWS}
      rowKey={(row) => row.coach}
      highlightRow={(row) => row.urgent === true}
      selection={{ selected, onSelect, name: (row) => row.coach, canSelect: (row) => row.calls > 0 }}
    />,
  );
  return onSelect;
}

/** 本文の行（見出しの行を除く）と、その先頭のセル。 */
function bodyRows() {
  return screen.getAllByRole("row").slice(1).map((row) => ({ row, first: row.querySelector("td") }));
}

test("選んだ行は 1 つだけ primary_container の面と先頭の 4px の線と aria-selected で示し、row_highlight より優先する", () => {
  renderTable("山田 花子");
  const [first, second, third] = bodyRows();
  expect(second?.row).toHaveAttribute("aria-selected", "true");
  expect(second?.row).toHaveClass("bg-primary-container");
  expect(second?.row).not.toHaveClass("bg-danger-container");
  expect(second?.first).toHaveClass("border-l-4", "border-l-primary", "pl-1");

  expect(first?.row).not.toHaveAttribute("aria-selected");
  expect(first?.row).toHaveClass("bg-danger-container");
  expect(first?.first).toHaveClass("pl-2");
  expect(first?.first).not.toHaveClass("border-l-4");
  expect(third?.row).not.toHaveAttribute("aria-selected");
});

test("select_cell は名前の列の button で「〜を選ぶ」と読み、押すとその行を渡す。選べない行は名前の字だけ", async () => {
  const onSelect = renderTable(undefined);
  const button = screen.getByRole("button", { name: "佐藤 健を選ぶ" });
  expect(button).toHaveTextContent("佐藤 健");
  expect(button).toHaveClass("min-h-12", "text-primary", "underline", "active:bg-primary-container");
  await userEvent.click(button);
  expect(onSelect).toHaveBeenCalledWith(ROWS[0]);

  expect(screen.queryByRole("button", { name: "鈴木 一郎を選ぶ" })).toBeNull();
  expect(screen.getByRole("cell", { name: "鈴木 一郎" })).toBeInTheDocument();
});

test("selection がない表は先頭のセルの左を 0 のまま、行に aria-selected を付けない", () => {
  render(
    <DataTable
      label="予約"
      columns={[{ header: "コーチ", cell: (row: Row) => row.coach }]}
      rows={ROWS}
      rowKey={(row) => row.coach}
    />,
  );
  const cell = screen.getByRole("cell", { name: "佐藤 健" });
  expect(cell).toHaveClass("first:pl-0");
  expect(cell.parentElement).not.toHaveAttribute("aria-selected");
  expect(screen.queryByRole("button")).toBeNull();
});
