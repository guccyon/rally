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
    expect(result).toEqual({ ok: false, message: "公開済みの枠と時間が重なっています。" });
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
      message: "トークンがないか無効です。ログインし直してください。",
    });
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
    expect(result).toEqual({ ok: false, message: "権限がないためアクセスできません。" });
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
    expect(result).toEqual({ ok: false, message: "権限がないためアクセスできません。" });
  });
});
