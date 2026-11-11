// 詳細の画面の部品（specs/design/components.yaml の description_list・timeline。0.8.0、#1319）。
import { render, screen, within } from "@testing-library/react";
import { expect, test } from "vitest";

import { DescriptionList } from "@/components/ui/DescriptionList";
import { StatusTag } from "@/components/ui/StatusTag";
import { Timeline, timelineEventSentence } from "@/components/ui/Timeline";

test("ラベルと値の並びは dl・dt・dd で、ラベルの列は 10em・項目のあいだに線、値がない項目も「—」で残す", () => {
  render(
    <DescriptionList
      label="予約の詳細"
      items={[
        { term: "レッスンメニュー", value: "フォアハンドの基礎（30 分）" },
        { term: "種類", value: null },
        { term: "今の状態", value: <StatusTag label="確定済み" status="success" /> },
      ]}
    />,
  );
  const list = screen.getByLabelText("予約の詳細");
  expect(list.tagName).toBe("DL");
  const terms = within(list).getAllByRole("term");
  expect(terms.map((term) => term.textContent)).toEqual(["レッスンメニュー", "種類", "今の状態"]);
  expect(terms[0]).toHaveClass("text-sm", "font-bold", "text-text-muted");
  const values = within(list).getAllByRole("definition");
  expect(values[1]).toHaveTextContent("—");
  expect(values[0]).toHaveClass("text-base", "tabular-nums");

  const item = terms[0]?.parentElement;
  // 狭い幅（600px 未満）では上下に積み、広い幅で term 10em と値の 2 列
  expect(item).toHaveClass("grid-cols-1", "min-[600px]:grid-cols-[10em_1fr]", "min-[600px]:gap-x-4", "py-2", "border-t");
  // 面と枠で囲わない
  expect(list.className).not.toMatch(/\bbg-|\brounded/);
});

test("stacked はどの幅でもラベルを値の上に積む", () => {
  render(<DescriptionList variant="stacked" items={[{ term: "本文", value: "1 行目\n2 行目" }]} />);
  const item = screen.getByRole("term").parentElement;
  expect(item).toHaveClass("grid-cols-1", "gap-y-1");
  expect(item?.className).not.toMatch(/grid-cols-\[10em/);
  expect(screen.getByRole("definition")).toHaveClass("whitespace-pre-line");
});

test("項目ごとの読み込み中と失敗は、その値だけを置き換え、ほかの項目は出す", () => {
  render(
    <DescriptionList
      items={[
        { term: "表示名", value: "鈴木 一郎" },
        { term: "スポーツ", loading: true },
        { term: "返却待ち", error: "件数を読めませんでした。画面を更新すると、もう一度読み込めます。" },
      ]}
    />,
  );
  expect(screen.getByText("鈴木 一郎")).toBeInTheDocument();
  expect(screen.getByRole("status")).toHaveTextContent("読み込み中…");
  expect(screen.getByRole("alert")).toHaveTextContent("件数を読めませんでした。");
  expect(screen.getByRole("alert")).toHaveClass("text-danger");
});

test("ラベルと値の並びの全体の読み込み中は section_text、読めなかったときは notice の inline で、dl を出さない", () => {
  const { container, rerender } = render(<DescriptionList items={[]} loading="予約を読み込んでいます…" />);
  expect(screen.getByRole("status")).toHaveTextContent("予約を読み込んでいます…");
  expect(container.querySelector("dl")).toBeNull();

  rerender(<DescriptionList items={[]} error="予約を読み込めませんでした。画面を更新すると、もう一度読み込めます。" />);
  expect(screen.getByRole("alert")).toHaveTextContent("予約を読み込めませんでした。");
  expect(container.querySelector("dl")).toBeNull();
});

test("経緯の並びは渡した順の ol で、出来事を 1 つの文で読み、点と線は読み上げない", () => {
  render(
    <Timeline
      label="状態が変わった記録"
      empty="状態が変わった記録はありません"
      events={[
        { key: "1", time: "10/20（火）9:12", label: "確定待ち", status: "warning" },
        { key: "2", time: "10/21（水）8:30", label: "確定済み", status: "success", meta: "本部: 鈴木" },
        { key: "3", time: "10/27（火）18:04", label: "キャンセル済み", status: "neutral" },
      ]}
    />,
  );
  const list = screen.getByRole("list", { name: "状態が変わった記録" });
  expect(list.tagName).toBe("OL");
  const items = within(list).getAllByRole("listitem");
  expect(items.map((item) => item.querySelector(".sr-only")?.textContent)).toEqual([
    "10/20（火）9:12、確定待ち",
    "10/21（水）8:30、確定済み、本部: 鈴木",
    "10/27（火）18:04、キャンセル済み",
  ]);
  // 点は booking_state の対応の色（warning・success・neutral は border-strong）
  const dots = screen.getAllByTestId("timeline-dot");
  expect(dots[0]).toHaveClass("bg-warning", "size-2");
  expect(dots[1]).toHaveClass("bg-success");
  expect(dots[2]).toHaveClass("bg-border-strong");
  for (const dot of dots) expect(dot.closest("[aria-hidden='true']")).not.toBeNull();
  // 状態の字は status_tag と同じ small 700（neutral は text-muted）
  expect(screen.getByText("キャンセル済み", { selector: "p" })).toHaveClass("text-sm", "font-bold", "text-text-muted");
});

test("状態の対応のない出来事は border-strong の点と body 700 の「〜した」", () => {
  render(
    <Timeline
      label="お知らせの経緯"
      empty="経緯はありません"
      events={[{ key: "w", time: "11/2（月）10:25", label: "取り下げた" }]}
    />,
  );
  expect(screen.getByTestId("timeline-dot")).toHaveClass("bg-border-strong");
  expect(screen.getByText("取り下げた", { selector: "p" })).toHaveClass("text-base", "font-bold");
});

test("経緯の並びは読み込み中・失敗・0 件のとき ol を出さない", () => {
  const props = { label: "状態が変わった記録", events: [], empty: "状態が変わった記録はありません" };
  const { container, rerender } = render(<Timeline {...props} loading="状態が変わった記録を読み込んでいます…" />);
  expect(screen.getByRole("status")).toHaveTextContent("読み込んでいます…");
  rerender(<Timeline {...props} error="状態が変わった記録を読めませんでした。画面を更新すると、もう一度読み込めます。" />);
  expect(screen.getByRole("alert")).toBeInTheDocument();
  rerender(<Timeline {...props} />);
  expect(screen.getByText("状態が変わった記録はありません")).toBeInTheDocument();
  expect(container.querySelector("ol")).toBeNull();
});

test("出来事の文は時刻・呼び名・meta を読点でつなぐ", () => {
  expect(timelineEventSentence({ time: "10/27（火）9:12", label: "確定済み", meta: "本部: 鈴木" })).toBe(
    "10/27（火）9:12、確定済み、本部: 鈴木",
  );
});
