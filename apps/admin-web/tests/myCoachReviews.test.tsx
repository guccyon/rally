import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { CoachApi } from "@/api/coach";
import { CoachReviewList } from "@/components/CoachReviewList";
import { CoachHome } from "@/components/coach/CoachHome";
import { MyCoachReviews } from "@/components/coach/MyCoachReviews";
import { formatDateTime } from "@/components/labels";

import { EMPTY_LESSON_STATS } from "./lessonStatsFixtures";
import { noNotifications } from "./notificationFixtures";

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
    lesson_menu: { lesson_menu_id: "77777777-7777-4777-8777-777777777777", name: "サーブ特訓" },
  },
  {
    review_id: "55555555-5555-4555-8555-555555555555",
    lesson_id: "66666666-6666-4666-8666-666666666666",
    rating: 3,
    comment: "もう少し時間がほしかった",
    submitted_at: "2026-10-01T09:30:00Z",
    lesson_menu: null,
    reply: { comment: "ご意見ありがとうございます。次回は長めに取ります。", replied_at: "2026-10-02T01:00:00Z" },
  },
];

/** 件数と平均の一覧（dl）を [見出し, 値] の組にする。 */
function summaryOf(list: HTMLElement) {
  return Array.from(list.querySelectorAll("dt")).map((dt) => [
    dt.textContent,
    dt.nextElementSibling?.textContent,
  ]);
}

type ReviewsApi = Pick<
  CoachApi,
  "getMyAccount" | "getCoachReviewSummary" | "listCoachReviews" | "replyToLessonReview"
>;

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
    replyToLessonReview: vi.fn<CoachApi["replyToLessonReview"]>(async (_reviewId, body) => ({
      ok: true,
      data: { comment: body.comment, replied_at: "2026-10-26T03:00:00Z" },
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
    // 評価したレッスンのメニューがあれば出し、なければこれまでどおり。
    expect(items[0]).toHaveTextContent("メニュー: サーブ特訓");
    expect(items[1]).not.toHaveTextContent("メニュー:");
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
      // 開いたときに出す「対応待ち」と、その上の実績が呼ぶ。
      listMyNotifications: noNotifications(),
      getMyLessonStats: vi.fn<CoachApi["getMyLessonStats"]>(async () => ({
        ok: true,
        data: EMPTY_LESSON_STATS,
      })),
      listMyCoachVideos: vi.fn<CoachApi["listMyCoachVideos"]>(async () => ({
        ok: true,
        data: { videos: [] },
      })),
      listMyCoachBookings: vi.fn<CoachApi["listMyCoachBookings"]>(async () => ({
        ok: true,
        data: { bookings: [] },
      })),
      listMessageThreads: vi.fn<CoachApi["listMessageThreads"]>(async () => ({
        ok: true,
        data: { threads: [] },
      })),
    });
    render(<CoachHome token="mock.coach.x" />);
    await userEvent.click(screen.getByRole("tab", { name: "自分の評価" }));
    const region = screen.getByRole("region", { name: "自分の評価" });

    expect(within(region).queryByLabelText("コーチ ID")).not.toBeInTheDocument();
    const summary = await within(region).findByLabelText("評価の件数と平均");
    expect(summaryOf(summary)).toContainEqual(["評価の件数", "2 件"]);
    expect(within(region).getByRole("list", { name: "評価の一覧" })).toBeInTheDocument();
  });
});

/** 開いて一覧を出し、各行（listitem）を返す。 */
async function renderLoaded(api: ReturnType<typeof mockApi>) {
  render(<MyCoachReviews api={api} />);
  const list = await screen.findByRole("list", { name: "評価の一覧" });
  return within(list).getAllByRole("listitem");
}

