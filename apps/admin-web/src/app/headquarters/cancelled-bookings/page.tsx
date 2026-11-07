"use client";

import { useSearchParams } from "next/navigation";
import { Suspense, useState } from "react";

import { RoleGate } from "@/components/RoleGate";
import {
  CANCELLED_BOOKING_CANCELLER_QUERY,
  CANCELLED_BOOKING_COACH_QUERY,
  CANCELLED_BOOKING_LESSON_MENU_QUERY,
  CANCELLED_BOOKING_SPORT_QUERY,
  CancelledBookingsHome,
  cancelledBookingCancellerOf,
  cancelledBookingSportOf,
} from "@/components/headquarters/CancelledBookingsHome";

function CancelledBookingsContent() {
  // コーチ・レッスンメニュー・スポーツ・キャンセルした人の絞り込みは `?coach_id=`・`?lesson_menu_id=`・`?sport=`・`?cancelled_by=` に
  // 残っているので、その絞り込みで開く。開いた後に絞り込みで URL を置き換えても
  // 読み直しは一覧がするので、ここでは開いたときの値だけを使う。
  const searchParams = useSearchParams();
  const [initialCoachId] = useState(() => searchParams.get(CANCELLED_BOOKING_COACH_QUERY) ?? "");
  const [initialLessonMenuId] = useState(() => searchParams.get(CANCELLED_BOOKING_LESSON_MENU_QUERY) ?? "");
  const [initialSport] = useState(() => cancelledBookingSportOf(searchParams.get(CANCELLED_BOOKING_SPORT_QUERY)));
  const [initialCancelledBy] = useState(() =>
    cancelledBookingCancellerOf(searchParams.get(CANCELLED_BOOKING_CANCELLER_QUERY)),
  );
  return (
    <RoleGate role="headquarters" width="wide">
      {(session) => (
        <CancelledBookingsHome
          token={session.token}
          initialCoachId={initialCoachId}
          initialLessonMenuId={initialLessonMenuId}
          initialSport={initialSport}
          initialCancelledBy={initialCancelledBy}
        />
      )}
    </RoleGate>
  );
}

export default function CancelledBookingsPage() {
  // useSearchParams はビルド時の静的な描画で中断するので、Suspense で囲む。
  return (
    <Suspense>
      <CancelledBookingsContent />
    </Suspense>
  );
}
