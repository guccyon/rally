"use client";

import { useSearchParams } from "next/navigation";
import { Suspense } from "react";

import { RoleGate } from "@/components/RoleGate";
import {
  COACH_ID_QUERY,
  CoachReviewSummaryHome,
} from "@/components/headquarters/CoachReviewSummaryHome";

function CoachReviewSummaryContent() {
  // 低い評価の一覧などから `?coach_id=` で、そのコーチを選んだ状態で開く。
  const initialCoachId = useSearchParams().get(COACH_ID_QUERY) ?? undefined;
  return (
    <RoleGate role="headquarters">
      {(session) => <CoachReviewSummaryHome token={session.token} initialCoachId={initialCoachId} />}
    </RoleGate>
  );
}

export default function CoachReviewSummaryPage() {
  // useSearchParams はビルド時の静的な描画で中断するので、Suspense で囲む。
  return (
    <Suspense>
      <CoachReviewSummaryContent />
    </Suspense>
  );
}
