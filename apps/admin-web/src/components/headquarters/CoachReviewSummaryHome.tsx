"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/ui/Section";

import { type Period, periodError } from "./bookingPeriod";
import { type CoachReviewFilter, CoachReviewPicker, type MaxRating, maxRatingOf } from "./CoachReviewPicker";

/** URL でコーチを選んだ状態にするクエリの名前（`?coach_id=`）。 */
export const COACH_ID_QUERY = "coach_id";

/** URL に点数の絞り込みを残すクエリの名前（契約の `listCoachReviews` のクエリと同じ）。 */
export const MAX_RATING_QUERY = "max_rating";

/** URL に期間の絞り込みを残すクエリの名前（契約の `listCoachReviews` のクエリと同じ）。 */
const FROM_QUERY = "from";
const TO_QUERY = "to";

const COACH_REVIEW_SUMMARY_PATH = "/headquarters/coach-review-summary";

/**
 * そのコーチ（と点数と期間）を選んだ状態で開く「コーチの評価」の画面の URL。
 * コーチを選んでいない・点数が「すべて」・期間で絞っていないなら、その条件はクエリに出さない。
 */
export function coachReviewSummaryPath(
  coachId: string,
  maxRating: MaxRating | null = null,
  period: Period | null = null,
): string {
  const query = new URLSearchParams();
  if (coachId !== "") query.set(COACH_ID_QUERY, coachId);
  if (maxRating !== null) query.set(MAX_RATING_QUERY, String(maxRating));
  if (period !== null) {
    query.set(FROM_QUERY, period.from);
    query.set(TO_QUERY, period.to);
  }
  const search = query.toString();
  return search === "" ? COACH_REVIEW_SUMMARY_PATH : `${COACH_REVIEW_SUMMARY_PATH}?${search}`;
}

/** URL のクエリから点数の絞り込みを読む。選択肢の点（1〜4）でなければ「すべて」として扱う。 */
export function initialMaxRatingFrom(params: Pick<URLSearchParams, "get">): MaxRating | null {
  return maxRatingOf(params.get(MAX_RATING_QUERY));
}

/** URL のクエリから期間の絞り込みを読む。開始日と終了日がそろって正しいときだけ使い、そうでなければ期間で絞らない。 */
export function initialPeriodFrom(params: Pick<URLSearchParams, "get">): Period | null {
  const period = { from: params.get(FROM_QUERY) ?? "", to: params.get(TO_QUERY) ?? "" };
  return periodError(period) === null ? period : null;
}

/**
 * 本部の画面: コーチを一覧から選んで、そのコーチの評価の件数と平均と一覧を見る。
 * `initialCoachId`（URL の `?coach_id=`）があれば、そのコーチを選んだ状態で開く。点数の絞り込みは `?max_rating=`、
 * 期間の絞り込みは `?from=&to=` で開き、コーチ・点数・期間を選び直したら URL のクエリに残す（再読み込み・共有で同じ一覧を開く）。
 */
export function CoachReviewSummaryHome({
  token,
  initialCoachId,
  initialMaxRating = null,
  initialPeriod = null,
}: {
  token: string;
  initialCoachId?: string;
  initialMaxRating?: MaxRating | null;
  initialPeriod?: Period | null;
}) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  const router = useRouter();
  // 選び直すたびに履歴を積まず URL だけを置き換える（戻るで条件を 1 つずつ戻さない）。
  const keepInUrl = useCallback(
    (filter: CoachReviewFilter) =>
      router.replace(coachReviewSummaryPath(filter.coachId, filter.maxRating, filter.period), { scroll: false }),
    [router],
  );
  return (
    <>
      <p className="mb-4 text-sm">
        <Link href="/headquarters" className="text-primary underline">
          本部の画面へ戻る
        </Link>
      </p>
      <Section title="コーチの評価">
        <CoachReviewPicker
          api={api}
          initialCoachId={initialCoachId}
          initialMaxRating={initialMaxRating}
          initialPeriod={initialPeriod}
          onFilterChange={keepInUrl}
        />
      </Section>
    </>
  );
}
