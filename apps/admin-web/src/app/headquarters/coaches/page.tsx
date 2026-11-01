"use client";

import { useSearchParams } from "next/navigation";
import { Suspense, useState } from "react";

import { RoleGate } from "@/components/RoleGate";
import { CoachAcceptanceHome, coachAcceptanceFilterFrom } from "@/components/headquarters/CoachAcceptanceHome";

function CoachAcceptanceContent() {
  // 絞り込みは `?acceptance_status=&sport=` に残っているので、その条件で開く。開いた後に絞り込みで URL を
  // 置き換えても読み直しは一覧がするので、ここでは開いたときの値だけを使う。
  const searchParams = useSearchParams();
  const [initialFilter] = useState(() => coachAcceptanceFilterFrom(searchParams));
  return (
    <RoleGate role="headquarters">
      {(session) => <CoachAcceptanceHome token={session.token} initialFilter={initialFilter} />}
    </RoleGate>
  );
}

export default function CoachAcceptancePage() {
  // useSearchParams はビルド時の静的な描画で中断するので、Suspense で囲む。
  return (
    <Suspense>
      <CoachAcceptanceContent />
    </Suspense>
  );
}
