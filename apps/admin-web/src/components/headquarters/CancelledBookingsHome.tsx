"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/ui/Section";

import { BOOKING_CANCELLERS, type CancelledBookingFilter, CancelledBookingList } from "./CancelledBookingList";
import { SPORTS } from "./PendingVideoReviewList";

/** 「キャンセルされた予約」の画面の URL。 */
export const CANCELLED_BOOKINGS_PATH = "/headquarters/cancelled-bookings";

/** URL にコーチの絞り込みを残すクエリの名前（契約の `listCancelledBookings` のクエリと同じ）。 */
export const CANCELLED_BOOKING_COACH_QUERY = "coach_id";

/** URL にレッスンメニューの絞り込みを残すクエリの名前（契約の `listCancelledBookings` のクエリと同じ）。 */
export const CANCELLED_BOOKING_LESSON_MENU_QUERY = "lesson_menu_id";

/** URL にスポーツの絞り込みを残すクエリの名前（契約の `listCancelledBookings` のクエリと同じ）。 */
export const CANCELLED_BOOKING_SPORT_QUERY = "sport";

/** URL にキャンセルした人の絞り込みを残すクエリの名前（契約の `listCancelledBookings` のクエリと同じ）。 */
export const CANCELLED_BOOKING_CANCELLER_QUERY = "cancelled_by";

/** URL のクエリのスポーツ。`Sport` の値でなければ（手で書き換えた URL など）「すべて」で開く。 */
export function cancelledBookingSportOf(value: string | null): CancelledBookingFilter["sport"] {
  return SPORTS.find((sport) => sport === value) ?? "";
}

/**
 * URL のクエリのキャンセルした人。`BookingCanceller` の値でなければ（手で書き換えた URL など）「すべて」で開く（スポーツと同じ。
 * 422 にしない）。
 */
export function cancelledBookingCancellerOf(value: string | null): CancelledBookingFilter["cancelledBy"] {
  return BOOKING_CANCELLERS.find((canceller) => canceller === value) ?? "";
}

/**
 * コーチ・レッスンメニュー・スポーツ・キャンセルした人の絞り込みを残した「キャンセルされた予約」の URL。「すべて」のコーチ・
 * レッスンメニュー・スポーツ・キャンセルした人はクエリに出さない（キャンセルした人は省ける）。期間は残さない。
 */
export function cancelledBookingsPath(
  filter: Pick<CancelledBookingFilter, "coachId" | "lessonMenuId" | "sport"> &
    Partial<Pick<CancelledBookingFilter, "cancelledBy">>,
): string {
  const query = new URLSearchParams();
  if (filter.coachId !== "") query.set(CANCELLED_BOOKING_COACH_QUERY, filter.coachId);
  if (filter.lessonMenuId !== "") query.set(CANCELLED_BOOKING_LESSON_MENU_QUERY, filter.lessonMenuId);
  if (filter.sport !== "") query.set(CANCELLED_BOOKING_SPORT_QUERY, filter.sport);
  if (filter.cancelledBy) query.set(CANCELLED_BOOKING_CANCELLER_QUERY, filter.cancelledBy);
  const search = query.toString();
  return search === "" ? CANCELLED_BOOKINGS_PATH : `${CANCELLED_BOOKINGS_PATH}?${search}`;
}

/**
 * 本部の画面: キャンセルされた予約を、キャンセルした日付の期間とコーチ・レッスンメニュー・スポーツ・キャンセルした人で見る（キャンセルが
 * 増えていないか・どのコーチの枠やどのメニュー・どの競技で多いか・コーチのお休みで本部がまとめてキャンセルした予約）。選んだコーチ・
 * レッスンメニュー・スポーツ・キャンセルした人は URL のクエリ（`?coach_id=`・`?lesson_menu_id=`・`?sport=`・`?cancelled_by=`）に残し、
 * 再読み込み・共有で同じ絞り込みの一覧を開く。
 */
export function CancelledBookingsHome({
  token,
  initialCoachId,
  initialLessonMenuId,
  initialSport,
  initialCancelledBy,
}: {
  token: string;
  initialCoachId?: string;
  initialLessonMenuId?: string;
  initialSport?: CancelledBookingFilter["sport"];
  initialCancelledBy?: CancelledBookingFilter["cancelledBy"];
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
          initialSport={initialSport}
          initialCancelledBy={initialCancelledBy}
          onFilterChange={keepInUrl}
        />
      </Section>
    </>
  );
}
