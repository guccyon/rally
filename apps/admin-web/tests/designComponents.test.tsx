// デザインシステムの基本の部品（specs/design/components.yaml）。
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { expect, test, vi } from "vitest";

import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { DataTable } from "@/components/ui/DataTable";
import { DateField, DateRangeField } from "@/components/ui/DateField";
import { dateRangeError, type DateRange } from "@/components/ui/dateRange";
import { EmptyState } from "@/components/ui/EmptyState";
import { ListRow } from "@/components/ui/ListRow";
import { LoadingBar, SectionLoading } from "@/components/ui/Loading";
import { Notice } from "@/components/ui/Notice";
import { StatusTag } from "@/components/ui/StatusTag";
import { SLOT_STATE_STATUS } from "@/components/ui/status";
import { TextArea, TextField } from "@/components/ui/TextField";

test("ボタンは種類ごとにトークンの色で、高さ 48px 以上の丸いボタン", () => {
  render(
    <>
      <Button>予約する</Button>
      <Button variant="secondary">保存する</Button>
      <Button variant="danger">キャンセルする</Button>
      <Button variant="text">予約をすべて見る</Button>
    </>,
  );
  const primary = screen.getByRole("button", { name: "予約する" });
  expect(primary).toHaveClass("bg-primary", "text-on-primary", "rounded-pill", "min-h-12");
  expect(screen.getByRole("button", { name: "保存する" })).toHaveClass("border-border-strong", "text-primary");
  expect(screen.getByRole("button", { name: "キャンセルする" })).toHaveClass("bg-danger");
  expect(screen.getByRole("button", { name: "予約をすべて見る" })).toHaveClass("text-primary");
});

test("読み込み中のボタンはラベルを残して spinner を出し、押せない", async () => {
  const onClick = vi.fn();
  render(
    <Button loading onClick={onClick}>
      送る
    </Button>,
  );
  const button = screen.getByRole("button", { name: "送る" });
  expect(button).toBeDisabled();
  expect(screen.getByTestId("spinner")).toBeInTheDocument();
  await userEvent.click(button);
  expect(onClick).not.toHaveBeenCalled();
});

test("入力欄は見えるラベルを欄に結び付け、helper と誤りを読み上げで一緒に読む", () => {
  render(
    <>
      <TextField label="表示名" helper="20 文字まで" error="表示名は 1〜20 文字で入れてください。" />
      <TextArea label="一言" helper="200 文字まで" counter="12 / 200" />
    </>,
  );
  const name = screen.getByLabelText("表示名");
  expect(name).toHaveAttribute("aria-invalid", "true");
  expect(name).toHaveAccessibleDescription("20 文字まで 表示名は 1〜20 文字で入れてください。");
  const comment = screen.getByLabelText("一言");
  expect(comment.tagName).toBe("TEXTAREA");
  expect(comment).toHaveAttribute("rows", "3");
  expect(screen.getByText("12 / 200")).toBeInTheDocument();
});

test("状態のタグは呼び名の文字を状態の面に出す", () => {
  render(<StatusTag label="確定待ち" status="warning" />);
  expect(screen.getByText("確定待ち")).toHaveClass("bg-warning-container", "text-warning-on-container");
});

test("知らせは失敗を role=alert、成功を role=status で出す", () => {
  render(
    <>
      <Notice status="danger" message="予約を読み込めませんでした。もう一度読み込んでください。" />
      <Notice status="success" message="受付を停止しました" />
    </>,
  );
  expect(screen.getByRole("alert")).toHaveTextContent("予約を読み込めませんでした");
  expect(screen.getByRole("alert")).toHaveClass("bg-danger-container");
  expect(screen.getByRole("status")).toHaveTextContent("受付を停止しました");
});

test("一覧の行は未読を文字のしるしと左の線で示し、押すと詳細へ", async () => {
  const onClick = vi.fn();
  render(<ListRow title="コーチから返事が届きました" subtitle="10/27 18:04" unread onClick={onClick} />);
  const row = screen.getByRole("button", {
    name: /未読.*コーチから返事が届きました/,
  });
  expect(row).toHaveClass("border-l-primary", "min-h-14");
  await userEvent.click(row);
  expect(onClick).toHaveBeenCalledOnce();
});

