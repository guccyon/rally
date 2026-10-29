import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import { CoachHome } from "@/components/coach/CoachHome";
import { formatDateTime } from "@/components/labels";

// CoachHome が作る API を差し替える（Node の Request は相対 URL の "/api" を解決しない）。
const { coachApiMock } = vi.hoisted(() => ({ coachApiMock: vi.fn() }));
vi.mock("@/api/coach", () => ({ coachApi: coachApiMock }));

const COACH_ID = "22222222-2222-4222-8222-222222222222";
const STUDENT_A = "33333333-3333-4333-8333-333333333331";
const STUDENT_B = "33333333-3333-4333-8333-333333333332";
const THREAD_A = "88888888-8888-4888-8888-888888888881";
const THREAD_B = "88888888-8888-4888-8888-888888888882";

const HOUR = 60 * 60 * 1000;
const at = (offsetHours: number) => new Date(Date.now() + offsetHours * HOUR).toISOString();

function video(
  id: string,
  name: string,
  submittedAt: string,
  status: Schemas["VideoReviewStatus"],
): Schemas["CoachVideo"] {
  return {
    video_id: id,
    submitted_at: submittedAt,
    review_status: status,
    student_display_name: name,
    ...(status === "returned" ? { returned_at: "2026-10-25T01:00:00Z" } : {}),
  };
}

let bookingSequence = 0;
function booking(
  studentId: string,
  name: string,
  status: Schemas["BookingStatus"],
  startsIn: number,
): Schemas["CoachBooking"] {
  bookingSequence += 1;
  const slotId = `55555555-5555-4555-8555-${String(bookingSequence).padStart(12, "0")}`;
  return {
    booking: {
      booking_id: `44444444-4444-4444-8444-${String(bookingSequence).padStart(12, "0")}`,
      slot_id: slotId,
      student_id: studentId,
      coach_id: COACH_ID,
      is_trial: true,
      status,
      starts_at: at(startsIn),
      created_at: at(-72),
    },
    slot: {
      slot_id: slotId,
      coach_id: COACH_ID,
      sport: "tennis",
      starts_at: at(startsIn),
      ends_at: at(startsIn + 1),
      status: "booked",
      capacity: 1,
      remaining_seats: 0,
    },
    student_display_name: name,
  };
}

function thread(threadId: string, studentId: string, unread: number): Schemas["MessageThread"] {
  return {
    thread_id: threadId,
    student_id: studentId,
    coach_id: COACH_ID,
    opened_at: "2026-10-01T01:00:00Z",
    last_message: null,
    unread_count: unread,
  };
}

// API が返す並び（添削待ちは送られた時刻の古い順、その後に返却済み）。
const VIDEOS = [
  video("66666666-6666-4666-8666-666666666661", "佐藤さん", "2026-10-20T01:00:00Z", "in_progress"),
  video("66666666-6666-4666-8666-666666666662", "鈴木さん", "2026-10-21T01:00:00Z", "in_progress"),
  video("66666666-6666-4666-8666-666666666663", "佐藤さん", "2026-10-10T01:00:00Z", "returned"),
];

// API が返す並び（開始時刻の古い順）。確定待ちで終わったもの・確定済み・キャンセル済みは数えない。
const PENDING_BOOKING = booking(STUDENT_B, "鈴木さん", "pending", 24);
const BOOKINGS = [
  booking(STUDENT_A, "佐藤さん", "pending", -48),
  booking(STUDENT_A, "佐藤さん", "confirmed", 12),
  PENDING_BOOKING,
  booking(STUDENT_B, "鈴木さん", "cancelled", 36),
];

// API が返す並び（最後の出来事の新しい順）。未読のないスレッドは数えない。
const THREADS = [thread(THREAD_A, STUDENT_A, 2), thread(THREAD_B, STUDENT_B, 0)];

