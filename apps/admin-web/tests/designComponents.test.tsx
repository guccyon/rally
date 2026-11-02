// デザインシステムの基本の部品（specs/design/components.yaml）。
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { expect, test, vi } from "vitest";

import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { EmptyState } from "@/components/ui/EmptyState";
import { ListRow } from "@/components/ui/ListRow";
import { LoadingBar, SectionLoading } from "@/components/ui/Loading";
import { Notice } from "@/components/ui/Notice";
import { StatusTag } from "@/components/ui/StatusTag";
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
  const row = screen.getByRole("button", { name: /未読.*コーチから返事が届きました/ });
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
      <EmptyState message="まだお知らせはありません" action={<Button variant="secondary">ホームを更新する</Button>} />
      <SectionLoading message="予約を読み込んでいます…" />
      <LoadingBar />
      <Badge count={3} />
    </>,
  );
  expect(screen.getByRole("button", { name: "10/28（水）12:00〜12:30" })).toHaveClass("bg-surface", "rounded-md");
  expect(screen.getByText("まだお知らせはありません")).toBeInTheDocument();
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
  const dialog = screen.getByRole("alertdialog", { name: "受付を停止しますか？" });
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
