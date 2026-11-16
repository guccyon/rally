"use client";

import { useRouter, useSearchParams } from "next/navigation";
import { Suspense, useCallback, useState } from "react";

import { RoleGate } from "@/components/RoleGate";
import { CoachHome, coachPath, initialViewFrom, type View } from "@/components/coach/CoachHome";

function CoachContent() {
  // 開いていた画面は `?tab=` に残っている。開いた後に切り替えて URL を置き換えても画面は CoachHome が持つので、ここでは開いたときの値だけを使う。
  const searchParams = useSearchParams();
  const [initialView] = useState(() => initialViewFrom(searchParams));
  const router = useRouter();
  // 切り替えるたびに履歴を積まず URL だけを置き換える（本部の画面の絞り込みと同じ）。
  const keepInUrl = useCallback((view: View) => router.replace(coachPath(view), { scroll: false }), [router]);
  return (
    <RoleGate role="coach">
      {(session) => <CoachHome token={session.token} initialView={initialView} onViewChange={keepInUrl} />}
    </RoleGate>
  );
}

export default function CoachPage() {
  // useSearchParams はビルド時の静的な描画で中断するので、Suspense で囲む。
  return (
    <Suspense>
      <CoachContent />
    </Suspense>
  );
}
