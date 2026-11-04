"use client";

import { useSearchParams } from "next/navigation";
import { Suspense } from "react";

import { RoleGate } from "@/components/RoleGate";
import { LOW_RATED_REVIEWS_STUDENT_QUERY, LowRatedReviewsHome } from "@/components/headquarters/LowRatedReviewsHome";

function LowRatedReviewsContent() {
  // 生徒の利用の様子から開いたときは `?student_id=` に生徒が残っているので、その生徒で絞って開く。
  // 「すべての生徒の低い評価を見る」でクエリが外れたら絞り込みも外す（一覧は生徒ごとに読み直す）。
  // 生徒はそのまま API に渡す（UUID でなければ API の 422 の理由を一覧が出す）。
  const studentId = useSearchParams().get(LOW_RATED_REVIEWS_STUDENT_QUERY) ?? "";
  return (
    <RoleGate role="headquarters">
      {(session) => <LowRatedReviewsHome token={session.token} studentId={studentId} />}
    </RoleGate>
  );
}

export default function LowRatedReviewsPage() {
  // useSearchParams はビルド時の静的な描画で中断するので、Suspense で囲む。
  return (
    <Suspense>
      <LowRatedReviewsContent />
    </Suspense>
  );
}
