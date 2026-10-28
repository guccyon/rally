import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, test, vi } from "vitest";

import type { CoachApi } from "@/api/coach";
import { LiveSessionPanel } from "@/components/coach/LiveSessionPanel";
import { PublishSlotForm } from "@/components/coach/PublishSlotForm";
import { ReturnVideoReviewForm } from "@/components/coach/ReturnVideoReviewForm";
import { formatDateTime } from "@/components/labels";

const COACH_ID = "22222222-2222-4222-8222-222222222222";
const STUDENT_ID = "33333333-3333-4333-8333-333333333333";
const BOOKING_ID = "44444444-4444-4444-8444-444444444444";
const SESSION_ID = "55555555-5555-4555-8555-555555555555";
const VIDEO_ID = "66666666-6666-4666-8666-666666666666";

describe("枠の公開", () => {
  test("入力した枠を publishSlot に渡し、公開した枠を一覧に出す", async () => {
    const publishSlot = vi.fn<CoachApi["publishSlot"]>(async (body) => ({
      ok: true,
      data: {
        slot_id: "11111111-1111-4111-8111-111111111111",
        coach_id: COACH_ID,
        status: "open",
        ...body,
      },
    }));
    const user = userEvent.setup();
    render(<PublishSlotForm api={{ publishSlot }} />);

    await user.selectOptions(screen.getByLabelText("スポーツ"), "野球");
    await user.type(screen.getByLabelText("開始"), "2026-11-01T10:00");
    await user.type(screen.getByLabelText("終了"), "2026-11-01T11:00");
    await user.click(screen.getByRole("button", { name: "枠を公開" }));

    expect(publishSlot).toHaveBeenCalledWith({
      sport: "baseball",
      starts_at: new Date("2026-11-01T10:00").toISOString(),
      ends_at: new Date("2026-11-01T11:00").toISOString(),
    });
    expect(await screen.findByRole("status")).toHaveTextContent("枠を公開しました。");
    const row = screen.getByRole("row", { name: /野球/ });
    expect(row).toHaveTextContent("公開中");
  });

  test("終了が開始より前なら API を呼ばずに知らせる", async () => {
    const publishSlot = vi.fn<CoachApi["publishSlot"]>();
    const user = userEvent.setup();
    render(<PublishSlotForm api={{ publishSlot }} />);

    await user.type(screen.getByLabelText("開始"), "2026-11-01T11:00");
    await user.type(screen.getByLabelText("終了"), "2026-11-01T10:00");
    await user.click(screen.getByRole("button", { name: "枠を公開" }));

    expect(publishSlot).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent("終了は開始より後にしてください。");
  });

  test("API のエラーの文言を出す", async () => {
    const publishSlot = vi.fn<CoachApi["publishSlot"]>(async () => ({
      ok: false,
      message: "公開済みの枠と時間が重なっています。",
    }));
    const user = userEvent.setup();
    render(<PublishSlotForm api={{ publishSlot }} />);

    await user.type(screen.getByLabelText("開始"), "2026-11-01T10:00");
    await user.type(screen.getByLabelText("終了"), "2026-11-01T11:00");
    await user.click(screen.getByRole("button", { name: "枠を公開" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("公開済みの枠と時間が重なっています。");
    expect(screen.getByText("まだ公開していません。")).toBeInTheDocument();
  });
});

describe("添削の返却", () => {
  test("動画 ID とコメントを returnVideoReview に渡す", async () => {
    const returnVideoReview = vi.fn<CoachApi["returnVideoReview"]>(async (videoId, body) => ({
      ok: true,
      data: {
        review_id: "77777777-7777-4777-8777-777777777777",
        video_id: videoId,
        student_id: STUDENT_ID,
        coach_id: COACH_ID,
        status: "returned",
        comment: body.comment,
        returned_at: "2026-11-01T01:00:00Z",
      },
    }));
    const user = userEvent.setup();
    render(<ReturnVideoReviewForm api={{ returnVideoReview }} />);

    await user.type(screen.getByLabelText("動画 ID"), VIDEO_ID);
    await user.type(screen.getByLabelText("添削コメント"), "腰の回転を早めましょう");
    await user.click(screen.getByRole("button", { name: "添削を返却" }));

    expect(returnVideoReview).toHaveBeenCalledWith(VIDEO_ID, { comment: "腰の回転を早めましょう" });
    expect(await screen.findByRole("status")).toHaveTextContent("添削を返却しました");
    expect(screen.getByLabelText("添削コメント")).toHaveValue("");
  });

  test("コメントが空白だけなら API を呼ばない", async () => {
    const returnVideoReview = vi.fn<CoachApi["returnVideoReview"]>();
    const user = userEvent.setup();
    render(<ReturnVideoReviewForm api={{ returnVideoReview }} />);

    await user.type(screen.getByLabelText("動画 ID"), VIDEO_ID);
    await user.type(screen.getByLabelText("添削コメント"), "   ");
    await user.click(screen.getByRole("button", { name: "添削を返却" }));

    expect(returnVideoReview).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent("添削コメントを入力してください。");
  });
});

describe("通話への参加", () => {
  const HOUR = 60 * 60 * 1000;
  const at = (offsetHours: number) => new Date(Date.now() + offsetHours * HOUR).toISOString();

  // API が返す並び（スロットの開始時刻の古い順）。終わった 2 件・通話中の時間帯の 1 件・これからの 2 件。
  function coachBooking(
    id: string,
    startsIn: number,
    status: "pending" | "confirmed" | "cancelled",
    studentName?: string,
  ) {
    return {
      booking: {
        booking_id: id,
        slot_id: id.replace(/^./, "1"),
        student_id: STUDENT_ID,
        coach_id: COACH_ID,
        is_trial: false,
        status,
        starts_at: at(startsIn),
        created_at: at(-100),
        confirmed_at: status === "confirmed" ? at(-99) : null,
        cancelled_at: status === "cancelled" ? at(-98) : null,
      },
      slot: {
        slot_id: id.replace(/^./, "1"),
        coach_id: COACH_ID,
        sport: "tennis" as const,
        starts_at: at(startsIn),
        ends_at: at(startsIn + 1),
        status: "booked" as const,
      },
      ...(studentName ? { student_display_name: studentName } : {}),
    };
  }
  const BOOKINGS = [
    coachBooking("a0000000-0000-4000-8000-000000000001", -48, "confirmed", "古田さん"),
    coachBooking("a0000000-0000-4000-8000-000000000002", -24, "cancelled", "中村さん"),
    coachBooking(BOOKING_ID, -0.5, "confirmed", "佐藤さん"),
    coachBooking("a0000000-0000-4000-8000-000000000004", 24, "pending", "鈴木さん"),
    coachBooking("a0000000-0000-4000-8000-000000000005", 48, "confirmed"),
  ];

  const started = {
    session_id: SESSION_ID,
    booking_id: BOOKING_ID,
    student_id: STUDENT_ID,
    coach_id: COACH_ID,
    status: "in_progress" as const,
    started_at: "2026-11-01T01:00:00Z",
    ended_at: null,
    duration_sec: null,
    room_token: "mock-room",
  };

  function listOf(bookings: typeof BOOKINGS) {
    return vi.fn<CoachApi["listMyCoachBookings"]>(async () => ({ ok: true, data: { bookings } }));
  }

  function rowTexts(table: HTMLElement) {
    return within(table)
      .getAllByRole("row")
      .slice(1)
      .map((row) => within(row).getAllByRole("cell")[1]!.textContent);
  }

  test("これからの予約を API の順で上に、終わった予約を新しい順で畳んで下に出す", async () => {
    render(
      <LiveSessionPanel
        api={{
          listMyCoachBookings: listOf(BOOKINGS),
          startLiveLessonSession: vi.fn(),
          endLiveLessonSession: vi.fn(),
        }}
      />,
    );

    const upcoming = await screen.findByRole("table", { name: "これからの予約" });
    expect(rowTexts(upcoming)).toEqual(["佐藤さん", "鈴木さん", "（名前を表示できません）"]);
    const sato = within(upcoming).getByRole("row", { name: /佐藤さん/ });
    expect(sato).toHaveTextContent(formatDateTime(BOOKINGS[2]!.slot.starts_at));
    expect(sato).toHaveTextContent("確定済み");
    expect(within(sato).getByRole("button", { name: "通話を開始" })).toBeEnabled();
    // 確定していない予約からは始められない。
    const suzuki = within(upcoming).getByRole("row", { name: /鈴木さん/ });
    expect(suzuki).toHaveTextContent("確定待ち");
    expect(within(suzuki).queryByRole("button")).not.toBeInTheDocument();

    const past = screen.getByRole("table", { name: "終わった予約" });
    expect(rowTexts(past)).toEqual(["中村さん", "古田さん"]);
    expect(within(past).getByRole("row", { name: /中村さん/ })).toHaveTextContent("キャンセル済み");
    expect(within(past).queryByRole("button")).not.toBeInTheDocument();
    const details = past.closest("details")!;
    expect(details).not.toHaveAttribute("open");
    expect(details.querySelector("summary")).toHaveTextContent("終わった予約（2 件）");

    expect(screen.queryByLabelText("予約 ID")).not.toBeInTheDocument();
    expect(screen.queryByText(/未実装/)).not.toBeInTheDocument();
  });

  test("確定した予約の行から通話を開始し、同じセッションを終了する", async () => {
    const startLiveLessonSession = vi.fn<CoachApi["startLiveLessonSession"]>(async () => ({
      ok: true,
      data: started,
    }));
    const endLiveLessonSession = vi.fn<CoachApi["endLiveLessonSession"]>(async () => ({
      ok: true,
      data: {
        ...started,
        status: "ended",
        ended_at: "2026-11-01T01:30:00Z",
        duration_sec: 1800,
        room_token: null,
      },
    }));
    const user = userEvent.setup();
    render(
      <LiveSessionPanel
        api={{
          listMyCoachBookings: listOf(BOOKINGS),
          startLiveLessonSession,
          endLiveLessonSession,
        }}
      />,
    );

    const row = await screen.findByRole("row", { name: /佐藤さん/ });
    await user.click(within(row).getByRole("button", { name: "通話を開始" }));

    expect(startLiveLessonSession).toHaveBeenCalledWith({ booking_id: BOOKING_ID });
    expect(await screen.findByText("通話中")).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("通話を開始しました。");
    for (const button of screen.getAllByRole("button", { name: "通話を開始" })) {
      expect(button).toBeDisabled();
    }

    await user.click(screen.getByRole("button", { name: "通話を終了" }));

    expect(endLiveLessonSession).toHaveBeenCalledWith(SESSION_ID);
    expect(await screen.findByText("1800 秒")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "通話を終了" })).not.toBeInTheDocument();
  });

  test("開始できなければ文言を出し、終了ボタンを出さない", async () => {
    const startLiveLessonSession = vi.fn<CoachApi["startLiveLessonSession"]>(async () => ({
      ok: false,
      message: "この通話はすでに開始済みか、終了しています。",
    }));
    const user = userEvent.setup();
    render(
      <LiveSessionPanel
        api={{
          listMyCoachBookings: listOf(BOOKINGS),
          startLiveLessonSession,
          endLiveLessonSession: vi.fn(),
        }}
      />,
    );

    const row = await screen.findByRole("row", { name: /佐藤さん/ });
    await user.click(within(row).getByRole("button", { name: "通話を開始" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("すでに開始済みか");
    expect(screen.queryByRole("button", { name: "通話を終了" })).not.toBeInTheDocument();
  });

  test("予約が 0 件ならその旨を出す", async () => {
    render(
      <LiveSessionPanel
        api={{
          listMyCoachBookings: listOf([]),
          startLiveLessonSession: vi.fn(),
          endLiveLessonSession: vi.fn(),
        }}
      />,
    );

    expect(await screen.findByText("まだ予約はありません。")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("これからの予約がなく終わった予約だけなら、これからの予約がない旨と終わった予約を出す", async () => {
    render(
      <LiveSessionPanel
        api={{
          listMyCoachBookings: listOf(BOOKINGS.slice(0, 2)),
          startLiveLessonSession: vi.fn(),
          endLiveLessonSession: vi.fn(),
        }}
      />,
    );

    expect(await screen.findByText("これからの予約はありません。")).toBeInTheDocument();
    expect(screen.getByRole("table", { name: "終わった予約" })).toBeInTheDocument();
  });

  test("一覧の API が失敗したら、その文言だけを出す", async () => {
    const listMyCoachBookings = vi.fn<CoachApi["listMyCoachBookings"]>(async () => ({
      ok: false,
      message: "権限がないためアクセスできません。",
    }));
    render(
      <LiveSessionPanel
        api={{ listMyCoachBookings, startLiveLessonSession: vi.fn(), endLiveLessonSession: vi.fn() }}
      />,
    );

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "通話を開始" })).not.toBeInTheDocument();
  });
});
