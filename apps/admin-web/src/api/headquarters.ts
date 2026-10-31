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
    listLowRatedLessonReviews: () => call(() => client.GET("/lesson-reviews/low")),
    listLessonMenus: (filter: { sport?: Schemas["Sport"]; published?: boolean }) =>
      call(() => client.GET("/lesson-menus", { params: { query: filter } })),
    listStudents: () => call(() => client.GET("/students")),
  };
}

export type HeadquartersApi = ReturnType<typeof headquartersApi>;
