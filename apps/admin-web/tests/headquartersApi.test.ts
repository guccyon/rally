import { describe, expect, test, vi } from "vitest";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";

// Node の Request は相対 URL を解決しないので、テストではオリジンを付ける（ブラウザでは既定の "/api"）。
const BASE_URL = "http://localhost/api";

const REPORT = {
  window_start: "2026-09-27T00:00:00Z",
  window_end: "2026-10-27T00:00:00Z",
  sports: [
    { sport: "tennis", status: "insufficient", published_slots: 10, booking_students: 8 },
    { sport: "golf", status: "sufficient", published_slots: 0, booking_students: 0 },
  ],
};

function json(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

function setup(response: Response) {
  const fetchMock = vi.fn<(request: Request) => Promise<Response>>(async () => response);
  const client = createApiClient("mock.headquarters.abc", {
    baseUrl: BASE_URL,
    fetch: (input) => fetchMock(input as Request),
  });
  return { api: headquartersApi(client), fetchMock };
}

describe("headquartersApi", () => {
  test("getSlotSupply はトークンを付けて GET /api/slot-supply を呼び、需給を返す", async () => {
    const { api, fetchMock } = setup(json(200, REPORT));
    const result = await api.getSlotSupply();
    expect(result).toEqual({ ok: true, data: REPORT });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("GET");
    expect(new URL(request.url).pathname).toBe("/api/slot-supply");
    expect(request.headers.get("Authorization")).toBe("Bearer mock.headquarters.abc");
  });

  test("本部以外のトークン（403）はロールの文言にする", async () => {
    const { api } = setup(json(403, { code: "forbidden", message: "forbidden" }));
    const result = await api.getSlotSupply();
    expect(result).toEqual({ ok: false, code: "forbidden", message: "権限がないためアクセスできません。" });
  });

  test("getCoachReviewSummary は指定したコーチの件数と平均を返す（評価が 0 件なら平均は null）", async () => {
    const coachId = "22222222-2222-4222-8222-222222222222";
    const summary = { coach_id: coachId, review_count: 0, average_rating: null };
    const { api, fetchMock } = setup(json(200, summary));
    const result = await api.getCoachReviewSummary(coachId);
    expect(result).toEqual({ ok: true, data: summary });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("GET");
    expect(new URL(request.url).pathname).toBe(`/api/coaches/${coachId}/review-summary`);
    expect(request.headers.get("Authorization")).toBe("Bearer mock.headquarters.abc");
  });

  test("UUID でないコーチ ID（404）は ID を確かめる文言にする", async () => {
    const { api } = setup(json(404, { code: "not_found", message: "not found" }));
    const result = await api.getCoachReviewSummary("not-a-uuid");
    expect(result).toEqual({ ok: false, code: "not_found", message: "対象が見つかりません（ID を確かめてください）。" });
  });

  test("listCoaches は GET /api/coaches を呼び、コーチの一覧を返す", async () => {
    const list = {
      coaches: [
        {
          coach_id: "22222222-2222-4222-8222-222222222222",
          display_name: "山田コーチ",
          acceptance_status: "accepting",
        },
      ],
    };
    const { api, fetchMock } = setup(json(200, list));
    const result = await api.listCoaches();
    expect(result).toEqual({ ok: true, data: list });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("GET");
    expect(new URL(request.url).pathname).toBe("/api/coaches");
    expect(request.headers.get("Authorization")).toBe("Bearer mock.headquarters.abc");
  });

  test("listCoaches は絞り込みをクエリにして GET /api/coaches を呼ぶ", async () => {
    const { api, fetchMock } = setup(json(200, { coaches: [] }));
    await api.listCoaches({ acceptance_status: "suspended", sport: "golf" });
    const url = new URL(fetchMock.mock.calls[0]![0].url);
    expect(url.pathname).toBe("/api/coaches");
    expect(url.searchParams.get("acceptance_status")).toBe("suspended");
    expect(url.searchParams.get("sport")).toBe("golf");
  });

  test("listCoaches は条件がなければクエリを付けない", async () => {
    const { api, fetchMock } = setup(json(200, { coaches: [] }));
    await api.listCoaches();
    expect(new URL(fetchMock.mock.calls[0]![0].url).search).toBe("");
  });

  test("listCoaches の 403 は権限の文言にする", async () => {
    const { api } = setup(json(403, { code: "forbidden", message: "forbidden" }));
    const result = await api.listCoaches();
    expect(result).toEqual({ ok: false, code: "forbidden", message: "権限がないためアクセスできません。" });
  });

  test("getCoachOverview は GET /api/coaches/{coach_id}/overview を呼び、コーチの様子を返す", async () => {
    const coachId = "22222222-2222-4222-8222-222222222222";
    const month = { live_lesson_count: 1, async_lesson_count: 0, review_count: 0, average_rating: null };
    const overview = {
      coach_id: coachId,
      display_name: "山田コーチ",
      acceptance_status: "accepting",
      review_count: 0,
      average_rating: null,
      current_month: { month: "2026-11", ...month },
      previous_month: { month: "2026-10", ...month },
      published_lesson_menu_count: 2,
    };
    const { api, fetchMock } = setup(json(200, overview));
    const result = await api.getCoachOverview(coachId);
    expect(result).toEqual({ ok: true, data: overview });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("GET");
    expect(new URL(request.url).pathname).toBe(`/api/coaches/${coachId}/overview`);
    expect(request.headers.get("Authorization")).toBe("Bearer mock.headquarters.abc");
  });

  test("getCoachOverview の 404 は code を not_found で返す", async () => {
    const { api } = setup(json(404, { code: "not_found", message: "not found" }));
    const result = await api.getCoachOverview("22222222-2222-4222-8222-222222222222");
    expect(result).toEqual({ ok: false, code: "not_found", message: "対象が見つかりません（ID を確かめてください）。" });
  });

  test("listCoachAcceptanceChanges は GET /api/coaches/{coach_id}/acceptance-changes を呼び、受付の記録を返す", async () => {
    const coachId = "22222222-2222-4222-8222-222222222222";
    const list = {
      coach_id: coachId,
      changes: [
        {
          acceptance_status: "suspended",
          changed_by: "33333333-3333-4333-8333-333333333333",
          changed_at: "2026-10-26T03:00:00Z",
        },
      ],
    };
    const { api, fetchMock } = setup(json(200, list));
    const result = await api.listCoachAcceptanceChanges(coachId);
    expect(result).toEqual({ ok: true, data: list });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("GET");
    expect(new URL(request.url).pathname).toBe(`/api/coaches/${coachId}/acceptance-changes`);
  });

  test("listCoachReviews は指定したコーチの評価の一覧を返す", async () => {
    const coachId = "22222222-2222-4222-8222-222222222222";
    const list = { coach_id: coachId, reviews: [] };
    const { api, fetchMock } = setup(json(200, list));
    const result = await api.listCoachReviews(coachId);
    expect(result).toEqual({ ok: true, data: list });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("GET");
    expect(new URL(request.url).pathname).toBe(`/api/coaches/${coachId}/reviews`);
  });

  test("listLessonMenus は絞り込みをクエリにして GET /api/lesson-menus を呼ぶ", async () => {
    const list = { lesson_menus: [] };
    const { api, fetchMock } = setup(json(200, list));
    const result = await api.listLessonMenus({ sport: "tennis", published: true });
    expect(result).toEqual({ ok: true, data: list });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("GET");
    const url = new URL(request.url);
    expect(url.pathname).toBe("/api/lesson-menus");
    expect(url.searchParams.get("sport")).toBe("tennis");
    expect(url.searchParams.get("published")).toBe("true");
    expect(request.headers.get("Authorization")).toBe("Bearer mock.headquarters.abc");
  });

  test("listLessonMenus は条件がなければクエリを付けない", async () => {
    const { api, fetchMock } = setup(json(200, { lesson_menus: [] }));
    await api.listLessonMenus({});
    expect(new URL(fetchMock.mock.calls[0]![0].url).search).toBe("");
  });

  test("unpublishLessonMenu は理由を本文にして POST /api/lesson-menus/{id}/unpublish を呼ぶ", async () => {
    const lessonMenuId = "aaaaaaaa-0000-4000-8000-000000000001";
    const menu = {
      lesson_menu_id: lessonMenuId,
      coach_id: "22222222-2222-4222-8222-222222222222",
      sport: "tennis",
      lesson_type: "live_lesson",
      name: "フォアハンドの基礎",
      description: "",
      duration_minutes: 60,
      is_published: false,
      created_at: "2026-10-01T00:00:00Z",
      headquarters_unpublish: { unpublished_at: "2026-10-26T03:00:00Z", reason: "誤った内容" },
    };
    const { api, fetchMock } = setup(json(200, menu));
    const result = await api.unpublishLessonMenu(lessonMenuId, { reason: "誤った内容" });
    expect(result).toEqual({ ok: true, data: menu });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("POST");
    expect(new URL(request.url).pathname).toBe(`/api/lesson-menus/${lessonMenuId}/unpublish`);
    expect(await request.json()).toEqual({ reason: "誤った内容" });
    expect(request.headers.get("Authorization")).toBe("Bearer mock.headquarters.abc");
  });

  test("unpublishLessonMenu の 409 は ErrorCode の文言と code にする", async () => {
    const { api } = setup(
      json(409, { code: "lesson_menu_already_unpublished_by_headquarters", message: "conflict" }),
    );
    const result = await api.unpublishLessonMenu("aaaaaaaa-0000-4000-8000-000000000001", {});
    expect(result).toEqual({
      ok: false,
      code: "lesson_menu_already_unpublished_by_headquarters",
      message: "このメニューはすでに本部が非公開にしています。",
    });
  });

  test("republishLessonMenu は本文なしで POST /api/lesson-menus/{id}/republish を呼ぶ", async () => {
    const lessonMenuId = "aaaaaaaa-0000-4000-8000-000000000001";
    const { api, fetchMock } = setup(
      json(409, { code: "lesson_menu_not_unpublished_by_headquarters", message: "conflict" }),
    );
    const result = await api.republishLessonMenu(lessonMenuId);
    expect(result).toEqual({
      ok: false,
      code: "lesson_menu_not_unpublished_by_headquarters",
      message: "このメニューは本部が非公開にしたものではありません。",
    });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("POST");
    expect(new URL(request.url).pathname).toBe(`/api/lesson-menus/${lessonMenuId}/republish`);
    expect(request.headers.get("Authorization")).toBe("Bearer mock.headquarters.abc");
  });

  test("listStudents は GET /api/students を呼び、生徒の一覧を返す", async () => {
    const list = {
      students: [
        {
          student_id: "11111111-1111-4111-8111-111111111111",
          display_name: "山田 花子",
          sport: "tennis",
          contract_type: null,
          plan: null,
          last_booked_at: null,
          registered_at: "2026-10-20T00:00:00Z",
        },
      ],
    };
    const { api, fetchMock } = setup(json(200, list));
    const result = await api.listStudents();
    expect(result).toEqual({ ok: true, data: list });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("GET");
    expect(new URL(request.url).pathname).toBe("/api/students");
    expect(request.headers.get("Authorization")).toBe("Bearer mock.headquarters.abc");
  });

  test("listStudents は絞り込みをクエリにして GET /api/students を呼ぶ", async () => {
    const { api, fetchMock } = setup(json(200, { students: [] }));
    await api.listStudents({ sport: "golf", not_booked_since: "2026-10-01" });
    const url = new URL(fetchMock.mock.calls[0]![0].url);
    expect(url.pathname).toBe("/api/students");
    expect(url.searchParams.get("sport")).toBe("golf");
    expect(url.searchParams.get("not_booked_since")).toBe("2026-10-01");
  });

  test("listStudents は条件がなければクエリを付けない", async () => {
    const { api, fetchMock } = setup(json(200, { students: [] }));
    await api.listStudents({});
    expect(new URL(fetchMock.mock.calls[0]![0].url).search).toBe("");
  });

  test("getStudentActivity は GET /api/students/{student_id}/activity を呼び、生徒 1 人の利用の様子を返す", async () => {
    const studentId = "11111111-1111-4111-8111-111111111111";
    const activity = {
      student_id: studentId,
      display_name: "山田 花子",
      upcoming_bookings: [],
      recent_lessons: [],
      cancelled_booking_count: 0,
    };
    const { api, fetchMock } = setup(json(200, activity));
    const result = await api.getStudentActivity(studentId);
    expect(result).toEqual({ ok: true, data: activity });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("GET");
    expect(new URL(request.url).pathname).toBe(`/api/students/${studentId}/activity`);
    expect(request.headers.get("Authorization")).toBe("Bearer mock.headquarters.abc");
  });

  test("getStudentActivity の 404 は code を not_found にして返す", async () => {
    const { api } = setup(json(404, { code: "not_found", message: "not found" }));
    const result = await api.getStudentActivity("11111111-1111-4111-8111-999999999999");
    expect(result).toEqual({
      ok: false,
      code: "not_found",
      message: "対象が見つかりません（ID を確かめてください）。",
    });
  });

  test("listLowRatedLessonReviews は GET /api/lesson-reviews/low を呼び、低い評価の一覧を返す", async () => {
    const list = {
      max_rating: 2,
      period_days: 30,
      reviews: [
        {
          review_id: "33333333-3333-4333-8333-333333333333",
          lesson_id: "44444444-4444-4444-8444-444444444444",
          coach_id: "22222222-2222-4222-8222-222222222222",
          coach_display_name: "鈴木 コーチ",
          student_id: "11111111-1111-4111-8111-111111111111",
          rating: 1,
          comment: "通話が途中で切れました",
          submitted_at: "2026-10-20T03:00:00Z",
          lesson_menu: null,
        },
      ],
    };
    const { api, fetchMock } = setup(json(200, list));
    const result = await api.listLowRatedLessonReviews();
    expect(result).toEqual({ ok: true, data: list });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("GET");
    expect(new URL(request.url).pathname).toBe("/api/lesson-reviews/low");
    expect(request.headers.get("Authorization")).toBe("Bearer mock.headquarters.abc");
  });

  test("listOverdueLiveLessonSessions は経過時間の下限をクエリにして GET /api/live-lesson-sessions/overdue を呼ぶ", async () => {
    const list = {
      older_than_hours: 24,
      sessions: [
        {
          session_id: "55555555-5555-4555-8555-555555555555",
          booking_id: "66666666-6666-4666-8666-666666666666",
          coach_id: "22222222-2222-4222-8222-222222222222",
          coach_display_name: "鈴木 コーチ",
          student_id: "11111111-1111-4111-8111-111111111111",
          student_display_name: "山田 花子",
          started_at: "2026-10-20T03:00:00Z",
          elapsed_sec: 90000,
        },
      ],
    };
    const { api, fetchMock } = setup(json(200, list));
    const result = await api.listOverdueLiveLessonSessions({ older_than_hours: 24 });
    expect(result).toEqual({ ok: true, data: list });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("GET");
    const url = new URL(request.url);
    expect(url.pathname).toBe("/api/live-lesson-sessions/overdue");
    expect(url.searchParams.get("older_than_hours")).toBe("24");
    expect(request.headers.get("Authorization")).toBe("Bearer mock.headquarters.abc");
  });

  test("listOverdueLiveLessonSessions は下限がなければクエリを付けない", async () => {
    const { api, fetchMock } = setup(json(200, { older_than_hours: 3, sessions: [] }));
    await api.listOverdueLiveLessonSessions();
    expect(new URL(fetchMock.mock.calls[0]![0].url).search).toBe("");
  });

  test("listBookings は期間をクエリにして GET /api/bookings を呼ぶ", async () => {
    const list = { from: "2026-10-26", to: "2026-11-01", bookings: [] };
    const { api, fetchMock } = setup(json(200, list));
    const result = await api.listBookings({ from: "2026-10-26", to: "2026-11-01" });
    expect(result).toEqual({ ok: true, data: list });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("GET");
    const url = new URL(request.url);
    expect(url.pathname).toBe("/api/bookings");
    expect(url.searchParams.get("from")).toBe("2026-10-26");
    expect(url.searchParams.get("to")).toBe("2026-11-01");
    expect(request.headers.get("Authorization")).toBe("Bearer mock.headquarters.abc");
  });

  test("listBookings の 422 は入力の誤りの文言にする", async () => {
    const { api } = setup(json(422, { code: "validation_failed", message: "to is before from" }));
    const result = await api.listBookings({ from: "2026-10-26", to: "2026-10-25" });
    expect(result).toEqual({
      ok: false,
      message: "入力の内容が正しくありません。",
      code: "validation_failed",
    });
  });

  test("suspendCoach は POST /api/coaches/{coach_id}/suspend を呼び、停止中にした受付を返す", async () => {
    const coachId = "22222222-2222-4222-8222-222222222222";
    const acceptance = { coach_id: coachId, acceptance_status: "suspended" };
    const { api, fetchMock } = setup(json(200, acceptance));
    const result = await api.suspendCoach(coachId);
    expect(result).toEqual({ ok: true, data: acceptance });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("POST");
    expect(new URL(request.url).pathname).toBe(`/api/coaches/${coachId}/suspend`);
    expect(request.headers.get("Authorization")).toBe("Bearer mock.headquarters.abc");
  });

  test("resumeCoach は POST /api/coaches/{coach_id}/resume を呼び、409 は ErrorCode の文言にする", async () => {
    const coachId = "22222222-2222-4222-8222-222222222222";
    const { api, fetchMock } = setup(json(409, { code: "coach_not_suspended", message: "not suspended" }));
    const result = await api.resumeCoach(coachId);
    expect(result).toEqual({
      ok: false,
      code: "coach_not_suspended",
      message: "このコーチの受付は停止していません。",
    });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("POST");
    expect(new URL(request.url).pathname).toBe(`/api/coaches/${coachId}/resume`);
  });

  test("sendAnnouncement は題・本文・スポーツを本文にして POST /api/announcements を呼び、送った記録を返す", async () => {
    const announcement = {
      announcement_id: "dddddddd-0000-4000-8000-000000000001",
      sender_id: "99999999-9999-4999-8999-999999999999",
      title: "メンテナンスのお知らせ",
      body: "11月3日 2:00〜4:00 は使えません。",
      sport: "tennis",
      recipient_count: 12,
      sent_at: "2026-10-26T03:00:00Z",
    };
    const { api, fetchMock } = setup(json(201, announcement));
    const result = await api.sendAnnouncement({
      title: announcement.title,
      body: announcement.body,
      sport: "tennis",
    });
    expect(result).toEqual({ ok: true, data: announcement });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("POST");
    expect(new URL(request.url).pathname).toBe("/api/announcements");
    expect(await request.json()).toEqual({
      title: announcement.title,
      body: announcement.body,
      sport: "tennis",
    });
    expect(request.headers.get("Authorization")).toBe("Bearer mock.headquarters.abc");
  });

  test("sendAnnouncement の 404（対象の生徒がいない）は code を付けて返す", async () => {
    const { api } = setup(json(404, { code: "not_found", message: "no students" }));
    const result = await api.sendAnnouncement({ title: "題", body: "本文", sport: null });
    expect(result).toEqual({ ok: false, code: "not_found", message: "対象が見つかりません（ID を確かめてください）。" });
  });

  test("listAnnouncements は GET /api/announcements を呼び、送ったお知らせの一覧を返す", async () => {
    const list = { announcements: [] };
    const { api, fetchMock } = setup(json(200, list));
    const result = await api.listAnnouncements();
    expect(result).toEqual({ ok: true, data: list });
    const request = fetchMock.mock.calls[0]![0];
    expect(request.method).toBe("GET");
    expect(new URL(request.url).pathname).toBe("/api/announcements");
    expect(request.headers.get("Authorization")).toBe("Bearer mock.headquarters.abc");
  });
});
