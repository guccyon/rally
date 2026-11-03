"use client";

import { useSearchParams } from "next/navigation";
import { Suspense, useState } from "react";

import { RoleGate } from "@/components/RoleGate";
import {
  OVERDUE_LIVE_SESSIONS_COACH_QUERY,
  OverdueLiveSessionsHome,
} from "@/components/headquarters/OverdueLiveSessionsHome";

function OverdueLiveSessionsContent() {
  // コーチの絞り込みは `?coach_id=` に残っているので、そのコーチで開く。開いた後に絞り込みで URL を置き換えても
  // 読み直しは一覧がするので、ここでは開いたときの値だけを使う。
  const searchParams = useSearchParams();
  const [initialCoachId] = useState(() => searchParams.get(OVERDUE_LIVE_SESSIONS_COACH_QUERY) || undefined);
  return (
    <RoleGate role="headquarters">
      {(session) => <OverdueLiveSessionsHome token={session.token} initialCoachId={initialCoachId} />}
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
