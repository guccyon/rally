import type { Schemas } from "@/api/client";
import { formatDateTime } from "@/components/labels";

/**
 * コーチが受けた評価の一覧（listCoachReviews）。並べ方は契約の正本どおり API が済ませている（送った時刻の新しい順）ので、
 * 返った順のまま出す。
 */
export function CoachReviewList({ reviews }: { reviews: Schemas["CoachReview"][] }) {
  if (reviews.length === 0) {
    return <p className="text-sm">まだ評価はありません</p>;
  }
  return (
    <ul aria-label="評価の一覧" className="divide-y rounded border text-sm">
      {reviews.map((review) => (
        <li key={review.review_id} className="space-y-1 px-3 py-2">
          <p>
            <span className="font-semibold">{review.rating} 点</span>
            <span className="text-gray-500">（5 点満点）</span>
          </p>
          <p>{review.comment}</p>
          <p className="text-gray-500">{formatDateTime(review.submitted_at)}</p>
        </li>
      ))}
    </ul>
  );
}
