"use client";

import { useSearchParams } from "next/navigation";
import { Suspense, useState } from "react";

import { RoleGate } from "@/components/RoleGate";
import {
  LESSON_MENU_COACH_QUERY,
  LESSON_MENU_NAME_QUERY,
  LessonMenuCatalogHome,
} from "@/components/headquarters/LessonMenuCatalogHome";

function LessonMenuCatalogContent() {
  // コーチの様子の画面から `?coach_id=` で、そのコーチで絞った状態で開く。名前の検索語は `?q=` に残っている。
  // 開いた後に絞り込みで URL を置き換えても読み直しは一覧がするので、ここでは開いたときの値だけを使う。
  const searchParams = useSearchParams();
  const [initial] = useState(() => ({
    coachId: searchParams.get(LESSON_MENU_COACH_QUERY) || undefined,
    query: searchParams.get(LESSON_MENU_NAME_QUERY) || undefined,
  }));
  return (
    <RoleGate role="headquarters" width="wide">
      {(session) => (
        <LessonMenuCatalogHome
          token={session.token}
          initialCoachId={initial.coachId}
          initialQuery={initial.query}
        />
      )}
    </RoleGate>
  );
}

export default function LessonMenuCatalogPage() {
  // useSearchParams はビルド時の静的な描画で中断するので、Suspense で囲む。
  return (
    <Suspense>
      <LessonMenuCatalogContent />
    </Suspense>
  );
}