test("押せるカードは全体で 1 つのボタン、空の表示は一言と次の操作、読み込み中は文と bar", () => {
  render(
    <>
      <Card onClick={() => {}}>
        <span>10/28（水）12:00〜12:30</span>
      </Card>
      <EmptyState message="まだお知らせはありません" action={{ label: "お知らせを書く", onClick: () => {} }} />
      <EmptyState
        variant="section"
        message="まだ低い評価はありません"
        action={{ label: "評価の一覧を見る", onClick: () => {} }}
      />
      <SectionLoading message="予約を読み込んでいます…" />
      <LoadingBar />
      <Badge count={3} />
    </>,
  );
  expect(screen.getByRole("button", { name: "10/28（水）12:00〜12:30" })).toHaveClass("bg-surface", "rounded-md");
  // 空の表示：screen は surface の面と primary、section は background の面と border の線と secondary。文は text の色
  const screenEmpty = screen.getByText("まだお知らせはありません").parentElement;
  expect(screenEmpty).toHaveClass("bg-surface", "text-text");
  expect(screen.getByRole("button", { name: "お知らせを書く" })).toHaveClass("bg-primary");
  const sectionEmpty = screen.getByText("まだ低い評価はありません").parentElement;
  expect(sectionEmpty).toHaveClass("bg-background", "border-border", "text-text");
  expect(sectionEmpty).not.toHaveClass("bg-surface");
  expect(screen.getByRole("button", { name: "評価の一覧を見る" })).toHaveClass("border-border-strong", "bg-background");
  expect(screen.getByRole("status")).toHaveTextContent("予約を読み込んでいます…");
  expect(screen.getByRole("progressbar")).toBeInTheDocument();
  expect(screen.getByText("3")).toHaveClass("bg-danger");
});

function DialogHarness({ onConfirm }: { onConfirm: () => void }) {
  const [open, setOpen] = useState(true);
  return (
    <ConfirmDialog
      open={open}
      danger
      title="受付を停止しますか？"
      body="山田 コーチの受付を停止します。新しい予約は入らなくなります。"
      confirmLabel="受付を停止する"
      onConfirm={onConfirm}
      onCancel={() => setOpen(false)}
    />
  );
}

test("確認のダイアログは題にフォーカスし、やめるを左・確定を右に置き、Escape で閉じる", async () => {
  const onConfirm = vi.fn();
  render(<DialogHarness onConfirm={onConfirm} />);
  const dialog = screen.getByRole("alertdialog", {
    name: "受付を停止しますか？",
  });
  expect(dialog).toHaveAccessibleDescription(/新しい予約は入らなくなります/);
  expect(screen.getByRole("heading", { name: "受付を停止しますか？" })).toHaveFocus();
  const buttons = screen.getAllByRole("button");
  expect(buttons.map((b) => b.textContent)).toEqual(["やめる", "受付を停止する"]);
  const confirm = screen.getByRole("button", { name: "受付を停止する" });
  expect(confirm).toHaveClass("bg-danger");
  await userEvent.click(confirm);
  expect(onConfirm).toHaveBeenCalledOnce();
  await userEvent.keyboard("{Escape}");
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
});

test("表は包みの中で、見出しに scope があり、セルは段の余白で 1 行（文の列だけ折り返す）", () => {
  type Row = { name: string; count: number; comment: string; urgent: boolean };
  render(
    <DataTable<Row>
      label="コーチごとの予約枠の需給"
      columns={[
        { header: "コーチ", cell: (row) => row.name },
        {
          header: "公開した枠",
          variant: "numeric",
          cell: (row) => `${row.count} 件`,
        },
        {
          header: "最新の一言",
          variant: "long_text",
          cell: (row) => row.comment,
        },
      ]}
      rows={[
        {
          name: "山田 花子",
          count: 12,
          comment: "直すところがよく分かりました。",
          urgent: true,
        },
        { name: "佐藤 健", count: 8, comment: "—", urgent: false },
      ]}
      rowKey={(row) => row.name}
      highlightRow={(row) => row.urgent}
    />,
  );
  const table = screen.getByRole("table", { name: "コーチごとの予約枠の需給" });
  expect(table).toHaveClass("w-full", "min-w-max");
  expect(table.parentElement).toHaveClass("overflow-x-auto", "max-w-full");

  const headers = screen.getAllByRole("columnheader");
  expect(headers.map((th) => th.textContent)).toEqual(["コーチ", "公開した枠", "最新の一言"]);
  for (const th of headers) {
    expect(th).toHaveAttribute("scope", "col");
    expect(th).toHaveClass("whitespace-nowrap", "px-2", "py-2");
  }

  // 並べ方は渡した順のまま
  const rows = screen.getAllByRole("row").slice(1);
  expect(rows.map((row) => row.firstChild?.textContent)).toEqual(["山田 花子", "佐藤 健"]);
  expect(rows[0]).toHaveClass("bg-danger-container");
  expect(rows[1]).not.toHaveClass("bg-danger-container");

  expect(screen.getByRole("cell", { name: "山田 花子" })).toHaveClass("whitespace-nowrap", "px-2", "py-2", "h-12");
  expect(screen.getByRole("cell", { name: "12 件" })).toHaveClass("whitespace-nowrap", "text-right", "tabular-nums");
  expect(screen.getByRole("cell", { name: "直すところがよく分かりました。" })).toHaveClass(
    "whitespace-normal",
    "min-w-[12em]",
    "max-w-[40em]",
  );
});

test("表が幅に収まるときは、包みをフォーカスの止まる場所にしない", () => {
  render(
    <DataTable
      label="受付の記録"
      columns={[{ header: "日時", cell: (row: string) => row }]}
      rows={["10/28 9:00"]}
      rowKey={(row) => row}
    />,
  );
  expect(screen.queryByRole("region", { name: "受付の記録" })).not.toBeInTheDocument();
  expect(screen.getByRole("table", { name: "受付の記録" }).parentElement).not.toHaveAttribute("tabindex");
});

