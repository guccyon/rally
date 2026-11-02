"use client";

import { useSearchParams } from "next/navigation";
import { Suspense } from "react";

import { RoleGate } from "@/components/RoleGate";
import { BOOKING_ID_QUERY, BookingDetailHome } from "@/components/headquarters/BookingDetailHome";

function BookingDetailContent() {
  // 予約の一覧の行から `?booking_id=` で開く。
  const bookingId = useSearchParams().get(BOOKING_ID_QUERY) ?? undefined;
  return (
    <RoleGate role="headquarters">
      {(session) => <BookingDetailHome token={session.token} bookingId={bookingId} />}
    </RoleGate>
  );
}

export default function BookingDetailPage() {
  // useSearchParams はビルド時の静的な描画で中断するので、Suspense で囲む。
  return (
    <Suspense>
      <BookingDetailContent />
    </Suspense>
  );
}
