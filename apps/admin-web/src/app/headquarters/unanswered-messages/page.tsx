"use client";

import { useSearchParams } from "next/navigation";
import { Suspense, useState } from "react";

import { RoleGate } from "@/components/RoleGate";
import {
  UnansweredMessageThreadsHome,
  unansweredMessageThreadFilterFrom,
} from "@/components/headquarters/UnansweredMessageThreadsHome";

function UnansweredMessageThreadsContent() {
  // 絞り込みは `?older_than_hours=&coach_id=&student_id=` に残っているので、その条件で開く。開いた後に絞り込みで URL を
  // 置き換えても読み直しは一覧がするので、ここでは開いたときの値だけを使う。
  const searchParams = useSearchParams();
  const [initialFilter] = useState(() => unansweredMessageThreadFilterFrom(searchParams));
  return (
    <RoleGate role="headquarters">
      {(session) => <UnansweredMessageThreadsHome token={session.token} initialFilter={initialFilter} />}
    </RoleGate>
  );
}

export default function UnansweredMessageThreadsPage() {
  // useSearchParams はビルド時の静的な描画で中断するので、Suspense で囲む。
  return (
    <Suspense>
      <UnansweredMessageThreadsContent />
    </Suspense>
  );
}
