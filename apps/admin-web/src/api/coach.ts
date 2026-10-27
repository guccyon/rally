import { call } from "./call";
import type { ApiClient, Schemas } from "./client";

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