describe("自分の評価への返信", () => {
  test("返信のない行は「返信する」、ある行は保存済みの返信と時刻と「返信を書き直す」を出す", async () => {
    const items = await renderLoaded(mockApi());

    expect(within(items[0]!).getByRole("button", { name: "返信する" })).toBeInTheDocument();
    expect(within(items[0]!).queryByLabelText("あなたの返信")).not.toBeInTheDocument();
    const saved = within(items[1]!).getByRole("region", { name: "あなたの返信" });
    expect(saved).toHaveTextContent("ご意見ありがとうございます。次回は長めに取ります。");
    expect(saved).toHaveTextContent(formatDateTime("2026-10-02T01:00:00Z"));
    expect(within(items[1]!).getByRole("button", { name: "返信を書き直す" })).toBeInTheDocument();
  });

  test("「返信する」で欄を開き、書いて保存すると replyToLessonReview を呼び、行に返信を出す", async () => {
    const api = mockApi();
    const items = await renderLoaded(api);
    const row = items[0]!;

    await userEvent.click(within(row).getByRole("button", { name: "返信する" }));
    const form = within(row).getByRole("form", { name: "評価に返信する" });
    expect(within(form).getByLabelText("返信")).toHaveValue("");
    expect(form).toHaveTextContent("あと 200 文字");
    await userEvent.type(within(form).getByLabelText("返信"), "ありがとうございます！");
    expect(form).toHaveTextContent("あと 189 文字");
    await userEvent.click(within(form).getByRole("button", { name: "保存する" }));

    expect(api.replyToLessonReview).toHaveBeenCalledWith(REVIEWS[0]!.review_id, {
      comment: "ありがとうございます！",
    });
    expect(await within(row).findByRole("status")).toHaveTextContent("返信を保存しました。");
    expect(within(row).queryByRole("form", { name: "評価に返信する" })).not.toBeInTheDocument();
    const saved = within(row).getByRole("region", { name: "あなたの返信" });
    expect(saved).toHaveTextContent("ありがとうございます！");
    expect(saved).toHaveTextContent(formatDateTime("2026-10-26T03:00:00Z"));
    expect(within(row).getByRole("button", { name: "返信を書き直す" })).toBeInTheDocument();
    // 一覧は取り直さない。
    expect(api.listCoachReviews).toHaveBeenCalledTimes(1);
  });

  test("「返信を書き直す」は今の返信を欄に入れ、保存すると置き換える", async () => {
    const api = mockApi();
    const items = await renderLoaded(api);
    const row = items[1]!;

    await userEvent.click(within(row).getByRole("button", { name: "返信を書き直す" }));
    const input = within(row).getByLabelText("返信");
    expect(input).toHaveValue("ご意見ありがとうございます。次回は長めに取ります。");
    expect(within(row).queryByRole("region", { name: "あなたの返信" })).not.toBeInTheDocument();
    await userEvent.clear(input);
    await userEvent.type(input, "またお待ちしています。");
    await userEvent.click(within(row).getByRole("button", { name: "保存する" }));

    expect(api.replyToLessonReview).toHaveBeenCalledWith(REVIEWS[1]!.review_id, {
      comment: "またお待ちしています。",
    });
    const saved = await within(row).findByRole("region", { name: "あなたの返信" });
    expect(saved).toHaveTextContent("またお待ちしています。");
    expect(saved).not.toHaveTextContent("次回は長めに取ります。");
  });

  test("「やめる」で欄を閉じ、保存済みの返信はそのまま", async () => {
    const api = mockApi();
    const items = await renderLoaded(api);
    const row = items[1]!;

    await userEvent.click(within(row).getByRole("button", { name: "返信を書き直す" }));
    await userEvent.type(within(row).getByLabelText("返信"), "追記");
    await userEvent.click(within(row).getByRole("button", { name: "やめる" }));

    expect(within(row).queryByRole("form", { name: "評価に返信する" })).not.toBeInTheDocument();
    expect(within(row).getByRole("region", { name: "あなたの返信" })).toHaveTextContent(
      "ご意見ありがとうございます。次回は長めに取ります。",
    );
    expect(api.replyToLessonReview).not.toHaveBeenCalled();
  });

  test("空白だけの返信は送らずに理由を出す", async () => {
    const api = mockApi();
    const items = await renderLoaded(api);
    const row = items[0]!;

    await userEvent.click(within(row).getByRole("button", { name: "返信する" }));
    await userEvent.type(within(row).getByLabelText("返信"), "   ");
    await userEvent.click(within(row).getByRole("button", { name: "保存する" }));

    expect(within(row).getByRole("alert")).toHaveTextContent("返信を入力してください。");
    expect(api.replyToLessonReview).not.toHaveBeenCalled();
  });

  test("200 文字（コードポイント）を超える返信は送らずに理由を出し、絵文字は 1 文字と数える", async () => {
    const api = mockApi();
    const items = await renderLoaded(api);
    const row = items[0]!;

    await userEvent.click(within(row).getByRole("button", { name: "返信する" }));
    const input = within(row).getByLabelText("返信");
    // 絵文字（UTF-16 では 2 単位）でちょうど 200 文字なら超えていない。
    fireEvent.change(input, { target: { value: "🎾".repeat(200) } });
    expect(row).toHaveTextContent("あと 0 文字");
    fireEvent.change(input, { target: { value: "あ".repeat(201) } });
    expect(row).toHaveTextContent("200 文字を 1 文字超えています");
    await userEvent.click(within(row).getByRole("button", { name: "保存する" }));

    expect(within(row).getByRole("alert")).toHaveTextContent(
      "返信は 200 文字までです。1 文字減らしてください。",
    );
    expect(api.replyToLessonReview).not.toHaveBeenCalled();
  });

  test("保存できなければその文言を出して欄と入力を残す", async () => {
    const api = mockApi({
      replyToLessonReview: vi.fn<CoachApi["replyToLessonReview"]>(async () => ({
        ok: false,
        code: "forbidden",
        message: "権限がないためアクセスできません。",
      })),
    });
    const items = await renderLoaded(api);
    const row = items[0]!;

    await userEvent.click(within(row).getByRole("button", { name: "返信する" }));
    await userEvent.type(within(row).getByLabelText("返信"), "ありがとうございます");
    await userEvent.click(within(row).getByRole("button", { name: "保存する" }));

    expect(await within(row).findByRole("alert")).toHaveTextContent(
      "権限がないためアクセスできません。",
    );
    expect(within(row).getByLabelText("返信")).toHaveValue("ありがとうございます");
    expect(within(row).queryByRole("region", { name: "あなたの返信" })).not.toBeInTheDocument();
    expect(within(row).queryByRole("status")).not.toBeInTheDocument();
  });

  test("本部の画面と同じ一覧の部品でも、返信の欄を渡さなければ出さない", async () => {
    render(<CoachReviewList reviews={REVIEWS} />);

    expect(screen.queryByRole("button", { name: /返信/ })).not.toBeInTheDocument();
  });
});
