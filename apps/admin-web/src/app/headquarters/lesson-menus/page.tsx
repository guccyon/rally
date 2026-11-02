"use client";

import { useSearchParams } from "next/navigation";
import { Suspense } from "react";

import { RoleGate } from "@/components/RoleGate";
import {
  LESSON_MENU_COACH_QUERY,
  LessonMenuCatalogHome,
} from "@/components/headquarters/LessonMenuCatalogHome";

function LessonMenuCatalogContent() {
  // コーチの様子の画面から `?coach_id=` で、そのコーチで絞った状態で開く。
  const initialCoachId = useSearchParams().get(LESSON_MENU_COACH_QUERY) || undefined;
  return (
    <RoleGate role="headquarters">
      {(session) => <LessonMenuCatalogHome token={session.token} initialCoachId={initialCoachId} />}
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
