"use client";

import { useSearchParams } from "next/navigation";
import { Suspense, useState } from "react";

import { RoleGate } from "@/components/RoleGate";
import {
  PendingVideoReviewsHome,
  pendingVideoReviewFilterFrom,
} from "@/components/headquarters/PendingVideoReviewsHome";

function PendingVideoReviewsContent() {
  // 絞り込みは `?sport=&min_elapsed_hours=&coach_id=&student_id=` に残っているので、その条件で開く。開いた後に絞り込みで URL を
  // 置き換えても読み直しは一覧がするので、ここでは開いたときの値だけを使う。
  const searchParams = useSearchParams();
  const [initialFilter] = useState(() => pendingVideoReviewFilterFrom(searchParams));
  return (
    <RoleGate role="headquarters">
      {(session) => <PendingVideoReviewsHome token={session.token} initialFilter={initialFilter} />}
    </RoleGate>
  );
}

export default function PendingVideoReviewsPage() {
  // useSearchParams はビルド時の静的な描画で中断するので、Suspense で囲む。
  return (
    <Suspense>
      <PendingVideoReviewsContent />
    </Suspense>
  );
}
