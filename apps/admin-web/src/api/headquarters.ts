import { call } from "./call";
import type { ApiClient, Schemas } from "./client";

/** 本部の操作（契約の operationId と 1 対 1）。 */
export function headquartersApi(client: ApiClient) {
  return {
    getSlotSupply: () => call(() => client.GET("/slot-supply")),
    listCoaches: (
      filter: { acceptance_status?: Schemas["CoachAcceptanceStatus"]; sport?: Schemas["Sport"] } = {},
    ) => call(() => client.GET("/coaches", { params: { query: filter } })),
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
    suspendCoach: (coachId: string) =>
      call(() =>
        client.POST("/coaches/{coach_id}/suspend", {
          params: { path: { coach_id: coachId } },
        }),
      ),
    resumeCoach: (coachId: string) =>
      call(() =>
        client.POST("/coaches/{coach_id}/resume", {
          params: { path: { coach_id: coachId } },
        }),
      ),
    listCoachAcceptanceChanges: (coachId: string) =>
      call(() =>
        client.GET("/coaches/{coach_id}/acceptance-changes", {
          params: { path: { coach_id: coachId } },
        }),
      ),
    getCoachOverview: (coachId: string) =>
      call(() =>
        client.GET("/coaches/{coach_id}/overview", {
          params: { path: { coach_id: coachId } },
        }),
      ),
    listIncompleteCoachProfiles: () => call(() => client.GET("/coaches/incomplete-profiles")),
    listLowRatedLessonReviews: () => call(() => client.GET("/lesson-reviews/low")),
    listPendingVideoReviews: (filter: { sport?: Schemas["Sport"]; min_elapsed_hours?: number }) =>
      call(() => client.GET("/video-reviews/pending", { params: { query: filter } })),
    listReturnedVideoReviews: (filter: { from?: string; to?: string; coach_id?: string }) =>
      call(() => client.GET("/video-reviews/returned", { params: { query: filter } })),
    listOverdueLiveLessonSessions: (filter: { older_than_hours?: number } = {}) =>
      call(() => client.GET("/live-lesson-sessions/overdue", { params: { query: filter } })),
    listLessonMenus: (filter: { sport?: Schemas["Sport"]; published?: boolean }) =>
      call(() => client.GET("/lesson-menus", { params: { query: filter } })),
    unpublishLessonMenu: (lessonMenuId: string, body: Schemas["UnpublishLessonMenuRequest"]) =>
      call(() =>
        client.POST("/lesson-menus/{lesson_menu_id}/unpublish", {
          params: { path: { lesson_menu_id: lessonMenuId } },
          body,
        }),
      ),
    republishLessonMenu: (lessonMenuId: string) =>
      call(() =>
        client.POST("/lesson-menus/{lesson_menu_id}/republish", {
          params: { path: { lesson_menu_id: lessonMenuId } },
        }),
      ),
    listStudents: (filter: { sport?: Schemas["Sport"]; not_booked_since?: string } = {}) =>
      call(() => client.GET("/students", { params: { query: filter } })),
    getStudentActivity: (studentId: string) =>
      call(() =>
        client.GET("/students/{student_id}/activity", {
          params: { path: { student_id: studentId } },
        }),
      ),
    listBookings: (period: { from?: string; to?: string }) =>
      call(() => client.GET("/bookings", { params: { query: period } })),
    sendAnnouncement: (body: Schemas["SendAnnouncementRequest"]) =>
      call(() => client.POST("/announcements", { body })),
    listAnnouncements: () => call(() => client.GET("/announcements")),
    createAnnouncementDraft: (body: Schemas["AnnouncementDraftRequest"]) =>
      call(() => client.POST("/announcement-drafts", { body })),
    listAnnouncementDrafts: () => call(() => client.GET("/announcement-drafts")),
    updateAnnouncementDraft: (announcementDraftId: string, body: Schemas["AnnouncementDraftRequest"]) =>
      call(() =>
        client.PUT("/announcement-drafts/{announcement_draft_id}", {
          params: { path: { announcement_draft_id: announcementDraftId } },
          body,
        }),
      ),
    deleteAnnouncementDraft: (announcementDraftId: string) =>
      call(() =>
        client.DELETE("/announcement-drafts/{announcement_draft_id}", {
          params: { path: { announcement_draft_id: announcementDraftId } },
        }),
      ),
    sendAnnouncementDraft: (announcementDraftId: string) =>
      call(() =>
        client.POST("/announcement-drafts/{announcement_draft_id}/send", {
          params: { path: { announcement_draft_id: announcementDraftId } },
        }),
      ),
  };
}

export type HeadquartersApi = ReturnType<typeof headquartersApi>;
