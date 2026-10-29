import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import { CoachChat } from "@/components/coach/CoachChat";
import { formatDateTime } from "@/components/labels";

const COACH_ID = "22222222-2222-4222-8222-222222222222";
const STUDENT_ID = "33333333-3333-4333-8333-333333333333";
const OTHER_STUDENT_ID = "33333333-3333-4333-8333-333333333334";
const THREAD_ID = "88888888-8888-4888-8888-888888888881";
const OTHER_THREAD_ID = "88888888-8888-4888-8888-888888888882";
const VIDEO_ID = "66666666-6666-4666-8666-666666666661";
const RETURNED_VIDEO_ID = "66666666-6666-4666-8666-666666666662";
const REVIEW_ID = "77777777-7777-4777-8777-777777777777";
const SYSTEM_ID = "00000000-0000-0000-0000-000000000000";

type ChatApi = Parameters<typeof CoachChat>[0]["api"];

let sequence = 0;
function message(
  fields: Partial<Schemas["Message"]> &
    Pick<Schemas["Message"], "kind" | "sender_role" | "body" | "sent_at">,
): Schemas["Message"] {
  sequence += 1;
  return {
    message_id: `99999999-9999-4999-8999-${String(sequence).padStart(12, "0")}`,
    thread_id: THREAD_ID,
    sender_id:
      fields.sender_role === "coach"
        ? COACH_ID
        : fields.sender_role === "student"
          ? STUDENT_ID
          : SYSTEM_ID,
    read_at: null,
    ...fields,
  };
}

function thread(
  threadId: string,
  studentId: string,
  unread: number,
  last: Schemas["Message"] | null,
): Schemas["MessageThread"] {
  return {
    thread_id: threadId,
    student_id: studentId,
    coach_id: COACH_ID,
    opened_at: "2026-10-01T01:00:00Z",
    last_message: last,
    unread_count: unread,
  };
}

function booking(studentId: string, name?: string): Schemas["CoachBooking"] {
  return {
    booking: {
      booking_id: `44444444-4444-4444-8444-${studentId.slice(-12)}`,
      slot_id: "55555555-5555-4555-8555-555555555555",
      student_id: studentId,
      coach_id: COACH_ID,
      is_trial: true,
      status: "confirmed",
      starts_at: "2026-10-02T01:00:00Z",
      created_at: "2026-10-01T01:00:00Z",
    },
    slot: {
      slot_id: "55555555-5555-4555-8555-555555555555",
      coach_id: COACH_ID,
      sport: "tennis",
      starts_at: "2026-10-02T01:00:00Z",
      ends_at: "2026-10-02T02:00:00Z",
      status: "booked",
      capacity: 1,
      remaining_seats: 0,
    },
    ...(name ? { student_display_name: name } : {}),
  };
}

const AI_COMMENT = "フォームの安定性に注目して、コーチの添削をお待ちください。";

const WAITING_VIDEO: Schemas["CoachVideo"] = {
  video_id: VIDEO_ID,
  submitted_at: "2026-10-20T01:00:00Z",
  review_status: "in_progress",
  student_display_name: "佐藤さん",
  ai_feedback: { comment: AI_COMMENT, created_at: "2026-10-20T01:00:05Z" },
};
const RETURNED_VIDEO: Schemas["CoachVideo"] = {
  video_id: RETURNED_VIDEO_ID,
  submitted_at: "2026-10-10T01:00:00Z",
  review_status: "returned",
  returned_at: "2026-10-12T01:00:00Z",
  student_display_name: "佐藤さん",
};

// 送った順。1 回目の依頼は返却済み、2 回目はまだ返していない。
const FIRST_REQUEST = message({
  kind: "video_review_request",
  sender_role: "student",
  body: "動画の添削を依頼しました",
  video_id: RETURNED_VIDEO_ID,
  sent_at: "2026-10-10T01:00:00Z",
});
const FIRST_RETURNED = message({
  kind: "video_review_returned",
  sender_role: "coach",
  body: "動画の添削を返しました",
  video_id: RETURNED_VIDEO_ID,
  review_id: REVIEW_ID,
  sent_at: "2026-10-12T01:00:00Z",
});
const STUDENT_TEXT = message({
  kind: "text",
  sender_role: "student",
  body: "ありがとうございます！",
  sent_at: "2026-10-13T01:00:00Z",
});
const COACH_TEXT = message({
  kind: "text",
  sender_role: "coach",
  body: "次も送ってください",
  sent_at: "2026-10-14T01:00:00Z",
  read_at: "2026-10-15T01:00:00Z",
});
const SECOND_REQUEST = message({
  kind: "video_review_request",
  sender_role: "student",
  body: "動画の添削を依頼しました",
  video_id: VIDEO_ID,
  sent_at: "2026-10-20T01:00:00Z",
});
const MESSAGES = [FIRST_REQUEST, FIRST_RETURNED, STUDENT_TEXT, COACH_TEXT, SECOND_REQUEST];

