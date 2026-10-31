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
      coaches: [{ coach_id: "22222222-2222-4222-8222-222222222222", display_name: "山田コーチ" }],
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
});
