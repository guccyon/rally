"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/Section";

import { periodError } from "./bookingPeriod";
import { ReturnedVideoReviewList, type ReturnedVideoReviewFilter, SPORTS } from "./ReturnedVideoReviewList";

/** 「返した動画添削」の画面の URL。 */
export const RETURNED_VIDEO_REVIEWS_PATH = "/headquarters/returned-video-reviews";

/** URL に絞り込みを残すクエリの名前（契約の `listReturnedVideoReviews` のクエリと同じ）。 */
const FROM_QUERY = "from";
const TO_QUERY = "to";
const COACH_QUERY = "coach_id";
const SPORT_QUERY = "sport";

/** 絞り込みを残した「返した動画添削」の URL。期間がない・「すべて」の条件はクエリに出さない。 */
export function returnedVideoReviewsPath(filter: ReturnedVideoReviewFilter): string {
  const query = new URLSearchParams();
  if (filter.period !== null) {
    query.set(FROM_QUERY, filter.period.from);
    query.set(TO_QUERY, filter.period.to);
  }
  if (filter.coachId !== "") query.set(COACH_QUERY, filter.coachId);
  if (filter.sport !== "") query.set(SPORT_QUERY, filter.sport);
  const search = query.toString();
  return search === "" ? RETURNED_VIDEO_REVIEWS_PATH : `${RETURNED_VIDEO_REVIEWS_PATH}?${search}`;
}

/**
 * URL のクエリから絞り込みを読む。期間は開始日と終了日がそろって正しいときだけ使い、そうでなければ API の既定に任せる。
 * スポーツは契約の値でなければ「すべて」として扱う。
 */
export function returnedVideoReviewFilterFrom(params: Pick<URLSearchParams, "get">): ReturnedVideoReviewFilter {
  const period = { from: params.get(FROM_QUERY) ?? "", to: params.get(TO_QUERY) ?? "" };
  const sport = params.get(SPORT_QUERY);
  return {
    period: periodError(period) === null ? period : null,
    coachId: params.get(COACH_QUERY) ?? "",
    sport: SPORTS.find((value) => value === sport) ?? "",
  };
}

/**
 * 本部の画面: コーチが返した動画添削を、期間・コーチ・スポーツで絞って、送ってから返すまでにかかった時間とともに見る。
 * 絞り込みは URL のクエリ（`?from=&to=&coach_id=&sport=`）に残し、再読み込み・共有で同じ一覧を開く。
 */
export function ReturnedVideoReviewsHome({
  token,
  initialFilter,
}: {
  token: string;
  initialFilter?: ReturnedVideoReviewFilter;
}) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  const router = useRouter();
  // 絞り込みを変えるたびに履歴を積まず URL だけを置き換える（戻るで条件を 1 つずつ戻さない）。
  const keepInUrl = useCallback(
    (filter: ReturnedVideoReviewFilter) => router.replace(returnedVideoReviewsPath(filter), { scroll: false }),
    [router],
  );
  return (
    <>
      <p className="mb-4 flex gap-4 text-sm">
        <Link href="/headquarters" className="text-primary underline">
          本部の画面へ戻る
        </Link>
        <Link href="/headquarters/pending-video-reviews" className="text-primary underline">
          返却待ちの動画添削を見る
        </Link>
      </p>
      <Section title="返した動画添削">
        <ReturnedVideoReviewList api={api} initialFilter={initialFilter} onFilterChange={keepInUrl} />
      </Section>
    </>
  );
}