const THREADS = [
  thread(THREAD_ID, STUDENT_ID, 2, SECOND_REQUEST),
  thread(OTHER_THREAD_ID, OTHER_STUDENT_ID, 0, null),
];

function setup(overrides: Partial<ChatApi> = {}) {
  const api = {
    listMessageThreads: vi.fn<CoachApi["listMessageThreads"]>(async () => ({
      ok: true,
      data: { threads: THREADS },
    })),
    listMyCoachBookings: vi.fn<CoachApi["listMyCoachBookings"]>(async () => ({
      ok: true,
      data: { bookings: [booking(STUDENT_ID, "佐藤さん"), booking(OTHER_STUDENT_ID)] },
    })),
    listMessages: vi.fn<CoachApi["listMessages"]>(async (threadId) => ({
      ok: true,
      data: { thread_id: threadId, messages: threadId === THREAD_ID ? MESSAGES : [] },
    })),
    markMessageThreadRead: vi.fn<CoachApi["markMessageThreadRead"]>(async (threadId) => ({
      ok: true,
      data: {
        ...(THREADS.find((item) => item.thread_id === threadId) ?? THREADS[0]!),
        unread_count: 0,
      },
    })),
    sendMessage: vi.fn<CoachApi["sendMessage"]>(),
    listMyCoachVideos: vi.fn<CoachApi["listMyCoachVideos"]>(async () => ({
      ok: true,
      data: { videos: [WAITING_VIDEO, RETURNED_VIDEO] },
    })),
    returnVideoReview: vi.fn<CoachApi["returnVideoReview"]>(),
    ...overrides,
  };
  render(<CoachChat api={api} />);
  return { api, user: userEvent.setup() };
}

async function openThread(user: ReturnType<typeof userEvent.setup>, name: RegExp) {
  const list = await screen.findByRole("list", { name: "生徒とのチャット" });
  await user.click(within(list).getByRole("button", { name }));
  return screen.findByRole("list", { name: "メッセージ" });
}

