"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/Section";

import { BookingDetail } from "./BookingDetail";

/** URL で予約を指すクエリの名前（`?booking_id=`）。 */
export const BOOKING_ID_QUERY = "booking_id";

/** その予約の「予約の詳細」の画面の URL。 */
export function bookingDetailPath(bookingId: string): string {
  return `/headquarters/bookings/detail?${new URLSearchParams({ [BOOKING_ID_QUERY]: bookingId })}`;
}

/** 本部の画面: 予約の一覧の行から開き、予約 1 件の中身と状態が変わった記録を見る。 */
export function BookingDetailHome({ token, bookingId }: { token: string; bookingId: string | undefined }) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return (
    <>
      <p className="mb-4 text-sm">
        <Link href="/headquarters/bookings" className="text-blue-600 underline">
          予約の一覧へ戻る
        </Link>
      </p>
      <Section title="予約の詳細">
        <BookingDetail api={api} bookingId={bookingId} />
      </Section>
    </>
  );
}