function mockApi(overrides: Partial<CoachApi> = {}) {
  const api = {
    listMyCoachVideos: vi.fn<CoachApi["listMyCoachVideos"]>(async () => ({
      ok: true,
      data: { videos: VIDEOS },
    })),
    listMyCoachBookings: vi.fn<CoachApi["listMyCoachBookings"]>(async () => ({
      ok: true,
      data: { bookings: BOOKINGS },
    })),
    listMessageThreads: vi.fn<CoachApi["listMessageThreads"]>(async () => ({
      ok: true,
      data: { threads: THREADS },
    })),
    listMessages: vi.fn<CoachApi["listMessages"]>(async (threadId) => ({
      ok: true,
      data: { thread_id: threadId, messages: [] },
    })),
    markMessageThreadRead: vi.fn<CoachApi["markMessageThreadRead"]>(async (threadId) => ({
      ok: true,
      data: { ...thread(threadId, STUDENT_A, 0) },
    })),
    ...overrides,
  };
  coachApiMock.mockReturnValue(api);
  return api;
}

function setup(overrides: Partial<CoachApi> = {}) {
  const api = mockApi(overrides);
  const user = userEvent.setup();
  render(<CoachHome token="mock.coach.x" />);
  return { api, user };
}

function tasksTab() {
  return screen.getByRole("tab", { name: /対応待ち/ });
}

beforeEach(() => {
  coachApiMock.mockReset();
});

