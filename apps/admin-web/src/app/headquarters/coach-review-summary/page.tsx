"use client";

import { useSearchParams } from "next/navigation";
import { Suspense, useState } from "react";

import { RoleGate } from "@/components/RoleGate";
import {
  COACH_ID_QUERY,
  CoachReviewSummaryHome,
  initialMaxRatingFrom,
  initialPeriodFrom,
} from "@/components/headquarters/CoachReviewSummaryHome";

function CoachReviewSummaryContent() {
  // 低い評価の一覧などから `?coach_id=` で、そのコーチを選んだ状態で開く。点数の絞り込みは `?max_rating=`、期間は `?from=&to=` に残っている。
  // 開いた後に選び直して URL を置き換えても読み直しは画面がするので、ここでは開いたときの値だけを使う。
  const searchParams = useSearchParams();
  const [initial] = useState(() => ({
    coachId: searchParams.get(COACH_ID_QUERY) ?? undefined,
    maxRating: initialMaxRatingFrom(searchParams),
    period: initialPeriodFrom(searchParams),
  }));
  return (
    <RoleGate role="headquarters">
      {(session) => (
        <CoachReviewSummaryHome
          token={session.token}
          initialCoachId={initial.coachId}
          initialMaxRating={initial.maxRating}
          initialPeriod={initial.period}
        />
      )}
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
