import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
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
  const HOUR = 60 * 60 * 1000;
  const at = (offsetHours: number) => new Date(Date.now() + offsetHours * HOUR).toISOString();

  function coachSlot(
    id: string,
    sport: Schemas["Sport"],
    startsIn: number,
    status: Schemas["SlotStatus"],
    hasActiveBooking: boolean,
    seats: { capacity: number; remaining: number } = {
      capacity: 1,
      remaining: hasActiveBooking ? 0 : 1,
    },
  ): Schemas["CoachSlot"] {
    return {
      slot: {
        slot_id: id,
        coach_id: COACH_ID,
        sport,
        starts_at: at(startsIn),
        ends_at: at(startsIn + 1),
        status,
        capacity: seats.capacity,
        remaining_seats: seats.remaining,
      },
      has_active_booking: hasActiveBooking,
    };
  }
  // API が返す並び（開始時刻の古い順）。終わった 2 件・いま開いている時間帯の 1 件・これからの 2 件。
  const SLOTS = [
    coachSlot("c0000000-0000-4000-8000-000000000001", "golf", -48, "closed", false),
    coachSlot("c0000000-0000-4000-8000-000000000002", "soccer", -24, "booked", true),
    coachSlot("c0000000-0000-4000-8000-000000000003", "tennis", -0.5, "booked", true),
    coachSlot("c0000000-0000-4000-8000-000000000004", "basketball", 24, "open", false),
    coachSlot("c0000000-0000-4000-8000-000000000005", "golf", 48, "booked", true),
  ];

  function listOf(...lists: Schemas["CoachSlot"][][]) {
    const fn = vi.fn<CoachApi["listMyCoachSlots"]>();
    for (const slots of lists) fn.mockResolvedValueOnce({ ok: true, data: { slots } });
    return fn;
  }

  // メニューを結びつける試験は tests/slotLessonMenu.test.tsx。ここではメニューのないコーチ。
  function noMenus() {
    return vi.fn<CoachApi["listMyLessonMenus"]>(async () => ({
      ok: true,
      data: { coach_id: COACH_ID, lesson_menus: [] },
    }));
  }

  function firstCells(table: HTMLElement) {
    return within(table)
      .getAllByRole("row")
      .slice(1)
      .map((row) => within(row).getAllByRole("cell")[0]!.textContent);
  }

  test("これからの枠を API の順で上に、終わった枠を新しい順で畳んで下に出し、予約が入っているかを出す", async () => {
    render(<PublishSlotForm api={{ listMyLessonMenus: noMenus(), withdrawMySlot: vi.fn(), publishSlot: vi.fn(), listMyCoachSlots: listOf(SLOTS) }} />);

    const upcoming = await screen.findByRole("table", { name: "これからの枠" });
    expect(firstCells(upcoming)).toEqual(["テニス", "バスケットボール", "ゴルフ"]);
    const tennis = within(upcoming).getByRole("row", { name: /テニス/ });
    expect(tennis).toHaveTextContent(formatDateTime(SLOTS[2]!.slot.starts_at));
    expect(tennis).toHaveTextContent("予約済み");
    expect(tennis).toHaveTextContent("予約あり");
    const basketball = within(upcoming).getByRole("row", { name: /バスケットボール/ });
    expect(basketball).toHaveTextContent("公開中");
    expect(basketball).toHaveTextContent("予約なし");

    const past = screen.getByRole("table", { name: "終わった枠" });
    expect(firstCells(past)).toEqual(["サッカー", "ゴルフ"]);
    expect(within(past).getByRole("row", { name: /ゴルフ/ })).toHaveTextContent("予約なし");
    const details = past.closest("details")!;
    expect(details).not.toHaveAttribute("open");
    expect(details.querySelector("summary")).toHaveTextContent("終わった枠（2 件）");

    expect(screen.queryByText(/未実装/)).not.toBeInTheDocument();
  });

  test("入力した枠を publishSlot に渡し、公開したら一覧を取り直す", async () => {
    const published = coachSlot("c0000000-0000-4000-8000-000000000006", "baseball", 72, "open", false);
    const publishSlot = vi.fn<CoachApi["publishSlot"]>(async () => ({
      ok: true,
      data: published.slot,
    }));
    const listMyCoachSlots = listOf([], [published]);
    const user = userEvent.setup();
    render(<PublishSlotForm api={{ listMyLessonMenus: noMenus(), withdrawMySlot: vi.fn(), publishSlot, listMyCoachSlots }} />);

    expect(await screen.findByText("まだ枠を公開していません。")).toBeInTheDocument();
    await user.selectOptions(screen.getByLabelText("スポーツ"), "野球");
    await user.type(screen.getByLabelText("開始"), "2026-11-01T10:00");
    await user.type(screen.getByLabelText("終了"), "2026-11-01T11:00");
    await user.click(screen.getByRole("button", { name: "枠を公開" }));

    expect(publishSlot).toHaveBeenCalledWith({
      sport: "baseball",
      starts_at: new Date("2026-11-01T10:00").toISOString(),
      ends_at: new Date("2026-11-01T11:00").toISOString(),
      capacity: 1,
    });
    expect(await screen.findByRole("status")).toHaveTextContent("枠を公開しました。");
    expect(listMyCoachSlots).toHaveBeenCalledTimes(2);
    const upcoming = await screen.findByRole("table", { name: "これからの枠" });
    expect(within(upcoming).getByRole("row", { name: /野球/ })).toHaveTextContent("公開中");
    expect(screen.queryByText("まだ枠を公開していません。")).not.toBeInTheDocument();
  });

  test("スポーツの欄に契約の全スポーツを出し、水泳の枠を公開すると一覧に「水泳」で出る", async () => {
    const published = coachSlot("c0000000-0000-4000-8000-000000000007", "swimming", 72, "open", false);
    const publishSlot = vi.fn<CoachApi["publishSlot"]>(async () => ({
      ok: true,
      data: published.slot,
    }));
    const listMyCoachSlots = listOf([], [published]);
    const user = userEvent.setup();
    render(<PublishSlotForm api={{ listMyLessonMenus: noMenus(), withdrawMySlot: vi.fn(), publishSlot, listMyCoachSlots }} />);

    await screen.findByText("まだ枠を公開していません。");
    const sportSelect = screen.getByLabelText("スポーツ");
    expect(within(sportSelect).getAllByRole("option").map((option) => option.textContent)).toEqual([
      "テニス",
      "ゴルフ",
      "野球",
      "サッカー",
      "バスケットボール",
      "水泳",
    ]);
    await user.selectOptions(sportSelect, "水泳");
    await user.type(screen.getByLabelText("開始"), "2026-11-01T10:00");
    await user.type(screen.getByLabelText("終了"), "2026-11-01T11:00");
    await user.click(screen.getByRole("button", { name: "枠を公開" }));

    expect(publishSlot).toHaveBeenCalledWith({
      sport: "swimming",
      starts_at: new Date("2026-11-01T10:00").toISOString(),
      ends_at: new Date("2026-11-01T11:00").toISOString(),
      capacity: 1,
    });
    const upcoming = await screen.findByRole("table", { name: "これからの枠" });
    expect(firstCells(upcoming)).toEqual(["水泳"]);
    expect(within(upcoming).getByRole("row", { name: /水泳/ })).toHaveTextContent("公開中");
  });

  test("定員の欄は 1〜6 人で既定は 1 人（マンツーマン）。選んだ定員を publishSlot に渡す", async () => {
    const published = coachSlot("c0000000-0000-4000-8000-000000000008", "tennis", 72, "open", false, {
      capacity: 4,
      remaining: 4,
    });
    const publishSlot = vi.fn<CoachApi["publishSlot"]>(async () => ({
      ok: true,
      data: published.slot,
    }));
    const listMyCoachSlots = listOf([], [published]);
    const user = userEvent.setup();
    render(<PublishSlotForm api={{ listMyLessonMenus: noMenus(), withdrawMySlot: vi.fn(), publishSlot, listMyCoachSlots }} />);

    await screen.findByText("まだ枠を公開していません。");
    const capacitySelect = screen.getByLabelText("定員");
    expect(within(capacitySelect).getAllByRole("option").map((option) => option.textContent)).toEqual([
      "1 人（マンツーマン）",
      "2 人",
      "3 人",
      "4 人",
      "5 人",
      "6 人",
    ]);
    expect(capacitySelect).toHaveDisplayValue("1 人（マンツーマン）");
    await user.selectOptions(capacitySelect, "4 人");
    await user.type(screen.getByLabelText("開始"), "2026-11-01T10:00");
    await user.type(screen.getByLabelText("終了"), "2026-11-01T11:00");
    await user.click(screen.getByRole("button", { name: "枠を公開" }));

    expect(publishSlot).toHaveBeenCalledWith({
      sport: "tennis",
      starts_at: new Date("2026-11-01T10:00").toISOString(),
      ends_at: new Date("2026-11-01T11:00").toISOString(),
      capacity: 4,
    });
    const upcoming = await screen.findByRole("table", { name: "これからの枠" });
    expect(within(upcoming).getByRole("row", { name: /テニス/ })).toHaveTextContent(
      "定員 4 人・残り 4 席",
    );
  });

  test("定員 2 以上の枠には定員と残りの席を出し、定員 1 の枠は予約の有無だけを出す", async () => {
    const slots = [
      coachSlot("c0000000-0000-4000-8000-000000000009", "soccer", 24, "open", true, {
        capacity: 3,
        remaining: 1,
      }),
      coachSlot("c0000000-0000-4000-8000-000000000010", "baseball", 48, "booked", true, {
        capacity: 6,
        remaining: 0,
      }),
      coachSlot("c0000000-0000-4000-8000-000000000011", "tennis", 72, "open", false),
    ];
    render(<PublishSlotForm api={{ listMyLessonMenus: noMenus(), withdrawMySlot: vi.fn(), publishSlot: vi.fn(), listMyCoachSlots: listOf(slots) }} />);

    const upcoming = await screen.findByRole("table", { name: "これからの枠" });
    const soccer = within(upcoming).getByRole("row", { name: /サッカー/ });
    expect(soccer).toHaveTextContent("予約あり");
    expect(soccer).toHaveTextContent("定員 3 人・残り 1 席");
    expect(within(upcoming).getByRole("row", { name: /野球/ })).toHaveTextContent(
      "定員 6 人・残り 0 席",
    );
    const tennis = within(upcoming).getByRole("row", { name: /テニス/ });
    expect(tennis).toHaveTextContent("予約なし");
    expect(tennis).not.toHaveTextContent(/定員|残り/);
  });

  test("枠が 0 件ならその旨を出す", async () => {
    render(<PublishSlotForm api={{ listMyLessonMenus: noMenus(), withdrawMySlot: vi.fn(), publishSlot: vi.fn(), listMyCoachSlots: listOf([]) }} />);

    expect(await screen.findByText("まだ枠を公開していません。")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("これからの枠がなく終わった枠だけなら、これからの枠がない旨と終わった枠を出す", async () => {
    render(
      <PublishSlotForm api={{ listMyLessonMenus: noMenus(), withdrawMySlot: vi.fn(), publishSlot: vi.fn(), listMyCoachSlots: listOf(SLOTS.slice(0, 2)) }} />,
    );

    expect(await screen.findByText("これからの枠はありません。")).toBeInTheDocument();
    expect(screen.getByRole("table", { name: "終わった枠" })).toBeInTheDocument();
  });

  test("一覧の API が失敗したら、その文言を出す", async () => {
    const listMyCoachSlots = vi.fn<CoachApi["listMyCoachSlots"]>(async () => ({
      ok: false,
      message: "権限がないためアクセスできません。",
    }));
    render(<PublishSlotForm api={{ listMyLessonMenus: noMenus(), withdrawMySlot: vi.fn(), publishSlot: vi.fn(), listMyCoachSlots }} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("終了が開始より前なら API を呼ばずに知らせる", async () => {
    const publishSlot = vi.fn<CoachApi["publishSlot"]>();
    const listMyCoachSlots = listOf([]);
    const user = userEvent.setup();
    render(<PublishSlotForm api={{ listMyLessonMenus: noMenus(), withdrawMySlot: vi.fn(), publishSlot, listMyCoachSlots }} />);

    await screen.findByText("まだ枠を公開していません。");
    await user.type(screen.getByLabelText("開始"), "2026-11-01T11:00");
    await user.type(screen.getByLabelText("終了"), "2026-11-01T10:00");
    await user.click(screen.getByRole("button", { name: "枠を公開" }));

    expect(publishSlot).not.toHaveBeenCalled();
    expect(listMyCoachSlots).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("alert")).toHaveTextContent("終了は開始より後にしてください。");
  });

  test("公開の API のエラーの文言を出し、一覧は取り直さない", async () => {
    const publishSlot = vi.fn<CoachApi["publishSlot"]>(async () => ({
      ok: false,
      message: "公開済みの枠と時間が重なっています。",
    }));
    const listMyCoachSlots = listOf([]);
    const user = userEvent.setup();
    render(<PublishSlotForm api={{ listMyLessonMenus: noMenus(), withdrawMySlot: vi.fn(), publishSlot, listMyCoachSlots }} />);

    await screen.findByText("まだ枠を公開していません。");
    await user.type(screen.getByLabelText("開始"), "2026-11-01T10:00");
    await user.type(screen.getByLabelText("終了"), "2026-11-01T11:00");
    await user.click(screen.getByRole("button", { name: "枠を公開" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("公開済みの枠と時間が重なっています。");
    expect(listMyCoachSlots).toHaveBeenCalledTimes(1);
    expect(screen.getByText("まだ枠を公開していません。")).toBeInTheDocument();
  });
});

describe("添削の返却", () => {
  // API が返す並び（添削待ちを送られた時刻の古い順、その後に返却済みを返却した時刻の新しい順）。
  function coachVideo(
    id: string,
    submittedAt: string,
    returnedAt?: string,
    studentName?: string,
  ): Schemas["CoachVideo"] {
    return {
      video_id: id,
      submitted_at: submittedAt,
      review_status: returnedAt ? "returned" : "in_progress",
      ...(returnedAt ? { returned_at: returnedAt } : {}),
      ...(studentName ? { student_display_name: studentName } : {}),
    };
  }
  const OTHER_VIDEO_ID = "b0000000-0000-4000-8000-000000000002";
  const VIDEOS = [
    coachVideo(VIDEO_ID, "2026-10-20T01:00:00Z", undefined, "佐藤さん"),
    coachVideo(OTHER_VIDEO_ID, "2026-10-21T01:00:00Z"),
    coachVideo("b0000000-0000-4000-8000-000000000003", "2026-10-10T01:00:00Z", "2026-10-15T01:00:00Z", "古田さん"),
    coachVideo("b0000000-0000-4000-8000-000000000004", "2026-10-11T01:00:00Z", "2026-10-12T01:00:00Z", "中村さん"),
  ];

  function listOf(...lists: Schemas["CoachVideo"][][]) {
    const fn = vi.fn<CoachApi["listMyCoachVideos"]>();
    for (const videos of lists) fn.mockResolvedValueOnce({ ok: true, data: { videos } });
    return fn;
  }

  function returned(videoId: string, comment: string) {
    return {
      ok: true as const,
      data: {
        review_id: "77777777-7777-4777-8777-777777777777",
        video_id: videoId,
        student_id: STUDENT_ID,
        coach_id: COACH_ID,
        status: "returned" as const,
        comment,
        returned_at: "2026-11-01T01:00:00Z",
      },
    };
  }

  function firstCells(table: HTMLElement) {
    return within(table)
      .getAllByRole("row")
      .slice(1)
      .map((row) => within(row).getAllByRole("cell")[0]!.textContent);
  }

  test("添削待ちを API の順で上に、返却済みを API の順のまま畳んで下に出す", async () => {
    render(
      <ReturnVideoReviewForm api={{ listMyCoachVideos: listOf(VIDEOS), returnVideoReview: vi.fn() }} />,
    );

    const waiting = await screen.findByRole("table", { name: "添削待ち" });
    expect(firstCells(waiting)).toEqual(["佐藤さん", "（名前を表示できません）"]);
    const sato = within(waiting).getByRole("row", { name: /佐藤さん/ });
    expect(sato).toHaveTextContent(formatDateTime(VIDEOS[0]!.submitted_at));
    expect(within(sato).getByRole("button", { name: "添削する" })).toBeEnabled();

    const done = screen.getByRole("table", { name: "返却済み" });
    expect(firstCells(done)).toEqual(["古田さん", "中村さん"]);
    expect(within(done).getByRole("row", { name: /古田さん/ })).toHaveTextContent(
      formatDateTime("2026-10-15T01:00:00Z"),
    );
    expect(within(done).queryByRole("button")).not.toBeInTheDocument();
    const details = done.closest("details")!;
    expect(details).not.toHaveAttribute("open");
    expect(details.querySelector("summary")).toHaveTextContent("返却済み（2 件）");

    // 選ぶまではコメントの欄を出さない。動画 ID の入力欄と「未実装」の注記はない。
    expect(screen.queryByLabelText("添削コメント")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("動画 ID")).not.toBeInTheDocument();
    expect(screen.queryByText(/未実装/)).not.toBeInTheDocument();
  });

  test("行を選んでコメントを書き、返却したら一覧を取り直す", async () => {
    const listMyCoachVideos = listOf(VIDEOS, [
      VIDEOS[1]!,
      { ...VIDEOS[0]!, review_status: "returned", returned_at: "2026-11-01T01:00:00Z" },
      ...VIDEOS.slice(2),
    ]);
    const returnVideoReview = vi.fn<CoachApi["returnVideoReview"]>(async (videoId, body) =>
      returned(videoId, body.comment),
    );
    const user = userEvent.setup();
    render(<ReturnVideoReviewForm api={{ listMyCoachVideos, returnVideoReview }} />);

    const row = await screen.findByRole("row", { name: /佐藤さん/ });
    await user.click(within(row).getByRole("button", { name: "添削する" }));
    const form = screen.getByRole("form", { name: "添削を書く" });
    expect(form).toHaveTextContent("佐藤さんの動画");
    await user.type(screen.getByLabelText("添削コメント"), "腰の回転を早めましょう");
    await user.click(screen.getByRole("button", { name: "添削を返却" }));

    expect(returnVideoReview).toHaveBeenCalledWith(VIDEO_ID, { comment: "腰の回転を早めましょう" });
    expect(await screen.findByRole("status")).toHaveTextContent("佐藤さんに添削を返却しました");
    expect(listMyCoachVideos).toHaveBeenCalledTimes(2);
    expect(screen.queryByLabelText("添削コメント")).not.toBeInTheDocument();
    expect(firstCells(screen.getByRole("table", { name: "添削待ち" }))).toEqual([
      "（名前を表示できません）",
    ]);
    expect(firstCells(screen.getByRole("table", { name: "返却済み" }))).toEqual([
      "佐藤さん",
      "古田さん",
      "中村さん",
    ]);
  });

  const AI_COMMENT = "フォームの安定性に注目して、コーチの添削をお待ちください。";
  function aiFeedback(comment: string) {
    return { comment, created_at: "2026-10-20T01:00:05Z" };
  }

  test("添削する動画に AI の一次フィードバックがあれば、コメントの欄の上に参考として出し、返却には入れない", async () => {
    const listMyCoachVideos = listOf(
      [{ ...VIDEOS[0]!, ai_feedback: aiFeedback(AI_COMMENT) }, ...VIDEOS.slice(1)],
      VIDEOS.slice(1),
    );
    const returnVideoReview = vi.fn<CoachApi["returnVideoReview"]>(async (videoId, body) =>
      returned(videoId, body.comment),
    );
    const user = userEvent.setup();
    render(<ReturnVideoReviewForm api={{ listMyCoachVideos, returnVideoReview }} />);

    // 選ぶまでは出さない。
    const row = await screen.findByRole("row", { name: /佐藤さん/ });
    expect(screen.queryByText(AI_COMMENT)).not.toBeInTheDocument();
    await user.click(within(row).getByRole("button", { name: "添削する" }));

    const ai = screen.getByRole("region", { name: "AI の一次フィードバック（参考）" });
    expect(ai).toHaveTextContent(AI_COMMENT);
    expect(ai).toHaveTextContent("コーチの添削ではなく");
    // コメントの欄より前に出し、欄には入れない。
    const textarea = screen.getByLabelText("添削コメント");
    expect(ai.compareDocumentPosition(textarea) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(textarea).toHaveValue("");

    await user.type(textarea, "腰の回転を早めましょう");
    await user.click(screen.getByRole("button", { name: "添削を返却" }));
    expect(returnVideoReview).toHaveBeenCalledWith(VIDEO_ID, { comment: "腰の回転を早めましょう" });
    expect(screen.queryByText(AI_COMMENT)).not.toBeInTheDocument();
  });

  test("AI の一次フィードバックがない（null・載っていない・空白だけ）ときは何も出さない", async () => {
    const third = "b0000000-0000-4000-8000-000000000005";
    const listMyCoachVideos = listOf([
      { ...VIDEOS[0]!, ai_feedback: null },
      VIDEOS[1]!,
      { ...coachVideo(third, "2026-10-22T01:00:00Z", undefined, "田中さん"), ai_feedback: aiFeedback("  ") },
    ]);
    const user = userEvent.setup();
    render(<ReturnVideoReviewForm api={{ listMyCoachVideos, returnVideoReview: vi.fn() }} />);

    const waiting = await screen.findByRole("table", { name: "添削待ち" });
    for (const name of [/佐藤さん/, /名前を表示できません/, /田中さん/]) {
      await user.click(
        within(within(waiting).getByRole("row", { name })).getByRole("button", { name: "添削する" }),
      );
      expect(screen.getByLabelText("添削コメント")).toBeInTheDocument();
      expect(screen.queryByText(/AI の一次フィードバック/)).not.toBeInTheDocument();
    }
  });

  test("返却済みの行には AI の一次フィードバックを出さない", async () => {
    render(
      <ReturnVideoReviewForm
        api={{
          listMyCoachVideos: listOf([
            VIDEOS[0]!,
            { ...VIDEOS[2]!, ai_feedback: aiFeedback(AI_COMMENT) },
            VIDEOS[3]!,
          ]),
          returnVideoReview: vi.fn(),
        }}
      />,
    );

    const done = await screen.findByRole("table", { name: "返却済み" });
    expect(within(done).getByRole("row", { name: /古田さん/ })).not.toHaveTextContent(AI_COMMENT);
    expect(screen.queryByText(AI_COMMENT)).not.toBeInTheDocument();
    expect(screen.queryByText(/AI の一次フィードバック/)).not.toBeInTheDocument();
  });

  test("コメントが空白だけなら API を呼ばない", async () => {
    const returnVideoReview = vi.fn<CoachApi["returnVideoReview"]>();
    const user = userEvent.setup();
    render(<ReturnVideoReviewForm api={{ listMyCoachVideos: listOf(VIDEOS), returnVideoReview }} />);

    const row = await screen.findByRole("row", { name: /佐藤さん/ });
    await user.click(within(row).getByRole("button", { name: "添削する" }));
    await user.type(screen.getByLabelText("添削コメント"), "   ");
    await user.click(screen.getByRole("button", { name: "添削を返却" }));

    expect(returnVideoReview).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent("添削コメントを入力してください。");
  });

  test("返却できなければ文言を出し、コメントの欄を残す", async () => {
    const listMyCoachVideos = listOf(VIDEOS);
    const returnVideoReview = vi.fn<CoachApi["returnVideoReview"]>(async () => ({
      ok: false,
      message: "この動画の添削はすでに返却済みです。",
    }));
    const user = userEvent.setup();
    render(<ReturnVideoReviewForm api={{ listMyCoachVideos, returnVideoReview }} />);

    const row = await screen.findByRole("row", { name: /佐藤さん/ });
    await user.click(within(row).getByRole("button", { name: "添削する" }));
    await user.type(screen.getByLabelText("添削コメント"), "腰の回転を早めましょう");
    await user.click(screen.getByRole("button", { name: "添削を返却" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("すでに返却済みです。");
    expect(screen.getByLabelText("添削コメント")).toHaveValue("腰の回転を早めましょう");
    expect(listMyCoachVideos).toHaveBeenCalledTimes(1);
  });

  test("動画が 0 件ならその旨を出す", async () => {
    render(
      <ReturnVideoReviewForm api={{ listMyCoachVideos: listOf([]), returnVideoReview: vi.fn() }} />,
    );

    expect(await screen.findByText("まだ動画は届いていません。")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("添削待ちがなく返却済みだけなら、添削待ちがない旨と返却済みを出す", async () => {
    render(
      <ReturnVideoReviewForm
        api={{ listMyCoachVideos: listOf(VIDEOS.slice(2)), returnVideoReview: vi.fn() }}
      />,
    );

    expect(await screen.findByText("添削待ちの動画はありません。")).toBeInTheDocument();
    expect(screen.getByRole("table", { name: "返却済み" })).toBeInTheDocument();
  });

  test("一覧の API が失敗したら、その文言だけを出す", async () => {
    const listMyCoachVideos = vi.fn<CoachApi["listMyCoachVideos"]>(async () => ({
      ok: false,
      message: "権限がないためアクセスできません。",
    }));
    render(<ReturnVideoReviewForm api={{ listMyCoachVideos, returnVideoReview: vi.fn() }} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "添削する" })).not.toBeInTheDocument();
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
        capacity: 1,
        remaining_seats: 0,
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
