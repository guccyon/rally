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

/** URL にレッスンメニューの絞り込みを残すクエリの名前（契約の `listCancelledBookings` のクエリと同じ）。 */
export const CANCELLED_BOOKING_LESSON_MENU_QUERY = "lesson_menu_id";

/**
 * コーチ・レッスンメニューの絞り込みを残した「キャンセルされた予約」の URL。「すべて」のコーチ・レッスンメニューはクエリに出さない。
 * 期間は残さない。
 */
export function cancelledBookingsPath(filter: Pick<CancelledBookingFilter, "coachId" | "lessonMenuId">): string {
  const query = new URLSearchParams();
  if (filter.coachId !== "") query.set(CANCELLED_BOOKING_COACH_QUERY, filter.coachId);
  if (filter.lessonMenuId !== "") query.set(CANCELLED_BOOKING_LESSON_MENU_QUERY, filter.lessonMenuId);
  const search = query.toString();
  return search === "" ? CANCELLED_BOOKINGS_PATH : `${CANCELLED_BOOKINGS_PATH}?${search}`;
}

/**
 * 本部の画面: キャンセルされた予約を、キャンセルした日付の期間とコーチ・レッスンメニューで見る（キャンセルが増えていないか・
 * どのコーチの枠やどのメニューで多いか）。選んだコーチ・レッスンメニューは URL のクエリ（`?coach_id=`・`?lesson_menu_id=`）に残し、
 * 再読み込み・共有で同じ絞り込みの一覧を開く。
 */
export function CancelledBookingsHome({
  token,
  initialCoachId,
  initialLessonMenuId,
}: {
  token: string;
  initialCoachId?: string;
  initialLessonMenuId?: string;
}) {
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
        <CancelledBookingList
          api={api}
          initialCoachId={initialCoachId}
          initialLessonMenuId={initialLessonMenuId}
          onFilterChange={keepInUrl}
        />
      </Section>
    </>
  );
}
