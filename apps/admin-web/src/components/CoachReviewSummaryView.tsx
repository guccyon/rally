import type { Result } from "@/api/call";
import type { Schemas } from "@/api/client";
import { CoachReviewList } from "@/components/CoachReviewList";

export type CoachReviewsApi = {
  getCoachReviewSummary: (coachId: string) => Promise<Result<Schemas["CoachReviewSummary"]>>;
  listCoachReviews: (coachId: string) => Promise<Result<Schemas["CoachReviewList"]>>;
};

export type CoachReviews = {
  summary: Schemas["CoachReviewSummary"];
  reviews: Schemas["CoachReview"][];
};

/** 点の平均の表示。丸めは契約（getCoachReviewSummary）の正本どおり API が済ませているので、桁を揃えるだけ。 */
export function formatAverageRating(averageRating: number | null): string {
  return averageRating === null ? "評価はまだありません" : averageRating.toFixed(2);
}

/**
 * コーチの評価の件数と平均（getCoachReviewSummary）と、同じ ID で評価の一覧（listCoachReviews）を取る。
 * どちらかが失敗したら、どちらも返さずに失敗の文言だけを返す。
 */
export async function loadCoachReviews(
  api: CoachReviewsApi,
  coachId: string,
): Promise<Result<CoachReviews>> {
  const [summaryResponse, listResponse] = await Promise.all([
    api.getCoachReviewSummary(coachId),
    api.listCoachReviews(coachId),
  ]);
  if (!summaryResponse.ok) return summaryResponse;
  if (!listResponse.ok) return listResponse;
  return { ok: true, data: { summary: summaryResponse.data, reviews: listResponse.data.reviews } };
}

/** 評価の件数と平均と、その下に評価の一覧。コーチの「自分の評価」と本部の画面で共通。 */
export function CoachReviewSummaryView({ summary, reviews }: CoachReviews) {
  return (
    <>
      <dl aria-label="評価の件数と平均" className="grid grid-cols-[auto_1fr] gap-x-4 text-sm">
        <dt>コーチ ID</dt>
        <dd className="font-mono">{summary.coach_id}</dd>
        <dt>評価の件数</dt>
        <dd>{summary.review_count} 件</dd>
        <dt>点の平均（1〜5）</dt>
        <dd>{formatAverageRating(summary.average_rating)}</dd>
      </dl>
      <div className="space-y-2">
        <h3 className="text-sm font-semibold">受けた評価（新しい順）</h3>
        <CoachReviewList reviews={reviews} />
      </div>
    </>
  );
}
