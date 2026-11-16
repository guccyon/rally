"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/ui/Section";

import { BookingDetail } from "./BookingDetail";
import { type BookingsQuery, bookingsPath } from "./bookingsPath";

/** URL で予約を指すクエリの名前（`?booking_id=`）。 */
export const BOOKING_ID_QUERY = "booking_id";

/**
 * その予約の「予約の詳細」の画面の URL。`from`（開いた一覧の絞り込みと期間）があれば、一覧と同じクエリ名で足す
 * （「予約の一覧へ戻る」がその一覧に戻るため）。
 */
export function bookingDetailPath(bookingId: string, from?: BookingsQuery): string {
  const query = new URLSearchParams({ [BOOKING_ID_QUERY]: bookingId });
  if (from !== undefined) {
    for (const [key, value] of new URLSearchParams(bookingsPath(from).split("?")[1] ?? "")) query.set(key, value);
  }
  return `/headquarters/bookings/detail?${query}`;
}

/** 本部の画面: 予約の一覧の行から開き（戻る先は、開いた元の一覧の生徒などの絞り込みと期間）、予約 1 件の中身と状態が変わった記録を見る。 */
export function BookingDetailHome({
  token,
  bookingId,
  list,
}: {
  token: string;
  bookingId: string | undefined;
  /** 開いた元の一覧の絞り込みと期間。なければ絞り込みなしの一覧へ戻る。 */
  list?: BookingsQuery;
}) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return (
    <>
      <p className="mb-4 text-sm">
        <Link href={bookingsPath(list ?? {})} className="text-primary underline">
          予約の一覧へ戻る
        </Link>
      </p>
      <Section title="予約の詳細">
        <BookingDetail api={api} bookingId={bookingId} />
      </Section>
    </>
  );
}
