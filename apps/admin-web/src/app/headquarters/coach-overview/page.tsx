"use client";

import { useSearchParams } from "next/navigation";
import { Suspense } from "react";

import { RoleGate } from "@/components/RoleGate";
import { COACH_OVERVIEW_QUERY, CoachOverviewHome } from "@/components/headquarters/CoachOverviewHome";

function CoachOverviewContent() {
  // コーチの受付・自己紹介の足りないコーチの一覧の行から `?coach_id=` で開く。
  const coachId = useSearchParams().get(COACH_OVERVIEW_QUERY) || undefined;
  return (
    <RoleGate role="headquarters">
      {(session) => <CoachOverviewHome token={session.token} coachId={coachId} />}
    </RoleGate>
  );
}

export default function CoachOverviewPage() {
  // useSearchParams はビルド時の静的な描画で中断するので、Suspense で囲む。
  return (
    <Suspense>
      <CoachOverviewContent />
    </Suspense>
  );
}
