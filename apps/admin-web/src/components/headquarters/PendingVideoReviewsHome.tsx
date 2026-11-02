"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/ui/Section";

import {
  MIN_ELAPSED_HOURS_OPTIONS,
  PendingVideoReviewList,
  type PendingVideoReviewFilter,
  SPORTS,
} from "./PendingVideoReviewList";

/** 「返却待ちの動画添削」の画面の URL。 */
export const PENDING_VIDEO_REVIEWS_PATH = "/headquarters/pending-video-reviews";

/** URL に絞り込みを残すクエリの名前（契約の `listPendingVideoReviews` のクエリと同じ）。 */
const SPORT_QUERY = "sport";
const MIN_ELAPSED_HOURS_QUERY = "min_elapsed_hours";
const COACH_QUERY = "coach_id";

/** 絞り込みを残した「返却待ちの動画添削」の URL。「すべて」の条件はクエリに出さない。 */
export function pendingVideoReviewsPath(filter: PendingVideoReviewFilter): string {
  const query = new URLSearchParams();
  if (filter.sport !== "") query.set(SPORT_QUERY, filter.sport);
  if (filter.minElapsedHours !== null) query.set(MIN_ELAPSED_HOURS_QUERY, String(filter.minElapsedHours));
  if (filter.coachId !== "") query.set(COACH_QUERY, filter.coachId);
  const search = query.toString();
  return search === "" ? PENDING_VIDEO_REVIEWS_PATH : `${PENDING_VIDEO_REVIEWS_PATH}?${search}`;
}

/**
 * URL のクエリから絞り込みを読む。スポーツと経過時間の下限は選択肢の値でなければ「すべて」として扱う。
 * コーチはそのまま API に渡す（UUID でなければ API の 422 の理由を一覧が出す）。
 */
export function pendingVideoReviewFilterFrom(params: Pick<URLSearchParams, "get">): PendingVideoReviewFilter {
  const sport = params.get(SPORT_QUERY);
  const minElapsedHours = params.get(MIN_ELAPSED_HOURS_QUERY);
  return {
    sport: SPORTS.find((value) => value === sport) ?? "",
    minElapsedHours: MIN_ELAPSED_HOURS_OPTIONS.find((hours) => String(hours) === minElapsedHours) ?? null,
    coachId: params.get(COACH_QUERY) ?? "",
  };
}

/**
 * 本部の画面: コーチがまだ返していない動画添削を、送った時刻の古い順に、スポーツ・経過時間・コーチで絞って見る。
 * 絞り込みは URL のクエリ（`?sport=&min_elapsed_hours=&coach_id=`）に残し、再読み込み・共有で同じ一覧を開く。
 */
export function PendingVideoReviewsHome({
  token,
  initialFilter,
}: {
  token: string;
  initialFilter?: PendingVideoReviewFilter;
}) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  const router = useRouter();
  // 絞り込みを変えるたびに履歴を積まず URL だけを置き換える（戻るで条件を 1 つずつ戻さない）。
  const keepInUrl = useCallback(
    (filter: PendingVideoReviewFilter) => router.replace(pendingVideoReviewsPath(filter), { scroll: false }),
    [router],
  );
  return (
    <>
      <p className="mb-4 flex gap-4 text-sm">
        <Link href="/headquarters" className="text-primary underline">
          本部の画面へ戻る
        </Link>
        <Link href="/headquarters/returned-video-reviews" className="text-primary underline">
          返した動画添削の記録を見る
        </Link>
      </p>
      <Section title="返却待ちの動画添削">
        <PendingVideoReviewList api={api} initialFilter={initialFilter} onFilterChange={keepInUrl} />
      </Section>
    </>
  );
}
