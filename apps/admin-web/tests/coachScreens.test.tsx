import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, test, vi } from "vitest";

import type { CoachApi } from "@/api/coach";
import { LiveSessionPanel } from "@/components/coach/LiveSessionPanel";
import { PublishSlotForm } from "@/components/coach/PublishSlotForm";
import { ReturnVideoReviewForm } from "@/components/coach/ReturnVideoReviewForm";

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

  test("予約 ID で通話を開始し、同じセッションを終了する", async () => {
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
    render(<LiveSessionPanel api={{ startLiveLessonSession, endLiveLessonSession }} />);

    await user.type(screen.getByLabelText("予約 ID"), BOOKING_ID);
    await user.click(screen.getByRole("button", { name: "通話を開始" }));

    expect(startLiveLessonSession).toHaveBeenCalledWith({ booking_id: BOOKING_ID });
    expect(await screen.findByText("通話中")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "通話を開始" })).toBeDisabled();

    await user.click(screen.getByRole("button", { name: "通話を終了" }));

    expect(endLiveLessonSession).toHaveBeenCalledWith(SESSION_ID);
    expect(await screen.findByText("1800 秒")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "通話を終了" })).not.toBeInTheDocument();
  });

  test("開始できなければ文言を出し、終了ボタンを出さない", async () => {
    const startLiveLessonSession = vi.fn<CoachApi["startLiveLessonSession"]>(async () => ({
      ok: false,
      message: "予約が確定していないか、キャンセルされています。",
    }));
    const user = userEvent.setup();
    render(
      <LiveSessionPanel api={{ startLiveLessonSession, endLiveLessonSession: vi.fn() }} />,
    );

    await user.type(screen.getByLabelText("予約 ID"), BOOKING_ID);
    await user.click(screen.getByRole("button", { name: "通話を開始" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("予約が確定していない");
    expect(screen.queryByRole("button", { name: "通話を終了" })).not.toBeInTheDocument();
  });
});
