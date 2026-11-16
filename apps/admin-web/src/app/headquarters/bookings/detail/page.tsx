"use client";

import { useSearchParams } from "next/navigation";
import { Suspense } from "react";

import { RoleGate } from "@/components/RoleGate";
import { BOOKING_ID_QUERY, BookingDetailHome } from "@/components/headquarters/BookingDetailHome";
import {
  BOOKINGS_COACH_QUERY,
  BOOKINGS_FROM_QUERY,
  BOOKINGS_LESSON_MENU_QUERY,
  BOOKINGS_STUDENT_QUERY,
  BOOKINGS_TO_QUERY,
} from "@/components/headquarters/bookingsPath";

function BookingDetailContent() {
  // 予約の一覧の行から `?booking_id=` で開く。
  const searchParams = useSearchParams();
  const bookingId = searchParams.get(BOOKING_ID_QUERY) ?? undefined;
  // 一覧の絞り込みと期間も同じ URL で受け取り、「予約の一覧へ戻る」でその一覧に戻す。
  const from = searchParams.get(BOOKINGS_FROM_QUERY);
  const to = searchParams.get(BOOKINGS_TO_QUERY);
  const list = {
    coachId: searchParams.get(BOOKINGS_COACH_QUERY) || undefined,
    studentId: searchParams.get(BOOKINGS_STUDENT_QUERY) || undefined,
    lessonMenuId: searchParams.get(BOOKINGS_LESSON_MENU_QUERY) || undefined,
    period: from && to ? { from, to } : undefined,
  };
  return (
    <RoleGate role="headquarters">
      {(session) => <BookingDetailHome token={session.token} bookingId={bookingId} list={list} />}
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
