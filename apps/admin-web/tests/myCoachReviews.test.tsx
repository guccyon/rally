import { render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { CoachApi } from "@/api/coach";
import { CoachHome } from "@/components/coach/CoachHome";
import { MyCoachReviews } from "@/components/coach/MyCoachReviews";
import { formatDateTime } from "@/components/labels";

// 配置のテストでは CoachHome が作る API を差し替える（Node の Request は相対 URL の "/api" を解決しない）。
const { coachApiMock } = vi.hoisted(() => ({ coachApiMock: vi.fn() }));
vi.mock("@/api/coach", () => ({ coachApi: coachApiMock }));

const COACH_ID = "22222222-2222-4222-8222-222222222222";

const ACCOUNT = { account_id: COACH_ID, role: "coach" as const, display_name: "山田コーチ" };

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

/** 件数と平均の一覧（dl）を [見出し, 値] の組にする。 */
function summaryOf(list: HTMLElement) {
  return Array.from(list.querySelectorAll("dt")).map((dt) => [
    dt.textContent,
    dt.nextElementSibling?.textContent,
  ]);
}

type ReviewsApi = Pick<CoachApi, "getMyAccount" | "getCoachReviewSummary" | "listCoachReviews">;

function mockApi(overrides: Partial<ReviewsApi> = {}) {
  return {
    getMyAccount: vi.fn<CoachApi["getMyAccount"]>(async () => ({ ok: true, data: ACCOUNT })),
    getCoachReviewSummary: vi.fn<CoachApi["getCoachReviewSummary"]>(async (coachId) => ({
      ok: true,
      data: { coach_id: coachId, review_count: 2, average_rating: 4 },
    })),
    listCoachReviews: vi.fn<CoachApi["listCoachReviews"]>(async (coachId) => ({
      ok: true,
      data: { coach_id: coachId, reviews: REVIEWS },
    })),
    ...overrides,
  };
}

beforeEach(() => {
  coachApiMock.mockReset();
});

describe("自分の評価", () => {
  test("開いたときに getMyAccount の本人の ID で件数と平均と一覧を出す（ID は入力しない）", async () => {
    const api = mockApi();
    render(<MyCoachReviews api={api} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    const summary = await screen.findByLabelText("評価の件数と平均");
    expect(api.getMyAccount).toHaveBeenCalledTimes(1);
    expect(api.getCoachReviewSummary).toHaveBeenCalledWith(COACH_ID);
    expect(api.listCoachReviews).toHaveBeenCalledWith(COACH_ID);
    expect(summaryOf(summary)).toEqual([
      ["コーチ ID", COACH_ID],
      ["評価の件数", "2 件"],
      ["点の平均（1〜5）", "4.00"],
    ]);
    const list = screen.getByRole("list", { name: "評価の一覧" });
    expect(summary.compareDocumentPosition(list) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    const items = within(list).getAllByRole("listitem");
    expect(items).toHaveLength(2);
    expect(items[0]).toHaveTextContent("5 点");
    expect(items[0]).toHaveTextContent("フォームがよく分かりました");
    expect(items[0]).toHaveTextContent(formatDateTime("2026-10-20T03:00:00Z"));
    expect(screen.queryByLabelText("コーチ ID")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "評価を見る" })).not.toBeInTheDocument();
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
  });

  test("評価が 0 件なら、平均も一覧も「まだない」と出す", async () => {
    const api = mockApi({
      getCoachReviewSummary: vi.fn<CoachApi["getCoachReviewSummary"]>(async (coachId) => ({
        ok: true,
        data: { coach_id: coachId, review_count: 0, average_rating: null },
      })),
      listCoachReviews: vi.fn<CoachApi["listCoachReviews"]>(async (coachId) => ({
        ok: true,
        data: { coach_id: coachId, reviews: [] },
      })),
    });
    render(<MyCoachReviews api={api} />);

    const summary = await screen.findByLabelText("評価の件数と平均");
    expect(summaryOf(summary)).toContainEqual(["点の平均（1〜5）", "評価はまだありません"]);
    expect(screen.getByText("まだ評価はありません")).toBeInTheDocument();
  });

  test("本人のアカウントが取れなければ（401 など）その文言を出し、評価の API は呼ばない", async () => {
    const api = mockApi({
      getMyAccount: vi.fn<CoachApi["getMyAccount"]>(async () => ({
        ok: false,
        message: "トークンがないか無効です。ログインし直してください。",
      })),
    });
    render(<MyCoachReviews api={api} />);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "トークンがないか無効です。ログインし直してください。",
    );
    expect(api.getCoachReviewSummary).not.toHaveBeenCalled();
    expect(api.listCoachReviews).not.toHaveBeenCalled();
    expect(screen.queryByLabelText("評価の件数と平均")).not.toBeInTheDocument();
  });

  test("本人のアカウントがコーチでなければ、その旨を出し、評価の API は呼ばない", async () => {
    const api = mockApi({
      getMyAccount: vi.fn<CoachApi["getMyAccount"]>(async () => ({
        ok: true,
        data: { ...ACCOUNT, role: "headquarters" },
      })),
    });
    render(<MyCoachReviews api={api} />);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "コーチのアカウントでログインしたときだけ、自分の評価を見られます。",
    );
    expect(api.getCoachReviewSummary).not.toHaveBeenCalled();
  });

  test("評価の API のエラーの文言を出し、件数と平均も一覧も出さない", async () => {
    const api = mockApi({
      listCoachReviews: vi.fn<CoachApi["listCoachReviews"]>(async () => ({
        ok: false,
        message: "サーバーに接続できません。",
      })),
    });
    render(<MyCoachReviews api={api} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("サーバーに接続できません。");
    expect(screen.queryByLabelText("評価の件数と平均")).not.toBeInTheDocument();
    expect(screen.queryByRole("list", { name: "評価の一覧" })).not.toBeInTheDocument();
  });

  test("コーチの画面の「自分の評価」は、開いたときから本人の評価を出す", async () => {
    coachApiMock.mockReturnValue({
      ...mockApi(),
      // 同じ画面の「予約枠」と「動画の添削」と「通話への参加」が開いたときに呼ぶ。
      listMyCoachSlots: vi.fn<CoachApi["listMyCoachSlots"]>(async () => ({
        ok: true,
        data: { slots: [] },
      })),
      listMyCoachVideos: vi.fn<CoachApi["listMyCoachVideos"]>(async () => ({
        ok: true,
        data: { videos: [] },
      })),
      listMyCoachBookings: vi.fn<CoachApi["listMyCoachBookings"]>(async () => ({
        ok: true,
        data: { bookings: [] },
      })),
    });
    render(<CoachHome token="mock.coach.x" />);
    const region = screen.getByRole("region", { name: "自分の評価" });

    expect(within(region).queryByLabelText("コーチ ID")).not.toBeInTheDocument();
    const summary = await within(region).findByLabelText("評価の件数と平均");
    expect(summaryOf(summary)).toContainEqual(["評価の件数", "2 件"]);
    expect(within(region).getByRole("list", { name: "評価の一覧" })).toBeInTheDocument();
  });
});
