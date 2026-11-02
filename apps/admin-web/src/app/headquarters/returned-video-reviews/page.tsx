"use client";

import { useSearchParams } from "next/navigation";
import { Suspense, useState } from "react";

import { RoleGate } from "@/components/RoleGate";
import {
  ReturnedVideoReviewsHome,
  returnedVideoReviewFilterFrom,
} from "@/components/headquarters/ReturnedVideoReviewsHome";

function ReturnedVideoReviewsContent() {
  // 絞り込みは `?from=&to=&coach_id=&sport=` に残っているので、その条件で開く。開いた後に絞り込みで URL を
  // 置き換えても読み直しは一覧がするので、ここでは開いたときの値だけを使う。
  const searchParams = useSearchParams();
  const [initialFilter] = useState(() => returnedVideoReviewFilterFrom(searchParams));
  return (
    <RoleGate role="headquarters">
      {(session) => <ReturnedVideoReviewsHome token={session.token} initialFilter={initialFilter} />}
    </RoleGate>
  );
}

export default function ReturnedVideoReviewsPage() {
  // useSearchParams はビルド時の静的な描画で中断するので、Suspense で囲む。
  return (
    <Suspense>
      <ReturnedVideoReviewsContent />
    </Suspense>
  );
}
