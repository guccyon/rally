import { describe, expect, test, vi } from "vitest";

import { createApiClient } from "@/api/client";
import { coachApi } from "@/api/coach";

// Node の Request は相対 URL を解決しないので、テストではオリジンを付ける（ブラウザでは既定の "/api"）。
const BASE_URL = "http://localhost/api";

const SLOT = {
  slot_id: "11111111-1111-4111-8111-111111111111",
  coach_id: "22222222-2222-4222-8222-222222222222",
  sport: "tennis",
  starts_at: "2026-11-01T01:00:00Z",
  ends_at: "2026-11-01T02:00:00Z",
  status: "open",
};

function json(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

function setup(response: Response | Error) {
  const fetchMock = vi.fn<(request: Request) => Promise<Response>>(async () => {
    if (response instanceof Error) throw response;
    return response;
  });
  const client = createApiClient("mock.coach.abc", {
    baseUrl: BASE_URL,
    fetch: (input) => fetchMock(input as Request),
  });
  return { api: coachApi(client), fetchMock };
}

describe("coachApi", () => {
  test("publishSlot はトークンを付けて POST /api/slots を呼び、公開したスロットを返す", async () => {
    const { api, fetchMock } = setup(json(201, SLOT));
    const result = await api.publishSlot({
      sport: "tennis",
      starts_at: SLOT.starts_at,
      ends_at: SLOT.ends_at,
    });
    expect(result).toEqual({ ok: true, data: SLOT });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("POST");
    expect(new URL(request.url).pathname).toBe("/api/slots");
    expect(request.headers.get("Authorization")).toBe("Bearer mock.coach.abc");
    expect(await request.json()).toEqual({
      sport: "tennis",
      starts_at: SLOT.starts_at,
      ends_at: SLOT.ends_at,
    });
  });

  test("契約の ErrorCode は日本語の文言にする", async () => {
    const { api } = setup(json(409, { code: "slot_overlaps", message: "overlap" }));
    const result = await api.publishSlot({
      sport: "tennis",
      starts_at: SLOT.starts_at,
      ends_at: SLOT.ends_at,
    });
    expect(result).toEqual({ ok: false, code: "slot_overlaps", message: "公開済みの枠と時間が重なっています。" });
  });

  test("未知の ErrorCode は汎用の文言にする", async () => {
    const { api } = setup(json(409, { code: "something_new", message: "x" }));
    const result = await api.returnVideoReview("v", { comment: "c" });
    expect(result).toEqual({
      ok: false,
      message: "エラーが発生しました。時間をおいてやり直してください。",
    });
  });

  test("接続できなければその旨を返す", async () => {
    const { api } = setup(new TypeError("fetch failed"));
    const result = await api.endLiveLessonSession("s");
    expect(result).toEqual({ ok: false, message: "サーバーに接続できません。" });
  });

  test("returnVideoReview・通話の開始と終了は契約のパスを呼ぶ", async () => {
    const paths: string[] = [];
    const client = createApiClient("t", {
      baseUrl: BASE_URL,
      fetch: async (input) => {
        const request = input as Request;
        paths.push(`${request.method} ${new URL(request.url).pathname}`);
        return json(200, {});
      },
    });
    const api = coachApi(client);
    await api.returnVideoReview("vid-1", { comment: "よいフォーム" });
    await api.startLiveLessonSession({ booking_id: "bk-1" });
    await api.endLiveLessonSession("ss-1");
    expect(paths).toEqual([
      "POST /api/videos/vid-1/review",
      "POST /api/live-lesson-sessions",
      "POST /api/live-lesson-sessions/ss-1/end",
    ]);
  });

  test("getMyAccount はトークンを付けて GET /api/me を呼び、本人のアカウントを返す", async () => {
    const account = { account_id: SLOT.coach_id, role: "coach", display_name: "山田コーチ" };
    const { api, fetchMock } = setup(json(200, account));
    const result = await api.getMyAccount();
    expect(result).toEqual({ ok: true, data: account });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("GET");
    expect(new URL(request.url).pathname).toBe("/api/me");
    expect(request.headers.get("Authorization")).toBe("Bearer mock.coach.abc");
  });

  test("getMyAccount の 401 はログインし直す文言にする", async () => {
    const { api } = setup(json(401, { code: "unauthorized", message: "unauthorized" }));
    const result = await api.getMyAccount();
    expect(result).toEqual({
      ok: false,
      code: "unauthorized",
      message: "トークンがないか無効です。ログインし直してください。",
    });
  });

  test("updateMyCoachProfile はトークンを付けて PUT /api/me/profile を呼び、置き換えたあとのアカウントを返す", async () => {
    const account = {
      account_id: SLOT.coach_id,
      role: "coach",
      display_name: "山田コーチ",
      bio: "テニス歴 20 年です。",
    };
    const { api, fetchMock } = setup(json(200, account));
    const result = await api.updateMyCoachProfile({ bio: "テニス歴 20 年です。" });
    expect(result).toEqual({ ok: true, data: account });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("PUT");
    expect(new URL(request.url).pathname).toBe("/api/me/profile");
    expect(request.headers.get("Authorization")).toBe("Bearer mock.coach.abc");
    expect(await request.json()).toEqual({ bio: "テニス歴 20 年です。" });
  });

  test("updateMyCoachProfile の 422 は入力の文言にする", async () => {
    const { api } = setup(json(422, { code: "validation_failed", message: "too long" }));
    const result = await api.updateMyCoachProfile({ bio: "あ".repeat(401) });
    expect(result).toEqual({ ok: false, code: "validation_failed", message: "入力の内容が正しくありません。" });
  });

  test("getCoachReviewSummary はトークンを付けて GET /api/coaches/{coach_id}/review-summary を呼ぶ", async () => {
    const summary = { coach_id: SLOT.coach_id, review_count: 2, average_rating: 4.5 };
    const { api, fetchMock } = setup(json(200, summary));
    const result = await api.getCoachReviewSummary(SLOT.coach_id);
    expect(result).toEqual({ ok: true, data: summary });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("GET");
    expect(new URL(request.url).pathname).toBe(`/api/coaches/${SLOT.coach_id}/review-summary`);
    expect(request.headers.get("Authorization")).toBe("Bearer mock.coach.abc");
  });

  test("他のコーチの評価（403）はロールの文言にする", async () => {
    const { api } = setup(json(403, { code: "forbidden", message: "forbidden" }));
    const result = await api.getCoachReviewSummary("99999999-9999-4999-8999-999999999999");
    expect(result).toEqual({ ok: false, code: "forbidden", message: "権限がないためアクセスできません。" });
  });

  test("listCoachReviews はトークンを付けて GET /api/coaches/{coach_id}/reviews を呼び、返った順のまま返す", async () => {
    const list = {
      coach_id: SLOT.coach_id,
      reviews: [
        {
          review_id: "33333333-3333-4333-8333-333333333333",
          lesson_id: "44444444-4444-4444-8444-444444444444",
          rating: 4,
          comment: "分かりやすかった",
          submitted_at: "2026-10-20T03:00:00Z",
        },
      ],
    };
    const { api, fetchMock } = setup(json(200, list));
    const result = await api.listCoachReviews(SLOT.coach_id);
    expect(result).toEqual({ ok: true, data: list });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("GET");
    expect(new URL(request.url).pathname).toBe(`/api/coaches/${SLOT.coach_id}/reviews`);
    expect(request.headers.get("Authorization")).toBe("Bearer mock.coach.abc");
  });

  test("他のコーチの評価の一覧（403）は権限の文言にする", async () => {
    const { api } = setup(json(403, { code: "forbidden", message: "forbidden" }));
    const result = await api.listCoachReviews("99999999-9999-4999-8999-999999999999");
    expect(result).toEqual({ ok: false, code: "forbidden", message: "権限がないためアクセスできません。" });
  });

  test("listMyCoachBookings はトークンを付けて GET /api/coaches/me/bookings を呼び、返った順のまま返す", async () => {
    const list = {
      bookings: [
        {
          booking: {
            booking_id: "44444444-4444-4444-8444-444444444444",
            slot_id: SLOT.slot_id,
            student_id: "33333333-3333-4333-8333-333333333333",
            coach_id: SLOT.coach_id,
            is_trial: true,
            status: "confirmed",
            starts_at: SLOT.starts_at,
            created_at: "2026-10-20T03:00:00Z",
            confirmed_at: "2026-10-20T03:00:01Z",
            cancelled_at: null,
          },
          slot: { ...SLOT, status: "booked" },
          student_display_name: "佐藤さん",
        },
      ],
    };
    const { api, fetchMock } = setup(json(200, list));
    const result = await api.listMyCoachBookings();
    expect(result).toEqual({ ok: true, data: list });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("GET");
    expect(new URL(request.url).pathname).toBe("/api/coaches/me/bookings");
    expect(request.headers.get("Authorization")).toBe("Bearer mock.coach.abc");
  });

  test("listMyCoachBookings の 403 は権限の文言にする", async () => {
    const { api } = setup(json(403, { code: "forbidden", message: "forbidden" }));
    const result = await api.listMyCoachBookings();
    expect(result).toEqual({ ok: false, code: "forbidden", message: "権限がないためアクセスできません。" });
  });

  test("listMyCoachSlots はトークンを付けて GET /api/coaches/me/slots を呼び、返った順のまま返す", async () => {
    const list = {
      slots: [
        { slot: { ...SLOT, status: "booked" }, has_active_booking: true },
        {
          slot: { ...SLOT, slot_id: "11111111-1111-4111-8111-111111111112", status: "open" },
          has_active_booking: false,
        },
      ],
    };
    const { api, fetchMock } = setup(json(200, list));
    const result = await api.listMyCoachSlots();
    expect(result).toEqual({ ok: true, data: list });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("GET");
    expect(new URL(request.url).pathname).toBe("/api/coaches/me/slots");
    expect(request.headers.get("Authorization")).toBe("Bearer mock.coach.abc");
  });

  test("listMyCoachSlots の 403 は権限の文言にする", async () => {
    const { api } = setup(json(403, { code: "forbidden", message: "forbidden" }));
    const result = await api.listMyCoachSlots();
    expect(result).toEqual({ ok: false, code: "forbidden", message: "権限がないためアクセスできません。" });
  });

  test("withdrawMySlot はトークンを付けて POST /api/coaches/me/slots/{slot_id}/withdraw を呼び、閉じた枠を返す", async () => {
    const closed = { ...SLOT, status: "closed" };
    const { api, fetchMock } = setup(json(200, closed));
    const result = await api.withdrawMySlot(SLOT.slot_id);
    expect(result).toEqual({ ok: true, data: closed });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("POST");
    expect(new URL(request.url).pathname).toBe(`/api/coaches/me/slots/${SLOT.slot_id}/withdraw`);
    expect(request.headers.get("Authorization")).toBe("Bearer mock.coach.abc");
  });

  test("withdrawMySlot の 409 は ErrorCode の文言にする", async () => {
    const { api } = setup(json(409, { code: "slot_has_active_booking", message: "booked" }));
    const result = await api.withdrawMySlot(SLOT.slot_id);
    expect(result).toEqual({
      ok: false,
      code: "slot_has_active_booking",
      message: "予約が入っている枠は取り下げられません。",
    });
  });

  test("listMyCoachVideos はトークンを付けて GET /api/coaches/me/videos を呼び、返った順のまま返す", async () => {
    const list = {
      videos: [
        {
          video_id: "66666666-6666-4666-8666-666666666666",
          student_display_name: "佐藤さん",
          submitted_at: "2026-10-20T03:00:00Z",
          review_status: "in_progress",
        },
        {
          video_id: "66666666-6666-4666-8666-666666666667",
          submitted_at: "2026-10-18T03:00:00Z",
          review_status: "returned",
          returned_at: "2026-10-19T03:00:00Z",
        },
      ],
    };
    const { api, fetchMock } = setup(json(200, list));
    const result = await api.listMyCoachVideos();
    expect(result).toEqual({ ok: true, data: list });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("GET");
    expect(new URL(request.url).pathname).toBe("/api/coaches/me/videos");
    expect(request.headers.get("Authorization")).toBe("Bearer mock.coach.abc");
  });

  test("listMyCoachVideos の 403 は権限の文言にする", async () => {
    const { api } = setup(json(403, { code: "forbidden", message: "forbidden" }));
    const result = await api.listMyCoachVideos();
    expect(result).toEqual({ ok: false, code: "forbidden", message: "権限がないためアクセスできません。" });
  });
  const THREAD_ID = "88888888-8888-4888-8888-888888888888";
  const MESSAGE = {
    message_id: "99999999-9999-4999-8999-999999999999",
    thread_id: THREAD_ID,
    sender_id: "22222222-2222-4222-8222-222222222222",
    sender_role: "coach",
    kind: "text",
    body: "こんにちは",
    sent_at: "2026-10-20T03:00:00Z",
    read_at: null,
  };
  const THREAD = {
    thread_id: THREAD_ID,
    student_id: "33333333-3333-4333-8333-333333333333",
    coach_id: "22222222-2222-4222-8222-222222222222",
    opened_at: "2026-10-01T03:00:00Z",
    last_message: MESSAGE,
    unread_count: 0,
  };

  test("listMessageThreads はトークンを付けて GET /api/message-threads を呼び、返った順のまま返す", async () => {
    const other = {
      ...THREAD,
      thread_id: "88888888-8888-4888-8888-888888888889",
      last_message: null,
      unread_count: 3,
    };
    const list = { threads: [THREAD, other] };
    const { api, fetchMock } = setup(json(200, list));
    const result = await api.listMessageThreads();
    expect(result).toEqual({ ok: true, data: list });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("GET");
    expect(new URL(request.url).pathname).toBe("/api/message-threads");
    expect(request.headers.get("Authorization")).toBe("Bearer mock.coach.abc");
  });

  test("listMessages は GET /api/message-threads/{thread_id}/messages を呼び、404 は見つからない旨の文言にする", async () => {
    const list = { thread_id: THREAD_ID, messages: [MESSAGE] };
    const ok = setup(json(200, list));
    expect(await ok.api.listMessages(THREAD_ID)).toEqual({ ok: true, data: list });
    const request = ok.fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("GET");
    expect(new URL(request.url).pathname).toBe(`/api/message-threads/${THREAD_ID}/messages`);

    const missing = setup(json(404, { code: "not_found", message: "not found" }));
    expect(await missing.api.listMessages(THREAD_ID)).toEqual({
      ok: false,
      code: "not_found",
      message: "対象が見つかりません（ID を確かめてください）。",
    });
  });

  test("sendMessage は本文を POST /api/message-threads/{thread_id}/messages に送り、送ったメッセージを返す", async () => {
    const { api, fetchMock } = setup(json(201, MESSAGE));
    const result = await api.sendMessage(THREAD_ID, { body: "こんにちは" });
    expect(result).toEqual({ ok: true, data: MESSAGE });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("POST");
    expect(new URL(request.url).pathname).toBe(`/api/message-threads/${THREAD_ID}/messages`);
    expect(request.headers.get("Authorization")).toBe("Bearer mock.coach.abc");
    expect(await request.json()).toEqual({ body: "こんにちは" });
  });

  test("markMessageThreadRead は POST /api/message-threads/{thread_id}/read を呼び、既読にしたスレッドを返す", async () => {
    const { api, fetchMock } = setup(json(200, THREAD));
    const result = await api.markMessageThreadRead(THREAD_ID);
    expect(result).toEqual({ ok: true, data: THREAD });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("POST");
    expect(new URL(request.url).pathname).toBe(`/api/message-threads/${THREAD_ID}/read`);
  });

  const LESSON_MENU = {
    lesson_menu_id: "55555555-5555-4555-8555-555555555555",
    coach_id: SLOT.coach_id,
    sport: "tennis",
    lesson_type: "live_lesson",
    name: "フォア改善",
    description: "初心者向け",
    duration_minutes: 60,
    is_published: true,
    created_at: "2026-10-30T00:00:00Z",
  };

  test("listMyLessonMenus はトークンを付けて GET /api/coaches/me/lesson-menus を呼び、返った順のまま返す", async () => {
    const list = { coach_id: SLOT.coach_id, lesson_menus: [LESSON_MENU] };
    const { api, fetchMock } = setup(json(200, list));
    expect(await api.listMyLessonMenus()).toEqual({ ok: true, data: list });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("GET");
    expect(new URL(request.url).pathname).toBe("/api/coaches/me/lesson-menus");
    expect(request.headers.get("Authorization")).toBe("Bearer mock.coach.abc");
  });

  test("createMyLessonMenu は POST /api/coaches/me/lesson-menus に送り、作ったメニューを返す", async () => {
    const { api, fetchMock } = setup(json(201, LESSON_MENU));
    const body = {
      sport: "tennis" as const,
      lesson_type: "live_lesson" as const,
      name: "フォア改善",
      description: "初心者向け",
      duration_minutes: 60,
    };
    expect(await api.createMyLessonMenu(body)).toEqual({ ok: true, data: LESSON_MENU });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("POST");
    expect(new URL(request.url).pathname).toBe("/api/coaches/me/lesson-menus");
    expect(await request.json()).toEqual(body);
  });

  test("updateMyLessonMenu は PUT /api/coaches/me/lesson-menus/{lesson_menu_id} に送り、404 は見つからない旨の文言にする", async () => {
    const hidden = { ...LESSON_MENU, is_published: false };
    const { api, fetchMock } = setup(json(200, hidden));
    const body = {
      sport: "tennis" as const,
      lesson_type: "live_lesson" as const,
      name: "フォア改善",
      description: "初心者向け",
      duration_minutes: 60,
      is_published: false,
    };
    expect(await api.updateMyLessonMenu(LESSON_MENU.lesson_menu_id, body)).toEqual({
      ok: true,
      data: hidden,
    });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("PUT");
    expect(new URL(request.url).pathname).toBe(
      `/api/coaches/me/lesson-menus/${LESSON_MENU.lesson_menu_id}`,
    );
    expect(await request.json()).toEqual(body);

    const missing = setup(json(404, { code: "not_found", message: "not found" }));
    expect(await missing.api.updateMyLessonMenu(LESSON_MENU.lesson_menu_id, body)).toEqual({
      ok: false,
      code: "not_found",
      message: "対象が見つかりません（ID を確かめてください）。",
    });
  });

  test("getMyLessonStats はトークンを付けて GET /api/coaches/me/lesson-stats を呼び、今月と先月の実績を返す", async () => {
    const stats = {
      coach_id: SLOT.coach_id,
      current_month: {
        month: "2026-10",
        live_lesson_count: 3,
        async_lesson_count: 1,
        review_count: 2,
        average_rating: 4.5,
      },
      previous_month: {
        month: "2026-09",
        live_lesson_count: 0,
        async_lesson_count: 0,
        review_count: 0,
        average_rating: null,
      },
    };
    const { api, fetchMock } = setup(json(200, stats));
    const result = await api.getMyLessonStats();
    expect(result).toEqual({ ok: true, data: stats });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("GET");
    expect(new URL(request.url).pathname).toBe("/api/coaches/me/lesson-stats");
    expect(request.headers.get("Authorization")).toBe("Bearer mock.coach.abc");
  });

  test("getMyLessonStats の 403 は権限の文言にする", async () => {
    const { api } = setup(json(403, { code: "forbidden", message: "forbidden" }));
    const result = await api.getMyLessonStats();
    expect(result).toEqual({ ok: false, code: "forbidden", message: "権限がないためアクセスできません。" });
  });
});
