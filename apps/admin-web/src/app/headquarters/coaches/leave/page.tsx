"use client";

import { useSearchParams } from "next/navigation";
import { Suspense } from "react";

import { RoleGate } from "@/components/RoleGate";
import { COACH_LEAVE_QUERY, CoachLeaveHome } from "@/components/headquarters/CoachLeaveHome";

function CoachLeaveContent() {
  // コーチの様子の「お休み」から `?coach_id=` で開く。
  const coachId = useSearchParams().get(COACH_LEAVE_QUERY) || undefined;
  return (
    <RoleGate role="headquarters">
      {(session) => <CoachLeaveHome token={session.token} coachId={coachId} />}
    </RoleGate>
  );
}

export default function CoachLeavePage() {
  // useSearchParams はビルド時の静的な描画で中断するので、Suspense で囲む。
  return (
    <Suspense>
      <CoachLeaveContent />
    </Suspense>
  );
}
