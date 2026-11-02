import { call } from "./call";
import type { ApiClient, Schemas } from "./client";

/** 本部の操作（契約の operationId と 1 対 1）。 */
export function headquartersApi(client: ApiClient) {
  return {
    getSlotSupply: () => call(() => client.GET("/slot-supply")),
    listCoaches: (
      filter: { acceptance_status?: Schemas["CoachAcceptanceStatus"]; sport?: Schemas["Sport"]; q?: string } = {},
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
    suspendCoach: (coachId: string, body: Schemas["SuspendCoachRequest"]) =>
      call(() =>
        client.POST("/coaches/{coach_id}/suspend", {
          params: { path: { coach_id: coachId } },
          body,
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
    listLowRatedLessonReviews: (filter: { handled?: boolean } = {}) =>
      call(() => client.GET("/lesson-reviews/low", { params: { query: filter } })),
    markLowRatedLessonReviewHandled: (reviewId: string) =>
      call(() =>
        client.PUT("/lesson-reviews/{review_id}/handling", {
          params: { path: { review_id: reviewId } },
        }),
      ),
    listPendingVideoReviews: (filter: {
      sport?: Schemas["Sport"];
      min_elapsed_hours?: number;
      coach_id?: string;
    }) =>
      call(() => client.GET("/video-reviews/pending", { params: { query: filter } })),
    listReturnedVideoReviews: (filter: { from?: string; to?: string; coach_id?: string }) =>
      call(() => client.GET("/video-reviews/returned", { params: { query: filter } })),
    listOverdueLiveLessonSessions: (filter: { older_than_hours?: number } = {}) =>
      call(() => client.GET("/live-lesson-sessions/overdue", { params: { query: filter } })),
    listLiveLessonSessions: (filter: { from: string; to: string; coach_id?: string }) =>
      call(() => client.GET("/live-lesson-sessions", { params: { query: filter } })),
    listLessonMenus: (filter: {
      q?: string;
      sport?: Schemas["Sport"];
      published?: boolean;
      coach_id?: string;
    }) =>
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
    listLessonMenuPublicationChanges: (lessonMenuId: string) =>
      call(() =>
        client.GET("/lesson-menus/{lesson_menu_id}/publication-changes", {
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
    listStudentAnnouncements: (studentId: string) =>
      call(() =>
        client.GET("/students/{student_id}/announcements", {
          params: { path: { student_id: studentId } },
        }),
      ),
    listBookings: (query: { from?: string; to?: string; coach_id?: string; trial?: boolean }) =>
      call(() => client.GET("/bookings", { params: { query } })),
    listCancelledBookings: (period: { from?: string; to?: string }) =>
      call(() => client.GET("/bookings/cancelled", { params: { query: period } })),
    getBooking: (bookingId: string) =>
      call(() =>
        client.GET("/bookings/{booking_id}", {
          params: { path: { booking_id: bookingId } },
        }),
      ),
    sendAnnouncement: (body: Schemas["SendAnnouncementRequest"]) =>
      call(() => client.POST("/announcements", { body })),
    listAnnouncements: () => call(() => client.GET("/announcements")),
    withdrawAnnouncement: (announcementId: string) =>
      call(() =>
        client.POST("/announcements/{announcement_id}/withdraw", {
          params: { path: { announcement_id: announcementId } },
        }),
      ),
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
