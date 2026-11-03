import type { ReactNode } from "react";

import type { Result } from "@/api/call";
import type { Schemas } from "@/api/client";
import { CoachReviewList } from "@/components/CoachReviewList";

export type CoachReviewsApi = {
  getCoachReviewSummary: (coachId: string) => Promise<Result<Schemas["CoachReviewSummary"]>>;
  listCoachReviews: (
    coachId: string,
    filter?: { max_rating?: number },
  ) => Promise<Result<Schemas["CoachReviewList"]>>;
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
 * `maxRating` を渡せば一覧だけをその点以下（含む）に絞る（`max_rating`）。件数と平均は絞らない（契約に絞り込みがない）。
 * どちらかが失敗したら、どちらも返さずに失敗の文言だけを返す。
 */
export async function loadCoachReviews(
  api: CoachReviewsApi,
  coachId: string,
  maxRating: number | null = null,
): Promise<Result<CoachReviews>> {
  const [summaryResponse, listResponse] = await Promise.all([
    api.getCoachReviewSummary(coachId),
    // 絞らないときは今までどおり ID だけで呼ぶ（コーチの「自分の評価」はこちら）。
    maxRating === null ? api.listCoachReviews(coachId) : api.listCoachReviews(coachId, { max_rating: maxRating }),
  ]);
  if (!summaryResponse.ok) return summaryResponse;
  if (!listResponse.ok) return listResponse;
  return { ok: true, data: { summary: summaryResponse.data, reviews: listResponse.data.reviews } };
}

/**
 * 評価の件数と平均と、その下に評価の一覧。コーチの「自分の評価」と本部の画面で共通。
 * `showLessonMenu` は一覧の各行にレッスンのメニューを出すか、`renderReply` は各行の下に出す返信の欄（どちらも CoachReviewList）。
 * `maxRating` は一覧をその点以下に絞って読んだとき（本部の画面だけ）。見出しと 0 件の文言に点を添える（件数と平均は絞らない）。
 */
export function CoachReviewSummaryView({
  summary,
  reviews,
  showLessonMenu = false,
  renderReply,
  maxRating = null,
}: CoachReviews & {
  showLessonMenu?: boolean;
  renderReply?: (review: Schemas["CoachReview"]) => ReactNode;
  maxRating?: number | null;
}) {
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
        <h3 className="text-sm font-semibold">
          {maxRating === null ? "受けた評価（新しい順）" : `受けた評価（${maxRating} 点以下・新しい順）`}
        </h3>
        <CoachReviewList
          reviews={reviews}
          showLessonMenu={showLessonMenu}
          renderReply={renderReply}
          emptyMessage={maxRating === null ? undefined : `${maxRating} 点以下の評価はありません`}
        />
      </div>
    </>
  );
}
