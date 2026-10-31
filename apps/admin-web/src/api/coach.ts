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
    withdrawMySlot: (slotId: string) =>
      call(() =>
        client.POST("/coaches/me/slots/{slot_id}/withdraw", {
          params: { path: { slot_id: slotId } },
        }),
      ),
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
    replyToLessonReview: (reviewId: string, body: Schemas["ReplyToLessonReviewRequest"]) =>
      call(() =>
        client.PUT("/lesson-reviews/{review_id}/reply", {
          params: { path: { review_id: reviewId } },
          body,
        }),
      ),
    getMyLessonStats: () => call(() => client.GET("/coaches/me/lesson-stats")),
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
    listMyNotifications: () => call(() => client.GET("/notifications/me")),
    markMyNotificationsRead: () => call(() => client.POST("/notifications/me/read")),
    listMyLessonMenus: () => call(() => client.GET("/coaches/me/lesson-menus")),
    createMyLessonMenu: (body: Schemas["CreateLessonMenuRequest"]) =>
      call(() => client.POST("/coaches/me/lesson-menus", { body })),
    updateMyLessonMenu: (lessonMenuId: string, body: Schemas["UpdateLessonMenuRequest"]) =>
      call(() =>
        client.PUT("/coaches/me/lesson-menus/{lesson_menu_id}", {
          params: { path: { lesson_menu_id: lessonMenuId } },
          body,
        }),
      ),
  };
}

export type CoachApi = ReturnType<typeof coachApi>;
