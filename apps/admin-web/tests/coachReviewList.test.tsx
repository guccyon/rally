import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, test, vi } from "vitest";

import type { CoachApi } from "@/api/coach";
import { CoachReviewSummaryLookup } from "@/components/CoachReviewSummaryLookup";
import { CoachHome } from "@/components/coach/CoachHome";
import { formatDateTime } from "@/components/labels";

vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace: vi.fn() }) }));

// 配置のテストでは CoachHome が作る API を差し替える（Node の Request は相対 URL の "/api" を解決しない）。
const { coachApiMock } = vi.hoisted(() => ({ coachApiMock: vi.fn() }));
vi.mock("@/api/coach", () => ({ coachApi: coachApiMock }));

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

function summaryApi(reviewCount: number, averageRating: number | null) {
  return vi.fn<CoachApi["getCoachReviewSummary"]>(async (coachId) => ({
    ok: true,
    data: { coach_id: coachId, review_count: reviewCount, average_rating: averageRating },
  }));
}

describe("コーチの評価の一覧", () => {
  test("同じコーチ ID で listCoachReviews を呼び、点・一言・送った時刻を API の順（新しい順）に出す", async () => {
    const getCoachReviewSummary = summaryApi(2, 4);
    const listCoachReviews = vi.fn<CoachApi["listCoachReviews"]>(async (coachId) => ({
      ok: true,
      data: { coach_id: coachId, reviews: REVIEWS },
    }));
    const user = userEvent.setup();
    render(
      <CoachReviewSummaryLookup api={{ getCoachReviewSummary, listCoachReviews }} description="説明" />,
    );

    await user.type(screen.getByLabelText("コーチ ID"), ` ${COACH_ID} `);
    await user.click(screen.getByRole("button", { name: "評価を見る" }));

    expect(listCoachReviews).toHaveBeenCalledWith(COACH_ID);
    expect(await screen.findByLabelText("評価の件数と平均")).toBeInTheDocument();
    const items = within(screen.getByRole("list", { name: "評価の一覧" })).getAllByRole("listitem");
    expect(items).toHaveLength(2);
    expect(items[0]).toHaveTextContent("5 点");
    expect(items[0]).toHaveTextContent("フォームがよく分かりました");
    expect(items[0]).toHaveTextContent(formatDateTime("2026-10-20T03:00:00Z"));
    expect(items[1]).toHaveTextContent("3 点");
    expect(items[1]).toHaveTextContent("もう少し時間がほしかった");
    expect(items[1]).toHaveTextContent(formatDateTime("2026-10-01T09:30:00Z"));
  });

  test("評価が 0 件なら「まだ評価はありません」と出す", async () => {
    const getCoachReviewSummary = summaryApi(0, null);
    const listCoachReviews = vi.fn<CoachApi["listCoachReviews"]>(async (coachId) => ({
      ok: true,
      data: { coach_id: coachId, reviews: [] },
    }));
    const user = userEvent.setup();
    render(
      <CoachReviewSummaryLookup api={{ getCoachReviewSummary, listCoachReviews }} description="説明" />,
    );

    await user.type(screen.getByLabelText("コーチ ID"), COACH_ID);
    await user.click(screen.getByRole("button", { name: "評価を見る" }));

    expect(await screen.findByText("まだ評価はありません")).toBeInTheDocument();
    expect(screen.queryByRole("list", { name: "評価の一覧" })).not.toBeInTheDocument();
  });

  test("一覧の API のエラー（他のコーチの分の 403 など）の文言を出し、件数と平均も一覧も出さない", async () => {
    const getCoachReviewSummary = summaryApi(2, 4);
    const listCoachReviews = vi
      .fn<CoachApi["listCoachReviews"]>()
      .mockResolvedValueOnce({ ok: true, data: { coach_id: COACH_ID, reviews: REVIEWS } })
      .mockResolvedValueOnce({ ok: false, message: "権限がないためアクセスできません。" });
    const user = userEvent.setup();
    render(
      <CoachReviewSummaryLookup api={{ getCoachReviewSummary, listCoachReviews }} description="説明" />,
    );

    await user.type(screen.getByLabelText("コーチ ID"), COACH_ID);
    await user.click(screen.getByRole("button", { name: "評価を見る" }));
    await screen.findByRole("list", { name: "評価の一覧" });

    await user.click(screen.getByRole("button", { name: "評価を見る" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("list", { name: "評価の一覧" })).not.toBeInTheDocument();
    expect(screen.queryByLabelText("評価の件数と平均")).not.toBeInTheDocument();
  });

  test("一覧の API を渡さなければ（本部の画面）一覧は出さない", async () => {
    const user = userEvent.setup();
    render(
      <CoachReviewSummaryLookup api={{ getCoachReviewSummary: summaryApi(0, null) }} description="説明" />,
    );

    await user.type(screen.getByLabelText("コーチ ID"), COACH_ID);
    await user.click(screen.getByRole("button", { name: "評価を見る" }));

    expect(await screen.findByLabelText("評価の件数と平均")).toBeInTheDocument();
    expect(screen.queryByText("まだ評価はありません")).not.toBeInTheDocument();
  });

  test("コーチの画面の「自分の評価」は件数と平均の下に一覧を出す", async () => {
    coachApiMock.mockReturnValue({
      getCoachReviewSummary: summaryApi(2, 4),
      listCoachReviews: async (coachId: string) => ({
        ok: true,
        data: { coach_id: coachId, reviews: REVIEWS },
      }),
    });
    const user = userEvent.setup();
    render(<CoachHome token="mock.coach.x" />);
    const region = screen.getByRole("region", { name: "自分の評価" });

    await user.type(within(region).getByLabelText("コーチ ID"), COACH_ID);
    await user.click(within(region).getByRole("button", { name: "評価を見る" }));

    const summary = await within(region).findByLabelText("評価の件数と平均");
    const list = within(region).getByRole("list", { name: "評価の一覧" });
    expect(summary.compareDocumentPosition(list) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });
});
