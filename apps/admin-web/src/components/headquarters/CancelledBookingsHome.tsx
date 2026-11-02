"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/ui/Section";

import { type CancelledBookingFilter, CancelledBookingList } from "./CancelledBookingList";

/** 「キャンセルされた予約」の画面の URL。 */
export const CANCELLED_BOOKINGS_PATH = "/headquarters/cancelled-bookings";

/** URL にコーチの絞り込みを残すクエリの名前（契約の `listCancelledBookings` のクエリと同じ）。 */
export const CANCELLED_BOOKING_COACH_QUERY = "coach_id";

/** コーチの絞り込みを残した「キャンセルされた予約」の URL。「すべて」のコーチはクエリに出さない。期間は残さない。 */
export function cancelledBookingsPath(filter: Pick<CancelledBookingFilter, "coachId">): string {
  return filter.coachId === ""
    ? CANCELLED_BOOKINGS_PATH
    : `${CANCELLED_BOOKINGS_PATH}?${new URLSearchParams({ [CANCELLED_BOOKING_COACH_QUERY]: filter.coachId })}`;
}

/**
 * 本部の画面: キャンセルされた予約を、キャンセルした日付の期間とコーチで見る（キャンセルが増えていないか・どのコーチの枠で多いか）。
 * 選んだコーチは URL のクエリ（`?coach_id=`）に残し、再読み込み・共有で同じコーチの一覧を開く。
 */
export function CancelledBookingsHome({ token, initialCoachId }: { token: string; initialCoachId?: string }) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  const router = useRouter();
  // 絞り込みを変えるたびに履歴を積まず URL だけを置き換える（戻るで条件を 1 つずつ戻さない）。
  const keepInUrl = useCallback(
    (filter: CancelledBookingFilter) => router.replace(cancelledBookingsPath(filter), { scroll: false }),
    [router],
  );
  return (
    <>
      <p className="mb-4 text-sm">
        <Link href="/headquarters" className="text-primary underline">
          本部の画面へ戻る
        </Link>
      </p>
      <Section title="キャンセルされた予約">
        <CancelledBookingList api={api} initialCoachId={initialCoachId} onFilterChange={keepInUrl} />
      </Section>
    </>
  );
}
