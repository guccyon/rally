import type { ApiClient, Schemas } from "./client";
import { errorMessage } from "./errorMessage";

/** 画面が扱う結果。失敗は表示用の文言にしてから返す。 */
export type Result<T> = { ok: true; data: T } | { ok: false; message: string };

const NETWORK_ERROR = "サーバーに接続できません。";

async function call<T>(
  request: () => Promise<{ data?: T; error?: unknown }>,
): Promise<Result<T>> {
  try {
    const { data, error } = await request();
    if (data !== undefined) return { ok: true, data };
    return { ok: false, message: errorMessage(error) };
  } catch {
    return { ok: false, message: NETWORK_ERROR };
  }
}

/** コーチの操作（契約の operationId と 1 対 1）。 */
export function coachApi(client: ApiClient) {
  return {
    publishSlot: (body: Schemas["PublishSlotRequest"]) =>
      call(() => client.POST("/slots", { body })),
    returnVideoReview: (videoId: string, body: Schemas["ReturnVideoReviewRequest"]) =>
      call(() =>
        client.POST("/videos/{video_id}/review", {
          params: { path: { video_id: videoId } },
          body,
        }),
      ),
    startLiveLessonSession: (body: Schemas["StartLiveLessonSessionRequest"]) =>
      call(() => client.POST("/live-lesson-sessions", { body })),
    endLiveLessonSession: (sessionId: string) =>
      call(() =>
        client.POST("/live-lesson-sessions/{session_id}/end", {
          params: { path: { session_id: sessionId } },
        }),
      ),
  };
}

export type CoachApi = ReturnType<typeof coachApi>;
