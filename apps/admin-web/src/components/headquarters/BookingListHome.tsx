"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/Section";

import { BookingList } from "./BookingList";

/** 「予約」の画面の URL。 */
export const BOOKINGS_PATH = "/headquarters/bookings";

/** URL でコーチを絞った状態にするクエリの名前（`?coach_id=`）。 */
export const BOOKINGS_COACH_QUERY = "coach_id";

/** そのコーチで絞った「予約」の画面の URL。 */
export function bookingListPath(coachId: string): string {
  return `${BOOKINGS_PATH}?${new URLSearchParams({ [BOOKINGS_COACH_QUERY]: coachId })}`;
}

/**
 * 本部の画面: 予約の一覧（日付ごとに、どの予約が入っていて、確定待ちのまま残っていないか）を見る。
 * `coachId`（URL の `?coach_id=`）があれば、そのコーチの枠への予約だけに絞った状態で開く。
 */
export function BookingListHome({ token, coachId }: { token: string; coachId?: string }) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return (
    <>
      <p className="mb-4 space-x-4 text-sm">
        <Link href="/headquarters" className="text-blue-600 underline">
          本部の画面へ戻る
        </Link>
        {coachId !== undefined && (
          <Link href={BOOKINGS_PATH} className="text-blue-600 underline">
            すべてのコーチの予約を見る
          </Link>
        )}
      </p>
      <Section title="予約">
        <BookingList api={api} coachId={coachId} />
      </Section>
    </>
  );
}
