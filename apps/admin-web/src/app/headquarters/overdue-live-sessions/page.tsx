"use client";

import { useSearchParams } from "next/navigation";
import { Suspense, useState } from "react";

import { RoleGate } from "@/components/RoleGate";
import {
  OverdueLiveSessionsHome,
  overdueLiveSessionFilterFrom,
} from "@/components/headquarters/OverdueLiveSessionsHome";

function OverdueLiveSessionsContent() {
  // スポーツ・コーチの絞り込みは `?sport=&coach_id=` に残っているので、その条件で開く。開いた後に絞り込みで URL を置き換えても
  // 読み直しは一覧がするので、ここでは開いたときの値だけを使う。
  const searchParams = useSearchParams();
  const [initialFilter] = useState(() => overdueLiveSessionFilterFrom(searchParams));
  return (
    <RoleGate role="headquarters">
      {(session) => <OverdueLiveSessionsHome token={session.token} initialFilter={initialFilter} />}
    </RoleGate>
  );
}

export default function OverdueLiveSessionsPage() {
  // useSearchParams はビルド時の静的な描画で中断するので、Suspense で囲む。
  return (
    <Suspense>
      <OverdueLiveSessionsContent />
    </Suspense>
  );
}
