// 指標の値の組（specs/design/components.yaml の kpi_figure。0.9.0、#1492）。
import { render, screen, within } from "@testing-library/react";
import { expect, test } from "vitest";

import { DataTable } from "@/components/ui/DataTable";
import {
  KpiCell,
  KpiFigure,
  formatKpiNumber,
  kpiReasonShort,
  kpiReasonSentence,
  type KpiValue,
} from "@/components/ui/KpiFigure";

const NORTH_STAR = { label: "初回レッスン後の 1 ヶ月継続率", unit: "percent", direction: "up_is_good" } as const;

test("値は単位ごとに桁をそろえる（割合は小数 1 桁に %、時間は「時間」、単位のない値は小数 3 桁）", () => {
  expect(formatKpiNumber(62.4, "percent")).toBe("62.4%");
  expect(formatKpiNumber(74, "percent")).toBe("74.0%");
  expect(formatKpiNumber(18.5, "hours")).toBe("18.5 時間");
  expect(formatKpiNumber(21, "hours")).toBe("21.0 時間");
  expect(formatKpiNumber(0.012, "none")).toBe("0.012");
});

test("出せない理由は hero の 1 文と、cell の 10 字までの語の 2 つの形", () => {
  expect(kpiReasonSentence({ reason: "too_few" })).toBe("対象が 5 件未満のため、出せません。");
  expect(kpiReasonSentence({ reason: "none" })).toBe("対象が 0 件のため、出せません。");
  expect(kpiReasonSentence({ reason: "not_yet", availableOn: "2026-11-30" })).toBe(
    "集計の期間が終わっていないため、11/30（月）に出ます。",
  );
  const shorts = [
    kpiReasonShort({ reason: "too_few" }),
    kpiReasonShort({ reason: "none" }),
    kpiReasonShort({ reason: "not_yet", availableOn: "2026-11-30" }),
  ];
  expect(shorts).toEqual(["5 件未満", "対象 0 件", "11/30 に出ます"]);
  for (const short of shorts) expect(short.length).toBeLessThanOrEqual(10);
});

test("hero は今月を figure、前月を右に text_muted で出し、見方を文字で添えて 1 文で読む", () => {
  render(<KpiFigure {...NORTH_STAR} current={62.4} previous={58.1} />);
  const group = screen.getByRole("group", {
    name: "初回レッスン後の 1 ヶ月継続率、今月 62.4%、前月 58.1%、高いほど良い",
  });
  const number = within(group).getByText("62.4");
  expect(number).toHaveClass("font-numeric", "text-figure", "tabular-nums");
  // 良い向きは値の色で示さない（値の色は text のまま）
  expect(number.parentElement).toHaveClass("text-text");
  expect(within(group).getByText("%")).toHaveClass("text-xl", "font-black");
  expect(within(group).getByText("前月 58.1%")).toHaveClass("text-sm", "text-text-muted");
  expect(within(group).getByText("見方: 高いほど良い")).toHaveClass("text-sm", "text-text-muted");
  expect(within(group).getByText(NORTH_STAR.label)).toHaveClass("text-xl", "font-black");
});

test("hero で出せないときは 0 や空欄にせず「—」と理由を出し、「—」は「出せません」と読む", () => {
  render(
    <KpiFigure
      {...NORTH_STAR}
      direction="down_is_good"
      current={{ reason: "not_yet", availableOn: "2026-11-30" }}
      previous={{ reason: "too_few" }}
    />,
  );
  const group = screen.getByRole("group");
  expect(group).toHaveAccessibleName(
    "初回レッスン後の 1 ヶ月継続率、今月 出せません、集計の期間が終わっていないため、11/30（月）に出ます、前月 出せません、低いほど良い",
  );
  expect(within(group).getByLabelText("出せません")).toHaveTextContent("—");
  expect(within(group).getByText("集計の期間が終わっていないため、11/30（月）に出ます。")).toHaveClass("text-text");
  expect(within(group).getByText("前月 —")).toBeInTheDocument();
  expect(group.textContent).not.toMatch(/0%|N\/A/);
});

test("hero の読み込み中は label を残して「読み込み中…」（「—」を使わない）、失敗は error_text", () => {
  const { unmount } = render(<KpiFigure {...NORTH_STAR} loading />);
  expect(screen.getByText(NORTH_STAR.label)).toBeInTheDocument();
  expect(screen.getByRole("status")).toHaveTextContent("読み込み中…");
  expect(screen.queryByText("—")).not.toBeInTheDocument();
  unmount();

  render(<KpiFigure {...NORTH_STAR} error />);
  expect(screen.getByText(NORTH_STAR.label)).toBeInTheDocument();
  expect(screen.getByRole("alert")).toHaveTextContent(
    "今月の値を読めませんでした。画面を更新すると、もう一度読み込めます。",
  );
  expect(screen.getByRole("alert")).toHaveClass("text-danger");
});

test("cell は data_table の numeric の列に置き、前月は text_muted、出せないときは「—」の右に短い理由", () => {
  type Row = { label: string; current: KpiValue; previous: KpiValue };
  render(
    <DataTable<Row>
      label="ほかの指標"
      columns={[
        { header: "指標", cell: (row) => row.label },
        { header: "今月", variant: "numeric", cell: (row) => <KpiCell value={row.current} unit="none" /> },
        { header: "前月", variant: "numeric", cell: (row) => <KpiCell value={row.previous} unit="none" previous /> },
      ]}
      rows={[{ label: "コーチ別継続率の分散", current: { reason: "too_few" }, previous: 0.012 }]}
      rowKey={(row) => row.label}
    />,
  );
  const cells = screen.getAllByRole("cell");
  expect(cells[1]).toHaveClass("text-right", "tabular-nums", "whitespace-nowrap");
  expect(cells[1]).toHaveTextContent("— 5 件未満");
  expect(within(cells[1] as HTMLElement).getByLabelText("出せません")).toHaveTextContent("—");
  expect(within(cells[1] as HTMLElement).getByText("5 件未満")).toHaveClass("text-text-muted");
  expect(within(cells[2] as HTMLElement).getByText("0.012")).toHaveClass("text-text-muted");
});