describe("コーチのチャット", () => {
  test("スレッドを API の順に、生徒の名前・未読の数・最後のメッセージとともに出す", async () => {
    setup();

    const list = await screen.findByRole("list", { name: "生徒とのチャット" });
    const items = within(list).getAllByRole("button");
    expect(items.map((item) => item.textContent)).toEqual([
      expect.stringContaining("佐藤さん"),
      expect.stringContaining("（名前を表示できません）"),
    ]);
    expect(items[0]).toHaveTextContent("未読 2 件");
    expect(items[0]).toHaveTextContent("動画の添削の依頼が届きました");
    expect(items[0]).toHaveTextContent(formatDateTime(SECOND_REQUEST.sent_at));
    expect(items[1]).not.toHaveTextContent("未読");
    expect(items[1]).toHaveTextContent("まだメッセージはありません");
    // 選ぶまではメッセージを読まない。
    expect(screen.queryByRole("list", { name: "メッセージ" })).not.toBeInTheDocument();
  });

  test("スレッドを開くとメッセージを送った順に出し、既読にして未読の数を消す", async () => {
    const { api, user } = setup();

    const messages = await openThread(user, /佐藤さん/);
    expect(api.listMessages).toHaveBeenCalledWith(THREAD_ID);
    expect(api.markMessageThreadRead).toHaveBeenCalledWith(THREAD_ID);
    const items = within(messages).getAllByRole("listitem");
    expect(items).toHaveLength(5);
    expect(items[2]).toHaveTextContent("佐藤さん");
    expect(items[2]).toHaveTextContent("ありがとうございます！");
    expect(items[3]).toHaveTextContent("あなた");
    expect(items[3]).toHaveTextContent("次も送ってください");
    expect(items[3]).toHaveTextContent("既読");
    expect(items[2]).not.toHaveTextContent("既読");

    const list = screen.getByRole("list", { name: "生徒とのチャット" });
    expect(within(list).getByRole("button", { name: /佐藤さん/ })).not.toHaveTextContent("未読");
  });

  test("添削の依頼と返却をカードで出し、返却済みの依頼には「添削する」を出さない", async () => {
    const { user } = setup();
    await openThread(user, /佐藤さん/);

    const first = screen.getByRole("article", { name: "添削の依頼（1 回目）" });
    expect(first).toHaveTextContent("佐藤さんが動画を送って、添削を依頼しました。");
    expect(first).toHaveTextContent(`返却済み（${formatDateTime("2026-10-12T01:00:00Z")}）`);
    expect(within(first).queryByRole("button")).not.toBeInTheDocument();

    const returned = screen.getByRole("article", { name: "添削の返却（1 回目の依頼）" });
    expect(returned).toHaveTextContent("あなたが動画の添削を返しました。");

    const second = screen.getByRole("article", { name: "添削の依頼（2 回目）" });
    expect(second).toHaveTextContent("まだ添削を返していません。");
    expect(within(second).getByRole("button", { name: "添削する" })).toBeEnabled();
    // 種類の説明の文（body）はカードに出さない。
    expect(screen.queryByText("動画の添削を依頼しました")).not.toBeInTheDocument();
  });

  test("依頼のカードの「添削する」で添削の欄を開き、返却したらメッセージと動画を取り直す", async () => {
    const returnedMessage = message({
      kind: "video_review_returned",
      sender_role: "coach",
      body: "動画の添削を返しました",
      video_id: VIDEO_ID,
      review_id: REVIEW_ID,
      sent_at: "2026-11-01T01:00:00Z",
    });
    const listMessages = vi
      .fn<CoachApi["listMessages"]>()
      .mockResolvedValueOnce({ ok: true, data: { thread_id: THREAD_ID, messages: MESSAGES } })
      .mockResolvedValueOnce({
        ok: true,
        data: { thread_id: THREAD_ID, messages: [...MESSAGES, returnedMessage] },
      });
    const listMyCoachVideos = vi
      .fn<CoachApi["listMyCoachVideos"]>()
      .mockResolvedValueOnce({ ok: true, data: { videos: [WAITING_VIDEO, RETURNED_VIDEO] } })
      .mockResolvedValueOnce({
        ok: true,
        data: {
          videos: [
            { ...WAITING_VIDEO, review_status: "returned", returned_at: "2026-11-01T01:00:00Z" },
            RETURNED_VIDEO,
          ],
        },
      });
    const returnVideoReview = vi.fn<CoachApi["returnVideoReview"]>(async (videoId, body) => ({
      ok: true,
      data: {
        review_id: REVIEW_ID,
        video_id: videoId,
        student_id: STUDENT_ID,
        coach_id: COACH_ID,
        status: "returned",
        comment: body.comment,
        returned_at: "2026-11-01T01:00:00Z",
      },
    }));
    const { api, user } = setup({ listMessages, listMyCoachVideos, returnVideoReview });
    await openThread(user, /佐藤さん/);

    const card = screen.getByRole("article", { name: "添削の依頼（2 回目）" });
    await user.click(within(card).getByRole("button", { name: "添削する" }));
    const form = within(card).getByRole("form", { name: "添削を書く" });
    expect(form).toHaveTextContent("佐藤さんの動画");
    // 「動画の添削」と同じく、AI の一次フィードバックを参考として欄の上に出し、コメントには入れない。
    const ai = within(form).getByRole("region", { name: "AI の一次フィードバック（参考）" });
    expect(ai).toHaveTextContent(AI_COMMENT);
    expect(within(form).getByLabelText("添削コメント")).toHaveValue("");

    await user.type(within(form).getByLabelText("添削コメント"), "腰の回転を早めましょう");
    await user.click(within(form).getByRole("button", { name: "添削を返却" }));

    expect(returnVideoReview).toHaveBeenCalledWith(VIDEO_ID, { comment: "腰の回転を早めましょう" });
    expect(await screen.findByRole("status")).toHaveTextContent("佐藤さんに添削を返却しました");
    expect(listMessages).toHaveBeenCalledTimes(2);
    expect(listMyCoachVideos).toHaveBeenCalledTimes(2);
    expect(api.listMessageThreads).toHaveBeenCalledTimes(2);
    expect(screen.queryByRole("form", { name: "添削を書く" })).not.toBeInTheDocument();
    expect(screen.getByRole("article", { name: "添削の依頼（2 回目）" })).toHaveTextContent(
      "返却済み",
    );
    expect(screen.getByRole("article", { name: "添削の返却（2 回目の依頼）" })).toBeInTheDocument();
  });

  test("返却できなければ文言を出し、添削の欄を残す", async () => {
    const returnVideoReview = vi.fn<CoachApi["returnVideoReview"]>(async () => ({
      ok: false,
      message: "この動画の添削はすでに返却済みです。",
    }));
    const { api, user } = setup({ returnVideoReview });
    await openThread(user, /佐藤さん/);

    const card = screen.getByRole("article", { name: "添削の依頼（2 回目）" });
    await user.click(within(card).getByRole("button", { name: "添削する" }));
    await user.type(within(card).getByLabelText("添削コメント"), "腰の回転を早めましょう");
    await user.click(within(card).getByRole("button", { name: "添削を返却" }));

    expect(await within(card).findByRole("alert")).toHaveTextContent("すでに返却済みです。");
    expect(within(card).getByLabelText("添削コメント")).toHaveValue("腰の回転を早めましょう");
    expect(api.listMessages).toHaveBeenCalledTimes(1);
  });

  test("動画の一覧を読めなければ、依頼のカードに「添削する」を出さずその旨を出す", async () => {
    const listMyCoachVideos = vi.fn<CoachApi["listMyCoachVideos"]>(async () => ({
      ok: false,
      message: "サーバーに接続できません。",
    }));
    const { user } = setup({ listMyCoachVideos });
    await openThread(user, /佐藤さん/);

    const card = screen.getByRole("article", { name: "添削の依頼（2 回目）" });
    expect(card).toHaveTextContent("添削の状態を確かめられませんでした。");
    expect(within(card).queryByRole("button")).not.toBeInTheDocument();
  });

  test("メッセージを送ると、送ったメッセージを足して欄を空にし、スレッドの一覧を取り直す", async () => {
    const sendMessage = vi.fn<CoachApi["sendMessage"]>(async (threadId, body) => ({
      ok: true,
      data: message({
        kind: "text",
        sender_role: "coach",
        body: body.body ?? "",
        thread_id: threadId,
        sent_at: "2026-11-01T02:00:00Z",
      }),
    }));
    const { api, user } = setup({ sendMessage });
    const messages = await openThread(user, /佐藤さん/);

    await user.type(screen.getByLabelText("送るメッセージ"), "  明日もよろしくお願いします ");
    await user.click(screen.getByRole("button", { name: "送る" }));

    expect(sendMessage).toHaveBeenCalledWith(THREAD_ID, { body: "明日もよろしくお願いします" });
    const items = await within(messages).findAllByRole("listitem");
    expect(items).toHaveLength(6);
    expect(items[5]).toHaveTextContent("あなた");
    expect(items[5]).toHaveTextContent("明日もよろしくお願いします");
    expect(screen.getByLabelText("送るメッセージ")).toHaveValue("");
    expect(api.listMessageThreads).toHaveBeenCalledTimes(2);
  });

  test("空白だけのメッセージは送らず、送れなければ文言を出して欄を残す", async () => {
    const sendMessage = vi.fn<CoachApi["sendMessage"]>(async () => ({
      ok: false,
      message: "入力の内容が正しくありません。",
    }));
    const { user } = setup({ sendMessage });
    await openThread(user, /佐藤さん/);

    await user.type(screen.getByLabelText("送るメッセージ"), "   ");
    await user.click(screen.getByRole("button", { name: "送る" }));
    expect(sendMessage).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent("メッセージを入力してください。");

    await user.clear(screen.getByLabelText("送るメッセージ"));
    await user.type(screen.getByLabelText("送るメッセージ"), "こんにちは");
    await user.click(screen.getByRole("button", { name: "送る" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("入力の内容が正しくありません。");
    expect(screen.getByLabelText("送るメッセージ")).toHaveValue("こんにちは");
  });

  test("お知らせと知らない種類のメッセージは、本文（短い説明）として出す", async () => {
    const notice = message({
      kind: "text",
      sender_role: "system",
      body: "メンテナンスのお知らせ",
      sent_at: "2026-10-21T01:00:00Z",
    });
    // 契約は種類が後方互換に増えるとしている（AI コンシェルジュの発言など）。
    const unknown = {
      ...message({
        kind: "text",
        sender_role: "student",
        body: "スタンプを送りました",
        sent_at: "2026-10-22T01:00:00Z",
      }),
      kind: "sticker",
    } as unknown as Schemas["Message"];
    const listMessages = vi.fn<CoachApi["listMessages"]>(async () => ({
      ok: true,
      data: { thread_id: THREAD_ID, messages: [notice, unknown] },
    }));
    const { user } = setup({ listMessages });
    const messages = await openThread(user, /佐藤さん/);

    const items = within(messages).getAllByRole("listitem");
    expect(items[0]).toHaveTextContent("お知らせ");
    expect(items[0]).toHaveTextContent("メンテナンスのお知らせ");
    expect(items[1]).toHaveTextContent("佐藤さん");
    expect(items[1]).toHaveTextContent("スタンプを送りました");
  });

  test("スタンプは絵柄と言葉を大きめに、送り手と既読とともに出す。知らないスタンプは本文（短い説明）として出す", async () => {
    const studentStamp = message({
      kind: "stamp",
      sender_role: "student",
      body: "スタンプ: ありがとう",
      stamp_id: "thanks",
      sent_at: "2026-10-21T01:00:00Z",
    });
    const coachStamp = message({
      kind: "stamp",
      sender_role: "coach",
      body: "スタンプ: ナイスショット",
      stamp_id: "nice_shot",
      sent_at: "2026-10-21T02:00:00Z",
      read_at: "2026-10-21T03:00:00Z",
    });
    // 契約より新しい版で増えたスタンプ。
    const unknownStamp = {
      ...message({
        kind: "stamp",
        sender_role: "student",
        body: "スタンプ: またね",
        sent_at: "2026-10-21T04:00:00Z",
      }),
      stamp_id: "see_you",
    } as unknown as Schemas["Message"];
    const listMessages = vi.fn<CoachApi["listMessages"]>(async () => ({
      ok: true,
      data: {
        thread_id: THREAD_ID,
        messages: [studentStamp, coachStamp, unknownStamp],
      },
    }));
    const { user } = setup({ listMessages });
    const messages = await openThread(user, /佐藤さん/);

    const items = within(messages).getAllByRole("listitem");
    const thanks = within(items[0]!).getByRole("img", {
      name: "スタンプ: ありがとう",
    });
    expect(thanks).toHaveTextContent("🙏");
    expect(thanks).toHaveTextContent("ありがとう");
    expect(within(thanks).getByText("🙏")).toHaveClass("text-5xl");
    expect(items[0]).toHaveTextContent("佐藤さん");
    expect(items[0]).not.toHaveTextContent("既読");
    // 知っているスタンプは説明の文（body）に頼らない。
    expect(screen.queryByText("スタンプ: ありがとう")).not.toBeInTheDocument();

    expect(
      within(items[1]!).getByRole("img", { name: "スタンプ: ナイスショット" }),
    ).toHaveTextContent("🎯");
    expect(items[1]).toHaveTextContent("あなた");
    expect(items[1]).toHaveTextContent("既読");

    expect(within(items[2]!).queryByRole("img")).not.toBeInTheDocument();
    expect(items[2]).toHaveTextContent("佐藤さん");
    expect(items[2]).toHaveTextContent("スタンプ: またね");
  });

  test("スレッドの一覧の最後のメッセージがスタンプなら、絵柄と言葉で出す", async () => {
    const stamp = message({
      kind: "stamp",
      sender_role: "student",
      body: "スタンプ: ありがとう",
      stamp_id: "thanks",
      sent_at: "2026-10-21T01:00:00Z",
    });
    setup({
      listMessageThreads: vi.fn<CoachApi["listMessageThreads"]>(async () => ({
        ok: true,
        data: { threads: [thread(THREAD_ID, STUDENT_ID, 1, stamp)] },
      })),
    });

    const list = await screen.findByRole("list", { name: "生徒とのチャット" });
    expect(within(list).getByRole("button", { name: /佐藤さん/ })).toHaveTextContent(
      "スタンプ: 🙏 ありがとう",
    );
  });

  test("「スタンプ」で 8 種類を出し、選んだスタンプを送ってスレッドに足し、選ぶ欄を閉じる", async () => {
    const sendMessage = vi.fn<CoachApi["sendMessage"]>(async (threadId, request) => ({
      ok: true,
      data: message({
        kind: "stamp",
        sender_role: "coach",
        body: "スタンプ: がんばろう",
        stamp_id: request.stamp_id,
        thread_id: threadId,
        sent_at: "2026-11-01T02:00:00Z",
      }),
    }));
    const { api, user } = setup({ sendMessage });
    const messages = await openThread(user, /佐藤さん/);
    await user.type(screen.getByLabelText("送るメッセージ"), "下書き");

    const toggle = screen.getByRole("button", { name: "スタンプ" });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    await user.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    const picker = screen.getByRole("group", {
      name: "送るスタンプを選んでください",
    });
    expect(
      within(picker)
        .getAllByRole("button")
        .map((button) => button.textContent),
    ).toEqual([
      "🙏ありがとう",
      "👍いいね",
      "👌了解",
      "💪がんばろう",
      "🎯ナイスショット",
      "🍵おつかれさま",
      "❓質問",
      "🙆OK",
    ]);

    await user.click(
      within(picker).getByRole("button", {
        name: "スタンプ「がんばろう」を送る",
      }),
    );

    expect(sendMessage).toHaveBeenCalledWith(THREAD_ID, {
      stamp_id: "lets_go",
    });
    const items = await within(messages).findAllByRole("listitem");
    expect(items).toHaveLength(6);
    expect(within(items[5]!).getByRole("img", { name: "スタンプ: がんばろう" })).toHaveTextContent(
      "💪",
    );
    expect(items[5]).toHaveTextContent("あなた");
    expect(
      screen.queryByRole("group", { name: "送るスタンプを選んでください" }),
    ).not.toBeInTheDocument();
    // 本文の下書きは残す。
    expect(screen.getByLabelText("送るメッセージ")).toHaveValue("下書き");
    expect(api.listMessageThreads).toHaveBeenCalledTimes(2);
  });

  test("スタンプを送れなければ文言を出し、選ぶ欄を残す", async () => {
    const sendMessage = vi.fn<CoachApi["sendMessage"]>(async () => ({
      ok: false,
      message: "入力の内容が正しくありません。",
    }));
    const { user } = setup({ sendMessage });
    const messages = await openThread(user, /佐藤さん/);

    await user.click(screen.getByRole("button", { name: "スタンプ" }));
    await user.click(screen.getByRole("button", { name: "スタンプ「了解」を送る" }));

    expect(sendMessage).toHaveBeenCalledWith(THREAD_ID, { stamp_id: "got_it" });
    expect(await screen.findByRole("alert")).toHaveTextContent("入力の内容が正しくありません。");
    expect(screen.getByRole("group", { name: "送るスタンプを選んでください" })).toBeInTheDocument();
    expect(within(messages).getAllByRole("listitem")).toHaveLength(5);
  });

  test("メッセージがないスレッドはその旨を出す", async () => {
    const { user } = setup();
    const list = await screen.findByRole("list", { name: "生徒とのチャット" });
    await user.click(within(list).getByRole("button", { name: /名前を表示できません/ }));

    expect(await screen.findByText("まだメッセージはありません。")).toBeInTheDocument();
    expect(screen.getByLabelText("送るメッセージ")).toBeInTheDocument();
  });

  test("「新しいメッセージを確かめる」でメッセージを取り直す", async () => {
    const { api, user } = setup();
    await openThread(user, /佐藤さん/);

    await user.click(screen.getByRole("button", { name: "新しいメッセージを確かめる" }));
    expect(api.listMessages).toHaveBeenCalledTimes(2);
    expect(api.markMessageThreadRead).toHaveBeenCalledTimes(2);
  });

  test("スレッドが 0 件ならその旨を出す", async () => {
    setup({
      listMessageThreads: vi.fn<CoachApi["listMessageThreads"]>(async () => ({
        ok: true,
        data: { threads: [] },
      })),
    });

    expect(await screen.findByText(/まだチャットはありません。/)).toBeInTheDocument();
    expect(screen.queryByRole("list")).not.toBeInTheDocument();
  });

  test("スレッドの一覧の API が失敗したら、その文言だけを出す", async () => {
    setup({
      listMessageThreads: vi.fn<CoachApi["listMessageThreads"]>(async () => ({
        ok: false,
        message: "権限がないためアクセスできません。",
      })),
    });

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("list")).not.toBeInTheDocument();
  });

  test("メッセージの一覧の API が失敗したら、その文言を出し送信の欄を出さない", async () => {
    const listMessages = vi.fn<CoachApi["listMessages"]>(async () => ({
      ok: false,
      message: "対象が見つかりません（ID を確かめてください）。",
    }));
    const { api, user } = setup({ listMessages });
    const list = await screen.findByRole("list", { name: "生徒とのチャット" });
    await user.click(within(list).getByRole("button", { name: /佐藤さん/ }));

    expect(await screen.findByRole("alert")).toHaveTextContent("対象が見つかりません");
    expect(screen.queryByLabelText("送るメッセージ")).not.toBeInTheDocument();
    expect(api.markMessageThreadRead).not.toHaveBeenCalled();
  });
});