describe("コーチの対応待ち", () => {
  test("開いたときは対応待ちを出し、添削待ち・確定待ち・未読のあるチャットを並べる", async () => {
    setup();

    expect(tasksTab()).toHaveAttribute("aria-selected", "true");
    const region = screen.getByRole("region", { name: "対応待ち" });

    const videos = await within(region).findByRole("list", { name: "添削待ちの動画" });
    const videoItems = within(videos).getAllByRole("listitem");
    expect(videoItems).toHaveLength(2);
    expect(videoItems[0]).toHaveTextContent("佐藤さん");
    expect(videoItems[0]).toHaveTextContent(formatDateTime(VIDEOS[0]!.submitted_at));
    expect(videoItems[1]).toHaveTextContent("鈴木さん");
    expect(within(region).getByRole("heading", { name: "添削待ちの動画（2 件）" })).toBeInTheDocument();

    const bookings = within(region).getByRole("list", { name: "確定待ちの予約" });
    const bookingItems = within(bookings).getAllByRole("listitem");
    expect(bookingItems).toHaveLength(1);
    expect(bookingItems[0]).toHaveTextContent("鈴木さん");
    expect(bookingItems[0]).toHaveTextContent(formatDateTime(PENDING_BOOKING.slot.starts_at));

    const threads = within(region).getByRole("list", { name: "未読のあるチャット" });
    const threadItems = within(threads).getAllByRole("listitem");
    expect(threadItems).toHaveLength(1);
    expect(threadItems[0]).toHaveTextContent("佐藤さん");
    expect(threadItems[0]).toHaveTextContent("未読 2 件");
  });

  test("メニューの「対応待ち」に未対応の件数（添削待ち＋確定待ち＋未読のスレッド）のバッジを出す", async () => {
    setup();

    const tab = await screen.findByRole("tab", { name: "対応待ち（未対応 4 件）" });
    expect(tab).toHaveTextContent(/^対応待ち4$/);
  });

  test("未対応が 0 件ならバッジを出さず、それぞれにない旨を出す", async () => {
    setup({
      listMyCoachVideos: vi.fn<CoachApi["listMyCoachVideos"]>(async () => ({
        ok: true,
        data: { videos: [VIDEOS[2]!] },
      })),
      listMyCoachBookings: vi.fn<CoachApi["listMyCoachBookings"]>(async () => ({
        ok: true,
        data: { bookings: [] },
      })),
      listMessageThreads: vi.fn<CoachApi["listMessageThreads"]>(async () => ({
        ok: true,
        data: { threads: [thread(THREAD_B, STUDENT_B, 0)] },
      })),
    });

    expect(await screen.findByText("添削待ちの動画はありません。")).toBeInTheDocument();
    expect(screen.getByText("確定待ちの予約はありません。")).toBeInTheDocument();
    expect(screen.getByText("まだ読んでいないメッセージはありません。")).toBeInTheDocument();
    expect(tasksTab()).toHaveAccessibleName("対応待ち");
  });

  test("一覧の API が 1 つ失敗したら、その欄に文言を出し、残りは出して数える", async () => {
    setup({
      listMyCoachVideos: vi.fn<CoachApi["listMyCoachVideos"]>(async () => ({
        ok: false,
        message: "サーバーに接続できません。",
      })),
    });

    expect(await screen.findByRole("alert")).toHaveTextContent("サーバーに接続できません。");
    expect(screen.queryByRole("list", { name: "添削待ちの動画" })).not.toBeInTheDocument();
    expect(screen.getByRole("list", { name: "確定待ちの予約" })).toBeInTheDocument();
    expect(tasksTab()).toHaveAccessibleName("対応待ち（未対応 2 件）");
  });

  test("添削待ちの「添削する」で動画の添削へ移り、その動画の添削の欄を開く", async () => {
    const { user } = setup({ returnVideoReview: vi.fn() });

    const videos = await screen.findByRole("list", { name: "添削待ちの動画" });
    const second = within(videos).getAllByRole("listitem")[1]!;
    await user.click(within(second).getByRole("button", { name: "添削する" }));

    expect(screen.getByRole("tab", { name: "動画の添削" })).toHaveAttribute("aria-selected", "true");
    const region = screen.getByRole("region", { name: "動画の添削" });
    const form = await within(region).findByRole("form", { name: "添削を書く" });
    expect(form).toHaveTextContent("鈴木さん");
    expect(within(region).getByRole("table", { name: "添削待ち" })).toHaveTextContent("選択中");
  });

  test("確定待ちの「予約を見る」で通話への参加（予約の一覧）へ移る", async () => {
    const { user } = setup({ startLiveLessonSession: vi.fn(), endLiveLessonSession: vi.fn() });

    const bookings = await screen.findByRole("list", { name: "確定待ちの予約" });
    await user.click(within(bookings).getByRole("button", { name: "予約を見る" }));

    expect(screen.getByRole("tab", { name: "通話への参加" })).toHaveAttribute("aria-selected", "true");
    const region = screen.getByRole("region", { name: "通話への参加" });
    expect(await within(region).findByRole("table", { name: "これからの予約" })).toHaveTextContent(
      "確定待ち",
    );
  });

  test("未読のあるチャットの「チャットを開く」でチャットへ移り、そのスレッドを開く。戻ると数え直す", async () => {
    const listMessageThreads = vi.fn<CoachApi["listMessageThreads"]>(async () => ({
      ok: true,
      data: { threads: THREADS },
    }));
    const { api, user } = setup({
      listMessageThreads,
      sendMessage: vi.fn(),
      returnVideoReview: vi.fn(),
    });

    const threads = await screen.findByRole("list", { name: "未読のあるチャット" });
    await user.click(within(threads).getByRole("button", { name: "チャットを開く" }));

    expect(screen.getByRole("tab", { name: "チャット" })).toHaveAttribute("aria-selected", "true");
    expect(await screen.findByRole("region", { name: "佐藤さんとのチャット" })).toBeInTheDocument();
    expect(api.listMessages).toHaveBeenCalledWith(THREAD_A);
    expect(api.markMessageThreadRead).toHaveBeenCalledWith(THREAD_A);

    // 既読にしたあとのスレッドの一覧では、未読がなくなっている。
    listMessageThreads.mockResolvedValue({
      ok: true,
      data: { threads: [thread(THREAD_A, STUDENT_A, 0), thread(THREAD_B, STUDENT_B, 0)] },
    });
    await user.click(tasksTab());

    expect(await screen.findByRole("tab", { name: "対応待ち（未対応 3 件）" })).toBeInTheDocument();
    expect(screen.getByText("まだ読んでいないメッセージはありません。")).toBeInTheDocument();
  });
});
