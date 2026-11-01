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

  test("listCoaches の 403 は権限の文言にする", async () => {
    const { api } = setup(json(403, { code: "forbidden", message: "forbidden" }));
    const result = await api.listCoaches();
    expect(result).toEqual({ ok: false, code: "forbidden", message: "権限がないためアクセスできません。" });
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
});
