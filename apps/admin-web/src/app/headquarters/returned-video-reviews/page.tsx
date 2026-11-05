"use client";

import { useSearchParams } from "next/navigation";
import { Suspense, useState } from "react";

import { RoleGate } from "@/components/RoleGate";
import {
  RETURNED_VIDEO_REVIEWS_STUDENT_QUERY,
  ReturnedVideoReviewsHome,
  returnedVideoReviewFilterFrom,
} from "@/components/headquarters/ReturnedVideoReviewsHome";

function ReturnedVideoReviewsContent() {
  // 生徒の絞り込みを外すリンク（`?student_id=` だけが変わる移動）では、同じページのままなので開き直させる。
  const searchParams = useSearchParams();
  return <ReturnedVideoReviewsScreen key={searchParams.get(RETURNED_VIDEO_REVIEWS_STUDENT_QUERY) ?? ""} />;
}

function ReturnedVideoReviewsScreen() {
  // 絞り込みは `?from=&to=&coach_id=&sport=&student_id=` に残っているので、その条件で開く。開いた後に絞り込みで URL を
  // 置き換えても読み直しは一覧がするので、ここでは開いたときの値だけを使う。
  const searchParams = useSearchParams();
  const [initialFilter] = useState(() => returnedVideoReviewFilterFrom(searchParams));
  return (
    <RoleGate role="headquarters" width="wide">
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
