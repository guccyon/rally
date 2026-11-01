import { call } from "./call";
import type { ApiClient, Schemas } from "./client";

/** 本部の操作（契約の operationId と 1 対 1）。 */
export function headquartersApi(client: ApiClient) {
  return {
    getSlotSupply: () => call(() => client.GET("/slot-supply")),
    listCoaches: () => call(() => client.GET("/coaches")),
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
    getCoachOverview: (coachId: string) =>
      call(() =>
        client.GET("/coaches/{coach_id}/overview", {
          params: { path: { coach_id: coachId } },
        }),
      ),
    listLowRatedLessonReviews: () => call(() => client.GET("/lesson-reviews/low")),
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
    listStudents: () => call(() => client.GET("/students")),
    listBookings: (period: { from?: string; to?: string }) =>
      call(() => client.GET("/bookings", { params: { query: period } })),
  };
}

export type HeadquartersApi = ReturnType<typeof headquartersApi>;
