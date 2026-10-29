import { call } from "./call";
import type { ApiClient, Schemas } from "./client";

/** コーチの操作（契約の operationId と 1 対 1）。 */
export function coachApi(client: ApiClient) {
  return {
    getMyAccount: () => call(() => client.GET("/me")),
    updateMyCoachProfile: (body: Schemas["UpdateCoachProfileRequest"]) =>
      call(() => client.PUT("/me/profile", { body })),
    publishSlot: (body: Schemas["PublishSlotRequest"]) =>
      call(() => client.POST("/slots", { body })),
    returnVideoReview: (videoId: string, body: Schemas["ReturnVideoReviewRequest"]) =>
      call(() =>
        client.POST("/videos/{video_id}/review", {
          params: { path: { video_id: videoId } },
          body,
        }),
      ),
    listMyCoachSlots: () => call(() => client.GET("/coaches/me/slots")),
    listMyCoachVideos: () => call(() => client.GET("/coaches/me/videos")),
    listMyCoachBookings: () => call(() => client.GET("/coaches/me/bookings")),
    startLiveLessonSession: (body: Schemas["StartLiveLessonSessionRequest"]) =>
      call(() => client.POST("/live-lesson-sessions", { body })),
    endLiveLessonSession: (sessionId: string) =>
      call(() =>
        client.POST("/live-lesson-sessions/{session_id}/end", {
          params: { path: { session_id: sessionId } },
        }),
      ),
    getCoachReviewSummary: (coachId: string) =>
      call(() =>
        client.GET("/coaches/{coach_id}/review-summary", {
          params: { path: { coach_id: coachId } },
        }),
      ),
    listCoachReviews: (coachId: string) =>
      call(() =>
        client.GET("/coaches/{coach_id}/reviews", {
          params: { path: { coach_id: coachId } },
        }),
      ),
    listMessageThreads: () => call(() => client.GET("/message-threads")),
    listMessages: (threadId: string) =>
      call(() =>
        client.GET("/message-threads/{thread_id}/messages", {
          params: { path: { thread_id: threadId } },
        }),
      ),
    sendMessage: (threadId: string, body: Schemas["SendMessageRequest"]) =>
      call(() =>
        client.POST("/message-threads/{thread_id}/messages", {
          params: { path: { thread_id: threadId } },
          body,
        }),
      ),
    markMessageThreadRead: (threadId: string) =>
      call(() =>
        client.POST("/message-threads/{thread_id}/read", {
          params: { path: { thread_id: threadId } },
        }),
      ),
  };
}

export type CoachApi = ReturnType<typeof coachApi>;
