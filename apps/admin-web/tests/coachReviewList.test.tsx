import { render, screen, within } from "@testing-library/react";
import { describe, expect, test } from "vitest";

import { CoachReviewList } from "@/components/CoachReviewList";
import { CoachReviewSummaryView } from "@/components/CoachReviewSummaryView";
import { formatDateTime } from "@/components/labels";

const COACH_ID = "22222222-2222-4222-8222-222222222222";

// API が返す並び（送った時刻の新しい順）。
const REVIEWS = [
  {
    review_id: "33333333-3333-4333-8333-333333333333",
    lesson_id: "44444444-4444-4444-8444-444444444444",
    rating: 5,
    comment: "フォームがよく分かりました",
    submitted_at: "2026-10-20T03:00:00Z",
  },
  {
    review_id: "55555555-5555-4555-8555-555555555555",
    lesson_id: "66666666-6666-4666-8666-666666666666",
    rating: 3,
    comment: "もう少し時間がほしかった",
    submitted_at: "2026-10-01T09:30:00Z",
  },
];

describe("コーチの評価の一覧", () => {
  test("点・一言・送った時刻を API の順（新しい順）に出す", () => {
    render(<CoachReviewList reviews={REVIEWS} />);

    const items = within(screen.getByRole("list", { name: "評価の一覧" })).getAllByRole("listitem");
    expect(items).toHaveLength(2);
    expect(items[0]).toHaveTextContent("5 点");
    expect(items[0]).toHaveTextContent("フォームがよく分かりました");
    expect(items[0]).toHaveTextContent(formatDateTime("2026-10-20T03:00:00Z"));
    expect(items[1]).toHaveTextContent("3 点");
    expect(items[1]).toHaveTextContent("もう少し時間がほしかった");
    expect(items[1]).toHaveTextContent(formatDateTime("2026-10-01T09:30:00Z"));
  });

  test("評価が 0 件なら「まだ評価はありません」と出す", () => {
    render(<CoachReviewList reviews={[]} />);

    expect(screen.getByText("まだ評価はありません")).toBeInTheDocument();
    expect(screen.queryByRole("list", { name: "評価の一覧" })).not.toBeInTheDocument();
  });

  test("件数と平均の下に一覧を出す", () => {
    render(
      <CoachReviewSummaryView
        summary={{ coach_id: COACH_ID, review_count: 2, average_rating: 4 }}
        reviews={REVIEWS}
      />,
    );

    const summary = screen.getByLabelText("評価の件数と平均");
    const list = screen.getByRole("list", { name: "評価の一覧" });
    expect(summary.compareDocumentPosition(list) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });
});
