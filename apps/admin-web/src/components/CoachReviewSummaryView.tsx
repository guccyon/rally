import type { ReactNode } from "react";

import type { Result } from "@/api/call";
import type { Schemas } from "@/api/client";
import { CoachReviewList } from "@/components/CoachReviewList";
import { formatDay } from "@/components/labels";

export type CoachReviewsApi = {
  getCoachReviewSummary: (coachId: string) => Promise<Result<Schemas["CoachReviewSummary"]>>;
  listCoachReviews: (
    coachId: string,
    filter?: CoachReviewListFilter,
  ) => Promise<Result<Schemas["CoachReviewList"]>>;
};

/** 評価の一覧（listCoachReviews）だけを絞る条件（契約のクエリの名前のまま）。付けない条件は省く。 */
export type CoachReviewListFilter = { max_rating?: number; from?: string; to?: string };

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
 * `filter` を渡せば一覧だけを絞る（点 `max_rating` 以下・評価の付いた期間 `from`〜`to`）。件数と平均は絞らない（契約に絞り込みがない）。
 * どちらかが失敗したら、どちらも返さずに失敗の文言だけを返す。
 */
export async function loadCoachReviews(
  api: CoachReviewsApi,
  coachId: string,
  filter: CoachReviewListFilter = {},
): Promise<Result<CoachReviews>> {
  const [summaryResponse, listResponse] = await Promise.all([
    api.getCoachReviewSummary(coachId),
    // 絞らないときは今までどおり ID だけで呼ぶ（コーチの「自分の評価」はこちら）。
    Object.keys(filter).length === 0 ? api.listCoachReviews(coachId) : api.listCoachReviews(coachId, filter),
  ]);
  if (!summaryResponse.ok) return summaryResponse;
  if (!listResponse.ok) return listResponse;
  return { ok: true, data: { summary: summaryResponse.data, reviews: listResponse.data.reviews } };
}

/**
 * 評価の件数と平均と、その下に評価の一覧。コーチの「自分の評価」と本部の画面で共通。
 * `showLessonMenu` は一覧の各行にレッスンのメニューを出すか、`renderReply` は各行の下に出す返信の欄（どちらも CoachReviewList）。
 * `maxRating`・`period` は一覧をその点以下・その期間（評価の付いた日付）に絞って読んだとき（本部の画面だけ）。
 * `coachName` を渡すと件数と平均の上に「コーチ」としてその表示名を出す（コーチの「自分の評価」。内部のコーチ ID は出さない #1446）。
 * 空なら「表示名未設定」と出す。本部の画面はコーチを表示名で選ぶので渡さない（#897）。
 * 一覧の見出しと 0 件の文言に条件を添える（件数と平均は絞らないので、その見出しは変えない）。
 */
export function CoachReviewSummaryView({
  summary,
  reviews,
  showLessonMenu = false,
  renderReply,
  maxRating = null,
  period = null,
  coachName,
}: CoachReviews & {
  showLessonMenu?: boolean;
  renderReply?: (review: Schemas["CoachReview"]) => ReactNode;
  maxRating?: number | null;
  period?: { from: string; to: string } | null;
  coachName?: string;
}) {
  const conditions = [
    ...(period === null ? [] : [`${formatDay(period.from)} 〜 ${formatDay(period.to)}`]),
    ...(maxRating === null ? [] : [`${maxRating} 点以下`]),
  ];
  return (
    <>
      <dl aria-label="評価の件数と平均" className="grid grid-cols-[auto_1fr] gap-x-4 text-sm">
        {coachName !== undefined && (
          <>
            <dt>コーチ</dt>
            <dd>{coachName.trim() === "" ? "表示名未設定" : coachName}</dd>
          </>
        )}
        <dt>評価の件数</dt>
        <dd>{summary.review_count} 件</dd>
        <dt>点の平均（1〜5）</dt>
        <dd>{formatAverageRating(summary.average_rating)}</dd>
      </dl>
      <div className="space-y-2">
        <h3 className="text-sm font-semibold">
          {`受けた評価（${[...conditions, "新しい順"].join("・")}）`}
        </h3>
        <CoachReviewList
          reviews={reviews}
          showLessonMenu={showLessonMenu}
          renderReply={renderReply}
          emptyMessage={emptyMessageOf(maxRating, period)}
        />
      </div>
    </>
  );
}

/** 一覧を絞って 0 件のときの文言。絞っていなければ一覧の既定の文言（まだ評価はありません）に任せる。 */
function emptyMessageOf(maxRating: number | null, period: { from: string; to: string } | null): string | undefined {
  if (period === null) return maxRating === null ? undefined : `${maxRating} 点以下の評価はありません`;
  return maxRating === null ? "この期間に付いた評価はありません" : `この期間に付いた ${maxRating} 点以下の評価はありません`;
}
