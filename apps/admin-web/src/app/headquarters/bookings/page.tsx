"use client";

import { useSearchParams } from "next/navigation";
import { Suspense } from "react";

import { RoleGate } from "@/components/RoleGate";
import { BOOKINGS_COACH_QUERY, BookingListHome } from "@/components/headquarters/BookingListHome";

function BookingListContent() {
  // コーチの様子の画面から `?coach_id=` で、そのコーチで絞った状態で開く。
  const coachId = useSearchParams().get(BOOKINGS_COACH_QUERY) || undefined;
  return (
    <RoleGate role="headquarters">
      {(session) => <BookingListHome token={session.token} coachId={coachId} />}
    </RoleGate>
  );
}

export default function BookingListPage() {
  // useSearchParams はビルド時の静的な描画で中断するので、Suspense で囲む。
  return (
    <Suspense>
      <BookingListContent />
    </Suspense>
  );
}
