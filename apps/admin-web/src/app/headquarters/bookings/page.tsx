"use client";

import { useSearchParams } from "next/navigation";
import { Suspense } from "react";

import { RoleGate } from "@/components/RoleGate";
import {
  BOOKINGS_COACH_QUERY,
  BOOKINGS_FROM_QUERY,
  BOOKINGS_LESSON_MENU_QUERY,
  BOOKINGS_STUDENT_QUERY,
  BOOKINGS_TO_QUERY,
  BookingListHome,
} from "@/components/headquarters/BookingListHome";

function BookingListContent() {
  // コーチの様子の画面から `?coach_id=` で、生徒の利用の様子の画面から `?student_id=` で、その人で絞った状態で開く。
  // レッスンメニューの画面から `?lesson_menu_id=` で、そのメニューで絞った状態で開く。
  const searchParams = useSearchParams();
  const coachId = searchParams.get(BOOKINGS_COACH_QUERY) || undefined;
  const studentId = searchParams.get(BOOKINGS_STUDENT_QUERY) || undefined;
  const lessonMenuId = searchParams.get(BOOKINGS_LESSON_MENU_QUERY) || undefined;
  // 予約の詳細から戻るときは `?from=&to=` で、見ていた期間のまま開く。
  const from = searchParams.get(BOOKINGS_FROM_QUERY);
  const to = searchParams.get(BOOKINGS_TO_QUERY);
  const period = from && to ? { from, to } : undefined;
  return (
    <RoleGate role="headquarters" width="wide">
      {(session) => (
        <BookingListHome
          token={session.token}
          coachId={coachId}
          studentId={studentId}
          lessonMenuId={lessonMenuId}
          period={period}
        />
      )}
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
