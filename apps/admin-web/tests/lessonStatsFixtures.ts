import type { Schemas } from "@/api/client";

/** 1 か月分の実績。省いた値は 0（平均は null）。 */
export function monthlyStats(
  month: string,
  values: Partial<Omit<Schemas["MonthlyLessonStats"], "month">> = {},
): Schemas["MonthlyLessonStats"] {
  return {
    month,
    live_lesson_count: 0,
    async_lesson_count: 0,
    review_count: 0,
    average_rating: null,
    ...values,
  };
}

/** 今月も先月も実績のない応答（コーチのホームを開くほかの画面のテストの既定）。 */
export const EMPTY_LESSON_STATS: Schemas["CoachLessonStats"] = {
  coach_id: "22222222-2222-4222-8222-222222222222",
  current_month: monthlyStats("2026-10"),
  previous_month: monthlyStats("2026-09"),
};