test("日付の欄は見えるラベルの付いた input type=date で、入力欄と同じ高さと輪郭", () => {
  render(<DateField label="送る日" defaultValue="2026-10-28" />);
  const field = screen.getByLabelText("送る日");
  expect(field).toHaveAttribute("type", "date");
  expect(field).toHaveValue("2026-10-28");
  expect(field).toHaveClass("min-h-12", "border-border-strong", "rounded-md", "tabular-nums", "min-w-[10em]");
});

test("期間の欄は「期間」の group に開始日と終了日を並べ、変えた値を YYYY-MM-DD で返す", () => {
  const onChange = vi.fn();
  render(
    <DateRangeField value={{ from: "2026-10-28", to: "2026-11-03" }} onChange={onChange} helper="31 日まで選べます" />,
  );
  const group = screen.getByRole("group", { name: "期間" });
  expect(group.tagName).toBe("FIELDSET");
  const from = screen.getByLabelText("開始日");
  expect(from).toHaveValue("2026-10-28");
  expect(from).toHaveAccessibleDescription("31 日まで選べます");
  expect(screen.getByLabelText("終了日")).toHaveValue("2026-11-03");

  fireEvent.change(screen.getByLabelText("終了日"), { target: { value: "2026-11-05" } });
  expect(onChange).toHaveBeenLastCalledWith({ from: "2026-10-28", to: "2026-11-05" });
});

test("期間の誤りは group の下に 1 つ出し、印の欄だけに aria-invalid と説明を結ぶ", () => {
  function Range({ value }: { value: DateRange }) {
    const [range, setRange] = useState(value);
    return <DateRangeField value={range} onChange={setRange} error={dateRangeError(range, 31)} />;
  }
  render(<Range value={{ from: "2026-11-03", to: "2026-10-28" }} />);
  const message = "終了日が開始日より前です。終了日は開始日と同じか、それより後の日付にしてください。";
  expect(screen.getAllByRole("alert")).toHaveLength(1);
  expect(screen.getByRole("alert")).toHaveTextContent(message);
  expect(screen.getByLabelText("開始日")).not.toHaveAttribute("aria-invalid");
  expect(screen.getByLabelText("終了日")).toHaveAttribute("aria-invalid", "true");
  expect(screen.getByLabelText("終了日")).toHaveAccessibleDescription(message);
});

test("既定の期間を読むあいだは欄を変えられず、読み込み中を伝える", () => {
  render(<DateRangeField value={{ from: "", to: "" }} onChange={vi.fn()} helper="31 日まで選べます" loading />);
  expect(screen.getByRole("group", { name: "期間" })).toHaveAttribute("aria-busy", "true");
  expect(screen.getByLabelText("開始日")).toBeDisabled();
  expect(screen.getByLabelText("終了日")).toBeDisabled();
  expect(screen.getByRole("status")).toHaveTextContent("期間を読み込んでいます…");
});

test.each([
  [{ from: "", to: "2026-11-03" }, "開始日か終了日が空いています。両方を選んでください。", "from"],
  [{ from: "2026-10-28", to: "" }, "開始日か終了日が空いています。両方を選んでください。", "to"],
  [{ from: "", to: "" }, "開始日か終了日が空いています。両方を選んでください。", "both"],
  [
    { from: "2026-11-03", to: "2026-10-28" },
    "終了日が開始日より前です。終了日は開始日と同じか、それより後の日付にしてください。",
    "to",
  ],
  [{ from: "2026-10-01", to: "2026-11-30" }, "選んだ期間は 61 日です。期間は 31 日までにしてください。", "both"],
] as const)("期間 %o の誤りは「%s」で、印は %s", (range, message, mark) => {
  expect(dateRangeError(range, 31)).toEqual({ message, mark });
});

test("期間は両端を含めて上限の日数までなら誤りにしない", () => {
  expect(dateRangeError({ from: "2026-10-28", to: "2026-10-28" }, 31)).toBeNull();
  expect(dateRangeError({ from: "2026-10-01", to: "2026-10-31" }, 31)).toBeNull();
  expect(dateRangeError({ from: "2026-10-01", to: "2026-11-01" }, 31)?.message).toBe(
    "選んだ期間は 32 日です。期間は 31 日までにしてください。",
  );
});

test.each([
  ["open", "公開中", "bg-info-container"],
  ["booked", "予約済み", "bg-success-container"],
  ["closed", "終了", "bg-surface"],
] as const)("スロットの状態 %s は「%s」を slot_state の色（%s）で出す", (state, label, container) => {
  render(<StatusTag label={label} status={SLOT_STATE_STATUS[state]} />);
  expect(screen.getByText(label)).toHaveClass(container);
});

test("neutral の状態のタグは border の 1px の輪郭を付ける", () => {
  render(<StatusTag label="キャンセル済み" status="neutral" />);
  expect(screen.getByText("キャンセル済み")).toHaveClass("border", "border-border", "bg-surface", "text-text-muted");
});
